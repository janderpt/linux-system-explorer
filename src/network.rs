//! Desktop-authenticated network mounts and native folder-sharing configuration.
use serde::{Deserialize, Serialize};
use std::{
    io,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::Duration,
};
use url::Url;
pub mod gvfs;
pub mod location;
#[derive(Default)]
pub struct Cancellation {
    cancelled: std::sync::atomic::AtomicBool,
    waker: std::sync::Mutex<Option<std::task::Waker>>,
}
impl Cancellation {
    pub fn cancel(&self) {
        self.cancelled
            .store(true, std::sync::atomic::Ordering::Release);
        if let Some(waker) = self.waker.lock().unwrap_or_else(|p| p.into_inner()).take() {
            waker.wake();
        }
    }
    fn is_cancelled(&self) -> bool {
        self.cancelled.load(std::sync::atomic::Ordering::Acquire)
    }
}
fn cancelled() -> io::Error {
    io::Error::new(
        io::ErrorKind::Interrupted,
        crate::i18n::tr("Conexão cancelada."),
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Backend {
    Kio,
    Gvfs,
}
fn is_kde() -> bool {
    std::env::var("XDG_CURRENT_DESKTOP")
        .is_ok_and(|d| d.split(':').any(|d| d.eq_ignore_ascii_case("KDE")))
}
fn choose_backend(kde: bool, kio: bool, gvfs: bool) -> Option<Backend> {
    if kde && kio {
        Some(Backend::Kio)
    } else if gvfs {
        Some(Backend::Gvfs)
    } else if kio {
        Some(Backend::Kio)
    } else {
        None
    }
}
fn available_services() -> Vec<String> {
    zbus::block_on(async {
        let Ok(conn) = zbus::connection::Builder::session()
            .method_timeout(Duration::from_secs(3))
            .build()
            .await
        else {
            return vec![];
        };
        let Ok(proxy) = zbus::fdo::DBusProxy::new(&conn).await else {
            return vec![];
        };
        let mut names: Vec<String> = proxy
            .list_names()
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|n| n.to_string())
            .collect();
        names.extend(
            proxy
                .list_activatable_names()
                .await
                .unwrap_or_default()
                .into_iter()
                .map(|n| n.to_string()),
        );
        names
    })
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Connection {
    pub url: String,
    #[serde(default)]
    pub label: String,
    #[serde(default, with = "crate::session::paths::optional")]
    pub mount: Option<PathBuf>,
}
impl Connection {
    pub fn title(&self) -> String {
        if !self.label.trim().is_empty() {
            return self.label.clone();
        }
        Url::parse(&self.url)
            .ok()
            .map(|u| format!("{}{}", u.host_str().unwrap_or(""), u.path()))
            .unwrap_or_else(|| self.url.clone())
    }
}

pub fn remote_url(input: &str) -> io::Result<String> {
    let input = input.trim();
    let normalized = if input.starts_with("\\\\") || input.starts_with("//") {
        format!("smb:{}", input.replace('\\', "/"))
    } else {
        input.to_owned()
    };
    let mut url = Url::parse(&normalized)
        .map_err(|_| invalid("Informe um endereço como smb://servidor/compartilhamento."))?;
    if !matches!(
        url.scheme(),
        "smb" | "sftp" | "nfs" | "ftp" | "dav" | "davs"
    ) || url.host_str().is_none()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(invalid(
            "Use um endereço SMB, SFTP, NFS, FTP ou WebDAV válido.",
        ));
    }
    if url.password().is_some() {
        return Err(invalid(
            "Não inclua senhas no endereço. A autenticação será solicitada pelo sistema.",
        ));
    }
    if url.scheme() == "smb" && url.path().trim_matches('/').is_empty() {
        url.set_path("/");
    }
    Ok(url.to_string())
}
fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, crate::i18n::tr(message))
}

