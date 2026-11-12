//! One cancellable worker per tab, with bounded result delivery and no file-content reads.
use crate::filesystem::{self, Entry};
use rustix::fs::{self, AtFlags, Dir, FileType, Mode, OFlags};
use std::{
    collections::BTreeSet,
    ffi::OsStr,
    io,
    os::unix::ffi::OsStrExt,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};

pub const MAX_RESULTS: usize = 20_000;
const MAX_VISITED: usize = 1_000_000;
const MAX_DEPTH: usize = 128;
#[derive(Clone, Debug, Default)]
pub struct Progress {
    pub visited: usize,
    pub matched: usize,
    pub skipped: usize,
    pub limited: bool,
}
pub enum Update {
    Batch(Vec<Entry>),
    Progress(Progress),
    Finished(io::Result<Progress>),
}
struct Request {
    root: PathBuf,
    query: String,
    hidden: bool,
    cancelled: Arc<AtomicBool>,
    sender: mpsc::SyncSender<Update>,
    locale: crate::i18n::Locale,
}
pub struct Worker {
    requests: mpsc::Sender<Request>,
}
pub struct Job {
    pub incoming: mpsc::Receiver<Update>,
    cancelled: Arc<AtomicBool>,
}
impl Drop for Job {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Release);
    }
}
impl Worker {
    pub fn new(ctx: &eframe::egui::Context) -> Self {
        let (requests, incoming) = mpsc::channel::<Request>();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            while let Ok(mut request) = incoming.recv() {
                while let Ok(newer) = incoming.try_recv() {
                    request = newer;
                }
                if request.cancelled.load(Ordering::Acquire) {
                    continue;
                }
                crate::i18n::set_locale(request.locale);
                let mut batch = Vec::new();
                let mut last = Instant::now();
                let result = scan(&request, |entry, progress| {
                    if let Some(entry) = entry {
                        batch.push(entry);
                    }
                    if batch.len() >= 128 || last.elapsed() >= Duration::from_millis(100) {
                        if !batch.is_empty()
                            && !deliver(&request, &ctx, Update::Batch(std::mem::take(&mut batch)))
                        {
                            return false;
                        }
                        if request
                            .sender
                            .try_send(Update::Progress(progress.clone()))
                            .is_ok()
                        {
                            ctx.request_repaint();
                        }
                        last = Instant::now();
                    }
                    true
                });
                if !batch.is_empty() {
                    deliver(&request, &ctx, Update::Batch(batch));
                }
                deliver(&request, &ctx, Update::Finished(result));
            }
        });
        Self { requests }
    }
    pub fn start(&self, root: PathBuf, query: String, hidden: bool) -> Job {
        let (sender, incoming) = mpsc::sync_channel(4);
        let cancelled = Arc::new(AtomicBool::new(false));
        let request = Request {
            root,
            query: query.to_lowercase(),
            hidden,
            cancelled: cancelled.clone(),
            sender,
            locale: crate::i18n::current(),
        };
        let _ = self.requests.send(request);
        Job {
            incoming,
            cancelled,
        }
    }
}
fn deliver(request: &Request, ctx: &eframe::egui::Context, mut update: Update) -> bool {
    ctx.request_repaint();
    loop {
        if request.cancelled.load(Ordering::Acquire) {
            return false;
        }
        match request.sender.try_send(update) {
            Ok(()) => {
                ctx.request_repaint();
                return true;
            }
            Err(mpsc::TrySendError::Disconnected(_)) => return false,
            Err(mpsc::TrySendError::Full(value)) => {
                update = value;
                std::thread::sleep(Duration::from_millis(5));
            }
        }
    }
}
struct Frame {
    path: PathBuf,
    dir: Dir,
}
#[derive(Clone, Copy)]
struct Limits {
    results: usize,
    visited: usize,
    depth: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            results: MAX_RESULTS,
            visited: MAX_VISITED,
            depth: MAX_DEPTH,
        }
    }
}
fn scan(
    request: &Request,
    emit: impl FnMut(Option<Entry>, &Progress) -> bool,
) -> io::Result<Progress> {
    scan_with_limits(request, Limits::default(), emit)
}
fn scan_with_limits(
    request: &Request,
    limits: Limits,
    mut emit: impl FnMut(Option<Entry>, &Progress) -> bool,
) -> io::Result<Progress> {
    let cancelled = || request.cancelled.load(Ordering::Acquire);
    if cancelled() {
        return Err(io::ErrorKind::Interrupted.into());
    }
    // An explicitly opened root may itself be a symlink. Descendants never follow directory links.
    let fd = fs::open(
        &request.root,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
        Mode::empty(),
    )?;
    let root = fs::fstat(&fd)?;
    let mut visited = BTreeSet::from([(root.st_dev, root.st_ino)]);
    let mut stack = vec![Frame {
        path: request.root.clone(),
        dir: Dir::new(fd)?,
    }];
    let mut progress = Progress::default();
    let identities = uzers::UsersCache::new();
    while !stack.is_empty() {
        if cancelled() {
            return Err(io::ErrorKind::Interrupted.into());
        }
        let depth = stack.len();
        let frame = stack.last_mut().unwrap();
        let item = match frame.dir.next() {
            None => {
                stack.pop();
                continue;
            }
            Some(Err(_)) => {
                progress.skipped += 1;
                stack.pop();
                continue;
            }
            Some(Ok(item)) => item,
        };
        let name = item.file_name();
        let bytes = name.to_bytes();
        if matches!(bytes, b"." | b"..") || !request.hidden && bytes.starts_with(b".") {
            continue;
        }
        progress.visited += 1;
        let path = frame.path.join(OsStr::from_bytes(bytes));
        let stat = match fs::statat(frame.dir.fd()?, name, AtFlags::SYMLINK_NOFOLLOW) {
            Ok(s) => s,
            Err(_) => {
                progress.skipped += 1;
                continue;
            }
        };
        if OsStr::from_bytes(bytes)
            .to_string_lossy()
            .to_lowercase()
            .contains(&request.query)
        {
            match filesystem::entry_with_identities(&path, &identities) {
                Ok(entry) => {
                    progress.matched += 1;
                    if !emit(Some(entry), &progress) {
                        return Err(io::ErrorKind::Interrupted.into());
                    }
                }
                Err(_) => progress.skipped += 1,
            }
        }
        if progress.matched >= limits.results || progress.visited >= limits.visited {
            progress.limited = true;
            break;
        }
        if FileType::from_raw_mode(stat.st_mode) == FileType::Directory
            && visited.insert((stat.st_dev, stat.st_ino))
        {
            if depth >= limits.depth {
                progress.limited = true;
                progress.skipped += 1;
                continue;
            }
            let child = match fs::openat(
                frame.dir.fd()?,
                name,
                OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::empty(),
            ) {
                Ok(child) => child,
                Err(_) => {
                    progress.skipped += 1;
                    continue;
                }
            };
            let actual = fs::fstat(&child)?;
            if (stat.st_dev, stat.st_ino) != (actual.st_dev, actual.st_ino) {
                progress.skipped += 1;
                continue;
            }
            match Dir::new(child) {
                Ok(dir) => stack.push(Frame { path, dir }),
                Err(_) => progress.skipped += 1,
            }
        }
        if !emit(None, &progress) {
            return Err(io::ErrorKind::Interrupted.into());
        }
    }
    Ok(progress)
}

#[cfg(test)]
mod tests;
