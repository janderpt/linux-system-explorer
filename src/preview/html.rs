//! Static HTML/CSS presentation; the original source remains available without this runtime.
use super::{Document, check, command, read_limited};
use std::{
    fs, io,
    path::Path,
    process::Command,
    sync::{Arc, atomic::AtomicBool},
};

pub(super) fn convert(path: &Path, cancel: &AtomicBool) -> io::Result<Document> {
    const MAX_HTML_BYTES: u64 = 8 * 1024 * 1024;
    let data = read_limited(path, MAX_HTML_BYTES, cancel)?;
    if data.len() as u64 > MAX_HTML_BYTES {
        return Err(io::Error::other(crate::i18n::tr(
            "O HTML excede o limite de tamanho da pré-visualização.",
        )));
    }
    let work = Arc::new(tempfile::tempdir()?);
    let snapshot = work.path().join("source.html");
    fs::write(&snapshot, data)?;
    let document = Document::new(work.clone());
    check(cancel)?;
    command(
        Command::new("python3")
            .arg("-c")
            .arg(include_str!("html_renderer.py"))
            .arg(&snapshot)
            .arg(path)
            .arg(document.path())
            .current_dir(work.path()),
        cancel,
    )
    .map_err(|error| {
        if error.kind() == io::ErrorKind::NotFound
            || error.to_string().contains("LSE_HTML_RUNTIME_MISSING")
        {
            io::Error::new(
                io::ErrorKind::Unsupported,
                crate::i18n::tr(
                    "A apresentação de HTML requer WeasyPrint. O código-fonte continua disponível.",
                ),
            )
        } else {
            error
        }
    })?;
    check(cancel)?;
    Ok(document)
}
