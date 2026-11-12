//! Installed desktop applications; execution is delegated to GIO so field codes,
//! terminal applications, Flatpak/Snap launchers and D-Bus activation stay native.
use freedesktop_desktop_entry::{DesktopEntry, Iter};
use std::{
    collections::BTreeSet,
    io::{self, Read, Seek},
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

#[derive(Clone, Debug)]
pub struct Application {
    pub id: String,
    pub path: PathBuf,
    pub name: String,
    pub description: String,
    pub recommended: bool,
    pub is_default: bool,
}

pub struct Applications {
    pub mime: Option<String>,
    pub items: Vec<Application>,
}

pub fn discover(path: &Path) -> io::Result<Applications> {
    // Do not scan or launch from the directory containing the selected file.
    // Only the system/user application registry supplies desktop entries.
    path.metadata()?;
    let mime = query(&["filetype"], Some(path));
    let default = mime
        .as_ref()
        .and_then(|mime| query(&["default", mime], None));
    let locales: &[&str] = match crate::i18n::current() {
        crate::i18n::Locale::Portuguese => &["pt_BR", "pt_PT", "pt"],
        crate::i18n::Locale::English => &["en_US", "en_GB", "en"],
    };
    let items = collect(
        freedesktop_desktop_entry::default_paths(),
        locales,
        mime.as_deref(),
        default.as_deref(),
    );
    Ok(Applications { mime, items })
}

fn query(arguments: &[&str], path: Option<&Path>) -> Option<String> {
    let mut command = Command::new("xdg-mime");
    command.arg("query").args(arguments);
    if let Some(path) = path {
        command.arg(path);
    }
    let output = command
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    let value = String::from_utf8(output.stdout).ok()?.trim().to_owned();
    (output.status.success() && !value.is_empty()).then_some(value)
}

fn collect(
    roots: impl IntoIterator<Item = PathBuf>,
    locales: &[&str],
    mime: Option<&str>,
    default: Option<&str>,
) -> Vec<Application> {
    let mut seen = BTreeSet::new();
    let mut apps = Vec::new();
    // Process each root completely before the next, including nested desktop IDs.
    for root in roots {
        let root = root.canonicalize().unwrap_or(root);
        for path in Iter::new(std::iter::once(root.clone())) {
            let Ok(relative) = path.strip_prefix(&root) else {
                continue;
            };
            let id = relative.to_string_lossy().replace('/', "-");
            // A Hidden user override must also mask its system entry.
            if !seen.insert(id.clone()) {
                continue;
            }
            let Ok(entry) = DesktopEntry::from_path(&path, Some(locales)) else {
                continue;
            };
            if entry.hidden() || entry.type_() != Some("Application") {
                continue;
            }
            let is_default = default == Some(id.as_str());
            let recommended = is_default
                || mime.is_some_and(|mime| {
                    entry.mime_type().is_some_and(|types| types.contains(&mime))
                });
            if entry.no_display() && !recommended {
                continue;
            }
            if entry
                .try_exec()
                .is_some_and(|exec| !executable(Path::new(exec)))
            {
                continue;
            }
            let Some(exec) = entry.exec() else {
                // D-Bus-only launchers are valid when registered for this type.
                if entry.dbus_activatable()
                    && recommended
                    && let Some(app) = application(&entry, id, locales, recommended, is_default)
                {
                    apps.push(app);
                }
                continue;
            };
            let Ok(arguments) = entry.parse_exec() else {
                continue;
            };
            if !entry.dbus_activatable()
                && !arguments
                    .first()
                    .is_some_and(|exec| executable(Path::new(exec)))
            {
                continue;
            }
            if !recommended && !file_argument(exec) {
                continue;
            }
            if let Some(app) = application(&entry, id, locales, recommended, is_default) {
                apps.push(app);
            }
        }
    }
    apps.sort_by_cached_key(|app| {
        (
            !app.is_default,
            !app.recommended,
            app.name.to_lowercase(),
            app.id.clone(),
        )
    });
    apps
}

fn application(
    entry: &DesktopEntry,
    id: String,
    locales: &[&str],
    recommended: bool,
    is_default: bool,
) -> Option<Application> {
    Some(Application {
        id,
        path: entry.path.clone(),
        name: entry.name(locales)?.into_owned(),
        description: entry
            .comment(locales)
            .map(|s| s.into_owned())
            .unwrap_or_default(),
        recommended,
        is_default,
    })
}

fn executable(path: &Path) -> bool {
    fn check(path: &Path) -> bool {
        path.is_file() && rustix::fs::access(path, rustix::fs::Access::EXEC_OK).is_ok()
    }
    if path.is_absolute() {
        return check(path);
    }
    if path.components().count() != 1 {
        return false;
    }
    std::env::var_os("PATH").is_some_and(|paths| {
        std::env::split_paths(&paths)
            .filter(|dir| dir.is_absolute())
            .any(|dir| check(&dir.join(path)))
    })
}

fn file_argument(exec: &str) -> bool {
    let mut bytes = exec.bytes();
    while let Some(byte) = bytes.next() {
        if byte == b'%' && matches!(bytes.next(), Some(b'f' | b'F' | b'u' | b'U')) {
            return true;
        }
    }
    false
}

fn run(command: &mut Command) -> io::Result<(std::process::ExitStatus, String)> {
    // A pipe could remain open until the editor exits. An anonymous file captures
    // diagnostics without delaying the result when a launched app inherits stderr.
    let mut diagnostics = tempfile::tempfile()?;
    let status = command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::from(diagnostics.try_clone()?))
        .status()?;
    diagnostics.rewind()?;
    let mut bytes = Vec::new();
    diagnostics.take(8192).read_to_end(&mut bytes)?;
    Ok((status, String::from_utf8_lossy(&bytes).trim().to_owned()))
}

