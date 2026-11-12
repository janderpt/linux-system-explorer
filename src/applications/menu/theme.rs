//! Theme lookup runs in the catalog worker, separate from bounded image decoding.
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Read,
    path::{Path, PathBuf},
};
pub(super) struct Icons {
    files: BTreeMap<String, PathBuf>,
    pixmaps: Vec<PathBuf>,
}
fn text(path: &Path) -> Option<String> {
    let mut bytes = Vec::new();
    fs::File::open(path)
        .ok()?
        .take(256 * 1024 + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    (bytes.len() <= 256 * 1024)
        .then(|| String::from_utf8(bytes).ok())
        .flatten()
}
fn value(text: &str, section: &str, key: &str) -> Option<String> {
    let mut active = false;
    for line in text.lines().map(str::trim) {
        if line.starts_with('[') {
            active = line == format!("[{section}]");
        } else if active
            && let Some((name, value)) = line.split_once('=')
            && name.trim() == key
        {
            return Some(value.trim().trim_matches('"').to_owned());
        }
    }
    None
}
fn component(name: &str) -> bool {
    !name.is_empty()
        && Path::new(name).components().count() == 1
        && matches!(
            Path::new(name).components().next(),
            Some(std::path::Component::Normal(_))
        )
}
impl Icons {
    pub(super) fn system() -> Self {
        let mut roots = Vec::new();
        if let Some(home) = dirs::home_dir() {
            roots.push(home.join(".icons"));
        }
        if let Some(data) = dirs::data_dir() {
            roots.push(data.join("icons"));
        }
        let data: Vec<PathBuf> = std::env::var_os("XDG_DATA_DIRS")
            .map(|s| std::env::split_paths(&s).collect())
            .unwrap_or_else(|| vec!["/usr/local/share".into(), "/usr/share".into()]);
        roots.extend(data.iter().map(|p| p.join("icons")));
        let mut themes = Vec::new();
        if let Some(config) = dirs::config_dir() {
            if let Some(t) = text(&config.join("kdeglobals"))
                && let Some(theme) = value(&t, "Icons", "Theme")
            {
                themes.push(theme);
            }
            for file in ["gtk-4.0/settings.ini", "gtk-3.0/settings.ini"] {
                if let Some(t) = text(&config.join(file))
                    && let Some(theme) = value(&t, "Settings", "gtk-icon-theme-name")
                {
                    themes.push(theme);
                }
            }
        }
        themes.extend(["hicolor".into(), "Adwaita".into(), "breeze".into()]);
        let mut this = Self {
            files: BTreeMap::new(),
            pixmaps: data.iter().map(|p| p.join("pixmaps")).collect(),
        };
        let mut seen = BTreeSet::new();
        for theme in themes {
            this.theme(&roots, &theme, &mut seen);
        }
        this
    }
    fn theme(&mut self, roots: &[PathBuf], name: &str, seen: &mut BTreeSet<String>) {
        if !component(name) || seen.len() >= 32 || !seen.insert(name.to_owned()) {
            return;
        }
        let mut inherits = Vec::new();
        for root in roots {
            let theme = root.join(name);
            let Some(index) = text(&theme.join("index.theme")) else {
                continue;
            };
            let mut directories = Vec::new();
            for key in ["Directories", "ScaledDirectories"] {
                if let Some(list) = value(&index, "Icon Theme", key) {
                    directories.extend(
                        list.split(',')
                            .map(str::trim)
                            .filter(|s| !s.is_empty())
                            .map(str::to_owned),
                    );
                }
            }
            directories.sort_by_key(|path| {
                (
                    value(&index, path, "Size")
                        .and_then(|s| s.parse::<i32>().ok())
                        .unwrap_or(48)
                        .abs_diff(48),
                    path.clone(),
                )
            });
            for directory in directories {
                let relative = Path::new(&directory);
                if relative.is_absolute()
                    || relative
                        .components()
                        .any(|c| !matches!(c, std::path::Component::Normal(_)))
                {
                    continue;
                }
                for entry in fs::read_dir(theme.join(relative))
                    .into_iter()
                    .flatten()
                    .flatten()
                    .take(30000)
                {
                    if self.files.len() >= 30000 {
                        return;
                    }
                    let path = entry.path();
                    if matches!(
                        path.extension().and_then(|s| s.to_str()),
                        Some("png" | "svg")
                    ) && let Some(stem) = path.file_stem().and_then(|s| s.to_str())
                    {
                        self.files.entry(stem.to_owned()).or_insert(path);
                    }
                }
            }
            if let Some(list) = value(&index, "Icon Theme", "Inherits") {
                inherits.extend(
                    list.split(',')
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .map(str::to_owned),
                );
            }
        }
        for inherited in inherits {
            self.theme(roots, &inherited, seen);
        }
    }
    pub(super) fn resolve(&self, name: &str) -> Option<PathBuf> {
        let path = Path::new(name);
        if path.is_absolute() {
            return path.is_file().then(|| path.to_owned());
        }
        if !component(name) {
            return None;
        }
        let stem = path.file_stem()?.to_str()?;
        if let Some(path) = self.files.get(name).or_else(|| self.files.get(stem)) {
            return Some(path.clone());
        }
        for root in &self.pixmaps {
            for extension in ["png", "svg"] {
                let file = root.join(format!("{stem}.{extension}"));
                if file.is_file() {
                    return Some(file);
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn icons_prefer_nearby_sizes_and_inherit_without_following_directory_traversal() {
        let root = tempfile::tempdir().unwrap();
        for theme in ["chosen", "fallback"] {
            std::fs::create_dir_all(root.path().join(theme).join("48/apps")).unwrap();
        }
        std::fs::write(root.path().join("chosen/index.theme"),"[Icon Theme]\nDirectories=16/apps,48/apps,../../outside\nInherits=fallback\n[16/apps]\nSize=16\n[48/apps]\nSize=48\n").unwrap();
        std::fs::create_dir_all(root.path().join("chosen/16/apps")).unwrap();
        std::fs::write(
            root.path().join("fallback/index.theme"),
            "[Icon Theme]\nDirectories=48/apps\nInherits=chosen\n",
        )
        .unwrap();
        for file in [
            "chosen/16/apps/app.png",
            "chosen/48/apps/app.png",
            "fallback/48/apps/other.svg",
        ] {
            std::fs::write(root.path().join(file), "fixture").unwrap();
        }
        let mut icons = Icons {
            files: Default::default(),
            pixmaps: vec![],
        };
        icons.theme(&[root.path().to_owned()], "chosen", &mut Default::default());
        assert_eq!(
            icons.resolve("app").unwrap(),
            root.path().join("chosen/48/apps/app.png")
        );
        assert!(icons.resolve("other").unwrap().ends_with("other.svg"));
        assert!(icons.resolve("../outside").is_none());
    }
}
