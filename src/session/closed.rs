use crate::{navigation::History, settings::FolderLayout};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, path::PathBuf};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct ClosedTab {
    pub id: u64,
    pub position: usize,
    pub left: Option<u64>,
    pub right: Option<u64>,
    #[serde(with = "super::paths::one")]
    pub path: PathBuf,
    pub context: Option<BrowsingContext>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct BrowsingContext {
    pub history: History,
    pub layout: FolderLayout,
    pub query: String,
    #[serde(with = "super::paths::set")]
    pub selected: BTreeSet<PathBuf>,
    #[serde(with = "super::paths::optional")]
    pub anchor: Option<PathBuf>,
    #[serde(with = "super::paths::optional")]
    pub cursor: Option<PathBuf>,
    pub reveal_hidden: bool,
}
impl ClosedTab {
    pub(super) fn normalize(&mut self) {
        if let Some(context) = &mut self.context {
            context.history.normalize();
            context.layout.normalize();
        }
    }
    pub(super) fn validate(&self) -> Result<(), String> {
        super::paths::validate(&self.path)?;
        if let Some(context) = &self.context {
            for path in context
                .history
                .paths()
                .iter()
                .chain(context.selected.iter())
                .chain(context.anchor.iter())
                .chain(context.cursor.iter())
            {
                super::paths::validate(path)?;
            }
        }
        Ok(())
    }
}