pub fn mounted_input(input: &str) -> io::Result<Option<PathBuf>> {
    let input = input.trim();
    if input.starts_with("//") || input.starts_with("\\\\") {
        return Ok(None);
    }
    let path = if input == "~" {
        dirs::home_dir()
    } else if let Some(rest) = input.strip_prefix("~/") {
        dirs::home_dir().map(|p| p.join(rest))
    } else {
        let p = PathBuf::from(input);
        p.is_absolute().then_some(p)
    };
    let Some(path) = path else { return Ok(None) };
    if !path.is_dir() {
        return Err(invalid(
            "Escolha um ponto de montagem que seja uma pasta existente.",
        ));
    }
    Ok(Some(crate::filesystem::absolute_path(&path)?))
}

pub fn is_local_input(input: &str) -> bool {
    let input = input.trim();
    (input.starts_with('/') && !input.starts_with("//")) || input == "~" || input.starts_with("~/")
}

#[cfg(test)]
fn connect(input: &str) -> io::Result<PathBuf> {
    connect_cancellable(input, &Cancellation::default())
}
pub fn connect_cancellable(input: &str, cancel: &Cancellation) -> io::Result<PathBuf> {
    let uri = remote_url(input)?;
    let services = available_services();
    if cancel.is_cancelled() {
        return Err(cancelled());
    }
    let gvfs_available = services.iter().any(|s| s == "org.gtk.vfs.Daemon");
    let backend = choose_backend(
        is_kde(),
        services.iter().any(|s| s == "org.kde.KIOFuse"),
        gvfs_available,
    )
    .ok_or_else(|| {
        io::Error::other(crate::i18n::tr(
            "Instale GVfs no GNOME ou KIO Fuse no KDE para conectar a pastas remotas.",
        ))
    })?;
    connect_with_fallback(
        &uri,
        backend,
        gvfs_available,
        cancel,
        |backend| match backend {
            Backend::Kio => connect_kio(&uri, cancel),
            Backend::Gvfs => connect_gvfs(&uri, cancel),
        },
    )
}

fn readable_mount(path: PathBuf, cancel: &Cancellation) -> io::Result<PathBuf> {
    if cancel.is_cancelled() {
        return Err(cancelled());
    }
    // Mount/stat success does not prove a remote folder can be listed. Force the
    // first directory read before saving a connection or leaving the old folder.
    std::fs::read_dir(&path)?.next().transpose()?;
    if cancel.is_cancelled() {
        return Err(cancelled());
    }
    Ok(path)
}
fn connect_with_fallback(
    uri: &str,
    preferred: Backend,
    gvfs_available: bool,
    cancel: &Cancellation,
    mut mount: impl FnMut(Backend) -> io::Result<PathBuf>,
) -> io::Result<PathBuf> {
    with_backend_fallback(uri, preferred, gvfs_available, cancel, |backend| {
        mount(backend).and_then(|path| readable_mount(path, cancel))
    })
}
fn with_backend_fallback<T>(
    uri: &str,
    preferred: Backend,
    gvfs_available: bool,
    cancel: &Cancellation,
    mut operation: impl FnMut(Backend) -> io::Result<T>,
) -> io::Result<T> {
    if cancel.is_cancelled() {
        return Err(cancelled());
    }
    let first = operation(preferred);
    if cancel.is_cancelled() {
        return Err(cancelled());
    }
    match first {
        Ok(path) => Ok(path),
        Err(primary)
            if preferred == Backend::Kio && gvfs_available && uri.starts_with("smb://") =>
        {
            let second = operation(Backend::Gvfs);
            if cancel.is_cancelled() {
                return Err(cancelled());
            }
            second.map_err(|secondary| {
                if secondary.kind() == io::ErrorKind::Interrupted {
                    return secondary;
                }
                io::Error::new(
                    secondary.kind(),
                    crate::i18n::message(
                        "Não foi possível conectar pelos serviços de rede disponíveis:\n{}\n{}",
                        &[primary.to_string(), secondary.to_string()],
                    ),
                )
            })
        }
        Err(error) => Err(error),
    }
}

