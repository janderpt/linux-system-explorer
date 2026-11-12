//! A mount-table snapshot keeps drag feedback free of remote stat calls.
use super::{Action, modifier_action};
use eframe::egui::Modifiers;
use std::{
    os::unix::ffi::OsStringExt,
    path::{Path, PathBuf},
};

struct Mount {
    path: PathBuf,
    device: Vec<u8>,
    remote: bool,
}
#[derive(Default)]
pub(crate) struct Mounts(Vec<Mount>);
impl Mounts {
    pub fn read() -> Self {
        std::fs::read("/proc/self/mountinfo")
            .map(|bytes| Self::parse(&bytes))
            .unwrap_or_default()
    }
    pub(crate) fn parse(bytes: &[u8]) -> Self {
        Self(
            bytes
                .split(|&b| b == b'\n')
                .filter_map(|line| {
                    let fields: Vec<_> = line.split(|&b| b == b' ').collect();
                    let separator = fields.iter().position(|field| *field == b"-")?;
                    let kind = *fields.get(separator + 1)?;
                    let mut path = Vec::new();
                    let input = *fields.get(4)?;
                    let mut i = 0;
                    while i < input.len() {
                        if input[i] == b'\\'
                            && i + 3 < input.len()
                            && input[i + 1..i + 4]
                                .iter()
                                .all(|b| (b'0'..=b'7').contains(b))
                        {
                            let value = u16::from(input[i + 1] - b'0') * 64
                                + u16::from(input[i + 2] - b'0') * 8
                                + u16::from(input[i + 3] - b'0');
                            path.push(u8::try_from(value).ok()?);
                            i += 4;
                        } else {
                            path.push(input[i]);
                            i += 1;
                        }
                    }
                    let path = PathBuf::from(std::ffi::OsString::from_vec(path));
                    if !path.is_absolute() {
                        return None;
                    }
                    // Treat virtual/FUSE mounts conservatively: protocol boundaries may
                    // share one device number (notably all GVfs connections).
                    let remote = kind.starts_with(b"fuse")
                        || [
                            b"cifs".as_slice(),
                            b"smb3",
                            b"nfs",
                            b"nfs4",
                            b"davfs",
                            b"9p",
                        ]
                        .contains(&kind);
                    Some(Mount {
                        path,
                        device: fields.get(2)?.to_vec(),
                        remote,
                    })
                })
                .collect(),
        )
    }
    fn at(&self, path: &Path) -> Option<&Mount> {
        self.0
            .iter()
            .filter(|mount| path.starts_with(&mount.path))
            .max_by_key(|mount| mount.path.components().count())
    }
    pub fn action(
        &self,
        paths: &[PathBuf],
        destination: Option<&Path>,
        modifiers: Modifiers,
    ) -> Action {
        if let Some(action) = modifier_action(modifiers) {
            return action;
        }
        let Some(target) = destination.and_then(|path| self.at(path)) else {
            return Action::Copy;
        };
        if !target.remote
            && !paths.is_empty()
            && paths.iter().all(|path| {
                self.at(path)
                    .is_some_and(|source| !source.remote && source.device == target.device)
            })
        {
            Action::Move
        } else {
            Action::Copy
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn same_local_device_moves_but_cross_device_network_mixed_and_unknown_targets_copy() {
        let mounts = Mounts::parse(b"1 0 8:1 / / rw - ext4 /dev/sda1 rw\n2 1 8:2 / /media/Other\\040disk rw - ext4 /dev/sda2 rw\n3 1 0:5 / /run/user/1000/gvfs rw - fuse.gvfsd-fuse gvfs rw\n4 1 0:6 / /mnt/share rw - cifs //server/share rw\n");
        for (sources, target, expected) in [
            (vec!["/home/a"], "/home/dest", Action::Move),
            (vec!["/home/a"], "/media/Other disk/dest", Action::Copy),
            (
                vec!["/home/a"],
                "/run/user/1000/gvfs/ftp:host=server/dest",
                Action::Copy,
            ),
            (
                vec!["/run/user/1000/gvfs/ftp:host=server/file"],
                "/home/dest",
                Action::Copy,
            ),
            (
                vec!["/run/user/1000/gvfs/ftp:host=server/file"],
                "/run/user/1000/gvfs/ftp:host=other",
                Action::Copy,
            ),
            (
                vec!["/home/a", "/media/Other disk/b"],
                "/home/dest",
                Action::Copy,
            ),
            (vec!["/mnt/share/a"], "/mnt/share/dest", Action::Copy),
            (vec!["/home/a"], "/mnt/share-prefix/dest", Action::Move),
        ] {
            let sources: Vec<_> = sources.into_iter().map(PathBuf::from).collect();
            for (modifiers, action) in [
                (Modifiers::NONE, expected),
                (Modifiers::CTRL, Action::Copy),
                (Modifiers::SHIFT, Action::Move),
                (Modifiers::CTRL | Modifiers::SHIFT, Action::Copy),
            ] {
                assert_eq!(
                    mounts.action(&sources, Some(Path::new(target)), modifiers),
                    action
                );
            }
        }
        assert_eq!(
            Mounts::default().action(
                &["/home/a".into()],
                Some(Path::new("/home/dest")),
                Modifiers::NONE
            ),
            Action::Copy
        );
        assert_eq!(mounts.action(&[], None, Modifiers::NONE), Action::Copy);
    }
}
