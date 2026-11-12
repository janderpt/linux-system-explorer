//! Poppler needs random access, while some mounted remote files only stream.
use super::{Document, check, regular};
use std::{
    fs::File,
    io::{self, Read, Seek, SeekFrom, Write},
    path::Path,
    sync::{Arc, atomic::AtomicBool},
};

const MAX_SNAPSHOT_BYTES: u64 = 256 * 1024 * 1024;

pub(super) fn prepare(path: &Path, cancel: &AtomicBool) -> io::Result<Option<Document>> {
    check(cancel)?;
    regular(path)?;
    let input = File::from(rustix::fs::open(
        path,
        rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::NONBLOCK | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::empty(),
    )?);
    let metadata = input.metadata()?;
    if !metadata.is_file() {
        return Err(io::Error::from(io::ErrorKind::InvalidInput));
    }
    prepare_reader(input, metadata.len(), MAX_SNAPSHOT_BYTES, cancel)
}

fn prepare_reader(
    mut input: impl Read + Seek,
    expected: u64,
    limit: u64,
    cancel: &AtomicBool,
) -> io::Result<Option<Document>> {
    check(cancel)?;
    match input.seek(SeekFrom::End(0)) {
        Ok(_) => return Ok(None),
        Err(error)
            if error.raw_os_error() == Some(rustix::io::Errno::SPIPE.raw_os_error())
                || error.kind() == io::ErrorKind::Unsupported => {}
        Err(error) => return Err(error),
    }
    let too_large = || {
        io::Error::other(crate::i18n::message(
            "O documento excede o limite de {} MiB da pré-visualização.",
            &[(limit / 1024 / 1024).to_string()],
        ))
    };
    if expected > limit {
        return Err(too_large());
    }
    let document = Document::new(Arc::new(tempfile::tempdir()?));
    let mut output = File::create(document.path())?;
    let mut total = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        check(cancel)?;
        let size = input.read(&mut buffer)?;
        if size == 0 {
            break;
        }
        total += size as u64;
        if total > limit {
            return Err(too_large());
        }
        output.write_all(&buffer[..size])?;
    }
    check(cancel)?;
    if total != expected {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            crate::i18n::tr("O arquivo mudou durante a leitura. Tente novamente."),
        ));
    }
    // Publish only a complete, closed local stream; Document owns its cleanup.
    drop(output);
    Ok(Some(document))
}

#[cfg(test)]
mod tests;
