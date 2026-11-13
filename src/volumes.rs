//! Native volumes use UDisks2; authorization remains with the desktop's Polkit agent.
use std::{
    collections::HashMap,
    ffi::OsString,
    os::unix::ffi::OsStringExt,
    path::{Path, PathBuf},
    time::Duration,
};
use zbus::{
    fdo::ManagedObjects,
    zvariant::{OwnedValue as OwnedVariant, Value as Variant},
};

const DESTINATION: &str = "org.freedesktop.UDisks2";
const ROOT: &str = "/org/freedesktop/UDisks2";
const BLOCK: &str = "org.freedesktop.UDisks2.Block";
const FILESYSTEM: &str = "org.freedesktop.UDisks2.Filesystem";
const DRIVE: &str = "org.freedesktop.UDisks2.Drive";
type Properties = HashMap<String, OwnedVariant>;
type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Space {
    pub total: u64,
    pub available: u64,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Volume {
    pub id: String,
    pub device: PathBuf,
    pub name: String,
    pub filesystem: String,
    pub size: u64,
    pub mounts: Vec<PathBuf>,
    pub space: Option<Space>,
    pub visible: bool,
    pub removable: bool,
    pub optical: bool,
    pub protected: bool,
    pub ejectable: bool,
    pub power_off: bool,
    drive: String,
    group: String,
    identity: String,
}
impl Volume {
    pub fn mount(&self) -> Option<&Path> {
        self.mounts.first().map(PathBuf::as_path)
    }
    pub fn can_unmount(&self) -> bool {
        !self.mounts.is_empty() && !self.protected
    }
    pub fn can_eject(&self) -> bool {
        !self.protected && (self.ejectable || self.power_off)
    }
    pub fn capacity(&self) -> String {
        if let Some(space) = &self.space {
            crate::i18n::message(
                "{} livres de {}",
                &[
                    crate::filesystem::format_size(space.available),
                    crate::filesystem::format_size(space.total),
                ],
            )
        } else if self.mount().is_some() {
            crate::i18n::message(
                "{} · espaço livre indisponível",
                &[crate::filesystem::format_size(self.size)],
            )
        } else {
            crate::i18n::message(
                "{} · Desmontado",
                &[crate::filesystem::format_size(self.size)],
            )
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Open,
    Unmount,
    Eject,
}
#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    Opened(PathBuf),
    Unmounted(Vec<PathBuf>),
    Ejected(Vec<PathBuf>),
    Cancelled,
}

impl Outcome {
    pub fn removed(&self) -> &[PathBuf] {
        match self {
            Self::Unmounted(paths) | Self::Ejected(paths) => paths,
            _ => &[],
        }
    }
}

fn text(p: &Properties, key: &str) -> String {
    p.get(key)
        .and_then(|v| <&str>::try_from(v).ok())
        .unwrap_or_default()
        .to_owned()
}
fn flag(p: &Properties, key: &str) -> bool {
    p.get(key)
        .and_then(|v| bool::try_from(v).ok())
        .unwrap_or(false)
}
fn number(p: &Properties, key: &str) -> u64 {
    p.get(key).and_then(|v| u64::try_from(v).ok()).unwrap_or(0)
}
fn object(p: &Properties, key: &str) -> String {
    p.get(key)
        .and_then(|v| <&zbus::zvariant::ObjectPath>::try_from(v).ok())
        .map(|p| p.to_string())
        .unwrap_or_default()
}
fn array<T>(p: &Properties, key: &str) -> Vec<T>
where
    for<'a> T: TryFrom<Variant<'a>, Error = zbus::zvariant::Error>,
{
    p.get(key)
        .and_then(|v| v.try_clone().ok())
        .and_then(|v| Vec::<T>::try_from(v).ok())
        .unwrap_or_default()
}
fn path(bytes: Vec<u8>) -> Option<PathBuf> {
    let bytes = bytes.strip_suffix(&[0]).unwrap_or(&bytes);
    if bytes.contains(&0) {
        return None;
    }
    let path = PathBuf::from(OsString::from_vec(bytes.to_vec()));
    path.is_absolute().then_some(path)
}
fn essential(path: &Path) -> bool {
    [
        "/",
        "/home",
        "/usr",
        "/var",
        "/boot",
        "/boot/efi",
        "/etc",
        "/opt",
        "/bin",
        "/sbin",
        "/lib",
        "/lib64",
        "/run",
        "/tmp",
    ]
    .iter()
    .any(|p| path == Path::new(p))
        || dirs::home_dir().is_some_and(|home| path == home)
}
fn parse(objects: &ManagedObjects) -> Vec<Volume> {
    let mut volumes = Vec::new();
    for (id, interfaces) in objects {
        let (Some(block), Some(fs)) = (interfaces.get(BLOCK), interfaces.get(FILESYSTEM)) else {
            continue;
        };
        if text(block, "IdUsage") != "filesystem"
            || interfaces.contains_key("org.freedesktop.UDisks2.Loop")
            || interfaces.contains_key("org.freedesktop.UDisks2.PartitionTable")
        {
            continue;
        }
        let Some(device) = block
            .get("Device")
            .and_then(|v| v.try_clone().ok())
            .and_then(|v| Vec::<u8>::try_from(v).ok())
            .and_then(path)
        else {
            continue;
        };
        let mut drive = object(block, "Drive");
        let mut backing = object(block, "CryptoBackingDevice");
        for _ in 0..16 {
            if !drive.is_empty() && drive != "/" {
                break;
            }
            let Some(back) = objects
                .iter()
                .find(|(p, _)| p.as_str() == backing)
                .and_then(|(_, i)| i.get(BLOCK))
            else {
                break;
            };
            drive = object(back, "Drive");
            backing = object(back, "CryptoBackingDevice");
        }
        let properties = objects
            .iter()
            .find(|(p, _)| p.as_str() == drive)
            .and_then(|(_, i)| i.get(DRIVE));
        let empty = Properties::new();
        let d = properties.unwrap_or(&empty);
        let mut mounts: Vec<_> = array::<Vec<u8>>(fs, "MountPoints")
            .into_iter()
            .filter_map(path)
            .collect();
        mounts.sort_by_key(|p| (p.components().count(), p.clone()));
        mounts.dedup();
        let label = text(block, "IdLabel");
        let hint = text(block, "HintName");
        let name = if !hint.is_empty() {
            hint
        } else if !label.is_empty() {
            label
        } else if mounts.first().is_some_and(|p| p == Path::new("/")) {
            crate::i18n::tr("Sistema de arquivos").into()
        } else if let Some(mount) = mounts.first() {
            mount
                .file_name()
                .unwrap_or(mount.as_os_str())
                .to_string_lossy()
                .into_owned()
        } else {
            device
                .file_name()
                .unwrap_or(device.as_os_str())
                .to_string_lossy()
                .into_owned()
        };
        let sibling = text(d, "SiblingId");
        volumes.push(Volume {
            id: id.to_string(),
            device,
            name,
            filesystem: text(block, "IdType"),
            size: number(block, "Size"),
            protected: mounts.iter().any(|p| essential(p)),
            mounts,
            space: None,
            visible: !flag(block, "HintIgnore"),
            removable: flag(d, "Removable")
                || flag(d, "MediaRemovable")
                || text(d, "ConnectionBus") == "usb",
            optical: flag(d, "Optical"),
            ejectable: flag(d, "Ejectable"),
            power_off: flag(d, "CanPowerOff"),
            identity: format!(
                "{}|{}|{}|{}",
                text(block, "IdUUID"),
                text(d, "Id"),
                text(d, "Serial"),
                number(block, "Size")
            ),
            group: if sibling.is_empty() {
                drive.clone()
            } else {
                format!("sibling:{sibling}")
            },
            drive,
        });
    }
    // A drive-level action affects every partition, including hidden system volumes.
    let protected: Vec<_> = volumes
        .iter()
        .filter(|v| v.protected)
        .map(|v| v.group.clone())
        .collect();
    for v in &mut volumes {
        if protected.contains(&v.group) {
            v.ejectable = false;
            v.power_off = false;
        }
    }
    volumes.sort_by_key(|v| (v.name.to_lowercase(), v.device.clone()));
    volumes
}
fn space(path: &Path) -> Option<Space> {
    let stat = rustix::fs::statvfs(path).ok()?;
    let total = stat.f_blocks.saturating_mul(stat.f_frsize);
    let available = stat.f_bavail.saturating_mul(stat.f_frsize).min(total);
    Some(Space { total, available })
}
async fn objects(connection: &zbus::Connection, destination: &str) -> Result<ManagedObjects> {
    let proxy = zbus::Proxy::new(
        connection,
        destination,
        ROOT,
        "org.freedesktop.DBus.ObjectManager",
    )
    .await?;
    Ok(proxy.call("GetManagedObjects", &()).await?)
}
async fn system(timeout: u64) -> Result<zbus::Connection> {
    if crate::sandbox::active() {
        return Err(crate::sandbox::host_operation_error().into());
    }
    Ok(zbus::connection::Builder::system()?
        .method_timeout(Duration::from_secs(timeout))
        .build()
        .await?)
}
pub fn list() -> std::result::Result<Vec<Volume>, String> {
    zbus::block_on(async {
        let connection = system(10).await?;
        let mut volumes = parse(&objects(&connection, DESTINATION).await?);
        volumes.retain(|v| v.visible);
        for volume in &mut volumes {
            volume.space = volume.mount().and_then(space);
        }
        Ok::<_, Box<dyn std::error::Error + Send + Sync>>(volumes)
    })
    .map_err(|e| e.to_string())
}
async fn invoke(
    connection: &zbus::Connection,
    destination: &str,
    path: &str,
    interface: &str,
    method: &str,
) -> Result<()> {
    let proxy = zbus::Proxy::new(connection, destination, path, interface).await?;
    // Empty options allow normal interactive Polkit authorization. Never force unmount.
    let options = HashMap::<&str, Variant<'_>>::new();
    proxy
        .call_with_flags::<_, _, ()>(
            method,
            zbus::proxy::MethodFlags::AllowInteractiveAuth.into(),
            &(options,),
        )
        .await?;
    Ok(())
}
async fn perform(
    connection: &zbus::Connection,
    destination: &str,
    selected: &Volume,
    action: Action,
) -> Result<Outcome> {
    let volumes = parse(&objects(connection, destination).await?);
    let current = volumes
        .iter()
        .find(|v| {
            v.id == selected.id && v.device == selected.device && v.identity == selected.identity
        })
        .ok_or_else(|| {
            std::io::Error::other(crate::i18n::tr(
                "A unidade mudou ou foi desconectada. Atualize a lista.",
            ))
        })?;
    match action {
        Action::Open => {
            if let Some(path) = current.mount() {
                return Ok(Outcome::Opened(path.to_owned()));
            }
            let proxy =
                zbus::Proxy::new(connection, destination, current.id.as_str(), FILESYSTEM).await?;
            let mounted: String = proxy
                .call_with_flags(
                    "Mount",
                    zbus::proxy::MethodFlags::AllowInteractiveAuth.into(),
                    &(HashMap::<&str, Variant<'_>>::new(),),
                )
                .await?
                .ok_or_else(|| {
                    std::io::Error::other(crate::i18n::tr(
                        "O sistema não retornou o ponto de montagem.",
                    ))
                })?;
            let path = PathBuf::from(mounted);
            if !path.is_absolute() {
                return Err(std::io::Error::other(crate::i18n::tr(
                    "O sistema retornou um ponto de montagem inválido.",
                ))
                .into());
            }
            Ok(Outcome::Opened(path))
        }
        Action::Unmount => {
            if !current.can_unmount() {
                return Err(std::io::Error::other(crate::i18n::tr(
                    "Não é possível desmontar este volume do sistema.",
                ))
                .into());
            }
            invoke(connection, destination, &current.id, FILESYSTEM, "Unmount").await?;
            Ok(Outcome::Unmounted(current.mounts.clone()))
        }
        Action::Eject => {
            if !current.can_eject() {
                return Err(std::io::Error::other(crate::i18n::tr(
                    "Esta unidade não permite ejeção segura.",
                ))
                .into());
            }
            let power_off = current.power_off && (!current.optical || !current.ejectable);
            let related: Vec<_> = volumes
                .iter()
                .filter(|v| {
                    if power_off {
                        v.group == current.group
                    } else {
                        v.drive == current.drive
                    }
                })
                .collect();
            if related.iter().any(|v| v.protected) {
                return Err(std::io::Error::other(crate::i18n::tr(
                    "Esta unidade contém volumes essenciais do sistema.",
                ))
                .into());
            }
            let removed = related.iter().flat_map(|v| v.mounts.clone()).collect();
            for volume in related {
                if volume.mount().is_some() {
                    invoke(connection, destination, &volume.id, FILESYSTEM, "Unmount").await?;
                }
            }
            invoke(
                connection,
                destination,
                &current.drive,
                DRIVE,
                if power_off { "PowerOff" } else { "Eject" },
            )
            .await?;
            Ok(Outcome::Ejected(removed))
        }
    }
}
pub fn execute(volume: &Volume, action: Action) -> std::result::Result<Outcome, String> {
    zbus::block_on(async {
        let connection = system(180).await?;
        perform(&connection, DESTINATION, volume, action).await
    })
    .or_else(|e| {
        if let Some(zbus::Error::MethodError(name, _, _)) = e.downcast_ref::<zbus::Error>()
            && name.as_str() == "org.freedesktop.UDisks2.Error.NotAuthorizedDismissed"
        {
            return Ok(Outcome::Cancelled);
        }
        Err(e)
    })
    .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests;

#[cfg(test)]
pub(crate) fn test_volume(mount: PathBuf) -> Volume {
    Volume {
        id: "/org/freedesktop/UDisks2/block_devices/test1".into(),
        device: "/dev/test1".into(),
        name: "Disco de teste".into(),
        filesystem: "ext4".into(),
        size: 1024 * 1024 * 1024,
        mounts: vec![mount],
        space: Some(Space {
            total: 1024 * 1024 * 1024,
            available: 512 * 1024 * 1024,
        }),
        visible: true,
        removable: true,
        optical: false,
        protected: false,
        ejectable: false,
        power_off: true,
        drive: "/org/freedesktop/UDisks2/drives/test".into(),
        group: "test".into(),
        identity: "test".into(),
    }
}
