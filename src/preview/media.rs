//! Optional GStreamer 1.x C ABI, independent of GTK, KDE and the window system.
//! Decoder/output libraries are runtime package dependencies, never a user app.
use super::{Pixels, regular};
use libloading::Library;
use std::{
    ffi::{CStr, CString, c_char, c_int, c_uint, c_void},
    io,
    path::PathBuf,
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};
type Ptr = *mut c_void;
#[repr(C)]
struct GError {
    domain: c_uint,
    code: c_int,
    message: *mut c_char,
}
// Public ABI prefixes from gstminiobject.h and gstmessage.h. repr(C) preserves
// pointer-sized GType and native alignment on both 32- and 64-bit Linux.
#[repr(C)]
struct MiniObject {
    type_: usize,
    refcount: c_int,
    lockstate: c_int,
    flags: c_uint,
    copy: Ptr,
    dispose: Ptr,
    free: Ptr,
    priv_uint: c_uint,
    priv_pointer: Ptr,
}
#[repr(C)]
struct Message {
    parent: MiniObject,
    kind: c_uint,
}

struct Libraries {
    gst: Library,
    app: Library,
    object: Library,
    glib: Library,
}
struct Api {
    make: unsafe extern "C" fn(*const c_char, *const c_char) -> Ptr,
    parse_bin: unsafe extern "C" fn(*const c_char, c_int, *mut *mut GError) -> Ptr,
    by_name: unsafe extern "C" fn(Ptr, *const c_char) -> Ptr,
    ref_sink: unsafe extern "C" fn(Ptr) -> Ptr,
    unref: unsafe extern "C" fn(Ptr),
    mini_unref: unsafe extern "C" fn(Ptr),
    set: unsafe extern "C" fn(Ptr, *const c_char, ...),
    set_state: unsafe extern "C" fn(Ptr, c_int) -> c_int,
    get_bus: unsafe extern "C" fn(Ptr) -> Ptr,
    pop: unsafe extern "C" fn(Ptr) -> Ptr,
    parse_error: unsafe extern "C" fn(Ptr, *mut *mut GError, *mut *mut c_char),
    error_free: unsafe extern "C" fn(*mut GError),
    query_position: unsafe extern "C" fn(Ptr, c_int, *mut i64) -> c_int,
    query_duration: unsafe extern "C" fn(Ptr, c_int, *mut i64) -> c_int,
    seek: unsafe extern "C" fn(Ptr, c_int, c_int, i64) -> c_int,
    pull: unsafe extern "C" fn(Ptr, u64) -> Ptr,
    preroll: unsafe extern "C" fn(Ptr, u64) -> Ptr,
    buffer: unsafe extern "C" fn(Ptr) -> Ptr,
    caps: unsafe extern "C" fn(Ptr) -> Ptr,
    structure: unsafe extern "C" fn(Ptr, c_uint) -> Ptr,
    get_int: unsafe extern "C" fn(Ptr, *const c_char, *mut c_int) -> c_int,
    extract: unsafe extern "C" fn(Ptr, usize, Ptr, usize) -> usize,
}
impl Api {
    fn get() -> io::Result<&'static Self> {
        static API: OnceLock<Result<Api, String>> = OnceLock::new();
        API.get_or_init(Self::load).as_ref().map_err(|error| {
            io::Error::other(format!(
                "{} {error}",
                crate::i18n::tr(
                    "O preview de mídia requer GStreamer e seus plugins de reprodução."
                )
            ))
        })
    }
    fn load() -> Result<Self, String> {
        static LIBRARIES: OnceLock<Result<Libraries, String>> = OnceLock::new();
        let libraries = LIBRARIES
            .get_or_init(|| unsafe {
                Ok(Libraries {
                    gst: Library::new("libgstreamer-1.0.so.0").map_err(|e| e.to_string())?,
                    app: Library::new("libgstapp-1.0.so.0").map_err(|e| e.to_string())?,
                    object: Library::new("libgobject-2.0.so.0").map_err(|e| e.to_string())?,
                    glib: Library::new("libglib-2.0.so.0").map_err(|e| e.to_string())?,
                })
            })
            .as_ref()
            .map_err(Clone::clone)?;
        // SAFETY: signatures match the public GStreamer/GLib C APIs. Libraries
        // remain loaded for the process lifetime because GTypes retain callbacks.
        unsafe {
            let (gst, app, object, glib) = (
                &libraries.gst,
                &libraries.app,
                &libraries.object,
                &libraries.glib,
            );
            macro_rules! symbol {
                ($lib:expr, $name:literal) => {
                    *$lib
                        .get(concat!($name, "\0").as_bytes())
                        .map_err(|e| e.to_string())?
                };
            }
            let init: unsafe extern "C" fn(
                *mut c_int,
                *mut *mut *mut c_char,
                *mut *mut GError,
            ) -> c_int = symbol!(gst, "gst_init_check");
            let error_free: unsafe extern "C" fn(*mut GError) = symbol!(glib, "g_error_free");
            let mut error = std::ptr::null_mut();
            if init(std::ptr::null_mut(), std::ptr::null_mut(), &mut error) == 0 {
                return Err(error_text(error, error_free));
            }
            Ok(Self {
                make: symbol!(gst, "gst_element_factory_make"),
                parse_bin: symbol!(gst, "gst_parse_bin_from_description"),
                by_name: symbol!(gst, "gst_bin_get_by_name"),
                ref_sink: symbol!(gst, "gst_object_ref_sink"),
                unref: symbol!(gst, "gst_object_unref"),
                mini_unref: symbol!(gst, "gst_mini_object_unref"),
                set: symbol!(object, "g_object_set"),
                set_state: symbol!(gst, "gst_element_set_state"),
                get_bus: symbol!(gst, "gst_element_get_bus"),
                pop: symbol!(gst, "gst_bus_pop"),
                parse_error: symbol!(gst, "gst_message_parse_error"),
                error_free,
                query_position: symbol!(gst, "gst_element_query_position"),
                query_duration: symbol!(gst, "gst_element_query_duration"),
                seek: symbol!(gst, "gst_element_seek_simple"),
                pull: symbol!(app, "gst_app_sink_try_pull_sample"),
                preroll: symbol!(app, "gst_app_sink_try_pull_preroll"),
                buffer: symbol!(gst, "gst_sample_get_buffer"),
                caps: symbol!(gst, "gst_sample_get_caps"),
                structure: symbol!(gst, "gst_caps_get_structure"),
                get_int: symbol!(gst, "gst_structure_get_int"),
                extract: symbol!(gst, "gst_buffer_extract"),
            })
        }
    }
}
unsafe fn error_text(error: *mut GError, free: unsafe extern "C" fn(*mut GError)) -> String {
    if error.is_null() {
        return "GStreamer could not initialize playback".into();
    }
    // SAFETY: ownership of non-null GError is transferred to the caller.
    let message = unsafe { CStr::from_ptr((*error).message) }
        .to_string_lossy()
        .into_owned();
    unsafe {
        free(error);
    }
    message
}
struct Object {
    pointer: Ptr,
    api: &'static Api,
}
impl Drop for Object {
    fn drop(&mut self) {
        unsafe {
            (self.api.unref)(self.pointer);
        }
    }
}
struct Player {
    pipeline: Object,
    sink: Object,
    bus: Object,
}
impl Drop for Player {
    fn drop(&mut self) {
        unsafe {
            (self.pipeline.api.set_state)(self.pipeline.pointer, 1);
        }
    }
}
impl Player {
    fn open(path: &std::path::Path, silent: bool) -> io::Result<Self> {
        regular(path)?;
        let uri = url::Url::from_file_path(path)
            .map_err(|_| io::Error::other("Invalid media file path"))?;
        let uri = CString::new(uri.as_str()).map_err(io::Error::other)?;
        let api = Api::get()?;
        // SAFETY: all object references below are owned and are released by RAII.
        // video-sink is always an appsink bin, so no external video window opens.
        unsafe {
            let pipeline = (api.make)(c"playbin".as_ptr(), std::ptr::null());
            if pipeline.is_null() {
                return Err(io::Error::other(crate::i18n::tr(
                    "O GStreamer não encontrou o plugin de reprodução.",
                )));
            }
            (api.ref_sink)(pipeline);
            let pipeline = Object {
                pointer: pipeline,
                api,
            };
            let mut error = std::ptr::null_mut();
            let video = (api.parse_bin)(c"videoconvert ! videoscale add-borders=false ! video/x-raw,format=RGBA,width=[1,960],height=[1,540],pixel-aspect-ratio=1/1 ! appsink name=lse_preview sync=true max-buffers=1 drop=true".as_ptr(), 1, &mut error);
            if !error.is_null() {
                if !video.is_null() {
                    (api.unref)(video);
                }
                return Err(io::Error::other(error_text(error, api.error_free)));
            }
            if video.is_null() {
                return Err(io::Error::other("GStreamer video output unavailable"));
            }
            (api.ref_sink)(video);
            let video = Object {
                pointer: video,
                api,
            };
            let sink = (api.by_name)(video.pointer, c"lse_preview".as_ptr());
            if sink.is_null() {
                return Err(io::Error::other("GStreamer appsink unavailable"));
            }
            let sink = Object { pointer: sink, api };
            (api.set)(
                pipeline.pointer,
                c"uri".as_ptr(),
                uri.as_ptr(),
                c"video-sink".as_ptr(),
                video.pointer,
                std::ptr::null::<c_char>(),
            );
            if silent {
                let audio = (api.make)(c"fakesink".as_ptr(), std::ptr::null());
                if audio.is_null() {
                    return Err(io::Error::other("GStreamer fakesink unavailable"));
                }
                (api.ref_sink)(audio);
                let audio = Object {
                    pointer: audio,
                    api,
                };
                (api.set)(
                    audio.pointer,
                    c"sync".as_ptr(),
                    1_i32,
                    std::ptr::null::<c_char>(),
                );
                (api.set)(
                    pipeline.pointer,
                    c"audio-sink".as_ptr(),
                    audio.pointer,
                    std::ptr::null::<c_char>(),
                );
            }
            (api.set)(
                pipeline.pointer,
                c"volume".as_ptr(),
                0.7_f64,
                std::ptr::null::<c_char>(),
            );
            let bus = (api.get_bus)(pipeline.pointer);
            if bus.is_null() {
                return Err(io::Error::other("GStreamer message bus unavailable"));
            }
            let player = Self {
                pipeline,
                sink,
                bus: Object { pointer: bus, api },
            };
            if (api.set_state)(player.pipeline.pointer, 4) == 0 {
                return Err(io::Error::other(crate::i18n::tr(
                    "Não foi possível iniciar a reprodução deste arquivo.",
                )));
            }
            Ok(player)
        }
    }
    fn frame(&self, paused: bool) -> io::Result<Option<Pixels>> {
        let api = self.pipeline.api;
        // SAFETY: a pulled sample owns its caps/buffer references until unref.
        unsafe {
            let mut sample = (api.pull)(self.sink.pointer, 0);
            if sample.is_null() && paused {
                sample = (api.preroll)(self.sink.pointer, 0);
            }
            if sample.is_null() {
                return Ok(None);
            }
            let result = (|| {
                let caps = (api.caps)(sample);
                let buffer = (api.buffer)(sample);
                if caps.is_null() || buffer.is_null() {
                    return Err(io::Error::other("Invalid video sample"));
                }
                let structure = (api.structure)(caps, 0);
                let (mut width, mut height) = (0, 0);
                if structure.is_null()
                    || (api.get_int)(structure, c"width".as_ptr(), &mut width) == 0
                    || (api.get_int)(structure, c"height".as_ptr(), &mut height) == 0
                    || !(1..=960).contains(&width)
                    || !(1..=540).contains(&height)
                {
                    return Err(io::Error::other("Invalid video frame dimensions"));
                }
                let mut rgba = vec![0; width as usize * height as usize * 4];
                if (api.extract)(buffer, 0, rgba.as_mut_ptr().cast(), rgba.len()) != rgba.len() {
                    return Err(io::Error::other("Incomplete video frame"));
                }
                Ok(Some(Pixels {
                    width: width as u32,
                    height: height as u32,
                    rgba,
                }))
            })();
            (api.mini_unref)(sample);
            result
        }
    }
    fn messages(&self) -> io::Result<bool> {
        let api = self.pipeline.api;
        let mut ended = false;
        // Drain all bus messages so tag/state notifications cannot accumulate.
        unsafe {
            loop {
                let message = (api.pop)(self.bus.pointer);
                if message.is_null() {
                    break;
                }
                let kind = (*message.cast::<Message>()).kind;
                let failure = if kind == 2 {
                    let mut error = std::ptr::null_mut();
                    (api.parse_error)(message, &mut error, std::ptr::null_mut());
                    Some(error_text(error, api.error_free))
                } else {
                    None
                };
                ended |= kind == 1;
                (api.mini_unref)(message);
                if let Some(error) = failure {
                    return Err(io::Error::other(error));
                }
            }
        }
        Ok(ended)
    }
}

