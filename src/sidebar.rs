use crate::{filesystem, icons::Icon, settings::folder_key};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    ffi::OsString,
    fs, io,
    os::unix::ffi::OsStringExt,
    path::{Path, PathBuf},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Item {
    Home,
    Desktop,
    Downloads,
    Documents,
    Pictures,
    Music,
    Videos,
    Root,
    Mounts,
    Media,
}

impl Item {
    pub const ALL: [Self; 10] = [
        Self::Home,
        Self::Desktop,
        Self::Downloads,
        Self::Documents,
        Self::Pictures,
        Self::Music,
        Self::Videos,
        Self::Root,
        Self::Mounts,
        Self::Media,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Home => crate::i18n::tr("Pasta pessoal"),
            Self::Desktop => crate::i18n::tr("Área de trabalho"),
            Self::Downloads => crate::i18n::tr("Downloads"),
            Self::Documents => crate::i18n::tr("Documentos"),
            Self::Pictures => crate::i18n::tr("Imagens"),
            Self::Music => crate::i18n::tr("Músicas"),
            Self::Videos => crate::i18n::tr("Vídeos"),
            Self::Root => crate::i18n::tr("Sistema de arquivos"),
            Self::Mounts => crate::i18n::tr("Montagens · /mnt"),
            Self::Media => crate::i18n::tr("Mídias · /media"),
        }
    }
    pub fn icon(self) -> Icon {
        match self {
            Self::Home => Icon::Home,
            Self::Desktop => Icon::Desktop,
            Self::Downloads => Icon::Download,
            Self::Documents => Icon::Document,
            Self::Pictures => Icon::Picture,
            Self::Music => Icon::Music,
            Self::Videos => Icon::Video,
            Self::Root => Icon::Drive,
            Self::Mounts => Icon::Mount,
            Self::Media => Icon::Removable,
        }
    }
    pub fn path(self) -> Option<PathBuf> {
        match self {
            Self::Home => dirs::home_dir(),
            Self::Desktop => dirs::desktop_dir(),
            Self::Downloads => dirs::download_dir(),
            Self::Documents => dirs::document_dir(),
            Self::Pictures => dirs::picture_dir(),
            Self::Music => dirs::audio_dir(),
            Self::Videos => dirs::video_dir(),
            Self::Root => Some("/".into()),
            Self::Mounts => Some("/mnt".into()),
            Self::Media => Some("/media".into()),
        }
    }
    pub fn computer(self) -> bool {
        matches!(self, Self::Root | Self::Mounts | Self::Media)
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Shortcut {
    #[serde(with = "saved_path")]
    pub path: PathBuf,
    #[serde(default)]
    pub label: String,
}

impl Shortcut {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            label: String::new(),
        }
    }
    pub fn title(&self) -> String {
        if !self.label.trim().is_empty() {
            return self.label.clone();
        }
        self.path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| self.path.display().to_string())
    }
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    pub visible: BTreeSet<Item>,
    pub show_applications: bool,
    pub show_pinned: bool,
    pub show_trash: bool,
    pub show_network: bool,
    pub show_local_shares: bool,
    pub show_volumes: bool,
    pub pinned: Vec<Shortcut>,
    pub network: Vec<Shortcut>,
    pub remote: Vec<crate::network::Connection>,
    pub hidden_network: BTreeSet<String>,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            visible: Item::ALL.into_iter().collect(),
            show_applications: true,
            show_pinned: true,
            show_trash: true,
            show_network: true,
            show_local_shares: true,
            show_volumes: true,
            pinned: Vec::new(),
            network: Vec::new(),
            remote: Vec::new(),
            hidden_network: BTreeSet::new(),
        }
    }
}

impl Preferences {
    pub fn normalize(&mut self) {
        let mut seen = BTreeSet::new();
        self.remote.retain_mut(|remote| {
            let Ok(url) = crate::network::remote_url(&remote.url) else {
                return false;
            };
            remote.url = url;
            remote.label = remote.label.trim().to_owned();
            if remote.mount.as_ref().is_some_and(|p| !p.is_absolute()) {
                remote.mount = None;
            }
            seen.insert(remote.url.clone())
        });
        for shortcuts in [&mut self.pinned, &mut self.network] {
            let mut seen = BTreeSet::new();
            shortcuts.retain(|shortcut| {
                shortcut.path.is_absolute()
                    && seen.insert(shortcut.path.components().collect::<PathBuf>())
            });
            for shortcut in shortcuts {
                shortcut.path = shortcut.path.components().collect();
            }
        }
    }
    pub fn pin(&mut self, path: &Path) -> io::Result<bool> {
        let path = filesystem::absolute_path(path)?;
        if !path.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::NotADirectory,
                crate::i18n::tr("Escolha uma pasta existente para fixar"),
            ));
        }
        if self.pinned.iter().any(|p| p.path == path) {
            return Ok(false);
        }
        self.pinned.push(Shortcut::new(path));
        self.show_pinned = true;
        Ok(true)
    }
}

pub fn network_mounts() -> Vec<Shortcut> {
    fs::read("/proc/self/mountinfo")
        .map(|bytes| parse_mountinfo(&bytes))
        .unwrap_or_default()
}

