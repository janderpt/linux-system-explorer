//! Window-session state is independent of preferences and shell processes.
use crate::settings;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{self, Read, Write},
    os::unix::ffi::OsStrExt,
    path::{Path, PathBuf},
};

const MAX_BYTES: u64 = 4 * 1024 * 1024;
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    pub version: u32,
    pub tabs: Vec<String>,
    pub active: usize,
}
impl Default for State {
    fn default() -> Self {
        Self {
            version: 1,
            tabs: vec![],
            active: 0,
        }
    }
}
impl State {
    pub fn from_paths(paths: impl IntoIterator<Item = PathBuf>, active: usize) -> Self {
        let tabs: Vec<_> = paths
            .into_iter()
            .map(|p| settings::folder_key(&p))
            .collect();
        Self {
            version: 1,
            active: active.min(tabs.len().saturating_sub(1)),
            tabs,
        }
    }
    pub fn paths(&self) -> io::Result<Vec<PathBuf>> {
        if self.version != 1 {
            return Err(invalid("Unsupported window-session version"));
        }
        self.tabs
            .iter()
            .map(|key| {
                let path = settings::folder_path(key)
                    .ok_or_else(|| invalid("Invalid saved folder path"))?;
                if (!path.is_absolute() && !crate::trashbin::is_location(&path))
                    || path.as_os_str().as_bytes().contains(&0)
                {
                    return Err(invalid(
                        "Saved tabs must contain absolute local paths or Trash",
                    ));
                }
                Ok(path)
            })
            .collect()
    }
    pub fn active_folder(&self) -> Option<PathBuf> {
        let paths = self.paths().ok()?;
        paths
            .get(self.active.min(paths.len().saturating_sub(1)))
            .cloned()
    }
    pub fn load(path: &Path) -> io::Result<Option<Self>> {
        let file = match fs::File::open(path) {
            Ok(file) => file,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e),
        };
        let mut bytes = vec![];
        file.take(MAX_BYTES + 1).read_to_end(&mut bytes)?;
        if bytes.len() as u64 > MAX_BYTES {
            return Err(invalid("Window-session file is too large"));
        }
        let mut state: Self =
            serde_json::from_slice(&bytes).map_err(|e| invalid(&e.to_string()))?;
        state.paths()?;
        state.active = state.active.min(state.tabs.len().saturating_sub(1));
        Ok(Some(state))
    }
    pub fn save(&self, path: &Path) -> io::Result<()> {
        self.paths()?;
        let data = serde_json::to_vec_pretty(self).map_err(io::Error::other)?;
        if data.len() as u64 > MAX_BYTES {
            return Err(invalid("Window-session file is too large"));
        }
        let parent = path
            .parent()
            .ok_or_else(|| invalid("Invalid window-session path"))?;
        fs::create_dir_all(parent)?;
        let mut file = tempfile::NamedTempFile::new_in(parent)?;
        file.write_all(&data)?;
        file.write_all(b"\n")?;
        file.as_file().sync_all()?;
        file.persist(path).map_err(|e| e.error)?;
        Ok(())
    }
}
fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.to_owned())
}

pub struct Store {
    path: Option<PathBuf>,
    last_attempted: Option<State>,
    blocked: bool,
    reported: bool,
}
impl Store {
    pub fn path() -> Option<PathBuf> {
        dirs::state_dir().map(|p| p.join("linux-system-explorer/session.json"))
    }
    pub fn save(&mut self, state: State, force: bool) -> io::Result<()> {
        if self.blocked || (!force && self.last_attempted.as_ref() == Some(&state)) {
            return Ok(());
        }
        self.last_attempted = Some(state.clone());
        if let Some(path) = &self.path {
            if let Err(error) = state.save(path) {
                eprintln!("Could not save explorer tabs: {error}");
                if !self.reported {
                    self.reported = true;
                    return Err(error);
                }
            } else {
                self.reported = false;
            }
        }
        Ok(())
    }
    #[cfg(test)]
    pub fn at(path: Option<PathBuf>) -> Self {
        Self {
            path,
            last_attempted: None,
            blocked: false,
            reported: false,
        }
    }
}

pub struct Startup {
    pub saved: Option<State>,
    pub store: Store,
    pub error: Option<String>,
}
impl Startup {
    pub fn load(restore: bool) -> Self {
        Self::at(Store::path(), restore)
    }
    pub fn at(path: Option<PathBuf>, restore: bool) -> Self {
        let loaded = path.as_ref().map(|p| State::load(p)).unwrap_or(Ok(None));
        let (saved, error) = match loaded {
            Ok(saved) => (if restore { saved } else { None }, None),
            Err(e) => (None, Some(e.to_string())),
        };
        let blocked = error.is_some();
        Self {
            saved,
            store: Store {
                path,
                last_attempted: None,
                blocked,
                reported: false,
            },
            error,
        }
    }
}
#[cfg(test)]
mod tests;
