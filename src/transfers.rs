use rustix::fs::{CWD, RenameFlags, renameat_with};
use std::{
    collections::BTreeSet,
    ffi::OsString,
    fs::{self, File, FileTimes, Metadata, OpenOptions},
    io::{self, Read, Write},
    os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt, symlink},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

#[cfg(test)]
mod tests;

struct Stage(tempfile::TempDir);

impl Drop for Stage {
    fn drop(&mut self) {
        writable_directories(self.0.path());
    }
}

fn writable_directories(path: &Path) {
    if fs::symlink_metadata(path).is_ok_and(|metadata| metadata.is_dir()) {
        if let Ok(metadata) = fs::symlink_metadata(path) {
            let _ = fs::set_permissions(path, fs::Permissions::from_mode(metadata.mode() | 0o700));
        }
        if let Ok(entries) = fs::read_dir(path) {
            for entry in entries.flatten() {
                writable_directories(&entry.path());
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Copy,
    Cut,
}

#[derive(Clone)]
pub struct Clipboard {
    pub id: u64,
    pub mode: Mode,
    pub paths: Vec<PathBuf>,
}

#[derive(Clone)]
pub struct Request {
    pub clipboard: Clipboard,
    pub destination: PathBuf,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Choice {
    KeepBoth,
    Skip,
    Cancel,
}

pub struct Decision {
    pub choice: Choice,
    pub apply_all: bool,
}
pub struct Conflict {
    pub source: PathBuf,
    pub destination: PathBuf,
}

#[derive(Clone, Default)]
pub struct Progress {
    pub done: usize,
    pub total: usize,
    pub bytes: u64,
    pub current: PathBuf,
}

pub struct Report {
    pub clipboard_id: u64,
    pub mode: Mode,
    pub completed: Vec<PathBuf>,
    pub skipped: usize,
    pub errors: Vec<String>,
    pub cancelled: bool,
}

struct Work<'a> {
    cancel: &'a AtomicBool,
    progress: Progress,
    notify: &'a mut dyn FnMut(Progress),
}

impl Work<'_> {
    fn check(&self) -> io::Result<()> {
        if self.cancel.load(Ordering::Relaxed) {
            Err(io::Error::new(
                io::ErrorKind::Interrupted,
                crate::i18n::tr("Transferência cancelada"),
            ))
        } else {
            Ok(())
        }
    }
}

#[derive(Clone)]
struct Stamp {
    relative: PathBuf,
    metadata: Metadata,
}

pub fn run(
    request: Request,
    cancel: Arc<AtomicBool>,
    mut resolve: impl FnMut(Conflict) -> Decision,
    mut notify: impl FnMut(Progress),
) -> Report {
    let mut report = Report {
        clipboard_id: request.clipboard.id,
        mode: request.clipboard.mode,
        completed: Vec::new(),
        skipped: 0,
        errors: Vec::new(),
        cancelled: false,
    };
    let mut work = Work {
        cancel: &cancel,
        progress: Progress {
            total: request.clipboard.paths.len(),
            ..Default::default()
        },
        notify: &mut notify,
    };
    let destination = match fs::canonicalize(&request.destination).and_then(|path| {
        if path.is_dir() {
            Ok(path)
        } else {
            Err(io::Error::new(
                io::ErrorKind::NotADirectory,
                crate::i18n::tr("O destino não é uma pasta"),
            ))
        }
    }) {
        Ok(path) => path,
        Err(error) => {
            report
                .errors
                .push(format!("{}: {error}", request.destination.display()));
            return report;
        }
    };
    let mut seen = BTreeSet::new();
    let mut policy = None;
    for source in request.clipboard.paths {
        if !seen.insert(source.clone()) {
            continue;
        }
        if cancel.load(Ordering::Relaxed) {
            report.cancelled = true;
            break;
        }
        work.progress.current = source.clone();
        (work.notify)(work.progress.clone());
        let outcome = (|| -> io::Result<bool> {
            let metadata = fs::symlink_metadata(&source)?;
            let name = source.file_name().ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    crate::i18n::tr("Não é possível transferir a raiz do sistema"),
                )
            })?;
            if metadata.is_dir() && destination.starts_with(fs::canonicalize(&source)?) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    crate::i18n::tr("Uma pasta não pode ser colada dentro dela mesma"),
                ));
            }
            let original = destination.join(name);
            if request.clipboard.mode == Mode::Cut && same_entry(&source, &original)? {
                return Ok(true);
            }
            let mut target = original.clone();
            loop {
                work.check()?;
                if exists(&target)? {
                    let choice = match policy {
                        Some(choice) => choice,
                        None => {
                            let decision = resolve(Conflict {
                                source: source.clone(),
                                destination: target.clone(),
                            });
                            if decision.apply_all && decision.choice != Choice::Cancel {
                                policy = Some(decision.choice);
                            }
                            decision.choice
                        }
                    };
                    match choice {
                        Choice::Skip => return Ok(false),
                        Choice::Cancel => {
                            cancel.store(true, Ordering::Relaxed);
                            work.check()?;
                        }
                        Choice::KeepBoth => {
                            target = unique_name(&destination, name)?;
                        }
                    }
                }
                match transfer_item(&source, &target, request.clipboard.mode, &mut work) {
                    Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                        target = original.clone();
                    }
                    result => {
                        result?;
                        return Ok(true);
                    }
                }
            }
        })();
        match outcome {
            Ok(true) => report.completed.push(source),
            Ok(false) => report.skipped += 1,
            Err(error)
                if cancel.load(Ordering::Relaxed) && error.kind() == io::ErrorKind::Interrupted =>
            {
                report.cancelled = true;
                break;
            }
            Err(error) => report.errors.push(format!("{}: {error}", source.display())),
        }
        work.progress.done += 1;
        (work.notify)(work.progress.clone());
    }
    report
}

