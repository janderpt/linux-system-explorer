//! Open the desktop's software manager; package changes remain in its own UI.
use super::*;
use std::{
    fs,
    io::{Read, Seek},
    thread,
    time::Duration,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Manager {
    Discover,
    Gnome,
}
impl Manager {
    fn program(self) -> &'static str {
        match self {
            Self::Discover => "plasma-discover",
            Self::Gnome => "gnome-software",
        }
    }
    fn command(self, app: &Application, component: Option<&str>) -> Command {
        let mut command = Command::new(self.program());
        match (self, component) {
            (Self::Discover, Some(id)) => {
                command.args(["--application", &format!("appstream://{id}")]);
            }
            (Self::Gnome, Some(id)) => {
                command.arg(format!("--details={id}"));
            }
            (_, None) => {
                command.arg(format!("--search={}", app.name));
            }
        }
        command
    }
}
fn choose(desktop: &str, installed: impl Fn(&str) -> bool) -> Option<Manager> {
    let kde = desktop.split(':').find(|s| matches!(*s, "KDE" | "GNOME")) == Some("KDE");
    let choices = if kde {
        [Manager::Discover, Manager::Gnome]
    } else {
        [Manager::Gnome, Manager::Discover]
    };
    choices.into_iter().find(|m| installed(m.program()))
}
fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 255
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
        && !id.starts_with('-')
}
fn metadata_id(xml: &str, desktop: &str) -> Option<String> {
    let document = roxmltree::Document::parse(xml).ok()?;
    let component = document.root_element();
    if !component.has_tag_name("component") {
        return None;
    }
    let id = component
        .children()
        .find(|n| n.has_tag_name("id"))?
        .text()?
        .trim();
    let matches = component.children().any(|n| {
        (n.has_tag_name("launchable") && n.attribute("type") == Some("desktop-id")
            || n.has_tag_name("desktop_file"))
            && n.text().is_some_and(|s| s.trim() == desktop)
    }) || id == desktop;
    (matches && valid_id(id)).then(|| id.to_owned())
}
fn read_metadata(path: &Path, remaining: &mut usize) -> Option<String> {
    let mut bytes = Vec::new();
    fs::File::open(path)
        .ok()?
        .take((1024 * 1024 + 1).min(*remaining) as u64)
        .read_to_end(&mut bytes)
        .ok()?;
    *remaining = remaining.saturating_sub(bytes.len());
    if bytes.len() > 1024 * 1024 {
        return None;
    }
    String::from_utf8(bytes).ok()
}
fn lookup(app: &Application, roots: &[PathBuf]) -> Option<String> {
    let mut bytes = 32 * 1024 * 1024;
    let mut count = 0;
    for root in roots {
        for directory in ["metainfo", "appdata"] {
            for entry in fs::read_dir(root.join(directory))
                .into_iter()
                .flatten()
                .flatten()
                .take(4096)
            {
                if count >= 4096 || bytes == 0 {
                    return None;
                }
                count += 1;
                let path = entry.path();
                if path.extension().is_some_and(|s| s == "xml")
                    && let Some(text) = read_metadata(&path, &mut bytes)
                    && let Some(id) = metadata_id(&text, &app.id)
                {
                    return Some(id);
                }
            }
        }
    }
    None
}
fn component(app: &Application) -> Option<String> {
    let mut roots = Vec::new();
    // Canonical paths include the app's own Flatpak export or Snap data tree.
    for path in [Some(app.path.clone()), app.path.canonicalize().ok()]
        .into_iter()
        .flatten()
    {
        if let Some(root) = path.parent().and_then(Path::parent) {
            roots.push(root.to_owned());
        }
    }
    if let Some(home) = dirs::data_dir() {
        roots.push(home);
    }
    roots.extend(
        std::env::var_os("XDG_DATA_DIRS")
            .map(|s| std::env::split_paths(&s).collect::<Vec<_>>())
            .unwrap_or_else(|| vec!["/usr/local/share".into(), "/usr/share".into()]),
    );
    let mut seen = BTreeSet::new();
    roots.retain(|p| p.is_absolute() && seen.insert(p.clone()));
    lookup(app, &roots).or_else(|| {
        let entry = DesktopEntry::from_path(&app.path, None::<&[&str]>).ok()?;
        let id = entry.desktop_entry("X-Flatpak")?;
        valid_id(id).then(|| id.to_owned())
    })
}
/// Returns true if we had to open a name search rather than an exact AppStream page.
pub fn open(app: &Application) -> Result<bool, String> {
    if crate::sandbox::active() {
        return Err(crate::sandbox::host_operation_error().to_string());
    }
    let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
    let manager = choose(&desktop, |p| executable(Path::new(p))).ok_or_else(|| crate::i18n::tr("Nenhum gerenciador de software compatível foi encontrado. Instale o GNOME Software ou o KDE Discover.").to_owned())?;
    let component = component(app);
    let mut command = manager.command(app, component.as_deref());
    let mut diagnostics = tempfile::tempfile().map_err(|e| launch_error(e.to_string()))?;
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::from(
            diagnostics
                .try_clone()
                .map_err(|e| launch_error(e.to_string()))?,
        ));
    let mut child = command
        .spawn()
        .map_err(|error| launch_error(error.to_string()))?;
    // Detect immediate startup failures without waiting for a graphical window to close.
    // This entire operation runs in a worker, including metadata lookup.
    for _ in 0..10 {
        match child.try_wait() {
            Ok(Some(status)) if !status.success() => {
                let _ = diagnostics.rewind();
                let mut bytes = Vec::new();
                let _ = diagnostics.take(8192).read_to_end(&mut bytes);
                let reason = String::from_utf8_lossy(&bytes);
                return Err(launch_error(if reason.trim().is_empty() {
                    status.to_string()
                } else {
                    reason.trim().to_owned()
                }));
            }
            Ok(Some(_)) => return Ok(component.is_none()),
            Err(error) => {
                thread::spawn(move || {
                    let _ = child.wait();
                });
                return Err(launch_error(error.to_string()));
            }
            Ok(None) => thread::sleep(Duration::from_millis(20)),
        }
    }
    thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(component.is_none())
}

fn launch_error(error: String) -> String {
    crate::i18n::message(
        "Não foi possível abrir o gerenciador de software: {}",
        &[error],
    )
}
#[cfg(test)]
mod tests;
