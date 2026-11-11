use crate::{
    filesystem::{self, Entry, Listing},
    transfers,
};
use std::{
    ffi::OsString,
    fs, io,
    os::unix::fs::MetadataExt,
    path::{Component, Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};

/// A virtual location, distinct from every absolute filesystem directory.
pub fn location() -> PathBuf {
    PathBuf::from("trash:")
}
pub fn is_location(path: &Path) -> bool {
    path == Path::new("trash:")
}

#[derive(Clone, Debug)]
pub struct Record {
    pub original_path: PathBuf,
    pub deleted: Option<SystemTime>,
    info_path: PathBuf,
    info_bytes: Vec<u8>,
    info_device: u64,
    info_inode: u64,
}

pub fn read() -> io::Result<Listing> {
    let items = trash::os_limited::list().map_err(io::Error::other)?;
    Ok(read_items(items))
}

fn read_items(items: Vec<trash::TrashItem>) -> Listing {
    let mut entries = Vec::new();
    let mut skipped = 0;
    for item in items {
        match read_item(item) {
            Ok(entry) => entries.push(entry),
            Err(_) => skipped += 1,
        }
    }
    Listing {
        path: location(),
        entries,
        skipped,
    }
}

fn read_item(item: trash::TrashItem) -> io::Result<Entry> {
    let info_path = PathBuf::from(&item.id);
    let info_parent = info_path
        .parent()
        .filter(|parent| parent.file_name().is_some_and(|name| name == "info"))
        .ok_or_else(|| io::Error::other(crate::i18n::tr("Registro da lixeira inválido")))?;
    let root = info_parent
        .parent()
        .ok_or_else(|| io::Error::other(crate::i18n::tr("Registro da lixeira inválido")))?;
    let name = info_path
        .file_stem()
        .filter(|_| {
            info_path
                .extension()
                .is_some_and(|extension| extension == "trashinfo")
        })
        .ok_or_else(|| io::Error::other(crate::i18n::tr("Registro da lixeira inválido")))?;
    let original_path = item.original_path();
    if !info_path.is_absolute()
        || !original_path.is_absolute()
        || original_path.file_name().is_none()
        || info_path
            .components()
            .chain(original_path.components())
            .any(|component| component == Component::ParentDir)
    {
        return Err(io::Error::other(crate::i18n::tr(
            "Registro da lixeira inválido",
        )));
    }
    let metadata = fs::symlink_metadata(&info_path)?;
    if !metadata.is_file() {
        return Err(io::Error::other(crate::i18n::tr(
            "Registro da lixeira inválido",
        )));
    }
    let info_bytes = fs::read(&info_path)?;
    let mut entry = filesystem::read_entry(&root.join("files").join(name))?;
    entry.name = item.name.to_string_lossy().into_owned();
    // The Trash lists every item, including names starting with a dot.
    entry.hidden = false;
    entry.trash = Some(Record {
        original_path,
        info_path,
        info_bytes,
        info_device: metadata.dev(),
        info_inode: metadata.ino(),
        deleted: u64::try_from(item.time_deleted)
            .ok()
            .and_then(|seconds| UNIX_EPOCH.checked_add(Duration::from_secs(seconds))),
    });
    Ok(entry)
}

pub struct Report {
    pub completed: Vec<PathBuf>,
    pub skipped: usize,
    pub errors: Vec<String>,
    pub cancelled: bool,
}

fn validate_info(record: &Record) -> io::Result<()> {
    let info = fs::symlink_metadata(&record.info_path)?;
    if !info.is_file()
        || info.dev() != record.info_device
        || info.ino() != record.info_inode
        || fs::read(&record.info_path)? != record.info_bytes
    {
        return Err(io::Error::other(crate::i18n::tr(
            "O item da lixeira mudou. Atualize a lista antes de restaurar.",
        )));
    }
    Ok(())
}

fn validate(entry: &Entry, record: &Record) -> io::Result<()> {
    let metadata = fs::symlink_metadata(&entry.path)?;
    if metadata.dev() != entry.device || metadata.ino() != entry.inode {
        return Err(io::Error::other(crate::i18n::tr(
            "O item da lixeira mudou. Atualize a lista antes de restaurar.",
        )));
    }
    validate_info(record)
}

fn alternative(original: &Path, directory: bool, number: usize) -> io::Result<PathBuf> {
    let name = original
        .file_name()
        .ok_or_else(|| io::Error::other(crate::i18n::tr("Destino inválido")))?;
    let filename = Path::new(name);
    let mut candidate: OsString = if directory {
        name
    } else {
        filename.file_stem().unwrap_or(name)
    }
    .to_owned();
    candidate.push(if number == 1 {
        crate::i18n::tr(" (restaurado)").to_owned()
    } else {
        crate::i18n::message(" (restaurado {})", &[number.to_string()])
    });
    if !directory && let Some(extension) = filename.extension() {
        candidate.push(".");
        candidate.push(extension);
    }
    Ok(original.with_file_name(candidate))
}

pub fn restore(
    entries: Vec<Entry>,
    cancel: Arc<AtomicBool>,
    mut resolve: impl FnMut(transfers::Conflict) -> transfers::Decision,
    mut progress: impl FnMut(transfers::Progress),
) -> Report {
    let total = entries.len();
    let mut report = Report {
        completed: Vec::new(),
        skipped: 0,
        errors: Vec::new(),
        cancelled: false,
    };
    let mut policy = None;
    for (index, entry) in entries.into_iter().enumerate() {
        if cancel.load(Ordering::Relaxed) {
            report.cancelled = true;
            break;
        }
        let Some(record) = &entry.trash else {
            report
                .errors
                .push(crate::i18n::tr("Registro da lixeira inválido").into());
            continue;
        };
        let original = &record.original_path;
        progress(transfers::Progress {
            done: index,
            total,
            current: original.clone(),
            bytes: 0,
        });
        let outcome = (|| -> io::Result<bool> {
            validate(&entry, record)?;
            let parent = original
                .parent()
                .ok_or_else(|| io::Error::other(crate::i18n::tr("Destino inválido")))?;
            fs::create_dir_all(parent)?;
            let mut target = original.clone();
            let mut keep_both = false;
            for number in 1..=100_000 {
                if cancel.load(Ordering::Relaxed) {
                    return Ok(false);
                }
                validate(&entry, record)?;
                // Detect conflicts before an expensive cross-filesystem copy. The final
                // move still uses NOREPLACE, so a concurrent destination is protected too.
                let result = match fs::symlink_metadata(&target) {
                    Ok(_) => Err(io::Error::from(io::ErrorKind::AlreadyExists)),
                    Err(error) if error.kind() == io::ErrorKind::NotFound => {
                        transfers::move_no_replace(&entry.path, &target, &cancel, |mut update| {
                            update.done = index;
                            update.total = total;
                            update.current = original.clone();
                            progress(update);
                        })
                    }
                    Err(error) => Err(error),
                };
                match result {
                    Ok(()) => {
                        // The data is restored. A failed metadata cleanup is a warning, not a retry.
                        report.completed.push(entry.path.clone());
                        if let Err(error) =
                            validate_info(record).and_then(|()| fs::remove_file(&record.info_path))
                        {
                            report.errors.push(crate::i18n::message("{} foi restaurado, mas o registro da lixeira não pôde ser removido: {}", &[original.display().to_string(), error.to_string()]));
                        }
                        return Ok(true);
                    }
                    Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                        if !keep_both {
                            let decision = match policy {
                                Some(choice) => transfers::Decision {
                                    choice,
                                    apply_all: true,
                                },
                                None => resolve(transfers::Conflict {
                                    source: original.clone(),
                                    destination: target.clone(),
                                }),
                            };
                            if decision.apply_all && decision.choice != transfers::Choice::Cancel {
                                policy = Some(decision.choice);
                            }
                            match decision.choice {
                                transfers::Choice::Cancel => {
                                    cancel.store(true, Ordering::Relaxed);
                                    return Ok(false);
                                }
                                transfers::Choice::Skip => return Ok(false),
                                transfers::Choice::KeepBoth => keep_both = true,
                            }
                        }
                        target = alternative(original, entry.is_dir && !entry.is_symlink, number)?;
                    }
                    Err(error) => return Err(error),
                }
            }
            Err(io::Error::other(crate::i18n::tr(
                "Não foi possível gerar um nome disponível",
            )))
        })();
        match outcome {
            Ok(true) => (),
            _ if cancel.load(Ordering::Relaxed) => {
                report.cancelled = true;
                break;
            }
            Ok(false) => report.skipped += 1,
            Err(error) => report
                .errors
                .push(format!("{}: {error}", original.display())),
        }
        progress(transfers::Progress {
            done: index + 1,
            total,
            current: original.clone(),
            bytes: 0,
        });
    }
    report
}

