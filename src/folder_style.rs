//! Per-folder appearance is independent of global theme and icon modes.
use crate::{icons, settings};
use eframe::egui::{Color32, Painter, Rect};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::Path};
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Style {
    pub icon: String,
    pub color: [u8; 3],
}
pub type Styles = BTreeMap<String, Style>;
impl Style {
    pub fn paint(&self, painter: &Painter, rect: Rect) {
        let glyph = icons::glyph(&self.icon)
            .or_else(|| icons::glyph("folder"))
            .expect("folder glyph");
        icons::paint_glyph(
            painter,
            rect,
            glyph,
            Color32::from_rgb(self.color[0], self.color[1], self.color[2]),
        );
    }
}
pub fn paint(styles: &Styles, path: &Path, painter: &Painter, rect: Rect) -> bool {
    if let Some(style) = styles.get(&settings::folder_key(path)) {
        style.paint(painter, rect);
        true
    } else {
        false
    }
}
pub fn normalize(styles: &mut Styles) {
    styles.retain(|key, style| {
        settings::folder_path(key).is_some_and(|p| p.is_absolute())
            && icons::glyph(&style.icon).is_some()
    });
}
pub fn rename(styles: &mut Styles, old: &Path, new: &Path) {
    let moved: Vec<_> = styles
        .keys()
        .filter_map(|key| {
            let path = settings::folder_path(key)?;
            let relative = path.strip_prefix(old).ok()?;
            let destination = if relative.as_os_str().is_empty() {
                new.to_owned()
            } else {
                new.join(relative)
            };
            Some((key.clone(), settings::folder_key(&destination)))
        })
        .collect();
    let values: Vec<_> = moved
        .into_iter()
        .filter_map(|(old, new)| styles.remove(&old).map(|style| (new, style)))
        .collect();
    styles.extend(values);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{ffi::OsString, os::unix::ffi::OsStringExt};
    #[test]
    fn styles_round_trip_raw_paths_normalize_invalid_entries_and_follow_folder_rename() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join(OsString::from_vec(b"raw-\xff".to_vec()));
        let child = path.join("child");
        let other = root.path().join("other");
        let style = Style {
            icon: "star".into(),
            color: [7, 101, 222],
        };
        let mut prefs = settings::Preferences::default();
        assert!(
            settings::Preferences::load(&root.path().join("missing.json"))
                .unwrap()
                .folder_styles
                .is_empty()
        );
        for path in [&path, &child, &other] {
            prefs
                .folder_styles
                .insert(settings::folder_key(path), style.clone());
        }
        prefs.folder_styles.insert("invalid".into(), style.clone());
        prefs.folder_styles.insert(
            settings::folder_key(&root.path().join("bad")),
            Style {
                icon: "not-a-real-icon".into(),
                color: [1, 2, 3],
            },
        );
        prefs.normalize();
        assert_eq!(prefs.folder_styles.len(), 3);
        let config = root.path().join("preferences.json");
        prefs.save(&config).unwrap();
        let restored = settings::Preferences::load(&config).unwrap();
        assert_eq!(restored.folder_styles, prefs.folder_styles);
        let mut styles = restored.folder_styles;
        let renamed = root.path().join("renamed");
        rename(&mut styles, &path, &renamed);
        assert!(!styles.contains_key(&settings::folder_key(&path)));
        assert_eq!(styles.get(&settings::folder_key(&renamed)), Some(&style));
        assert_eq!(
            styles.get(&settings::folder_key(&renamed.join("child"))),
            Some(&style)
        );
        assert_eq!(styles.get(&settings::folder_key(&other)), Some(&style));
    }
}