pub enum Control {
    Playing(bool),
    Seek(f64),
    Volume(f64),
}
#[derive(Default)]
pub struct Snapshot {
    pub frame: Option<Pixels>,
    pub position: f64,
    pub duration: f64,
    pub playing: bool,
    pub ended: bool,
    pub error: Option<String>,
}
pub struct Session {
    state: Arc<Mutex<Snapshot>>,
    control: mpsc::Sender<Control>,
    cancelled: Arc<AtomicBool>,
}
impl Session {
    pub fn start(path: PathBuf, ctx: &eframe::egui::Context) -> Self {
        Self::start_with_audio(path, ctx, false)
    }
    fn start_with_audio(path: PathBuf, ctx: &eframe::egui::Context, silent: bool) -> Self {
        let state = Arc::new(Mutex::new(Snapshot {
            playing: true,
            ..Default::default()
        }));
        let cancelled = Arc::new(AtomicBool::new(false));
        let (control, commands) = mpsc::channel();
        let (shared, cancel, context) = (state.clone(), cancelled.clone(), ctx.clone());
        let locale = crate::i18n::current();
        std::thread::spawn(move || {
            crate::i18n::set_locale(locale);
            if let Err(error) = run(path, &shared, &commands, &cancel, &context, silent) {
                let mut state = shared.lock().unwrap_or_else(|e| e.into_inner());
                state.error = Some(error.to_string());
                state.playing = false;
                context.request_repaint();
            }
        });
        Self {
            state,
            control,
            cancelled,
        }
    }
    pub fn control(&self, control: Control) {
        let _ = self.control.send(control);
    }
    pub fn poll(&self) -> Snapshot {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        Snapshot {
            frame: state.frame.take(),
            position: state.position,
            duration: state.duration,
            playing: state.playing,
            ended: state.ended,
            error: state.error.take(),
        }
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Release);
    }
}
fn run(
    path: PathBuf,
    shared: &Mutex<Snapshot>,
    commands: &mpsc::Receiver<Control>,
    cancel: &AtomicBool,
    ctx: &eframe::egui::Context,
    silent: bool,
) -> io::Result<()> {
    let player = Player::open(&path, silent)?;
    let api = player.pipeline.api;
    let mut playing = true;
    let mut ended = false;
    let mut updated = Instant::now() - Duration::from_secs(1);
    while !cancel.load(Ordering::Acquire) {
        for command in commands.try_iter() {
            // SAFETY: the worker is the sole owner of the pipeline/control state.
            unsafe {
                match command {
                    Control::Playing(value) => {
                        if value && ended {
                            (api.seek)(player.pipeline.pointer, 3, 5, 0);
                            ended = false;
                        }
                        (api.set_state)(player.pipeline.pointer, if value { 4 } else { 3 });
                        playing = value;
                    }
                    Control::Seek(seconds) if seconds.is_finite() => {
                        (api.seek)(
                            player.pipeline.pointer,
                            3,
                            5,
                            (seconds.clamp(0.0, 1.0e9) * 1.0e9) as i64,
                        );
                        ended = false;
                    }
                    Control::Volume(volume) if volume.is_finite() => {
                        (api.set)(
                            player.pipeline.pointer,
                            c"volume".as_ptr(),
                            volume.clamp(0.0, 1.0),
                            std::ptr::null::<c_char>(),
                        );
                    }
                    _ => (),
                }
            }
        }
        let frame = player.frame(!playing)?;
        if player.messages()? {
            playing = false;
            ended = true;
            unsafe {
                (api.set_state)(player.pipeline.pointer, 3);
            }
        }
        if frame.is_some() || updated.elapsed() >= Duration::from_millis(100) {
            let (mut position, mut duration) = (0_i64, 0_i64);
            unsafe {
                (api.query_position)(player.pipeline.pointer, 3, &mut position);
                (api.query_duration)(player.pipeline.pointer, 3, &mut duration);
            }
            let mut state = shared.lock().unwrap_or_else(|e| e.into_inner());
            if frame.is_some() {
                state.frame = frame;
            }
            state.position = position.max(0) as f64 / 1.0e9;
            state.duration = duration.max(0) as f64 / 1.0e9;
            state.playing = playing;
            state.ended = ended;
            updated = Instant::now();
            ctx.request_repaint();
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    Ok(())
}

#[cfg(test)]
mod tests;

#[cfg(test)]
pub(crate) fn runtime_available() -> bool {
    Api::get().is_ok()
}
