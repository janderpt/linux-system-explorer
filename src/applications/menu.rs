//! Menu applications are launchers, never file entries or a directory hierarchy.
use super::*;
pub mod software;
mod theme;

pub fn location() -> PathBuf {
    PathBuf::from("applications:")
}
pub fn is_location(path: &Path) -> bool {
    path == Path::new("applications:")
}

#[derive(Clone, Debug)]
pub struct Launcher {
    pub application: Application,
    pub icon: Option<crate::filesystem::Entry>,
}

pub fn load() -> io::Result<Vec<Launcher>> {
    if crate::sandbox::active() {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            crate::i18n::tr(
                "A lista de aplicativos do sistema ainda não está disponível no Flatpak experimental.",
            ),
        ));
    }
    let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
    let locales: &[&str] = match crate::i18n::current() {
        crate::i18n::Locale::Portuguese => &["pt_BR", "pt_PT", "pt"],
        crate::i18n::Locale::English => &["en_US", "en_GB", "en"],
    };
    let applications = collect(
        freedesktop_desktop_entry::default_paths(),
        locales,
        &desktop,
    );
    let icons = theme::Icons::system();
    Ok(applications
        .into_iter()
        .map(|(application, icon)| Launcher {
            application,
            icon: icon
                .and_then(|name| icons.resolve(&name))
                .and_then(|path| crate::filesystem::read_entry(&path).ok()),
        })
        .collect())
}

fn visible_on(entry: &DesktopEntry, desktop: &str) -> bool {
    let only = entry.only_show_in();
    let excluded = entry.not_show_in();
    for name in desktop.split(':').filter(|name| !name.is_empty()) {
        if only.as_ref().is_some_and(|names| names.contains(&name)) {
            return true;
        }
        if excluded.as_ref().is_some_and(|names| names.contains(&name)) {
            return false;
        }
    }
    only.is_none()
}

fn collect(
    roots: impl IntoIterator<Item = PathBuf>,
    locales: &[&str],
    desktop: &str,
) -> Vec<(Application, Option<String>)> {
    let mut seen = BTreeSet::new();
    let mut apps = Vec::new();
    for root in roots {
        let root = root.canonicalize().unwrap_or(root);
        for path in Iter::new(std::iter::once(root.clone())) {
            let Ok(relative) = path.strip_prefix(&root) else {
                continue;
            };
            let id = relative.to_string_lossy().replace('/', "-");
            if !seen.insert(id.clone()) {
                continue;
            }
            let Ok(entry) = DesktopEntry::from_path(&path, Some(locales)) else {
                continue;
            };
            if entry.hidden()
                || entry.no_display()
                || entry.type_() != Some("Application")
                || !visible_on(&entry, desktop)
                || entry
                    .try_exec()
                    .is_some_and(|exec| !executable(Path::new(exec)))
            {
                continue;
            }
            if !entry.dbus_activatable()
                && !entry
                    .parse_exec()
                    .ok()
                    .and_then(|args| args.first().cloned())
                    .is_some_and(|exec| executable(Path::new(&exec)))
            {
                continue;
            }
            if let Some(app) = application(&entry, id, locales, false, false)
                && !app.name.trim().is_empty()
            {
                apps.push((app, entry.icon().map(str::to_owned)));
            }
        }
    }
    apps.sort_by_cached_key(|(app, _)| (app.name.to_lowercase(), app.id.clone()));
    apps
}

pub fn launch(app: &Application) -> Result<(), String> {
    if crate::sandbox::active() {
        return Err(crate::sandbox::host_operation_error().to_string());
    }
    let result = run(Command::new("gio")
        .arg("launch")
        .arg(&app.path)
        .current_dir(dirs::home_dir().unwrap_or_else(|| "/".into())));
    match result {
        Ok((status, _)) if status.success() => Ok(()),
        Ok((status, diagnostic)) => Err(crate::i18n::message(
            "Não foi possível iniciar {}: {}",
            &[
                app.name.clone(),
                if diagnostic.is_empty() {
                    status.to_string()
                } else {
                    diagnostic
                },
            ],
        )),
        Err(error) => Err(crate::i18n::message(
            "Não foi possível iniciar {}: {}",
            &[app.name.clone(), error.to_string()],
        )),
    }
}

#[cfg(test)]
mod tests;
