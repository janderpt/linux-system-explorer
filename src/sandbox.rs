//! Flatpak uses portals for host application launching, never host shell escapes.
use std::{
    collections::HashMap,
    fs::File,
    io,
    path::Path,
    pin::Pin,
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};
use zbus::zvariant::{Fd, OwnedObjectPath, OwnedValue as OwnedVariant, Value as Variant};

pub fn active() -> bool {
    std::env::var_os("FLATPAK_ID").is_some() || Path::new("/.flatpak-info").exists()
}

pub fn host_operation_error() -> io::Error {
    io::Error::new(
        io::ErrorKind::Unsupported,
        crate::i18n::tr(
            "Esta integração com o sistema ainda não está disponível no Flatpak experimental.",
        ),
    )
}

pub fn open_file(path: &Path, ask: bool) -> Result<(), String> {
    open(path, ask).map_err(|error| {
        crate::i18n::message(
            "Não foi possível abrir o arquivo pelo portal do sistema: {}",
            &[error.to_string()],
        )
    })
}

fn open(path: &Path, ask: bool) -> Result<(), Box<dyn std::error::Error>> {
    static SERIAL: AtomicU64 = AtomicU64::new(0);
    let flags = rustix::fs::OFlags::NONBLOCK | rustix::fs::OFlags::CLOEXEC;
    let (descriptor, writable) = match rustix::fs::open(
        path,
        flags | rustix::fs::OFlags::RDWR,
        rustix::fs::Mode::empty(),
    ) {
        Ok(fd) => (fd, true),
        Err(_) => (
            rustix::fs::open(
                path,
                flags | rustix::fs::OFlags::RDONLY,
                rustix::fs::Mode::empty(),
            )?,
            false,
        ),
    };
    let file = File::from(descriptor);
    if !file.metadata()?.is_file() {
        return Err(io::Error::from(io::ErrorKind::InvalidInput).into());
    }
    zbus::block_on(async {
        let connection = zbus::connection::Builder::session()?
            .method_timeout(Duration::from_secs(15))
            .build()
            .await?;
        let sender = connection
            .unique_name()
            .ok_or_else(|| io::Error::other("Missing D-Bus unique name"))?
            .as_str()
            .trim_start_matches(':')
            .replace('.', "_");
        let token = format!(
            "lse_{}_{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        );
        let request_path = format!("/org/freedesktop/portal/desktop/request/{sender}/{token}");
        let request = zbus::Proxy::new(
            &connection,
            "org.freedesktop.portal.Desktop",
            request_path.as_str(),
            "org.freedesktop.portal.Request",
        )
        .await?;
        // Subscribe before OpenFile: a backend may respond before the method returns.
        let mut responses = request.receive_signal("Response").await?;
        let portal = zbus::Proxy::new(
            &connection,
            "org.freedesktop.portal.Desktop",
            "/org/freedesktop/portal/desktop",
            "org.freedesktop.portal.OpenURI",
        )
        .await?;
        let mut options = HashMap::from([
            ("handle_token", Variant::from(token.as_str())),
            ("ask", Variant::from(ask)),
            ("writable", Variant::from(writable)),
        ]);
        let activation = std::env::var("XDG_ACTIVATION_TOKEN").unwrap_or_default();
        if !activation.is_empty() {
            options.insert("activation_token", Variant::from(activation.as_str()));
        }
        let handle: OwnedObjectPath = portal
            .call("OpenFile", &("", Fd::from(&file), options))
            .await?;
        if handle.as_str() != request_path {
            let _ = zbus::Proxy::new(
                &connection,
                "org.freedesktop.portal.Desktop",
                handle,
                "org.freedesktop.portal.Request",
            )
            .await?
            .call::<_, _, ()>("Close", &())
            .await;
            return Err(io::Error::other("Unexpected portal request handle").into());
        }
        let response = std::future::poll_fn(|cx| {
            zbus::export::futures_core::Stream::poll_next(Pin::new(&mut responses), cx)
        })
        .await
        .ok_or_else(|| io::Error::other("Portal closed without a response"))?;
        let (code, _): (u32, HashMap<String, OwnedVariant>) = response.body().deserialize()?;
        match code {
            0 | 1 => Ok(()), // Cancellation is not an operational failure.
            _ => Err(io::Error::other(crate::i18n::tr(
                "O sistema não conseguiu abrir o arquivo selecionado.",
            ))
            .into()),
        }
    })
}

#[cfg(test)]
pub(crate) mod tests;
