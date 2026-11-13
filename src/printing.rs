//! Always obtain fresh user confirmation with PreparePrint before sending its token to Print.
use crate::{
    filesystem::Entry,
    preview::print::{self, Prepared},
};
use std::{
    collections::HashMap,
    fs::File,
    io,
    os::unix::fs::OpenOptionsExt,
    path::Path,
    pin::Pin,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    task::{Poll, Waker},
    time::Duration,
};
use zbus::zvariant::{Fd, OwnedObjectPath, OwnedValue as OwnedVariant, Value as Variant};
pub fn supported(entry: &Entry) -> bool {
    print::supported(entry)
}
pub fn run(entries: &[Entry], cancel: &Arc<AtomicBool>) -> Result<bool, String> {
    let result = execute(entries, cancel);
    if cancel.load(Ordering::Acquire) {
        return Ok(false);
    }
    result.map_err(|error| {
        crate::i18n::message("Não foi possível imprimir: {}", &[error.to_string()])
    })
}
fn execute(
    entries: &[Entry],
    cancel: &Arc<AtomicBool>,
) -> Result<bool, Box<dyn std::error::Error>> {
    if entries.is_empty() || entries.len() > 100 || entries.iter().any(|e| !supported(e)) {
        return Err(io::Error::other(crate::i18n::tr(
            "Selecione até 100 arquivos de tipos compatíveis com impressão.",
        ))
        .into());
    }
    let mut documents = Vec::<Prepared>::new();
    let mut bytes = 0u64;
    for entry in entries {
        if cancel.load(Ordering::Acquire) {
            return Ok(false);
        }
        let document = print::prepare(entry, cancel)?;
        bytes = bytes.saturating_add(std::fs::metadata(&document.path)?.len());
        if bytes > 512 * 1024 * 1024 {
            return Err(io::Error::other(crate::i18n::tr(
                "A seleção excede o limite de 512 MiB para impressão.",
            ))
            .into());
        }
        documents.push(document);
    }
    let merged = if documents.len() > 1 {
        print::merge(&documents, cancel)?
    } else {
        documents.pop().unwrap()
    };
    if cancel.load(Ordering::Acquire) {
        return Ok(false);
    }
    let title = if entries.len() == 1 {
        entries[0].name.clone()
    } else {
        crate::i18n::plural(
            "{} arquivo selecionado",
            "{} arquivos selecionados",
            entries.len(),
        )
    };
    let file = readonly(&merged.path)?;
    portal(&file, &title, cancel)
}
fn readonly(path: &Path) -> io::Result<File> {
    let file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(rustix::fs::OFlags::NONBLOCK.bits() as i32)
        .open(path)?;
    if !file.metadata()?.is_file() {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    Ok(file)
}
struct WakeCancel {
    finished: Arc<AtomicBool>,
    waker: Arc<Mutex<Option<Waker>>>,
}
impl WakeCancel {
    fn new(cancel: Arc<AtomicBool>) -> Self {
        let finished = Arc::new(AtomicBool::new(false));
        let waker = Arc::new(Mutex::new(None::<Waker>));
        let stop = finished.clone();
        let notify = waker.clone();
        std::thread::spawn(move || {
            while !stop.load(Ordering::Acquire) {
                if cancel.load(Ordering::Acquire) {
                    if let Some(w) = notify.lock().unwrap().as_ref() {
                        w.wake_by_ref();
                    }
                    break;
                }
                std::thread::sleep(Duration::from_millis(25));
            }
        });
        Self { finished, waker }
    }
}
impl Drop for WakeCancel {
    fn drop(&mut self) {
        self.finished.store(true, Ordering::Release);
    }
}
#[derive(Clone, Copy)]
enum Phase {
    Prepare,
    Submit(u32),
}
fn portal(
    file: &File,
    title: &str,
    cancel: &Arc<AtomicBool>,
) -> Result<bool, Box<dyn std::error::Error>> {
    zbus::block_on(async {
        let connection = zbus::connection::Builder::session()?
            .method_timeout(Duration::from_secs(15))
            .build()
            .await?;
        let Some(settings) =
            portal_request(&connection, file, title, cancel, Phase::Prepare).await?
        else {
            return Ok(false);
        };
        // KDE requires a QPrinter created by PreparePrint. Never reuse a previous
        // print job's configuration or call Print without this job's confirmation.
        let token = settings
            .get("token")
            .and_then(|value| u32::try_from(value).ok())
            .ok_or_else(|| {
                io::Error::other(crate::i18n::tr(
                    "O sistema não retornou uma configuração de impressão válida.",
                ))
            })?;
        if cancel.load(Ordering::Acquire) {
            return Ok(false);
        }
        Ok(
            portal_request(&connection, file, title, cancel, Phase::Submit(token))
                .await?
                .is_some(),
        )
    })
}
async fn portal_request(
    connection: &zbus::Connection,
    file: &File,
    title: &str,
    cancel: &Arc<AtomicBool>,
    phase: Phase,
) -> Result<Option<HashMap<String, OwnedVariant>>, Box<dyn std::error::Error>> {
    static SERIAL: AtomicU64 = AtomicU64::new(0);
    if cancel.load(Ordering::Acquire) {
        return Ok(None);
    }
    let sender = connection
        .unique_name()
        .ok_or_else(|| io::Error::other("Missing bus name"))?
        .as_str()
        .trim_start_matches(':')
        .replace('.', "_");
    let handle_token = format!(
        "lse_print_{}_{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    );
    let path = format!("/org/freedesktop/portal/desktop/request/{sender}/{handle_token}");
    let request = zbus::Proxy::new(
        connection,
        "org.freedesktop.portal.Desktop",
        path.as_str(),
        "org.freedesktop.portal.Request",
    )
    .await?;
    let mut responses = request.receive_signal("Response").await?;
    let portal = zbus::Proxy::new(
        connection,
        "org.freedesktop.portal.Desktop",
        "/org/freedesktop/portal/desktop",
        "org.freedesktop.portal.Print",
    )
    .await?;
    let mut options = HashMap::from([
        ("handle_token", Variant::from(handle_token.as_str())),
        ("modal", Variant::from(true)),
        ("supported_output_file_formats", Variant::from(vec!["pdf"])),
    ]);
    let handle: OwnedObjectPath = match phase {
        Phase::Prepare => {
            portal
                .call(
                    "PreparePrint",
                    &(
                        "",
                        title,
                        HashMap::<String, Variant>::new(),
                        HashMap::<String, Variant>::new(),
                        options,
                    ),
                )
                .await?
        }
        Phase::Submit(token) => {
            options.insert("token", Variant::from(token));
            portal
                .call("Print", &("", title, Fd::from(file), options))
                .await?
        }
    };
    if handle.as_str() != path {
        let _ = zbus::Proxy::new(
            connection,
            "org.freedesktop.portal.Desktop",
            handle,
            "org.freedesktop.portal.Request",
        )
        .await?
        .call::<_, _, ()>("Close", &())
        .await;
        return Err(io::Error::other("Unexpected print portal request handle").into());
    }
    let notify = WakeCancel::new(cancel.clone());
    let response = std::future::poll_fn(|cx| {
        *notify.waker.lock().unwrap() = Some(cx.waker().clone());
        if cancel.load(Ordering::Acquire) {
            return Poll::Ready(None);
        }
        zbus::export::futures_core::Stream::poll_next(Pin::new(&mut responses), cx)
    })
    .await;
    if cancel.load(Ordering::Acquire) {
        let _ = request.call::<_, _, ()>("Close", &()).await;
        return Ok(None);
    }
    let response = response.ok_or_else(|| {
        io::Error::other(crate::i18n::tr(
            "O diálogo de impressão foi encerrado sem resposta.",
        ))
    })?;
    let (code, results): (u32, HashMap<String, OwnedVariant>) = response.body().deserialize()?;
    match code {
        0 => Ok(Some(results)),
        1 => Ok(None),
        _ => Err(io::Error::other(crate::i18n::tr(match phase {
            Phase::Prepare => "O sistema não conseguiu abrir a configuração de impressão.",
            Phase::Submit(_) => "O sistema não conseguiu concluir a solicitação de impressão.",
        }))
        .into()),
    }
}
#[cfg(test)]
mod tests;