pub fn available_places() -> Vec<(Item, PathBuf)> {
    Item::ALL
        .into_iter()
        .filter_map(|item| {
            item.path()
                .filter(|path| path.is_dir())
                .map(|path| (item, path))
        })
        .collect()
}

fn parse_mountinfo(bytes: &[u8]) -> Vec<Shortcut> {
    let mut mounts = Vec::new();
    let mut seen = BTreeSet::new();
    for line in bytes.split(|&b| b == b'\n') {
        let fields: Vec<_> = line.split(|&b| b == b' ').collect();
        let Some(separator) = fields.iter().position(|field| *field == b"-") else {
            continue;
        };
        let Some(kind) = fields.get(separator + 1) else {
            continue;
        };
        if ![
            b"cifs".as_slice(),
            b"smb3",
            b"nfs",
            b"nfs4",
            b"fuse.sshfs",
            b"sshfs",
            b"davfs",
            b"fuse.davfs",
            b"fuse.rclone",
        ]
        .contains(kind)
        {
            continue;
        }
        let Some(path) = fields
            .get(4)
            .map(|bytes| PathBuf::from(OsString::from_vec(unescape(bytes))))
        else {
            continue;
        };
        if path.is_absolute() && seen.insert(path.clone()) {
            mounts.push(Shortcut::new(path));
        }
    }
    mounts
}

fn unescape(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\\'
            && i + 3 < bytes.len()
            && bytes[i + 1..i + 4]
                .iter()
                .all(|b| (b'0'..=b'7').contains(b))
        {
            let value = (bytes[i + 1] - b'0') as u16 * 64
                + (bytes[i + 2] - b'0') as u16 * 8
                + (bytes[i + 3] - b'0') as u16;
            if let Ok(value) = u8::try_from(value) {
                out.push(value);
                i += 4;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    out
}

mod saved_path {
    use super::*;
    use serde::{Deserializer, Serializer, de::Error};
    pub fn serialize<S: Serializer>(path: &Path, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&folder_key(path))
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<PathBuf, D::Error> {
        let value = String::deserialize(deserializer)?;
        let Some(bytes) = value.strip_prefix("unix-bytes:") else {
            return Ok(PathBuf::from(value));
        };
        if bytes.len() % 2 != 0 || !bytes.is_ascii() {
            return Err(D::Error::custom(crate::i18n::tr(
                "Caminho codificado inválido",
            )));
        }
        let raw = (0..bytes.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&bytes[i..i + 2], 16))
            .collect::<Result<Vec<_>, _>>()
            .map_err(D::Error::custom)?;
        Ok(PathBuf::from(OsString::from_vec(raw)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_network_mounts_with_optional_fields_escaped_names_and_no_duplicates() {
        let input=b"1 0 8:1 / / rw - ext4 /dev/root rw\n2 1 0:2 / /mnt/Equipe\\040A rw shared:5 - cifs //user:secret@server/share rw\n3 1 0:3 / /mnt/nfs rw - nfs4 server:/export rw\n4 1 0:4 / /mnt/ssh rw - fuse.sshfs user@server:/ rw\n5 1 0:2 / /mnt/Equipe\\040A rw - cifs //server/share rw\nmalformed\n";
        let locations = parse_mountinfo(input);
        assert_eq!(locations.len(), 3);
        assert_eq!(locations[0].path, Path::new("/mnt/Equipe A"));
        assert_eq!(locations[0].title(), "Equipe A");
        assert!(!locations[0].title().contains("secret"));
        assert_eq!(locations[1].path, Path::new("/mnt/nfs"));
        assert_eq!(locations[2].path, Path::new("/mnt/ssh"));
    }

    #[test]
    fn sidebar_preferences_preserve_visibility_and_non_utf8_pins_and_network_paths() {
        let directory = tempfile::tempdir().unwrap();
        let folder = directory.path().join("folder");
        fs::create_dir(&folder).unwrap();
        let mut sidebar = Preferences::default();
        assert!(sidebar.pin(&folder).unwrap());
        assert!(!sidebar.pin(&folder).unwrap());
        sidebar.visible.remove(&Item::Downloads);
        let raw = PathBuf::from(OsString::from_vec(b"/tmp/pinned-\xff".to_vec()));
        sidebar.pinned.push(Shortcut::new(raw.clone()));
        sidebar.pinned.push(Shortcut::new(raw.clone()));
        sidebar.pinned.push(Shortcut::new("relative".into()));
        sidebar.network.push(Shortcut {
            path: "/mnt/server".into(),
            label: "Servidor".into(),
        });
        sidebar.normalize();
        assert_eq!(sidebar.pinned.len(), 2);
        let preferences = crate::settings::Preferences {
            sidebar: sidebar.clone(),
            ..Default::default()
        };
        let path = directory.path().join("preferences.json");
        preferences.save(&path).unwrap();
        let loaded = crate::settings::Preferences::load(&path).unwrap();
        assert!(loaded.sidebar == sidebar);
        assert_eq!(loaded.sidebar.pinned[1].path, raw);
        assert!(!loaded.sidebar.visible.contains(&Item::Downloads));
        assert_eq!(loaded.sidebar.network[0].title(), "Servidor");
        let legacy: crate::settings::Preferences = serde_json::from_str("{}").unwrap();
        assert!(legacy.sidebar == Preferences::default());
    }
}