fn exists(path: &Path) -> io::Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e),
    }
}

fn same_entry(source: &Path, target: &Path) -> io::Result<bool> {
    match (fs::symlink_metadata(source), fs::symlink_metadata(target)) {
        (Ok(a), Ok(b)) => Ok(a.dev() == b.dev() && a.ino() == b.ino()),
        (_, Err(e)) if e.kind() == io::ErrorKind::NotFound => Ok(false),
        (Err(e), _) | (_, Err(e)) => Err(e),
    }
}

fn rename_new(source: &Path, destination: &Path) -> io::Result<()> {
    renameat_with(CWD, source, CWD, destination, RenameFlags::NOREPLACE).map_err(Into::into)
}

fn unique_name(directory: &Path, name: &std::ffi::OsStr) -> io::Result<PathBuf> {
    let name = Path::new(name);
    for number in 1..=100_000 {
        let mut candidate: OsString = name.file_stem().unwrap_or(name.as_os_str()).to_owned();
        candidate.push(if number == 1 {
            crate::i18n::tr(" (cópia)").to_owned()
        } else {
            crate::i18n::message(" (cópia {number})", &[(number).to_string()])
        });
        if let Some(extension) = name.extension() {
            candidate.push(".");
            candidate.push(extension);
        }
        let path = directory.join(candidate);
        if !exists(&path)? {
            return Ok(path);
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        crate::i18n::tr("Não foi possível gerar um nome disponível"),
    ))
}

/// Shared by restoring trash items, including a safe fallback across filesystems.
pub fn move_no_replace(
    source: &Path,
    destination: &Path,
    cancel: &AtomicBool,
    mut notify: impl FnMut(Progress),
) -> io::Result<()> {
    let mut work = Work {
        cancel,
        progress: Progress {
            total: 1,
            current: source.to_owned(),
            ..Default::default()
        },
        notify: &mut notify,
    };
    transfer_item(source, destination, Mode::Cut, &mut work)
}

fn transfer_item(
    source: &Path,
    destination: &Path,
    mode: Mode,
    work: &mut Work<'_>,
) -> io::Result<()> {
    work.check()?;
    if mode == Mode::Cut {
        match rename_new(source, destination) {
            Ok(()) => return Ok(()),
            Err(error) if error.kind() == io::ErrorKind::CrossesDevices => (),
            Err(error) => return Err(error),
        }
    }
    copy_then_publish(source, destination, mode, work)
}

