//! Header-only archive inspection; native parsers are isolated from the GUI process.
use super::{Content, check, command, read_limited};
use serde::Deserialize;
use std::{io, path::Path, process::Command, sync::atomic::AtomicBool};

#[derive(Deserialize)]
pub struct Listing {
    pub entries: Vec<Entry>,
    pub truncated: bool,
}
#[derive(Deserialize)]
pub struct Entry {
    pub path: String,
    pub size: Option<u64>,
    pub directory: bool,
    pub link: bool,
}

pub(super) fn load(path: &Path, cancel: &AtomicBool) -> io::Result<Content> {
    let work = tempfile::tempdir()?;
    let output = work.path().join("listing.json");
    command(
        Command::new("python3")
            .arg("-c")
            .arg(include_str!("archive_reader.py"))
            .arg(path)
            .arg(&output)
            .current_dir(work.path()),
        cancel,
    )
    .map_err(|error| match error.kind() {
        io::ErrorKind::Interrupted | io::ErrorKind::TimedOut => error,
        io::ErrorKind::NotFound => runtime_missing(),
        _ if error.to_string().contains("LSE_ARCHIVE_RUNTIME_MISSING") => runtime_missing(),
        _ => io::Error::other(crate::i18n::tr(
            "Não foi possível listar o arquivo compactado. Ele pode estar danificado, protegido por senha ou exceder os limites do preview.",
        )),
    })?;
    let bytes = read_limited(&output, 8 * 1024 * 1024, cancel)?;
    let listing = serde_json::from_slice(&bytes).map_err(io::Error::other)?;
    check(cancel)?;
    Ok(Content::Archive(listing))
}
fn runtime_missing() -> io::Error {
    io::Error::new(
        io::ErrorKind::Unsupported,
        crate::i18n::tr("O preview de compactados requer Python 3 e libarchive."),
    )
}
