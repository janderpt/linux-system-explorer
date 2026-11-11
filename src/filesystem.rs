pub use crate::columns::Column as SortBy;
use std::{
    cmp::Ordering,
    ffi::OsStr,
    fs, io,
    os::unix::fs::MetadataExt,
    path::{Component, Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use uzers::{Groups, Users, UsersCache};

#[derive(Clone, Debug)]
pub struct Entry {
    pub path: PathBuf,
    pub name: String,
    pub is_dir: bool,
    pub is_symlink: bool,
    pub hidden: bool,
    pub size: Option<u64>,
    pub modified: Option<SystemTime>,
    pub created: Option<SystemTime>,
    pub accessed: Option<SystemTime>,
    pub changed: Option<SystemTime>,
    pub owner: String,
    pub group: String,
    pub uid: u32,
    pub gid: u32,
    pub mode: u32,
    pub inode: u64,
    pub links: u64,
    pub blocks: u64,
    pub device: u64,
    pub symlink_target: Option<PathBuf>,
    pub trash: Option<crate::trashbin::Record>,
}

impl Entry {
    pub fn kind(&self) -> String {
        if self.is_symlink {
            if self.is_dir {
                crate::i18n::tr("Link para pasta").into()
            } else {
                crate::i18n::tr("Link simbólico").into()
            }
        } else if self.is_dir {
            crate::i18n::tr("Pasta").into()
        } else {
            match self.mode & 0o170000 {
                0o010000 => return "FIFO".into(),
                0o020000 => return crate::i18n::tr("Dispositivo de caracteres").into(),
                0o060000 => return crate::i18n::tr("Dispositivo de blocos").into(),
                0o140000 => return "Socket".into(),
                _ => (),
            }
            self.display_path().extension().map_or_else(
                || crate::i18n::tr("Arquivo").into(),
                |ext| {
                    crate::i18n::message(
                        "Arquivo {}",
                        &[(ext.to_string_lossy().to_uppercase()).to_string()],
                    )
                },
            )
        }
    }

    pub(crate) fn display_path(&self) -> &Path {
        self.trash
            .as_ref()
            .map(|record| record.original_path.as_path())
            .unwrap_or(&self.path)
    }

    pub fn extension(&self) -> String {
        if self.is_dir {
            return String::new();
        }
        self.display_path()
            .extension()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default()
    }

    pub fn allocated(&self) -> u64 {
        self.blocks.saturating_mul(512)
    }
    pub fn read_only(&self) -> bool {
        self.mode & 0o222 == 0
    }

    pub fn permissions(&self) -> String {
        let kind = match self.mode & 0o170000 {
            0o040000 => 'd',
            0o120000 => 'l',
            0o010000 => 'p',
            0o020000 => 'c',
            0o060000 => 'b',
            0o140000 => 's',
            _ => '-',
        };
        let mut result = String::from(kind);
        for shift in [6, 3, 0] {
            result.push(if self.mode & (0o4 << shift) != 0 {
                'r'
            } else {
                '-'
            });
            result.push(if self.mode & (0o2 << shift) != 0 {
                'w'
            } else {
                '-'
            });
            let execute = self.mode & (1 << shift) != 0;
            let special = self.mode
                & match shift {
                    6 => 0o4000,
                    3 => 0o2000,
                    _ => 0o1000,
                }
                != 0;
            result.push(match (special, execute, shift) {
                (true, true, 0) => 't',
                (true, false, 0) => 'T',
                (true, true, _) => 's',
                (true, false, _) => 'S',
                (false, true, _) => 'x',
                _ => '-',
            });
        }
        result
    }
}

pub struct Listing {
    pub path: PathBuf,
    pub entries: Vec<Entry>,
    pub skipped: usize,
}

pub fn read_directory(path: &Path) -> io::Result<Listing> {
    // Keep the logical path: entering a symlink and going up behaves like Explorer.
    let path = absolute_path(path)?;
    let mut entries = Vec::new();
    let mut skipped = 0;
    let identities = UsersCache::new();
    for item in fs::read_dir(&path)? {
        let item = match item {
            Ok(item) => item,
            Err(_) => {
                skipped += 1;
                continue;
            }
        };
        match entry_with_identities(&item.path(), &identities) {
            Ok(entry) => entries.push(entry),
            Err(_) => skipped += 1,
        }
    }
    Ok(Listing {
        path,
        entries,
        skipped,
    })
}

pub fn read_entry(path: &Path) -> io::Result<Entry> {
    entry_with_identities(path, &UsersCache::new())
}

fn entry_with_identities(path: &Path, identities: &UsersCache) -> io::Result<Entry> {
    let metadata = path.symlink_metadata()?;
    let is_symlink = metadata.is_symlink();
    let target = if is_symlink {
        fs::metadata(path).ok()
    } else {
        None
    };
    let effective = target.as_ref().unwrap_or(&metadata);
    let name = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    Ok(Entry {
        path: path.to_owned(),
        hidden: name.starts_with('.'),
        name,
        is_dir: effective.is_dir(),
        is_symlink,
        size: (!metadata.is_dir()).then_some(metadata.len()),
        modified: metadata.modified().ok(),
        created: metadata.created().ok(),
        accessed: metadata.accessed().ok(),
        changed: unix_time(metadata.ctime(), metadata.ctime_nsec()),
        owner: identities
            .get_user_by_uid(metadata.uid())
            .map(|u| u.name().to_string_lossy().into_owned())
            .unwrap_or_else(|| metadata.uid().to_string()),
        group: identities
            .get_group_by_gid(metadata.gid())
            .map(|g| g.name().to_string_lossy().into_owned())
            .unwrap_or_else(|| metadata.gid().to_string()),
        uid: metadata.uid(),
        gid: metadata.gid(),
        mode: metadata.mode(),
        inode: metadata.ino(),
        links: metadata.nlink(),
        blocks: metadata.blocks(),
        device: metadata.dev(),
        trash: None,
        symlink_target: is_symlink.then(|| fs::read_link(path).ok()).flatten(),
    })
}

pub fn absolute_path(path: &Path) -> io::Result<PathBuf> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };
    // Do not collapse `..` across symlinks; the OS must resolve those components.
    if absolute
        .components()
        .any(|part| matches!(part, Component::ParentDir))
    {
        fs::canonicalize(absolute)
    } else {
        Ok(absolute.components().collect())
    }
}

