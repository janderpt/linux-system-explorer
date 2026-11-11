use std::path::{Path, PathBuf};

/// History changes only after the requested directory has been read successfully.
#[derive(Default)]
pub struct History {
    paths: Vec<PathBuf>,
    cursor: usize,
}

#[derive(Clone, Copy)]
pub enum Navigation {
    Visit,
    Back,
    Forward,
    Refresh,
}

impl History {
    pub fn rename_folder(&mut self, old: &Path, new: &Path) {
        for path in &mut self.paths {
            if let Ok(relative) = path.strip_prefix(old) {
                *path = new.join(relative);
            }
        }
    }

    pub fn back(&self) -> Option<&Path> {
        self.cursor.checked_sub(1).map(|i| self.paths[i].as_path())
    }

    pub fn forward(&self) -> Option<&Path> {
        self.paths.get(self.cursor + 1).map(PathBuf::as_path)
    }

    pub fn commit(&mut self, path: PathBuf, navigation: Navigation) {
        match navigation {
            Navigation::Back => self.cursor = self.cursor.saturating_sub(1),
            Navigation::Forward => {
                self.cursor = (self.cursor + 1).min(self.paths.len().saturating_sub(1))
            }
            Navigation::Refresh => (),
            Navigation::Visit => {
                if self.paths.get(self.cursor) == Some(&path) {
                    return;
                }
                if !self.paths.is_empty() {
                    self.paths.truncate(self.cursor + 1);
                }
                self.paths.push(path);
                self.cursor = self.paths.len() - 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn visiting_after_back_discards_forward_history() {
        let mut history = History::default();
        for path in ["/a", "/b", "/c"] {
            history.commit(path.into(), Navigation::Visit);
        }
        history.commit("/b".into(), Navigation::Back);
        assert_eq!(history.forward(), Some(Path::new("/c")));
        history.commit("/d".into(), Navigation::Visit);
        assert_eq!(history.back(), Some(Path::new("/b")));
        assert!(history.forward().is_none());
    }

    #[test]
    fn refresh_and_repeated_visits_do_not_add_history() {
        let mut history = History::default();
        history.commit("/a".into(), Navigation::Visit);
        history.commit("/a".into(), Navigation::Refresh);
        history.commit("/a".into(), Navigation::Visit);
        assert!(history.back().is_none());
        assert!(history.forward().is_none());
    }
}
