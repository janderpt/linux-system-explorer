use eframe::icon_data::IconDataExt;
use std::{
    fs, io,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

pub const APP_ID: &str = env!("LSE_APP_ID");
const MANAGED: &str = "X-LinuxSystemExplorer-Managed=true";

/// Wayland identifies applications by app_id and resolves icons from desktop entries.
pub fn register() -> io::Result<()> {
    let data =
        dirs::data_dir().ok_or_else(|| io::Error::other("XDG data directory unavailable"))?;
    let mut data_dirs: Vec<PathBuf> = std::env::var_os("XDG_DATA_DIRS")
        .map(|paths| std::env::split_paths(&paths).collect())
        .unwrap_or_else(|| vec!["/usr/local/share".into(), "/usr/share".into()]);
    data_dirs.push(data.clone());
    if std::env::var("FLATPAK_ID").is_ok_and(|id| id == APP_ID)
        || has_installed_launcher(&data_dirs)
    {
        // The installer owns its launcher and icon. Remove only our own development override.
        let generated = data.join("applications").join(format!("{APP_ID}.desktop"));
        if fs::read_to_string(&generated)
            .is_ok_and(|entry| entry.lines().any(|line| line == MANAGED))
        {
            fs::remove_file(generated)?;
        }
        return Ok(());
    }
    let changed = register_at(&data, &std::env::current_exe()?)?;
    if changed
        && std::env::var("XDG_CURRENT_DESKTOP").is_ok_and(|desktop| {
            desktop
                .split(':')
                .any(|name| name.eq_ignore_ascii_case("KDE"))
        })
    {
        // Make the identity available before KWin creates the first native Wayland window.
        let _ = Command::new("kbuildsycoca6")
            .arg("--noincremental")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
    Ok(())
}

fn has_installed_launcher(data_dirs: &[PathBuf]) -> bool {
    data_dirs.iter().any(|data| {
        fs::read_to_string(data.join("applications").join(format!("{APP_ID}.desktop")))
            .is_ok_and(|entry| !entry.lines().any(|line| line == MANAGED))
    })
}

fn write_changed(path: &Path, bytes: &[u8]) -> io::Result<bool> {
    if fs::read(path).is_ok_and(|current| current == bytes) {
        return Ok(false);
    }
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("Invalid desktop integration path"))?;
    fs::create_dir_all(parent)?;
    use std::io::Write;
    let mut file = tempfile::NamedTempFile::new_in(parent)?;
    file.write_all(bytes)?;
    file.persist(path).map_err(|error| error.error)?;
    Ok(true)
}

fn entry_value(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
}

fn executable(value: &str) -> String {
    // Exec has its own quoted-argument escaping in addition to desktop string escaping.
    let mut quoted = String::from("\"");
    for ch in value.chars() {
        if "\\\"`$".contains(ch) {
            quoted.push('\\');
        }
        quoted.push(ch);
        if ch == '%' {
            quoted.push('%');
        }
    }
    quoted.push('"');
    entry_value(&quoted)
}

fn register_at(data: &Path, binary: &Path) -> io::Result<bool> {
    let icon = crate::system_style::application_icon();
    let png = icon.to_png_bytes().map_err(io::Error::other)?;
    let icon_path = data
        .join("icons/hicolor/256x256/apps")
        .join(format!("{APP_ID}.png"));
    let utf8 = |path: &Path| {
        path.to_str()
            .map(str::to_owned)
            .ok_or_else(|| io::Error::other("Desktop entry paths must be UTF-8"))
    };
    let entry = format!(
        "[Desktop Entry]\nType=Application\nVersion=1.0\nName=File Explorer\nName[pt]=Explorador de Arquivos\nName[pt_BR]=Explorador de Arquivos\nExec={} %f\nIcon={APP_ID}\nTerminal=false\nStartupNotify=true\nStartupWMClass={APP_ID}\nCategories=System;FileTools;FileManager;\nMimeType=inode/directory;\n{MANAGED}\n",
        executable(&utf8(binary)?)
    );
    let icon_changed = write_changed(&icon_path, &png)?;
    let entry_changed = write_changed(
        &data.join("applications").join(format!("{APP_ID}.desktop")),
        entry.as_bytes(),
    )?;
    Ok(icon_changed || entry_changed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distinguishes_installed_launchers_from_development_overrides() {
        let root = tempfile::tempdir().unwrap();
        let data = root.path().to_owned();
        register_at(&data, Path::new("/tmp/explorer")).unwrap();
        assert!(!has_installed_launcher(std::slice::from_ref(&data)));
        fs::write(
            data.join("applications").join(format!("{APP_ID}.desktop")),
            "[Desktop Entry]\nType=Application\nName=Explorer\nExec=linux-system-explorer\n",
        )
        .unwrap();
        assert!(has_installed_launcher(&[data]));
    }

    #[test]
    fn registers_matching_identity_and_the_canonical_window_icon_atomically() {
        let root = tempfile::tempdir().unwrap();
        let binary = root.path().join("a space/explorer%bin");
        assert!(register_at(root.path(), &binary).unwrap());
        assert!(!register_at(root.path(), &binary).unwrap());
        let path = root
            .path()
            .join("applications")
            .join(format!("{APP_ID}.desktop"));
        let entry = fs::read_to_string(&path).unwrap();
        assert!(entry.contains(&format!("StartupWMClass={APP_ID}")));
        assert!(entry.contains("explorer%%bin\" %f"));
        assert!(entry.contains(&format!("Icon={APP_ID}\n")));
        let icon_path = root
            .path()
            .join("icons/hicolor/256x256/apps")
            .join(format!("{APP_ID}.png"));
        let icon = eframe::icon_data::from_png_bytes(&fs::read(icon_path).unwrap()).unwrap();
        assert_eq!(icon.width, 256);
        assert_eq!(icon.rgba, crate::system_style::application_icon().rgba);
        if let Ok(status) = Command::new("desktop-file-validate").arg(&path).status() {
            assert!(status.success());
        }
    }
}