fn copy_then_publish(
    source: &Path,
    destination: &Path,
    mode: Mode,
    work: &mut Work<'_>,
) -> io::Result<()> {
    let parent = destination.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            crate::i18n::tr("Destino inválido"),
        )
    })?;
    let stage = Stage(
        tempfile::Builder::new()
            .prefix(".linux-system-explorer-transfer-")
            .tempdir_in(parent)?,
    );
    let payload = stage.0.path().join("item");
    let mut manifest = Vec::new();
    copy_node(source, &payload, Path::new(""), &mut manifest, work)?;
    work.check()?;
    for stamp in &manifest {
        verify(
            &source_path(source, &stamp.relative),
            &stamp.metadata,
            false,
        )?;
    }
    rename_new(&payload, destination)?;
    if mode == Mode::Cut {
        // Never remove an unverified or newly added source. The complete destination is already committed.
        for stamp in manifest.iter().rev() {
            let path = source_path(source, &stamp.relative);
            verify(&path, &stamp.metadata, stamp.metadata.is_dir()).map_err(|e| {
                io::Error::other(crate::i18n::message(
                    "Cópia concluída, mas a origem foi preservada parcialmente: {e}",
                    &[(e).to_string()],
                ))
            })?;
            let result = if stamp.metadata.is_dir() {
                fs::remove_dir(&path)
            } else {
                fs::remove_file(&path)
            };
            result.map_err(|e| {
                io::Error::other(crate::i18n::message(
                    "Cópia concluída, mas não foi possível remover toda a origem: {e}",
                    &[(e).to_string()],
                ))
            })?;
        }
    }
    Ok(())
}

fn source_path(root: &Path, relative: &Path) -> PathBuf {
    if relative.as_os_str().is_empty() {
        root.to_owned()
    } else {
        root.join(relative)
    }
}

fn verify(path: &Path, original: &Metadata, directory_removal: bool) -> io::Result<()> {
    let current = fs::symlink_metadata(path)?;
    let same = current.dev() == original.dev()
        && current.ino() == original.ino()
        && current.file_type() == original.file_type();
    let unchanged = directory_removal
        || (current.len() == original.len()
            && current.mtime() == original.mtime()
            && current.mtime_nsec() == original.mtime_nsec()
            && current.ctime() == original.ctime()
            && current.ctime_nsec() == original.ctime_nsec());
    if same && unchanged {
        Ok(())
    } else {
        Err(io::Error::other(crate::i18n::message(
            "A origem mudou durante a transferência: {}",
            &[(path.display()).to_string()],
        )))
    }
}

fn copy_node(
    source: &Path,
    destination: &Path,
    relative: &Path,
    manifest: &mut Vec<Stamp>,
    work: &mut Work<'_>,
) -> io::Result<()> {
    work.check()?;
    let metadata = fs::symlink_metadata(source)?;
    manifest.push(Stamp {
        relative: relative.to_owned(),
        metadata: metadata.clone(),
    });
    if metadata.is_symlink() {
        symlink(fs::read_link(source)?, destination)?;
    } else if metadata.is_dir() {
        fs::create_dir(destination)?;
        let entries = fs::read_dir(source)?;
        verify(source, &metadata, false)?;
        for entry in entries {
            let entry = entry?;
            copy_node(
                &entry.path(),
                &destination.join(entry.file_name()),
                &relative.join(entry.file_name()),
                manifest,
                work,
            )?;
        }
        apply_metadata(destination, &metadata)?;
    } else if metadata.is_file() {
        let mut input = OpenOptions::new()
            .read(true)
            .custom_flags(
                (rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::NONBLOCK).bits() as i32,
            )
            .open(source)?;
        let opened = input.metadata()?;
        if opened.dev() != metadata.dev() || opened.ino() != metadata.ino() {
            return Err(io::Error::other(crate::i18n::tr(
                "A origem mudou antes de ser lida",
            )));
        }
        let mut output = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(destination)?;
        let mut buffer = vec![0u8; 256 * 1024];
        loop {
            work.check()?;
            let count = input.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            output.write_all(&buffer[..count])?;
            work.progress.bytes = work.progress.bytes.saturating_add(count as u64);
            (work.notify)(work.progress.clone());
        }
        output.sync_all()?;
        apply_metadata(destination, &metadata)?;
    } else {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            crate::i18n::tr(
                "Sockets, FIFOs e dispositivos não podem ser copiados como arquivos comuns",
            ),
        ));
    }
    Ok(())
}

fn apply_metadata(path: &Path, metadata: &Metadata) -> io::Result<()> {
    let file = File::open(path)?;
    let mut times = FileTimes::new();
    if let Ok(time) = metadata.modified() {
        times = times.set_modified(time);
    }
    if let Ok(time) = metadata.accessed() {
        times = times.set_accessed(time);
    }
    file.set_times(times)?;
    file.set_permissions(metadata.permissions())
}
