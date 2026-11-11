use crate::filesystem::{Entry, format_size};
use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use std::{cmp::Ordering, time::SystemTime};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Column {
    Name,
    Modified,
    Kind,
    Size,
    Extension,
    Created,
    Accessed,
    Changed,
    Owner,
    Group,
    Uid,
    Gid,
    Permissions,
    OctalMode,
    Inode,
    Links,
    Allocated,
    Blocks,
    Device,
    ReadOnly,
    Hidden,
    SymlinkTarget,
    Path,
    OriginalPath,
    Deleted,
}

impl Column {
    pub const ALL: [Self; 25] = [
        Self::Name,
        Self::Modified,
        Self::Kind,
        Self::Size,
        Self::Extension,
        Self::Created,
        Self::Accessed,
        Self::Changed,
        Self::Owner,
        Self::Group,
        Self::Uid,
        Self::Gid,
        Self::Permissions,
        Self::OctalMode,
        Self::Inode,
        Self::Links,
        Self::Allocated,
        Self::Blocks,
        Self::Device,
        Self::ReadOnly,
        Self::Hidden,
        Self::SymlinkTarget,
        Self::Path,
        Self::OriginalPath,
        Self::Deleted,
    ];
    pub const DEFAULT: [Self; 4] = [Self::Name, Self::Modified, Self::Kind, Self::Size];

