use std::{
    fs, io,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Trash,
    Permanent,
}

#[derive(Clone)]
pub struct Target {
    pub path: PathBuf,
    pub device: u64,
    pub inode: u64,
}

#[derive(Default)]
pub struct Progress {
    pub done: usize,
    pub total: usize,
    pub current: PathBuf,
}

pub struct Report {
    pub mode: Mode,
    pub completed: Vec<PathBuf>,
    pub errors: Vec<String>,
    pub cancelled: bool,
}

pub fn run(
    targets: Vec<Target>,
    mode: Mode,
    cancel: &AtomicBool,
    mut progress: impl FnMut(Progress),
) -> Report {
    run_with(targets, mode, cancel, &mut progress, |path| {
        trash::delete(path).map_err(io::Error::other)
    })
}

fn run_with(
    targets: Vec<Target>,
    mode: Mode,
    cancel: &AtomicBool,
    progress: &mut impl FnMut(Progress),
    mut to_trash: impl FnMut(&Path) -> io::Result<()>,
) -> Report {
    let total = targets.len();
    let mut report = Report {
        mode,
        completed: Vec::new(),
        errors: Vec::new(),
        cancelled: false,
    };
    for (index, target) in targets.into_iter().enumerate() {
        if cancel.load(Ordering::Relaxed) {
            report.cancelled = true;
            break;
        }
        progress(Progress {
            done: index,
            total,
            current: target.path.clone(),
        });
        let result = (|| {
            // Never operate on a filesystem root or a relative/ambiguous target.
            if !target.path.is_absolute()
                || target.path.file_name().is_none()
                || target
                    .path
                    .components()
                    .any(|part| matches!(part, std::path::Component::ParentDir))
            {
                return Err(io::Error::other(crate::i18n::tr(
                    "Caminho inválido para exclusão",
                )));
            }
            if mode == Mode::Trash
                && crate::network::location::requires_permanent_deletion(&target.path, &[])
            {
                return Err(io::Error::new(
                    io::ErrorKind::Unsupported,
                    crate::i18n::tr("FTP não oferece lixeira. Confirme a exclusão permanente."),
                ));
            }
            let metadata = fs::symlink_metadata(&target.path)?;
            if metadata.dev() != target.device || metadata.ino() != target.inode {
                return Err(io::Error::other(crate::i18n::tr(
                    "O item mudou desde a seleção. Atualize a pasta e selecione-o novamente",
                )));
            }
            match mode {
                Mode::Trash => to_trash(&target.path),
                Mode::Permanent if metadata.is_dir() => fs::remove_dir_all(&target.path),
                Mode::Permanent => fs::remove_file(&target.path),
            }
        })();
        match result {
            Ok(()) => report.completed.push(target.path.clone()),
            Err(error) => report
                .errors
                .push(format!("{}: {error}", target.path.display())),
        }
        progress(Progress {
            done: index + 1,
            total,
            current: target.path,
        });
    }
    report
}

#[cfg(test)]
mod tests;