fn connect_gvfs(uri: &str, cancel: &Cancellation) -> io::Result<PathBuf> {
    connect_gvfs_with(&std::env::current_exe()?, uri, cancel)
}
fn connect_gvfs_with(program: &Path, uri: &str, cancel: &Cancellation) -> io::Result<PathBuf> {
    let output = run_gvfs_helper(program, "--mount-network", uri, cancel)?;
    let key: String = serde_json::from_slice(&output).map_err(io::Error::other)?;
    crate::settings::folder_path(&key)
        .filter(|p| p.is_absolute() && p.is_dir())
        .ok_or_else(|| {
            io::Error::other(crate::i18n::tr(
                "O sistema não disponibilizou uma pasta local para o compartilhamento.",
            ))
        })
}
fn run_gvfs_helper(
    program: &Path,
    mode: &str,
    uri: &str,
    cancel: &Cancellation,
) -> io::Result<Vec<u8>> {
    use std::io::{Read, Seek, SeekFrom};
    let mut stdout = tempfile::tempfile()?;
    let mut stderr = tempfile::tempfile()?;
    let mut child = Command::new(program)
        .args([mode, uri])
        .env("LSE_MOUNT_PARENT", std::process::id().to_string())
        .stdin(Stdio::null())
        .stdout(stdout.try_clone()?)
        .stderr(stderr.try_clone()?)
        .env_remove("G_MESSAGES_DEBUG")
        .spawn()?;
    let deadline = std::time::Instant::now() + Duration::from_secs(120);
    loop {
        if cancel.is_cancelled() {
            let _ = child.kill();
            let _ = child.wait();
            return Err(cancelled());
        }
        if child.try_wait()?.is_some() {
            break;
        }
        if std::time::Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                crate::i18n::tr("A conexão com o compartilhamento excedeu o tempo limite."),
            ));
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    let status = child.wait()?;
    stderr.seek(SeekFrom::Start(0))?;
    stdout.seek(SeekFrom::Start(0))?;
    let mut errors = Vec::new();
    stderr.take(8192).read_to_end(&mut errors)?;
    if !status.success() {
        return Err(io::Error::other(
            String::from_utf8_lossy(&errors).trim().to_owned(),
        ));
    }
    let mut bytes = Vec::new();
    stdout.take(4 * 1024 * 1024 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > 4 * 1024 * 1024 {
        return Err(invalid(
            "A resposta do servidor excedeu o tamanho permitido.",
        ));
    }
    Ok(bytes)
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServerShare {
    pub name: String,
    pub url: String,
}

pub fn is_smb_server(uri: &str) -> bool {
    Url::parse(uri).is_ok_and(|url| {
        url.scheme() == "smb" && url.host_str().is_some() && url.path().trim_matches('/').is_empty()
    })
}
fn shares_from_names(uri: &str, names: Vec<String>) -> io::Result<Vec<ServerShare>> {
    if names.len() > 10000 {
        return Err(invalid("O servidor retornou compartilhamentos demais."));
    }
    let root = remote_url(uri)?;
    if !is_smb_server(&root) {
        return Err(invalid(
            "Informe o endereço de um servidor SMB, como smb://servidor/.",
        ));
    }
    let root = Url::parse(&root).map_err(io::Error::other)?;
    let mut result = Vec::new();
    for name in names {
        if name.is_empty()
            || name.len() > 1024
            || matches!(name.as_str(), "." | "..")
            || name
                .chars()
                .any(|c| c.is_control() || matches!(c, '/' | '\\'))
        {
            return Err(invalid(
                "O servidor retornou um nome de compartilhamento inválido.",
            ));
        }
        let mut url = root.clone();
        url.path_segments_mut()
            .map_err(|()| invalid("Endereço de servidor inválido."))?
            .clear()
            .push(&name);
        result.push(ServerShare {
            name,
            url: url.into(),
        });
    }
    result.sort_by_key(|share| share.name.to_lowercase());
    result.dedup_by(|a, b| a.name.eq_ignore_ascii_case(&b.name));
    Ok(result)
}
pub fn browse_server(uri: &str, cancel: &Cancellation) -> io::Result<Vec<ServerShare>> {
    let uri = remote_url(uri)?;
    if !is_smb_server(&uri) {
        return Err(invalid(
            "Informe o endereço de um servidor SMB, como smb://servidor/.",
        ));
    }
    if cancel.is_cancelled() {
        return Err(cancelled());
    }
    let services = available_services();
    let gvfs_available = services.iter().any(|s| s == "org.gtk.vfs.Daemon");
    let backend = choose_backend(
        is_kde(),
        services.iter().any(|s| s == "org.kde.KIOFuse"),
        gvfs_available,
    )
    .ok_or_else(|| {
        io::Error::other(crate::i18n::tr(
            "Instale GVfs no GNOME ou KIO Fuse no KDE para conectar a pastas remotas.",
        ))
    })?;
    browse_with_fallback(
        &uri,
        backend,
        gvfs_available,
        cancel,
        |backend| match backend {
            Backend::Kio => {
                let path = connect_kio(&uri, cancel)?;
                let mut names = Vec::new();
                for entry in std::fs::read_dir(path)? {
                    if cancel.is_cancelled() {
                        return Err(cancelled());
                    }
                    let entry = entry?;
                    if !entry.file_type()?.is_dir() {
                        continue;
                    }
                    names.push(entry.file_name().into_string().map_err(|_| {
                        invalid("O servidor retornou um nome de compartilhamento inválido.")
                    })?);
                    if names.len() > 10000 {
                        return Err(invalid("O servidor retornou compartilhamentos demais."));
                    }
                }
                Ok(names)
            }
            Backend::Gvfs => {
                let output =
                    run_gvfs_helper(&std::env::current_exe()?, "--browse-network", &uri, cancel)?;
                serde_json::from_slice(&output).map_err(io::Error::other)
            }
        },
    )
}
fn browse_with_fallback(
    uri: &str,
    preferred: Backend,
    gvfs_available: bool,
    cancel: &Cancellation,
    mut list: impl FnMut(Backend) -> io::Result<Vec<String>>,
) -> io::Result<Vec<ServerShare>> {
    with_backend_fallback(uri, preferred, gvfs_available, cancel, |backend| {
        let names = list(backend)?;
        shares_from_names(uri, names)
    })
}

fn connect_kio(uri: &str, cancel: &Cancellation) -> io::Result<PathBuf> {
    // KIO opens its native authentication dialog; credentials never enter preferences.
    zbus::block_on(async {
        let conn = zbus::connection::Builder::session()
            .method_timeout(Duration::from_secs(120))
            .build()
            .await
            .map_err(io::Error::other)?;
        use std::future::Future;
        let args = (uri,);
        let mut call = std::pin::pin!(conn.call_method(
            Some("org.kde.KIOFuse"),
            "/org/kde/KIOFuse",
            Some("org.kde.KIOFuse.VFS"),
            "mountUrl",
            &args
        ));
        let reply=std::future::poll_fn(|cx| {
            *cancel.waker.lock().unwrap_or_else(|p|p.into_inner())=Some(cx.waker().clone());
            if cancel.is_cancelled(){std::task::Poll::Ready(None)}else{call.as_mut().poll(cx).map(Some)}
        }).await.ok_or_else(cancelled)?.map_err(|error|io::Error::other(crate::i18n::message("Não foi possível conectar pelo KIO Fuse: {}. Verifique a rede e a instalação de kio-fuse e kio-extras.",&[error.to_string()])))?;
        let local: String = reply.body().deserialize().map_err(io::Error::other)?;
        let path = PathBuf::from(local);
        if !path.is_absolute() || !path.is_dir() {
            return Err(io::Error::other(crate::i18n::tr(
                "O sistema não disponibilizou uma pasta local para o compartilhamento.",
            )));
        }
        Ok(path)
    })
}

fn program(name: &str) -> Option<PathBuf> {
    std::env::var_os("PATH")
        .into_iter()
        .flat_map(|p| std::env::split_paths(&p).collect::<Vec<_>>())
        .map(|p| p.join(name))
        .find(|p| p.is_file())
}
pub fn sharing_program() -> Option<PathBuf> {
    ["kioclient6", "kioclient", "kioclient5"]
        .into_iter()
        .find_map(program)
}
fn sharing_uri(path: &Path) -> io::Result<Url> {
    if !path.is_absolute() || !path.is_dir() {
        return Err(invalid("Escolha uma pasta existente para compartilhar."));
    }
    Url::from_file_path(path).map_err(|_| invalid("Caminho de pasta inválido para compartilhar."))
}
pub fn configure_sharing(path: &Path) -> io::Result<()> {
    let uri = sharing_uri(path)?;
    if is_kde()
        && let Some(program) = sharing_program()
    {
        return run_sharing_program(&program, &uri);
    }
    if available_services()
        .iter()
        .any(|name| name == "org.gnome.Nautilus")
    {
        return configure_nautilus(&uri);
    }
    if let Some(program) = sharing_program() {
        return run_sharing_program(&program, &uri);
    }
    Err(io::Error::new(
        io::ErrorKind::NotFound,
        crate::i18n::tr(
            "Instale Nautilus e nautilus-share no GNOME, ou kioclient e kdenetwork-filesharing no KDE, para configurar o compartilhamento Samba.",
        ),
    ))
}
fn configure_nautilus(uri: &Url) -> io::Result<()> {
    zbus::block_on(async {
        let conn = zbus::connection::Builder::session()
            .method_timeout(Duration::from_secs(20))
            .build()
            .await
            .map_err(io::Error::other)?;
        // Address Nautilus directly: the public FileManager1 name belongs to us.
        conn.call_method(
            Some("org.gnome.Nautilus"),
            "/org/freedesktop/FileManager1",
            Some("org.freedesktop.FileManager1"),
            "ShowItemProperties",
            &(vec![uri.as_str()], String::new()),
        )
        .await
        .map_err(io::Error::other)?;
        Ok(())
    })
}
fn run_sharing_program(program: &Path, uri: &Url) -> io::Result<()> {
    let status = Command::new(program)
        .arg("openProperties")
        .arg(uri.as_str())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::other(crate::i18n::tr(
            "A configuração de compartilhamento do sistema não pôde ser aberta.",
        )))
    }
}

