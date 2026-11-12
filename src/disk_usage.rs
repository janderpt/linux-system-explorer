//! Folder totals using fd-relative, no-follow traversal; never opens file contents.
use rustix::fs::{self, AtFlags, Dir, FileType, Mode, OFlags, Stat};
use std::{
    collections::BTreeSet,
    io,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};

#[derive(Clone, Debug, Default)]
pub struct Totals {
    pub bytes: u64,
    pub allocated: u64,
    pub files: u64,
    pub directories: u64,
    pub skipped: u64,
}
#[derive(Clone, Default)]
pub struct Progress {
    pub totals: Totals,
    pub done: bool,
    pub error: Option<String>,
}
pub struct Job {
    pub path: PathBuf,
    pub progress: Progress,
    incoming: mpsc::Receiver<Progress>,
    cancelled: Arc<AtomicBool>,
}
impl Job {
    pub fn start(path: PathBuf, ctx: &eframe::egui::Context) -> Self {
        let (out, incoming) = mpsc::sync_channel(1);
        let cancelled = Arc::new(AtomicBool::new(false));
        let flag = cancelled.clone();
        let context = ctx.clone();
        let source = path.clone();
        std::thread::spawn(move || {
            let mut last = Instant::now();
            let result = scan(&source, &flag, |totals| {
                if last.elapsed() >= Duration::from_millis(100) {
                    let _ = out.try_send(Progress {
                        totals: totals.clone(),
                        done: false,
                        error: None,
                    });
                    context.request_repaint();
                    last = Instant::now();
                }
            });
            if flag.load(Ordering::Acquire) {
                return;
            }
            let progress = match result {
                Ok(totals) => Progress {
                    totals,
                    done: true,
                    error: None,
                },
                Err(error) => Progress {
                    totals: Totals::default(),
                    done: true,
                    error: Some(error.to_string()),
                },
            };
            context.request_repaint();
            let _ = out.send(progress);
            context.request_repaint();
        });
        Self {
            path,
            progress: Progress::default(),
            incoming,
            cancelled,
        }
    }
    pub fn poll(&mut self) {
        for progress in self.incoming.try_iter() {
            self.progress = progress;
        }
    }
}
impl Drop for Job {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Release);
    }
}
fn add_allocated(totals: &mut Totals, stat: &Stat, links: &mut BTreeSet<(u64, u64)>) {
    if stat.st_nlink <= 1 || links.insert((stat.st_dev, stat.st_ino)) {
        totals.allocated = totals
            .allocated
            .saturating_add((stat.st_blocks.max(0) as u64).saturating_mul(512));
    }
}
fn scan(path: &Path, cancel: &AtomicBool, mut progress: impl FnMut(&Totals)) -> io::Result<Totals> {
    let flags = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC;
    if cancel.load(Ordering::Acquire) {
        return Err(io::ErrorKind::Interrupted.into());
    }
    let fd = fs::open(path, flags, Mode::empty())?;
    let root = fs::fstat(&fd)?;
    let mut totals = Totals::default();
    let mut links = BTreeSet::new();
    add_allocated(&mut totals, &root, &mut links);
    let mut visited = BTreeSet::from([(root.st_dev, root.st_ino)]);
    let mut stack = vec![Dir::new(fd)?];
    while let Some(dir) = stack.last_mut() {
        if cancel.load(Ordering::Acquire) {
            return Err(io::ErrorKind::Interrupted.into());
        }
        let item = match dir.next() {
            None => {
                stack.pop();
                continue;
            }
            Some(Err(_)) => {
                totals.skipped += 1;
                stack.pop();
                continue;
            }
            Some(Ok(item)) => item,
        };
        if matches!(item.file_name().to_bytes(), b"." | b"..") {
            continue;
        }
        let stat = match fs::statat(dir.fd()?, item.file_name(), AtFlags::SYMLINK_NOFOLLOW) {
            Ok(stat) => stat,
            Err(_) => {
                totals.skipped += 1;
                continue;
            }
        };
        if FileType::from_raw_mode(stat.st_mode) == FileType::Directory {
            if !visited.insert((stat.st_dev, stat.st_ino)) {
                continue;
            }
            totals.directories += 1;
            add_allocated(&mut totals, &stat, &mut links);
            // NOFOLLOW prevents a folder replaced by a symlink from escaping the tree.
            let child = match fs::openat(dir.fd()?, item.file_name(), flags, Mode::empty()) {
                Ok(fd) => fd,
                Err(_) => {
                    totals.skipped += 1;
                    continue;
                }
            };
            let actual = fs::fstat(&child)?;
            if (stat.st_dev, stat.st_ino) != (actual.st_dev, actual.st_ino) {
                totals.skipped += 1;
                continue;
            }
            match Dir::new(child) {
                Ok(dir) => stack.push(dir),
                Err(_) => totals.skipped += 1,
            }
        } else {
            totals.files += 1;
            totals.bytes = totals.bytes.saturating_add(stat.st_size.max(0) as u64);
            add_allocated(&mut totals, &stat, &mut links);
        }
        progress(&totals);
    }
    Ok(totals)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::{
        ffi::OsStringExt,
        fs::{MetadataExt, symlink},
    };
    #[test]
    fn folder_usage_includes_hidden_sparse_and_hardlinked_files_without_following_symlinks() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::fs::write(outside.path().join("do-not-count"), vec![0; 4096]).unwrap();
        std::fs::create_dir(root.path().join("nested")).unwrap();
        let file = root.path().join("nested/.sparse");
        std::fs::File::create(&file)
            .unwrap()
            .set_len(32 * 1024 * 1024)
            .unwrap();
        std::fs::write(root.path().join("plain"), b"hello").unwrap();
        std::fs::hard_link(root.path().join("plain"), root.path().join("alias")).unwrap();
        symlink(outside.path(), root.path().join("link")).unwrap();
        symlink("missing", root.path().join("broken")).unwrap();
        let result = scan(root.path(), &AtomicBool::new(false), |_| {}).unwrap();
        assert_eq!(result.files, 5);
        assert_eq!(result.directories, 1);
        assert_eq!(result.skipped, 0);
        let link_size = std::fs::symlink_metadata(root.path().join("link"))
            .unwrap()
            .len()
            + 7;
        assert_eq!(result.bytes, 32 * 1024 * 1024 + 10 + link_size);
        let allocated = [
            root.path().to_owned(),
            root.path().join("nested"),
            file,
            root.path().join("plain"),
            root.path().join("link"),
            root.path().join("broken"),
        ]
        .iter()
        .map(|p| std::fs::symlink_metadata(p).unwrap().blocks() * 512)
        .sum::<u64>();
        assert_eq!(result.allocated, allocated);
        assert!(result.allocated < result.bytes);
    }
    #[test]
    fn cancellation_unavailable_roots_and_symlink_roots_are_not_silently_counted() {
        let root = tempfile::tempdir().unwrap();
        assert_eq!(
            scan(root.path(), &AtomicBool::new(true), |_| {})
                .err()
                .unwrap()
                .kind(),
            io::ErrorKind::Interrupted
        );
        assert!(
            scan(
                &root.path().join("missing"),
                &AtomicBool::new(false),
                |_| {}
            )
            .is_err()
        );
        let link = root.path().join("link");
        symlink(root.path(), &link).unwrap();
        assert!(scan(&link, &AtomicBool::new(false), |_| {}).is_err());
        let name = std::ffi::OsString::from_vec(vec![b'a', 255]);
        std::fs::write(root.path().join(name), b"abc").unwrap();
        assert_eq!(
            scan(root.path(), &AtomicBool::new(false), |_| {})
                .unwrap()
                .bytes,
            3 + std::fs::symlink_metadata(link).unwrap().len()
        );
    }
}

#[cfg(test)]
mod partial_tests {
    use super::*;
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    #[test]
    fn unreadable_folders_keep_their_known_allocation_but_mark_totals_partial() {
        if rustix::process::geteuid().is_root() {
            return;
        }
        let root = tempfile::tempdir().unwrap();
        let child = root.path().join("blocked");
        std::fs::create_dir(&child).unwrap();
        std::fs::write(child.join("file"), b"secret").unwrap();
        let expected =
            (root.path().metadata().unwrap().blocks() + child.metadata().unwrap().blocks()) * 512;
        std::fs::set_permissions(&child, std::fs::Permissions::from_mode(0o0)).unwrap();
        let result = scan(root.path(), &AtomicBool::new(false), |_| {});
        std::fs::set_permissions(child, std::fs::Permissions::from_mode(0o700)).unwrap();
        let totals = result.unwrap();
        assert_eq!(totals.directories, 1);
        assert_eq!(totals.files, 0);
        assert_eq!(totals.skipped, 1);
        assert_eq!(totals.allocated, expected);
    }
}
