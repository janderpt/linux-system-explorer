//! Cross-desktop reveal integration. The headless bridge never opens files in an editor.
use std::{
    collections::BTreeSet,
    ffi::OsString,
    os::unix::ffi::OsStrExt,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::mpsc::{self, Receiver},
    thread,
    time::Duration,
};
use url::Url;
pub mod window;

pub const BUS_NAME: &str = "org.freedesktop.FileManager1";
pub const OBJECT_PATH: &str = "/org/freedesktop/FileManager1";
#[cfg(test)]
const CONTROL: &str = "io.github.janderpt.LinuxSystemExplorer.FileManagerControl";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operation {
    Folders,
    Items,
    Properties,
}

#[derive(Debug, PartialEq, Eq)]
pub struct OpenRequest {
    pub folder: PathBuf,
    pub selected: Vec<PathBuf>,
    pub properties: bool,
}

fn local_path(uri: &str) -> zbus::fdo::Result<PathBuf> {
    let url =
        Url::parse(uri).map_err(|_| zbus::fdo::Error::InvalidArgs("Invalid file URI".into()))?;
    if url.scheme() != "file" || url.query().is_some() || url.fragment().is_some() {
        return Err(zbus::fdo::Error::InvalidArgs(
            "Only local file URIs without query or fragment are supported".into(),
        ));
    }
    let path = url
        .to_file_path()
        .map_err(|_| zbus::fdo::Error::InvalidArgs("The URI is not a local file path".into()))?;
    if path.as_os_str().as_bytes().contains(&0) {
        return Err(zbus::fdo::Error::InvalidArgs(
            "File paths cannot contain NUL".into(),
        ));
    }
    Ok(path)
}

pub fn plan(operation: Operation, uris: &[String]) -> zbus::fdo::Result<Vec<OpenRequest>> {
    if uris.len() > 1024 || uris.iter().map(String::len).sum::<usize>() > 1_048_576 {
        return Err(zbus::fdo::Error::InvalidArgs(
            "Too many items in one request".into(),
        ));
    }
    let paths = uris
        .iter()
        .map(|uri| local_path(uri))
        .collect::<zbus::fdo::Result<Vec<_>>>()?;
    let mut seen = BTreeSet::new();
    let mut requests: Vec<OpenRequest> = Vec::new();
    for path in paths {
        if !seen.insert(path.clone()) {
            continue;
        }
        let folder = if operation == Operation::Folders {
            path.clone()
        } else {
            path.parent().unwrap_or(&path).to_owned()
        };
        if operation == Operation::Items
            && let Some(request) = requests.iter_mut().find(|r| r.folder == folder)
        {
            request.selected.push(path);
        } else {
            let selected = if operation != Operation::Folders && path.parent().is_some() {
                vec![path]
            } else {
                vec![]
            };
            requests.push(OpenRequest {
                folder,
                selected,
                properties: operation == Operation::Properties,
            });
        }
    }
    if requests.len() > 32 {
        return Err(zbus::fdo::Error::InvalidArgs(
            "Too many windows in one request".into(),
        ));
    }
    Ok(requests)
}

impl OpenRequest {
    fn arguments(&self) -> Vec<OsString> {
        if self.selected.is_empty() {
            vec!["--".into(), self.folder.clone().into_os_string()]
        } else {
            let flag = if self.properties {
                "--properties"
            } else {
                "--select"
            };
            std::iter::once(flag.into())
                .chain(self.selected.iter().map(|p| p.as_os_str().to_owned()))
                .collect()
        }
    }
}

struct FileManager {
    executable: PathBuf,
}

impl FileManager {
    async fn dispatch(
        &self,
        connection: &zbus::Connection,
        operation: Operation,
        uris: &[String],
        startup_id: &str,
    ) -> zbus::fdo::Result<()> {
        if startup_id.len() > 4096 {
            return Err(zbus::fdo::Error::InvalidArgs(
                "Startup ID is too long".into(),
            ));
        }
        for request in plan(operation, uris)? {
            if window::reuse(connection, &request, startup_id).await {
                continue;
            }
            let mut command = Command::new(&self.executable);
            command
                .args(request.arguments())
                .stdin(Stdio::null())
                .env_remove("DESKTOP_STARTUP_ID")
                .env_remove("XDG_ACTIVATION_TOKEN");
            if !startup_id.is_empty() {
                command
                    .env("DESKTOP_STARTUP_ID", startup_id)
                    .env("XDG_ACTIVATION_TOKEN", startup_id);
            }
            let mut child = command.spawn().map_err(|e| {
                zbus::fdo::Error::Failed(format!("Could not launch the explorer: {e}"))
            })?;
            thread::spawn(move || {
                let _ = child.wait();
            });
        }
        Ok(())
    }
}

#[zbus::interface(name = "org.freedesktop.FileManager1")]
impl FileManager {
    async fn show_folders(
        &self,
        uris: Vec<String>,
        startup_id: String,
        #[zbus(connection)] connection: &zbus::Connection,
    ) -> zbus::fdo::Result<()> {
        self.dispatch(connection, Operation::Folders, &uris, &startup_id)
            .await
    }
    async fn show_items(
        &self,
        uris: Vec<String>,
        startup_id: String,
        #[zbus(connection)] connection: &zbus::Connection,
    ) -> zbus::fdo::Result<()> {
        self.dispatch(connection, Operation::Items, &uris, &startup_id)
            .await
    }
    async fn show_item_properties(
        &self,
        uris: Vec<String>,
        startup_id: String,
        #[zbus(connection)] connection: &zbus::Connection,
    ) -> zbus::fdo::Result<()> {
        self.dispatch(connection, Operation::Properties, &uris, &startup_id)
            .await
    }
}

struct Control {
    stop: mpsc::Sender<()>,
}

#[zbus::interface(name = "io.github.janderpt.LinuxSystemExplorer.FileManagerControl")]
impl Control {
    // Stops only the headless bridge; existing explorer windows keep running.
    fn quit(&self) {
        let _ = self.stop.send(());
    }
    #[zbus(property)]
    fn application_id(&self) -> &str {
        crate::desktop::APP_ID
    }
}

fn start(executable: &Path) -> Result<(zbus::Connection, Receiver<()>), String> {
    let (stop, stopped) = mpsc::channel();
    let connection = zbus::block_on(
        zbus::connection::Builder::session()
            .method_timeout(Duration::from_secs(3))
            .allow_name_replacements(false)
            .replace_existing_names(false)
            .name(BUS_NAME)
            .serve_at(
                OBJECT_PATH,
                FileManager {
                    executable: executable.to_owned(),
                },
            )
            .serve_at(OBJECT_PATH, Control { stop })
            .build(),
    )
    .map_err(|e| {
        format!("Could not activate FileManager1 (another file manager may own it): {e}")
    })?;
    Ok((connection, stopped))
}

pub fn serve() -> Result<(), String> {
    let executable = std::env::current_exe().map_err(|e| e.to_string())?;
    let (connection, stopped) = start(&executable)?;
    loop {
        match stopped.recv_timeout(Duration::from_secs(1)) {
            Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => return Ok(()),
            Err(mpsc::RecvTimeoutError::Timeout) if connection.is_closed() => return Ok(()),
            Err(mpsc::RecvTimeoutError::Timeout) => (),
        }
    }
}

#[cfg(test)]
mod tests;