fn unix_time(seconds: i64, nanos: i64) -> Option<SystemTime> {
    let value = if seconds >= 0 {
        UNIX_EPOCH.checked_add(Duration::from_secs(seconds as u64))?
    } else {
        UNIX_EPOCH.checked_sub(Duration::from_secs(seconds.unsigned_abs()))?
    };
    value.checked_add(Duration::from_nanos(u64::try_from(nanos).ok()?))
}

pub fn visible_entries(
    entries: &[Entry],
    query: &str,
    hidden: bool,
    sort: SortBy,
    descending: bool,
) -> Vec<usize> {
    let query = query.to_lowercase();
    let mut visible: Vec<_> = entries
        .iter()
        .enumerate()
        .filter(|(_, e)| (hidden || !e.hidden) && e.name.to_lowercase().contains(&query))
        .map(|(index, _)| index)
        .collect();
    visible.sort_by(|&a, &b| {
        let (a, b) = (&entries[a], &entries[b]);
        let folders = b.is_dir.cmp(&a.is_dir);
        if folders != Ordering::Equal {
            return folders;
        }
        let order = sort.compare(a, b).then_with(|| a.path.cmp(&b.path));
        if descending { order.reverse() } else { order }
    });
    visible
}

pub fn format_size(size: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut value = size as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{size} B")
    } else {
        format!("{} {}", crate::i18n::decimal(value), UNITS[unit])
    }
}

