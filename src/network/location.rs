//! Translate local mount paths for presentation; filesystem operations keep the
//! original paths. This resolver does no network I/O and never mounts anything.
use super::{Connection, remote_url};
use percent_encoding::{NON_ALPHANUMERIC, percent_decode_str, percent_encode};
use std::{
    collections::BTreeMap,
    path::{Component, Path, PathBuf},
};
use url::Url;
const SEGMENT: &percent_encoding::AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'.')
    .remove(b'_')
    .remove(b'~');

pub struct Crumb {
    pub label: String,
    pub address: String,
    pub local: Option<PathBuf>,
}
pub struct Location {
    pub address: String,
    pub title: String,
    pub crumbs: Vec<Crumb>,
}

/// FTP has no trash operation. Recognize saved and native GVfs/KIO mounts even
/// after their sidebar shortcut is removed, without performing remote I/O.
pub fn requires_permanent_deletion(path: &Path, connections: &[Connection]) -> bool {
    if let Some(connection) = connections
        .iter()
        .filter(|connection| {
            connection
                .mount
                .as_ref()
                .is_some_and(|mount| mount.is_absolute() && path.starts_with(mount))
        })
        .max_by_key(|connection| connection.mount.as_ref().unwrap().components().count())
    {
        return remote_url(&connection.url).is_ok_and(|url| url.starts_with("ftp://"));
    }
    let runtime = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(format!("/run/user/{}", rustix::process::getuid().as_raw()))
        });
    let gvfs = path.strip_prefix(runtime.join("gvfs")).ok().and_then(|relative| relative.components().next()).is_some_and(|component| {
        matches!(component, Component::Normal(spec) if spec.as_encoded_bytes().starts_with(b"ftp:"))
    });
    if gvfs {
        return true;
    }
    let Ok(relative) = path.strip_prefix(&runtime) else {
        return false;
    };
    let mut components = relative.components();
    matches!((components.next(), components.next()), (Some(Component::Normal(mount)), Some(Component::Normal(scheme))) if mount.as_encoded_bytes().starts_with(b"kio-fuse-") && scheme == "ftp")
}

pub fn resolve(path: &Path, connections: &[Connection]) -> Option<Location> {
    let saved = connections
        .iter()
        .filter_map(|c| {
            c.mount
                .as_ref()
                .filter(|m| m.is_absolute() && path.starts_with(m))
                .map(|m| (c, m))
        })
        .max_by_key(|(_, mount)| mount.components().count());
    if let Some((connection, mount)) = saved
        && let Some(location) = from_mount(path, mount, &connection.url)
    {
        return Some(location);
    }
    // A removed sidebar shortcut must not expose the GVfs implementation path.
    // Decode its SMB mount specification using GVfs' escaped key/value format.
    let runtime = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(format!("/run/user/{}", rustix::process::getuid().as_raw()))
        });
    let gvfs = runtime.join("gvfs");
    let relative = path.strip_prefix(&gvfs).ok()?;
    let Component::Normal(spec) = relative.components().next()? else {
        return None;
    };
    let fields = spec.to_str()?.strip_prefix("smb-share:")?;
    let values: BTreeMap<_, _> = fields
        .split(',')
        .map(|field| {
            let (key, value) = field.split_once('=')?;
            Some((
                key,
                percent_decode_str(value).decode_utf8().ok()?.into_owned(),
            ))
        })
        .collect::<Option<_>>()?;
    let server = values.get("server")?;
    let share = values.get("share")?;
    if share.is_empty() || share.contains(['/', '\\']) || matches!(share.as_str(), "." | "..") {
        return None;
    }
    let mut uri = Url::parse("smb://placeholder/").ok()?;
    uri.set_host(Some(server)).ok()?;
    if let Some(port) = values.get("port") {
        uri.set_port(Some(port.parse().ok()?)).ok()?;
    }
    if let Some(user) = values.get("user") {
        let user = match values.get("domain") {
            Some(domain) => format!("{domain};{user}"),
            None => user.clone(),
        };
        uri.set_username(&user).ok()?;
    }
    uri.path_segments_mut().ok()?.push(share);
    if let Some(prefix) = values.get("prefix") {
        for part in Path::new(prefix).components() {
            if let Component::Normal(name) = part {
                append_segment(&mut uri, name.as_encoded_bytes());
            } else if part != Component::RootDir {
                return None;
            }
        }
    }
    from_mount(path, &gvfs.join(spec), uri.as_str())
}

fn append_segment(uri: &mut Url, bytes: &[u8]) {
    uri.set_path(&format!(
        "{}/{}",
        uri.path().trim_end_matches('/'),
        percent_encode(bytes, SEGMENT)
    ));
}

