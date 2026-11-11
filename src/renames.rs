use crate::filesystem::{self, Entry};
use rustix::fs::{AtFlags, Mode, OFlags, RenameFlags, open, renameat_with, statat};
use std::{io, path::PathBuf};

/// Rename one directory entry in its existing parent. Never replace another entry
/// or follow a symbolic link to rename its target.
pub fn rename(entry: &Entry, name: &str) -> io::Result<PathBuf> {
    if entry.trash.is_some() {
        return Err(io::Error::other(crate::i18n::tr(
            "Restaure o item da lixeira antes de renomeá-lo.",
        )));
    }
    let parent = entry
        .path
        .parent()
        .ok_or_else(|| io::Error::other(crate::i18n::tr("Destino inválido")))?;
    let destination = filesystem::child_path(parent, name)?;
    let source_name = entry
        .path
        .file_name()
        .ok_or_else(|| io::Error::other(crate::i18n::tr("Destino inválido")))?;
    let directory = open(
        parent,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
        Mode::empty(),
    )?;
    let metadata = statat(&directory, source_name, AtFlags::SYMLINK_NOFOLLOW)?;
    if metadata.st_dev != entry.device || metadata.st_ino != entry.inode {
        return Err(io::Error::other(crate::i18n::tr(
            "O item foi substituído desde a leitura da pasta. Atualize a lista e tente novamente.",
        )));
    }
    // Preserve exact filename bytes when an unchanged lossy display is confirmed.
    if name == entry.name {
        return Ok(entry.path.clone());
    }
    renameat_with(
        &directory,
        source_name,
        &directory,
        name,
        RenameFlags::NOREPLACE,
    )
    .map_err(|error| {
        let error = io::Error::from(error);
        if error.kind() == io::ErrorKind::AlreadyExists {
            io::Error::new(
                error.kind(),
                crate::i18n::tr("Já existe um arquivo ou pasta com esse nome."),
            )
        } else {
            error
        }
    })?;
    Ok(destination)
}

#[cfg(test)]
mod tests;