/// Restrict mutations to a single child, never an arbitrary user-supplied path.
pub fn child_path(parent: &Path, name: &str) -> io::Result<PathBuf> {
    let path = Path::new(name);
    if name.trim().is_empty()
        || name.contains('\0')
        || path.file_name() != Some(OsStr::new(name))
        || path.components().count() != 1
        || !matches!(path.components().next(), Some(Component::Normal(_)))
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            crate::i18n::tr("Informe um nome sem barras, diferente de . e .."),
        ));
    }
    Ok(parent.join(path))
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "explorer-test-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn lists_real_files_filters_hidden_and_keeps_folders_first() {
        let dir = Fixture::new();
        fs::create_dir(dir.0.join("z-folder")).unwrap();
        fs::write(dir.0.join("Alpha.txt"), "hello").unwrap();
        fs::write(dir.0.join(".secret"), "hidden").unwrap();
        let listing = read_directory(&dir.0).unwrap();
        let visible = visible_entries(&listing.entries, "", false, SortBy::Name, true);
        assert_eq!(visible.len(), 2);
        assert!(listing.entries[visible[0]].is_dir);
        let filtered = visible_entries(&listing.entries, "ALPHA", false, SortBy::Name, false);
        assert_eq!(listing.entries[filtered[0]].size, Some(5));
        assert_eq!(
            visible_entries(&listing.entries, "", true, SortBy::Name, false).len(),
            3
        );
    }

    #[test]
    fn invalid_directory_is_an_error() {
        let dir = Fixture::new();
        assert!(read_directory(&dir.0.join("missing")).is_err());
        fs::write(dir.0.join("file"), "data").unwrap();
        assert!(read_directory(&dir.0.join("file")).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn handles_directory_and_broken_symlinks() {
        let dir = Fixture::new();
        fs::create_dir(dir.0.join("target")).unwrap();
        std::os::unix::fs::symlink("target", dir.0.join("link")).unwrap();
        std::os::unix::fs::symlink("missing", dir.0.join("broken")).unwrap();
        let listing = read_directory(&dir.0).unwrap();
        assert!(
            listing
                .entries
                .iter()
                .any(|e| e.name == "link" && e.is_dir && e.is_symlink)
        );
        assert!(
            listing
                .entries
                .iter()
                .any(|e| e.name == "broken" && !e.is_dir && e.is_symlink)
        );
        assert_eq!(
            read_directory(&dir.0.join("link")).unwrap().path,
            dir.0.join("link")
        );
    }

    #[test]
    fn child_names_cannot_escape_the_parent() {
        for name in ["", " ", ".", "..", "../escape", "a/b", "/tmp", "a\0b"] {
            assert!(child_path(Path::new("/safe"), name).is_err(), "{name:?}");
        }
        assert_eq!(
            child_path(Path::new("/safe"), "Nova pasta").unwrap(),
            Path::new("/safe/Nova pasta")
        );
    }

    #[test]
    fn exposes_linux_metadata_and_sorts_numeric_columns_numerically() {
        use std::os::unix::fs::PermissionsExt;
        let dir = Fixture::new();
        let small = dir.0.join("z-small");
        let large = dir.0.join("a-large");
        fs::write(&small, "ab").unwrap();
        fs::write(&large, "0123456789").unwrap();
        fs::set_permissions(&small, fs::Permissions::from_mode(0o4750)).unwrap();
        fs::hard_link(&small, dir.0.join("hard-link")).unwrap();
        let listing = read_directory(&dir.0).unwrap();
        let entry = listing.entries.iter().find(|e| e.path == small).unwrap();
        let metadata = fs::symlink_metadata(&small).unwrap();
        assert_eq!(entry.uid, metadata.uid());
        assert_eq!(entry.gid, metadata.gid());
        assert_eq!(entry.inode, metadata.ino());
        assert_eq!(entry.device, metadata.dev());
        assert_eq!(entry.links, 2);
        assert_eq!(entry.allocated(), metadata.blocks() * 512);
        assert_eq!(entry.permissions(), "-rwsr-x---");
        assert_eq!(SortBy::OctalMode.value(entry), "4750");
        assert!(entry.changed.is_some() && entry.accessed.is_some());
        assert!(!entry.owner.is_empty() && !entry.group.is_empty());
        let sorted = visible_entries(&listing.entries, "", true, SortBy::Size, false);
        assert_eq!(listing.entries[*sorted.last().unwrap()].size, Some(10));
        assert_eq!(
            unix_time(-1, 500_000_000),
            UNIX_EPOCH.checked_sub(Duration::from_millis(500))
        );
    }
}
