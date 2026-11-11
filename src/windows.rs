use std::{io, path::Path, process::Command};

/// Run directly with argument boundaries intact; never send a forced new window
/// through the system directory handler, which intentionally reuses windows.
pub fn open_new(path: &Path) -> io::Result<()> {
    run(&std::env::current_exe()?, path)
}

fn run(executable: &Path, path: &Path) -> io::Result<()> {
    let status = Command::new(executable)
        .args(["--new-window", "--"])
        .arg(path)
        .env_remove("XDG_ACTIVATION_TOKEN")
        .env_remove("DESKTOP_STARTUP_ID")
        .spawn()?
        .wait()?;
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::other(format!("{status}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::{
        ffi::{OsStrExt, OsStringExt},
        fs::PermissionsExt,
    };
    #[test]
    fn explicit_new_window_preserves_raw_folder_arguments_without_using_a_shell() {
        let root = tempfile::tempdir().unwrap();
        let executable = root.path().join("capture");
        std::fs::write(
            &executable,
            "#!/bin/sh\nprintf '%s\\0' \"$@\" > \"${0%/*}/arguments\"\n",
        )
        .unwrap();
        std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o755)).unwrap();
        let folder = root.path().join(std::ffi::OsString::from_vec(
            b"folder ' $literal;-\xff".to_vec(),
        ));
        run(&executable, &folder).unwrap();
        let captured = std::fs::read(root.path().join("arguments")).unwrap();
        let args: Vec<_> = captured
            .split(|b| *b == 0)
            .filter(|v| !v.is_empty())
            .collect();
        assert_eq!(
            args,
            vec![
                b"--new-window".as_slice(),
                b"--".as_slice(),
                folder.as_os_str().as_bytes()
            ]
        );
    }
}