pub fn open_default(path: &Path) -> Result<(), String> {
    if crate::sandbox::active() {
        return crate::sandbox::open_file(path, false);
    }
    let mut command = Command::new("gio");
    command.arg("open").arg(path);
    let (tool, result) = match run(&mut command) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            ("xdg-open", run(Command::new("xdg-open").arg(path)))
        }
        result => ("gio", result),
    };
    let (status, diagnostics) = result.map_err(|error| {
        crate::i18n::message(
            "Não foi possível executar {}: {}",
            &[tool.into(), error.to_string()],
        )
    })?;
    if status.success() {
        return Ok(());
    }
    let message = crate::i18n::message(
        "Não foi possível abrir {} ({}: {}).",
        &[path.display().to_string(), tool.into(), status.to_string()],
    );
    Err(if diagnostics.is_empty() {
        message
    } else {
        format!("{message}\n{diagnostics}")
    })
}

pub fn valid_mime(mime: &str) -> bool {
    let Some((kind, subtype)) = mime.split_once('/') else {
        return false;
    };
    [kind, subtype].into_iter().all(|part| {
        part.as_bytes()
            .first()
            .is_some_and(u8::is_ascii_alphanumeric)
            && part
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"!#$&^_.+-".contains(&byte))
    })
}

pub fn set_default(app: &Application, mime: &str) -> Result<(), String> {
    if !valid_mime(mime) {
        return Err(crate::i18n::tr(
            "Não foi possível identificar um tipo MIME válido para salvar a associação.",
        )
        .into());
    }
    let (status, diagnostics) = run(Command::new("gio").arg("mime").arg(mime).arg(&app.id))
        .map_err(|error| {
            crate::i18n::message(
                "Não foi possível salvar a associação padrão para {}: {}",
                &[mime.into(), error.to_string()],
            )
        })?;
    if !status.success() {
        let message = crate::i18n::message(
            "Não foi possível definir {} como padrão para {} (gio: {}).",
            &[app.name.clone(), mime.into(), status.to_string()],
        );
        return Err(if diagnostics.is_empty() {
            message
        } else {
            format!("{message}\n{diagnostics}")
        });
    }
    Ok(())
}

pub fn launch(app: &Application, path: &Path) -> Result<(), String> {
    let (status, diagnostics) = run(Command::new("gio").arg("launch").arg(&app.path).arg(path))
        .map_err(|error| {
            crate::i18n::message(
                "Não foi possível executar gio. Instale as ferramentas GIO do sistema: {}",
                &[error.to_string()],
            )
        })?;
    if status.success() {
        Ok(())
    } else {
        let message = crate::i18n::message(
            "Não foi possível abrir {} com {} (gio: {}).",
            &[
                path.display().to_string(),
                app.name.clone(),
                status.to_string(),
            ],
        );
        Err(if diagnostics.is_empty() {
            message
        } else {
            format!("{message}\n{diagnostics}")
        })
    }
}

#[cfg(test)]
mod tests;
