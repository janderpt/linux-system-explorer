use eframe::icon_data::IconDataExt;
use std::{
    fs, io,
    path::Path,
    process::{Command, Stdio},
};

pub const APP_ID: &str = "io.github.linux-system-explorer";

/// Wayland identifies applications by app_id and resolves icons from desktop entries.
pub fn register(accent: eframe::egui::Color32) -> io::Result<()> {
    let data =
        dirs::data_dir().ok_or_else(|| io::Error::other("XDG data directory unavailable"))?;
    let changed = register_at(&data, &std::env::current_exe()?, accent)?;
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

fn register_at(data: &Path, binary: &Path, accent: eframe::egui::Color32) -> io::Result<bool> {
    let icon = crate::system_style::application_icon(accent);
    let png = icon.to_png_bytes().map_err(io::Error::other)?;
    let icon_path = data.join("icons/hicolor/256x256/apps").join(format!(
        "{APP_ID}-{:02x}{:02x}{:02x}.png",
        accent.r(),
        accent.g(),
        accent.b()
    ));
    let utf8 = |path: &Path| {
        path.to_str()
            .map(str::to_owned)
            .ok_or_else(|| io::Error::other("Desktop entry paths must be UTF-8"))
    };
    let entry = format!(
        "[Desktop Entry]\nType=Application\nVersion=1.0\nName=File Explorer\nName[pt]=Explorador de Arquivos\nName[pt_BR]=Explorador de Arquivos\nExec={} %f\nIcon={}\nTerminal=false\nStartupNotify=true\nStartupWMClass={APP_ID}\nCategories=System;FileTools;FileManager;\nMimeType=inode/directory;\n",
        executable(&utf8(binary)?),
        entry_value(&utf8(&icon_path)?)
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
    use eframe::egui::Color32;

    #[test]
    fn registers_matching_wayland_identity_and_accent_icon_atomically() {
        let root = tempfile::tempdir().unwrap();
        let binary = root.path().join("a space/explorer%bin");
        assert!(register_at(root.path(), &binary, Color32::from_rgb(61, 174, 233)).unwrap());
        assert!(!register_at(root.path(), &binary, Color32::from_rgb(61, 174, 233)).unwrap());
        let path = root
            .path()
            .join("applications")
            .join(format!("{APP_ID}.desktop"));
        let entry = fs::read_to_string(&path).unwrap();
        assert!(entry.contains(&format!("StartupWMClass={APP_ID}")));
        assert!(entry.contains("explorer%%bin\" %f"));
        let icon_path = entry
            .lines()
            .find_map(|line| line.strip_prefix("Icon="))
            .unwrap();
        let icon = eframe::icon_data::from_png_bytes(&fs::read(icon_path).unwrap()).unwrap();
        assert_eq!(icon.width, 256);
        assert!(icon.rgba.as_chunks::<4>().0.contains(&[61, 174, 233, 255]));
        if let Ok(status) = Command::new("desktop-file-validate").arg(&path).status() {
            assert!(status.success());
        }
        assert!(register_at(root.path(), &binary, Color32::from_rgb(200, 20, 40)).unwrap());
        assert!(fs::read_to_string(path).unwrap().contains("-c81428.png"));
    }
}
