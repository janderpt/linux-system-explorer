use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error};
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

pub(super) fn decode(key: &str) -> Result<PathBuf, String> {
    let path = crate::settings::folder_path(key).ok_or("Invalid saved folder path")?;
    validate(&path)?;
    Ok(path)
}
pub(super) fn validate(path: &Path) -> Result<(), String> {
    use std::os::unix::ffi::OsStrExt;
    if (!path.is_absolute() && !crate::trashbin::is_location(path))
        || path.as_os_str().as_bytes().contains(&0)
    {
        return Err("Saved tabs must contain absolute local paths or Trash".into());
    }
    Ok(())
}
pub mod one {
    use super::*;
    pub fn serialize<S: Serializer>(path: &Path, serializer: S) -> Result<S::Ok, S::Error> {
        crate::settings::folder_key(path).serialize(serializer)
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<PathBuf, D::Error> {
        decode(&String::deserialize(deserializer)?).map_err(D::Error::custom)
    }
}
pub mod list {
    use super::*;
    pub fn serialize<S: Serializer>(paths: &[PathBuf], serializer: S) -> Result<S::Ok, S::Error> {
        paths
            .iter()
            .map(|p| crate::settings::folder_key(p))
            .collect::<Vec<_>>()
            .serialize(serializer)
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Vec<PathBuf>, D::Error> {
        Vec::<String>::deserialize(deserializer)?
            .iter()
            .map(|p| decode(p).map_err(D::Error::custom))
            .collect()
    }
}
pub mod set {
    use super::*;
    pub fn serialize<S: Serializer>(
        paths: &BTreeSet<PathBuf>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        paths
            .iter()
            .map(|p| crate::settings::folder_key(p))
            .collect::<Vec<_>>()
            .serialize(serializer)
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<BTreeSet<PathBuf>, D::Error> {
        Ok(list::deserialize(deserializer)?.into_iter().collect())
    }
}
pub mod optional {
    use super::*;
    pub fn serialize<S: Serializer>(
        path: &Option<PathBuf>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        path.as_ref()
            .map(|p| crate::settings::folder_key(p))
            .serialize(serializer)
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Option<PathBuf>, D::Error> {
        Option::<String>::deserialize(deserializer)?
            .as_deref()
            .map(decode)
            .transpose()
            .map_err(D::Error::custom)
    }
}