/// Permanently remove only the confirmed snapshot, preserving later additions to the trash.
pub fn purge(
    entries: Vec<Entry>,
    cancel: Arc<AtomicBool>,
    mut progress: impl FnMut(transfers::Progress),
) -> Report {
    let total = entries.len();
    let mut report = Report {
        completed: Vec::new(),
        skipped: 0,
        errors: Vec::new(),
        cancelled: false,
    };
    for (index, entry) in entries.into_iter().enumerate() {
        if cancel.load(Ordering::Relaxed) {
            report.cancelled = true;
            break;
        }
        let Some(record) = &entry.trash else {
            report
                .errors
                .push(crate::i18n::tr("Registro da lixeira inválido").into());
            continue;
        };
        progress(transfers::Progress {
            done: index,
            total,
            current: record.original_path.clone(),
            bytes: 0,
        });
        let result = (|| -> io::Result<()> {
            validate(&entry, record)?;
            // lstat and remove_dir_all keep links as links, including links inside folders.
            if fs::symlink_metadata(&entry.path)?.is_dir() {
                fs::remove_dir_all(&entry.path)?;
            } else {
                fs::remove_file(&entry.path)?;
            }
            report.completed.push(entry.path.clone());
            validate_info(record).and_then(|()| fs::remove_file(&record.info_path))
        })();
        if let Err(error) = result {
            report
                .errors
                .push(format!("{}: {error}", record.original_path.display()));
        }
        progress(transfers::Progress {
            done: index + 1,
            total,
            current: record.original_path.clone(),
            bytes: 0,
        });
    }
    report
}

#[cfg(test)]
mod tests;