    pub fn label(self) -> &'static str {
        match self {
            Self::Name => crate::i18n::tr("Nome"),
            Self::Modified => crate::i18n::tr("Data de modificação"),
            Self::Kind => crate::i18n::tr("Tipo"),
            Self::Size => crate::i18n::tr("Tamanho"),
            Self::Extension => crate::i18n::tr("Extensão"),
            Self::Created => crate::i18n::tr("Data de criação"),
            Self::Accessed => crate::i18n::tr("Último acesso"),
            Self::Changed => crate::i18n::tr("Alteração de metadados"),
            Self::Owner => crate::i18n::tr("Proprietário"),
            Self::Group => crate::i18n::tr("Grupo"),
            Self::Uid => "UID",
            Self::Gid => "GID",
            Self::Permissions => crate::i18n::tr("Permissões"),
            Self::OctalMode => crate::i18n::tr("Modo octal"),
            Self::Inode => "Inode",
            Self::Links => crate::i18n::tr("Links físicos"),
            Self::Allocated => crate::i18n::tr("Espaço alocado"),
            Self::Blocks => crate::i18n::tr("Blocos de 512 B"),
            Self::Device => crate::i18n::tr("Dispositivo (ID)"),
            Self::ReadOnly => crate::i18n::tr("Sem bits de escrita"),
            Self::Hidden => crate::i18n::tr("Oculto"),
            Self::SymlinkTarget => crate::i18n::tr("Destino do link"),
            Self::Path => crate::i18n::tr("Caminho completo"),
            Self::OriginalPath => crate::i18n::tr("Caminho original"),
            Self::Deleted => crate::i18n::tr("Data de exclusão"),
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Self::Created => crate::i18n::tr(
                "Nascimento do arquivo (btime), quando fornecido pelo sistema de arquivos.",
            ),
            Self::Accessed => {
                crate::i18n::tr("atime: depende das opções de montagem, como relatime e noatime.")
            }
            Self::Changed => crate::i18n::tr(
                "ctime: última alteração dos metadados; não representa a criação do arquivo.",
            ),
            Self::ReadOnly => crate::i18n::tr(
                "Indica ausência dos bits POSIX de escrita. ACLs, privilégios e montagem também afetam o acesso.",
            ),
            Self::Allocated => crate::i18n::tr(
                "Blocos reportados pelo sistema × 512 bytes. Pode diferir do tamanho lógico em arquivos esparsos.",
            ),
            Self::Permissions => crate::i18n::tr(
                "Modo POSIX com tipo e bits rwx, incluindo setuid, setgid e sticky bit.",
            ),
            Self::Device => {
                crate::i18n::tr("Identificador numérico st_dev do sistema de arquivos.")
            }
            Self::SymlinkTarget => {
                crate::i18n::tr("Destino registrado no link simbólico, que pode ser relativo.")
            }
            _ => self.label(),
        }
    }

    pub fn width(self) -> f32 {
        match self {
            Self::Name => 180.0,
            Self::Created | Self::Modified | Self::Accessed | Self::Deleted => 180.0,
            Self::Changed => 205.0,
            Self::Path | Self::SymlinkTarget | Self::OriginalPath => 260.0,
            Self::Owner | Self::Group | Self::Kind | Self::Permissions => 145.0,
            Self::ReadOnly | Self::Device | Self::Allocated | Self::Blocks => 160.0,
            _ => 110.0,
        }
    }

    pub fn value(self, entry: &Entry) -> String {
        match self {
            Self::Name => entry.name.clone(),
            Self::Modified => date(entry.modified),
            Self::Kind => entry.kind(),
            Self::Size => entry.size.map(format_size).unwrap_or_else(unavailable),
            Self::Extension => entry.extension(),
            Self::Created => date(entry.created),
            Self::Accessed => date(entry.accessed),
            Self::Changed => date(entry.changed),
            Self::Owner => entry.owner.clone(),
            Self::Group => entry.group.clone(),
            Self::Uid => entry.uid.to_string(),
            Self::Gid => entry.gid.to_string(),
            Self::Permissions => entry.permissions(),
            Self::OctalMode => format!("{:04o}", entry.mode & 0o7777),
            Self::Inode => entry.inode.to_string(),
            Self::Links => entry.links.to_string(),
            Self::Allocated => format_size(entry.allocated()),
            Self::Blocks => entry.blocks.to_string(),
            Self::Device => entry.device.to_string(),
            Self::ReadOnly => yes_no(entry.read_only()),
            Self::Hidden => yes_no(entry.hidden),
            Self::SymlinkTarget => entry
                .symlink_target
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_else(unavailable),
            Self::Path => entry.path.display().to_string(),
            Self::OriginalPath => entry
                .trash
                .as_ref()
                .map(|record| record.original_path.display().to_string())
                .unwrap_or_else(unavailable),
            Self::Deleted => date(entry.trash.as_ref().and_then(|record| record.deleted)),
        }
    }

    pub fn compare(self, a: &Entry, b: &Entry) -> Ordering {
        match self {
            Self::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
            Self::Modified => a.modified.cmp(&b.modified),
            Self::Size => a.size.cmp(&b.size),
            Self::Created => a.created.cmp(&b.created),
            Self::Accessed => a.accessed.cmp(&b.accessed),
            Self::Changed => a.changed.cmp(&b.changed),
            Self::Uid => a.uid.cmp(&b.uid),
            Self::Gid => a.gid.cmp(&b.gid),
            Self::Inode => a.inode.cmp(&b.inode),
            Self::Links => a.links.cmp(&b.links),
            Self::Allocated => a.allocated().cmp(&b.allocated()),
            Self::Blocks => a.blocks.cmp(&b.blocks),
            Self::Device => a.device.cmp(&b.device),
            Self::OctalMode | Self::Permissions => (a.mode & 0o7777).cmp(&(b.mode & 0o7777)),
            Self::ReadOnly => a.read_only().cmp(&b.read_only()),
            Self::Hidden => a.hidden.cmp(&b.hidden),
            Self::SymlinkTarget => a.symlink_target.cmp(&b.symlink_target),
            Self::Path => a.path.cmp(&b.path),
            Self::OriginalPath => a
                .trash
                .as_ref()
                .map(|record| &record.original_path)
                .cmp(&b.trash.as_ref().map(|record| &record.original_path)),
            Self::Deleted => a
                .trash
                .as_ref()
                .and_then(|record| record.deleted)
                .cmp(&b.trash.as_ref().and_then(|record| record.deleted)),
            _ => self
                .value(a)
                .to_lowercase()
                .cmp(&self.value(b).to_lowercase()),
        }
    }
}

fn date(time: Option<SystemTime>) -> String {
    time.map(|time| {
        DateTime::<Local>::from(time)
            .format(crate::i18n::date_format())
            .to_string()
    })
    .unwrap_or_else(unavailable)
}

fn unavailable() -> String {
    "—".into()
}
fn yes_no(value: bool) -> String {
    if value {
        crate::i18n::tr("Sim")
    } else {
        crate::i18n::tr("Não")
    }
    .into()
}