#[cfg(test)]
mod integration_tests;
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn network_addresses_normalize_unc_and_reject_passwords_and_non_network_targets() {
        assert_eq!(
            remote_url(r"\\server\share\folder").unwrap(),
            "smb://server/share/folder"
        );
        assert_eq!(remote_url("//server/share").unwrap(), "smb://server/share");
        for root in [
            r"\\server\",
            r"\\server",
            "smb://server",
            "smb://server/",
            "//server/",
        ] {
            assert_eq!(remote_url(root).unwrap(), "smb://server/");
            assert!(is_smb_server(&remote_url(root).unwrap()));
        }

        assert_eq!(
            remote_url("sftp://user@server/home/user").unwrap(),
            "sftp://user@server/home/user"
        );
        for invalid in [
            "file:///tmp",
            "https://server/share",
            "smb://user:secret@server/share",
            "smb:///share",
            "smb://server/share#fragment",
        ] {
            assert!(remote_url(invalid).is_err(), "{invalid}");
        }
    }
    #[test]
    fn discovered_share_names_are_encoded_as_single_components_on_the_requested_server() {
        let shares = shares_from_names(
            "smb://user@server:445/",
            vec!["Docs #1".into(), "C$".into()],
        )
        .unwrap();
        assert_eq!(shares[0].url, "smb://user@server:445/C$");
        assert_eq!(shares[1].url, "smb://user@server:445/Docs%20%231");
        for name in ["..", "/other-host/share", "x/y", "x\\y", "a\nb", ""] {
            assert!(shares_from_names("smb://server/", vec![name.into()]).is_err());
        }
        assert!(shares_from_names("smb://user:secret@server/", vec!["Docs".into()]).is_err());
    }
    #[test]
    fn local_mounts_and_sharing_paths_are_validated_without_executing_shell_text() {
        use std::os::unix::ffi::OsStringExt;
        let root = tempfile::tempdir().unwrap();
        let path = root
            .path()
            .join(std::ffi::OsString::from_vec(b"dir-\xff # $(test)".to_vec()));
        std::fs::create_dir(&path).unwrap();
        let uri = sharing_uri(&path).unwrap();
        assert_eq!(uri.to_file_path().unwrap(), path);
        assert!(sharing_uri(&root.path().join("missing")).is_err());
        assert_eq!(
            mounted_input(root.path().to_str().unwrap()).unwrap(),
            Some(root.path().to_owned())
        );
        assert!(mounted_input("smb://server/share").unwrap().is_none());
    }
}