fn from_mount(path: &Path, mount: &Path, address: &str) -> Option<Location> {
    let normalized = remote_url(address).ok()?;
    let base = Url::parse(&normalized).ok()?;
    let segments: Vec<_> = base
        .path_segments()?
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect();
    let mut uri = base.clone();
    uri.set_path("/");
    let mut crumbs = vec![Crumb {
        label: uri.as_str().trim_end_matches('/').into(),
        address: uri.to_string(),
        local: segments.is_empty().then(|| mount.to_owned()),
    }];
    for (i, segment) in segments.iter().enumerate() {
        uri.set_path(&format!("{}/{}", uri.path().trim_end_matches('/'), segment));
        crumbs.push(Crumb {
            label: percent_decode_str(segment).decode_utf8_lossy().into_owned(),
            address: uri.to_string(),
            local: (i + 1 == segments.len()).then(|| mount.to_owned()),
        });
    }
    let mut local = mount.to_owned();
    for component in path.strip_prefix(mount).ok()?.components() {
        let Component::Normal(name) = component else {
            return None;
        };
        local.push(name);
        append_segment(&mut uri, name.as_encoded_bytes());
        crumbs.push(Crumb {
            label: name.to_string_lossy().into_owned(),
            address: uri.to_string(),
            local: Some(local.clone()),
        });
    }
    Some(Location {
        address: uri.to_string(),
        title: crumbs.last()?.label.clone(),
        crumbs,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ftp_deletion_capability_recognizes_saved_and_unbookmarked_gvfs_mounts_with_path_boundaries()
    {
        let connection = Connection {
            url: "ftp://user@server/".into(),
            label: "FTP".into(),
            mount: Some("/mount/ftp".into()),
        };
        assert!(requires_permanent_deletion(
            Path::new("/mount/ftp/nested/file.txt"),
            std::slice::from_ref(&connection)
        ));
        assert!(!requires_permanent_deletion(
            Path::new("/mount/ftp-other/file.txt"),
            std::slice::from_ref(&connection)
        ));
        let smb = Connection {
            url: "smb://server/share/".into(),
            label: "SMB".into(),
            mount: Some("/mount/ftp/nested".into()),
        };
        assert!(!requires_permanent_deletion(
            Path::new("/mount/ftp/nested/file.txt"),
            &[connection, smb]
        ));
        let runtime = std::env::var_os("XDG_RUNTIME_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                PathBuf::from(format!("/run/user/{}", rustix::process::getuid().as_raw()))
            });
        assert!(requires_permanent_deletion(
            &runtime.join("gvfs/ftp:host=server,user=user/folder/file.txt"),
            &[]
        ));
        assert!(!requires_permanent_deletion(
            &runtime.join("gvfs/smb-share:server=server,share=D/file.txt"),
            &[]
        ));
        assert!(!requires_permanent_deletion(
            Path::new("/tmp/ftp:host=server/file.txt"),
            &[]
        ));
        assert!(!requires_permanent_deletion(
            Path::new("/home/user/file.txt"),
            &[]
        ));
        assert!(requires_permanent_deletion(
            &runtime.join("kio-fuse-ABC/ftp/user@server/folder/file.txt"),
            &[]
        ));
        assert!(!requires_permanent_deletion(
            &runtime.join("kio-fuse-ABC/sftp/user@server/file.txt"),
            &[]
        ));
    }
    #[test]
    fn saved_mounts_keep_share_case_and_map_descendants_without_prefix_collisions() {
        let remote = Connection {
            url: "smb://Administrador@server/D/".into(),
            label: "Custom sidebar name".into(),
            mount: Some("/mount/share".into()),
        };
        let root = resolve(Path::new("/mount/share"), std::slice::from_ref(&remote)).unwrap();
        assert_eq!(root.title, "D");
        assert_eq!(root.address, "smb://Administrador@server/D");
        assert_eq!(root.crumbs[0].local, None);
        assert_eq!(
            root.crumbs[1].local.as_deref(),
            Some(Path::new("/mount/share"))
        );
        let sub = resolve(
            Path::new("/mount/share/Projetos #1/a+b"),
            std::slice::from_ref(&remote),
        )
        .unwrap();
        assert_eq!(sub.title, "a+b");
        assert_eq!(
            sub.address,
            "smb://Administrador@server/D/Projetos%20%231/a%2Bb"
        );
        assert!(resolve(Path::new("/mount/share-other"), &[remote]).is_none());
    }
    #[test]
    fn gvfs_smb_locations_remain_readable_without_a_sidebar_bookmark() {
        let runtime = std::env::var_os("XDG_RUNTIME_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                PathBuf::from(format!("/run/user/{}", rustix::process::getuid().as_raw()))
            });
        let mount = runtime
            .join("gvfs/smb-share:server=192.0.2.46,share=shared%20folder,user=Admin%2Cistrador");
        let sub = resolve(&mount.join("Relatórios"), &[]).unwrap();
        assert_eq!(
            sub.address,
            "smb://Admin,istrador@192.0.2.46/shared%20folder/Relat%C3%B3rios"
        );
        assert_eq!(sub.crumbs[1].label, "shared folder");
        assert_eq!(sub.title, "Relatórios");
        assert_eq!(sub.crumbs[1].local.as_deref(), Some(mount.as_path()));
        assert!(
            resolve(
                Path::new("/home/example/gvfs/smb-share:server=server,share=d"),
                &[]
            )
            .is_none()
        );
    }
    #[test]
    fn most_specific_mount_wins_and_non_utf8_names_remain_encoded() {
        use std::os::unix::ffi::OsStringExt;
        let connections = vec![
            Connection {
                url: "smb://server/Outer".into(),
                label: String::new(),
                mount: Some("/mount".into()),
            },
            Connection {
                url: "sftp://user@other/home/Reports".into(),
                label: String::new(),
                mount: Some("/mount/nested".into()),
            },
        ];
        let path =
            Path::new("/mount/nested").join(std::ffi::OsString::from_vec(b"raw-\xff".to_vec()));
        let result = resolve(&path, &connections).unwrap();
        assert_eq!(result.address, "sftp://user@other/home/Reports/raw-%FF");
        assert_eq!(result.crumbs[1].local, None);
        assert_eq!(
            result.crumbs[2].local.as_deref(),
            Some(Path::new("/mount/nested"))
        );
    }
}
