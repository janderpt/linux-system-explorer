//! GIO/GTK run in an isolated helper process so their native main loop never
//! competes with eframe. Libraries are optional and loaded from the desktop.
use libloading::Library;
use std::{
    ffi::{CStr, CString, OsString, c_char, c_int, c_uint, c_void},
    io,
    os::unix::ffi::OsStringExt,
    path::PathBuf,
    sync::OnceLock,
};

type Ptr = *mut c_void;
type Ready = unsafe extern "C" fn(Ptr, Ptr, Ptr);
#[repr(C)]
struct GError {
    domain: c_uint,
    code: c_int,
    message: *mut c_char,
}
type Init3 = unsafe extern "C" fn(*mut c_int, *mut *mut *mut c_char) -> c_int;
type Init4 = unsafe extern "C" fn() -> c_int;
enum Init {
    Gtk3(Init3),
    Gtk4(Init4),
}
struct Api {
    _gio: Library,
    _glib: Library,
    _gobject: Library,
    _gtk: Library,
    init: Init,
    file_new: unsafe extern "C" fn(*const c_char) -> Ptr,
    file_path: unsafe extern "C" fn(Ptr) -> *mut c_char,
    mount_new: unsafe extern "C" fn(Ptr) -> Ptr,
    mount: unsafe extern "C" fn(Ptr, c_uint, Ptr, Ptr, Option<Ready>, Ptr),
    finish: unsafe extern "C" fn(Ptr, Ptr, *mut *mut GError) -> c_int,
    loop_new: unsafe extern "C" fn(Ptr, c_int) -> Ptr,
    loop_run: unsafe extern "C" fn(Ptr),
    loop_quit: unsafe extern "C" fn(Ptr),
    loop_unref: unsafe extern "C" fn(Ptr),
    unref: unsafe extern "C" fn(Ptr),
    free: unsafe extern "C" fn(Ptr),
    error_free: unsafe extern "C" fn(*mut GError),
}
impl Api {
    fn get() -> io::Result<&'static Self> {
        // GObject's type registry retains function pointers for the process lifetime.
        static API: OnceLock<Result<Api, String>> = OnceLock::new();
        API.get_or_init(Self::load).as_ref().map_err(|_|io::Error::other(crate::i18n::tr("Instale GIO, GTK, gvfs-backends e gvfs-fuse para acessar compartilhamentos no GNOME.")))
    }
    fn load() -> Result<Self, String> {
        // SAFETY: ABI signatures below match the public GLib/GIO/GTK C APIs.
        // Libraries are retained in the process-wide API object until process exit.
        unsafe {
            let gio = Library::new("libgio-2.0.so.0").map_err(|e| e.to_string())?;
            let glib = Library::new("libglib-2.0.so.0").map_err(|e| e.to_string())?;
            let gobject = Library::new("libgobject-2.0.so.0").map_err(|e| e.to_string())?;
            let (gtk, gtk3) = match Library::new("libgtk-3.so.0") {
                Ok(gtk) => (gtk, true),
                Err(_) => (
                    Library::new("libgtk-4.so.1").map_err(|e| e.to_string())?,
                    false,
                ),
            };
            macro_rules! symbol {
                ($lib:expr,$name:literal) => {
                    *$lib
                        .get(concat!($name, "\0").as_bytes())
                        .map_err(|e| e.to_string())?
                };
            }
            Ok(Self {
                init: if gtk3 {
                    Init::Gtk3(symbol!(gtk, "gtk_init_check"))
                } else {
                    Init::Gtk4(symbol!(gtk, "gtk_init_check"))
                },
                file_new: symbol!(gio, "g_file_new_for_uri"),
                file_path: symbol!(gio, "g_file_get_path"),
                mount_new: symbol!(gtk, "gtk_mount_operation_new"),
                mount: symbol!(gio, "g_file_mount_enclosing_volume"),
                finish: symbol!(gio, "g_file_mount_enclosing_volume_finish"),
                loop_new: symbol!(glib, "g_main_loop_new"),
                loop_run: symbol!(glib, "g_main_loop_run"),
                loop_quit: symbol!(glib, "g_main_loop_quit"),
                loop_unref: symbol!(glib, "g_main_loop_unref"),
                unref: symbol!(gobject, "g_object_unref"),
                free: symbol!(glib, "g_free"),
                error_free: symbol!(glib, "g_error_free"),
                _gio: gio,
                _glib: glib,
                _gobject: gobject,
                _gtk: gtk,
            })
        }
    }
    unsafe fn local_path(&self, file: Ptr) -> Option<PathBuf> {
        // SAFETY: file is a live GFile; get_path returns an owned NUL-terminated buffer.
        let raw = unsafe { (self.file_path)(file) };
        if raw.is_null() {
            return None;
        }
        let path = PathBuf::from(OsString::from_vec(
            unsafe { CStr::from_ptr(raw) }.to_bytes().to_vec(),
        ));
        unsafe {
            (self.free)(raw.cast());
        }
        path.is_absolute().then_some(path)
    }
}
struct Completion {
    api: &'static Api,
    main_loop: Ptr,
    result: Option<Result<PathBuf, String>>,
}
unsafe extern "C" fn mounted(file: Ptr, result: Ptr, user: Ptr) {
    // SAFETY: user points to the boxed Completion kept alive through loop_run;
    // GIO invokes this callback once, on this helper's default main context.
    let state = unsafe { &mut *user.cast::<Completion>() };
    let mut error = std::ptr::null_mut();
    unsafe {
        (state.api.finish)(file, result, &mut error);
    }
    let path = unsafe { state.api.local_path(file) };
    let message = if error.is_null() {
        None
    } else {
        let message = unsafe { CStr::from_ptr((*error).message) }
            .to_string_lossy()
            .into_owned();
        unsafe {
            (state.api.error_free)(error);
        }
        Some(message)
    };
    state.result=Some(path.ok_or_else(||message.unwrap_or_else(||crate::i18n::tr("O GVfs não disponibilizou um caminho local. Verifique se gvfs-fuse está instalado.").into())));
    unsafe {
        (state.api.loop_quit)(state.main_loop);
    }
}

