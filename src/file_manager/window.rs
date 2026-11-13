//! Each GUI owns a bus name for its lifetime. The GUI thread decides whether its
//! current (or currently loading) folder still matches before accepting a reveal.
use super::*;
use std::{sync::mpsc::SyncSender, time::Instant};

const PREFIX: &str = "io.github.janderpt.LinuxSystemExplorer.Window1.p";
pub(super) const INTERFACE: &str = "io.github.janderpt.LinuxSystemExplorer.Window1";
pub(super) const PATH: &str = "/io/github/janderpt/LinuxSystemExplorer/Window1";
const RESPONSE_TIMEOUT: Duration = Duration::from_secs(2);

pub struct Request {
    request: OpenRequest,
    startup_id: String,
    reply: SyncSender<bool>,
    deadline: Instant,
}

impl Request {
    pub fn folder(&self) -> &Path {
        &self.request.folder
    }
    #[cfg(test)]
    pub(crate) fn for_test(request: OpenRequest) -> (Self, Receiver<bool>) {
        let (reply, response) = mpsc::sync_channel(1);
        (
            Self {
                request,
                startup_id: "test-token".into(),
                reply,
                deadline: Instant::now() + RESPONSE_TIMEOUT,
            },
            response,
        )
    }
    pub fn accept(self, folder: &Path) -> Option<(OpenRequest, String)> {
        let matches = self.request.folder == folder && Instant::now() < self.deadline;
        // A closed/unresponsive window cannot claim a request after the caller gave up.
        if self.reply.send(matches).is_ok() && matches {
            Some((self.request, self.startup_id))
        } else {
            None
        }
    }
}

struct Endpoint {
    sender: SyncSender<Request>,
    context: eframe::egui::Context,
}

#[zbus::interface(name = "io.github.janderpt.LinuxSystemExplorer.Window1")]
impl Endpoint {
    fn reveal(
        &self,
        folder: String,
        items: Vec<String>,
        properties: bool,
        startup_id: String,
    ) -> zbus::fdo::Result<bool> {
        let folder = local_path(&folder)?;
        if startup_id.len() > 4096 {
            return Err(zbus::fdo::Error::InvalidArgs(
                "Startup ID is too long".into(),
            ));
        }
        let mut requests = plan(
            if properties {
                Operation::Properties
            } else {
                Operation::Items
            },
            &items,
        )?;
        if requests.len() > 1 || requests.first().is_some_and(|r| r.folder != folder) {
            return Err(zbus::fdo::Error::InvalidArgs(
                "Items must belong to the requested folder".into(),
            ));
        }
        let request = requests.pop().unwrap_or(OpenRequest {
            folder,
            selected: vec![],
            properties,
        });
        let (reply, response) = mpsc::sync_channel(1);
        if self
            .sender
            .try_send(Request {
                request,
                startup_id,
                reply,
                deadline: Instant::now() + RESPONSE_TIMEOUT,
            })
            .is_err()
        {
            return Ok(false);
        }
        self.context.request_repaint();
        Ok(response.recv_timeout(RESPONSE_TIMEOUT).unwrap_or(false))
    }
}

pub struct Window {
    _connection: zbus::Connection,
    requests: Receiver<Request>,
}

impl Window {
    pub fn attach(context: &eframe::egui::Context) -> Result<Self, zbus::Error> {
        Self::connect(context, None)
    }

    #[cfg(test)]
    pub(super) fn with_name(
        context: &eframe::egui::Context,
        name: &str,
    ) -> Result<Self, zbus::Error> {
        Self::connect(context, Some(name))
    }

    fn connect(context: &eframe::egui::Context, name: Option<&str>) -> Result<Self, zbus::Error> {
        let (sender, requests) = mpsc::sync_channel(16);
        let connection = zbus::block_on(async {
            let mut builder = zbus::connection::Builder::session()?
                .allow_name_replacements(false)
                .replace_existing_names(false)
                .serve_at(
                    PATH,
                    Endpoint {
                        sender,
                        context: context.clone(),
                    },
                )?;
            if let Some(name) = name {
                builder = builder.name(name)?;
            }
            let connection = builder.build().await?;
            if name.is_none() {
                // Different Flatpak instances may have the same namespace PID.
                // A bus-assigned unique name avoids collisions across sandboxes.
                let suffix = connection
                    .unique_name()
                    .unwrap()
                    .as_str()
                    .trim_start_matches(':')
                    .replace('.', "_");
                connection
                    .request_name_with_flags(
                        format!("{PREFIX}{suffix}"),
                        zbus::fdo::RequestNameFlags::DoNotQueue.into(),
                    )
                    .await?;
            }
            Ok::<_, zbus::Error>(connection)
        })?;
        Ok(Self {
            _connection: connection,
            requests,
        })
    }

    pub fn pending(&self) -> Vec<Request> {
        self.requests.try_iter().collect()
    }
}

pub(super) async fn reuse(
    connection: &zbus::Connection,
    request: &OpenRequest,
    startup_id: &str,
) -> bool {
    let Ok(proxy) = zbus::fdo::DBusProxy::new(connection).await else {
        return false;
    };
    let Ok(names) = proxy.list_names().await else {
        return false;
    };
    let Some(folder) = Url::from_file_path(&request.folder).ok().map(String::from) else {
        return false;
    };
    let Some(items) = request
        .selected
        .iter()
        .map(|path| Url::from_file_path(path).ok().map(String::from))
        .collect::<Option<Vec<_>>>()
    else {
        return false;
    };
    let mut windows: Vec<_> = names
        .into_iter()
        .filter(|name| name.as_str().starts_with(PREFIX))
        .collect();
    windows.sort_by(|a, b| b.as_str().cmp(a.as_str()));
    for name in windows {
        if let Ok(reply) = connection
            .call_method(
                Some(name.as_str()),
                PATH,
                Some(INTERFACE),
                "Reveal",
                &(
                    folder.clone(),
                    items.clone(),
                    request.properties,
                    startup_id,
                ),
            )
            .await
            && reply.body().deserialize::<bool>().unwrap_or(false)
        {
            return true;
        }
    }
    false
}

/// The desktop launcher can hand its request to a GUI without creating another
/// native window or starting the public FileManager1 service.
pub fn reuse_folder(folder: &Path, startup_id: &str) -> bool {
    if crate::trashbin::is_location(folder) {
        return false;
    }
    let Ok(folder) = crate::filesystem::absolute_path(folder) else {
        return false;
    };
    zbus::block_on(async {
        let Ok(builder) = zbus::connection::Builder::session() else {
            return false;
        };
        let Ok(connection) = builder.method_timeout(Duration::from_secs(3)).build().await else {
            return false;
        };
        reuse(
            &connection,
            &OpenRequest {
                folder,
                selected: vec![],
                properties: false,
            },
            startup_id,
        )
        .await
    })
}

#[cfg(test)]
mod tests;
