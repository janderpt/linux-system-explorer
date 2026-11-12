use super::*;
#[test]
fn archive_actions_recognize_compressed_formats_and_select_installed_desktop_tools() {
    for name in [
        "data.zip",
        "data.TAR.GZ",
        "data.7z",
        "data.rar",
        "data.tar.xz",
        "data.tgz",
    ] {
        assert!(supported(Path::new(name)));
    }
    for name in [
        "report.pdf",
        "image.png",
        "document.docx",
        "folder",
        "unknown.bin",
    ] {
        assert!(!supported(Path::new(name)));
    }
    assert_eq!(choose("KDE", |_| true), Some(Backend::Ark));
    assert_eq!(choose("ubuntu:GNOME", |_| true), Some(Backend::FileRoller));
    assert_eq!(choose("MATE", |_| true), Some(Backend::Engrampa));
    assert_eq!(choose("XFCE", |p| p == "engrampa"), Some(Backend::Engrampa));
    assert_eq!(choose("XFCE", |p| p == "ark"), Some(Backend::Ark));
    assert_eq!(choose("KDE", |_| false), None);
}
#[test]
fn archive_commands_preserve_arguments_and_never_force_overwrites_or_use_a_shell() {
    use std::os::unix::ffi::OsStringExt;
    let paths = vec![
        PathBuf::from("/tmp/-$(touch PWNED) ação.zip"),
        PathBuf::from(std::ffi::OsString::from_vec(
            b"/tmp/raw-\xff.tar.gz".to_vec(),
        )),
    ];
    for backend in [Backend::Ark, Backend::FileRoller, Backend::Engrampa] {
        for mode in [Extraction::Here, Extraction::ChooseDestination] {
            let command = backend.command(mode, &paths);
            let arguments: Vec<_> = command.get_args().collect();
            assert_eq!(
                &arguments[arguments.len() - 2..],
                &[paths[0].as_os_str(), paths[1].as_os_str()]
            );
            assert_eq!(arguments[arguments.len() - 3], "--");
            assert!(
                !arguments
                    .iter()
                    .any(|a| *a == "--force" || *a == "--overwrite")
            );
            assert_eq!(command.get_current_dir(), Some(Path::new("/tmp")));
            assert_eq!(command.get_program(), backend.program());
            match (backend, mode) {
                (Backend::Ark, Extraction::Here) => assert_eq!(arguments[0], "--batch"),
                (Backend::Ark, Extraction::ChooseDestination) => {
                    assert_eq!(arguments[1], "--dialog")
                }
                (_, Extraction::Here) => assert_eq!(arguments[0], "--extract-here"),
                (_, Extraction::ChooseDestination) => assert_eq!(arguments[0], "--extract"),
            }
        }
    }
}
#[test]
fn native_archiver_invocation_errors_and_validation_are_isolated_from_user_files() {
    use std::{fs, os::unix::fs::PermissionsExt};
    let Some(root) = std::env::var_os("LSE_ARCHIVER_TEST") else {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir(root.path().join("bin")).unwrap();
        let program = root.path().join("bin/ark");
        fs::write(
            &program,
            "#!/bin/sh\nprintf '%s\\0' \"$@\" > \"$LSE_ARCHIVER_CAPTURE\"\n",
        )
        .unwrap();
        fs::set_permissions(&program, fs::Permissions::from_mode(0o755)).unwrap();
        let result = Command::new(std::env::current_exe().unwrap()).args(["--exact","applications::archives::tests::native_archiver_invocation_errors_and_validation_are_isolated_from_user_files","--nocapture"])
            .env("LSE_ARCHIVER_TEST",root.path()).env("LSE_ARCHIVER_CAPTURE",root.path().join("captured"))
            .env("XDG_CURRENT_DESKTOP","KDE").env("PATH",root.path().join("bin")).env_remove("FLATPAK_ID").output().unwrap();
        assert!(
            result.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
        return;
    };
    let root = PathBuf::from(root);
    let source = root.join("$(not-shell) ação.tar.gz");
    fs::write(&source, "original").unwrap();
    extract(std::slice::from_ref(&source), Extraction::Here).unwrap();
    let captured = fs::read(root.join("captured")).unwrap();
    assert!(captured.starts_with(b"--batch\0--autodestination\0--autosubfolder\0--\0"));
    assert!(captured.ends_with(&[source.as_os_str().as_encoded_bytes(), b"\0"].concat()));
    fs::remove_file(root.join("captured")).unwrap();
    assert!(extract(&[], Extraction::Here).is_err());
    assert!(extract(&[source.clone(), root.clone()], Extraction::Here).is_err());
    assert!(!root.join("captured").exists());
    fs::write(
        root.join("bin/ark"),
        "#!/bin/sh\necho archive-failure >&2\nexit 2\n",
    )
    .unwrap();
    assert!(
        extract(std::slice::from_ref(&source), Extraction::ChooseDestination)
            .unwrap_err()
            .contains("archive-failure")
    );
    assert_eq!(fs::read_to_string(source).unwrap(), "original");
}