pub fn mount(uri: &str) -> io::Result<PathBuf> {
    // This helper must not leave an authentication dialog behind if the
    // explorer exits, including before the worker gets to observe cancellation.
    let parent = rustix::process::getppid();
    rustix::process::set_parent_process_death_signal(Some(rustix::process::Signal::KILL))?;
    let current = rustix::process::getppid();
    let expected = std::env::var("LSE_MOUNT_PARENT")
        .ok()
        .and_then(|p| p.parse::<u32>().ok());
    if parent != current
        || expected.is_some_and(|p| current.map(|id| id.as_raw_nonzero().get() as u32) != Some(p))
    {
        return Err(io::Error::new(
            io::ErrorKind::Interrupted,
            "Mount requester exited",
        ));
    }
    let api = Api::get()?;
    let uri = CString::new(uri).map_err(io::Error::other)?;
    // SAFETY: this function is called only by the dedicated CLI helper on its
    // main thread; all pointers are owned until the async callback has completed.
    unsafe {
        let initialized = match api.init {
            Init::Gtk3(init) => init(std::ptr::null_mut(), std::ptr::null_mut()),
            Init::Gtk4(init) => init(),
        };
        if initialized == 0 {
            return Err(io::Error::other(crate::i18n::tr(
                "Não foi possível iniciar a autenticação gráfica do GNOME.",
            )));
        }
        let file = (api.file_new)(uri.as_ptr());
        if file.is_null() {
            return Err(io::Error::other("GIO could not create a file object"));
        }
        if let Some(path) = api.local_path(file).filter(|p| p.is_dir()) {
            (api.unref)(file);
            return Ok(path);
        }
        let operation = (api.mount_new)(std::ptr::null_mut());
        let main_loop = (api.loop_new)(std::ptr::null_mut(), 0);
        if operation.is_null() || main_loop.is_null() {
            if !operation.is_null() {
                (api.unref)(operation);
            }
            if !main_loop.is_null() {
                (api.loop_unref)(main_loop);
            }
            (api.unref)(file);
            return Err(io::Error::other(
                "Could not initialize the GIO mount operation",
            ));
        }
        let mut state = Box::new(Completion {
            api,
            main_loop,
            result: None,
        });
        (api.mount)(
            file,
            0,
            operation,
            std::ptr::null_mut(),
            Some(mounted),
            (&mut *state as *mut Completion).cast(),
        );
        (api.loop_run)(main_loop);
        (api.unref)(operation);
        (api.unref)(file);
        (api.loop_unref)(main_loop);
        state
            .result
            .take()
            .unwrap_or_else(|| Err("GIO mount did not complete".into()))
            .map_err(io::Error::other)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn gio_abi_round_trips_a_local_path_without_a_display() {
        let Ok(api) = Api::get() else { return };
        let root = tempfile::tempdir().unwrap();
        let uri = CString::new(url::Url::from_file_path(root.path()).unwrap().as_str()).unwrap();
        // SAFETY: the API owns the GFile and returned buffer, both released below.
        unsafe {
            let file = (api.file_new)(uri.as_ptr());
            assert!(!file.is_null());
            assert_eq!(api.local_path(file).unwrap(), root.path());
            (api.unref)(file);
        }
    }
}
