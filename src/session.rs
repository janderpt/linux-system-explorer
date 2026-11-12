//! Window-session state is independent of preferences and shell processes.
use crate::settings;
use serde::{Deserialize, Serialize};
pub(crate) mod closed;
pub(crate) mod paths;
use std::{
    collections::{BTreeSet, VecDeque},
    fs,
    io::{self, Read, Write},
    path::{Path, PathBuf},
};

const MAX_BYTES: u64 = 4 * 1024 * 1024;

/// Pane references are indices in the saved tab list, never runtime tab IDs.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Split {
    pub left: usize,
    pub right: usize,
    pub ratio: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    pub version: u32,
    pub tabs: Vec<String>,
    pub active: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub split: Option<Split>,
    #[serde(skip_serializing_if = "VecDeque::is_empty")]
    pub closed_tabs: VecDeque<closed::ClosedTab>,
}
impl Default for State {
    fn default() -> Self {
        Self {
            version: 1,
            tabs: vec![],
            active: 0,
            split: None,
            closed_tabs: Default::default(),
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
            split: None,
            closed_tabs: Default::default(),
        }
    }
    pub fn paths(&self) -> io::Result<Vec<PathBuf>> {
        if self.version != 1 {
            return Err(invalid("Unsupported window-session version"));
        }
        let mut ids = BTreeSet::new();
        for closed in &self.closed_tabs {
            closed.validate().map_err(|e| invalid(&e))?;
            if closed.id < self.tabs.len() as u64 || !ids.insert(closed.id) {
                return Err(invalid(
                    "Closed tab identifiers conflict with the saved session",
                ));
            }
        }
        self.tabs
            .iter()
            .map(|key| paths::decode(key).map_err(|e| invalid(&e)))
            .collect()
    }
    pub fn active_folder(&self) -> Option<PathBuf> {
        let paths = self.paths().ok()?;
        paths
            .get(self.active.min(paths.len().saturating_sub(1)))
            .cloned()
    }
    pub fn normalized_split(&self) -> Option<Split> {
        let mut split = self.split?;
        let active = self.active.min(self.tabs.len().saturating_sub(1));
        if split.left == split.right
            || split.left >= self.tabs.len()
            || split.right >= self.tabs.len()
            || ![split.left, split.right].contains(&active)
        {
            return None;
        }
        split.ratio = if split.ratio.is_finite() {
            split.ratio.clamp(0.05, 0.95)
        } else {
            0.5
        };
        Some(split)
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
        while state.closed_tabs.len() > 25 {
            state.closed_tabs.pop_front();
        }
        for closed in &mut state.closed_tabs {
            closed.normalize();
        }
        state.active = state.active.min(state.tabs.len().saturating_sub(1));
        // Invalid optional pane metadata must not discard otherwise valid tabs.
        state.split = state.normalized_split();
        Ok(Some(state))
    }
    pub fn save(&self, path: &Path) -> io::Result<()> {
        self.paths()?;
        let mut state = self.clone();
        state.split = state.normalized_split();
        let data = serde_json::to_vec_pretty(&state).map_err(io::Error::other)?;
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
    pub closed_tabs: VecDeque<closed::ClosedTab>,
    pub store: Store,
    pub error: Option<String>,
}
impl Startup {
    pub fn load(restore: bool) -> Self {
        Self::at(Store::path(), restore)
    }
    pub fn at(path: Option<PathBuf>, restore: bool) -> Self {
        let loaded = path.as_ref().map(|p| State::load(p)).unwrap_or(Ok(None));
        let closed_tabs = loaded
            .as_ref()
            .ok()
            .and_then(|state| state.as_ref())
            .map(|state| state.closed_tabs.clone())
            .unwrap_or_default();
        let (saved, error) = match loaded {
            Ok(saved) => (if restore { saved } else { None }, None),
            Err(e) => (None, Some(e.to_string())),
        };
        let blocked = error.is_some();
        Self {
            saved,
            closed_tabs,
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
