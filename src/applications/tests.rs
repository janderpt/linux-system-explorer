use super::*;
use std::{
    fs,
    os::unix::ffi::{OsStrExt, OsStringExt},
    time::{Duration, Instant},
};

fn desktop(root: &Path, id: &str, values: &str) {
    let path = root.join(id);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(
        path,
        format!("[Desktop Entry]\nType=Application\nName={id}\nExec=/bin/sh %f\n{values}\n"),
    )
    .unwrap();
}

#[test]
fn discovery_honors_overrides_availability_localization_and_mime_ranking() {
    let root = tempfile::tempdir().unwrap();
    let user = root.path().join("user/applications");
    let system = root.path().join("system/applications");
    desktop(&system, "masked.desktop", "MimeType=text/plain;");
    desktop(&user, "masked.desktop", "Hidden=true");
    desktop(&system, "nested/app.desktop", "MimeType=text/plain;");
    desktop(&user, "nested-app.desktop", "Hidden=true");
    desktop(
        &system,
        "editor.desktop",
        "Name=English Editor\nName[pt_BR]=Editor português\nMimeType=text/plain;",
    );
    desktop(
        &user,
        "editor.desktop",
        "Name=User Editor\nName[pt_BR]=Editor do usuário\nMimeType=text/plain;",
    );
    desktop(&system, "default.desktop", "Name=Z editor\nNoDisplay=true");
    desktop(&system, "internal.desktop", "NoDisplay=true");
    desktop(
        &system,
        "missing.desktop",
        "TryExec=/nonexistent-explorer-test-command",
    );
    desktop(
        &system,
        "missing-exec.desktop",
        "Exec=/nonexistent-explorer-test-command %f",
    );
    desktop(&system, "helper.desktop", "Exec=/bin/sh -c true");
    desktop(&system, "link.desktop", "Type=Link");
    desktop(
        &system,
        "other.desktop",
        "Name=Image viewer\nMimeType=image/png;",
    );
    let linked_user = root.path().join("linked-applications");
    std::os::unix::fs::symlink(&user, &linked_user).unwrap();
    let apps = collect(
        [linked_user, system],
        &["pt_BR", "pt"],
        Some("text/plain"),
        Some("default.desktop"),
    );
    assert_eq!(
        apps.iter().map(|app| app.id.as_str()).collect::<Vec<_>>(),
        ["default.desktop", "editor.desktop", "other.desktop"]
    );
    assert!(apps[0].is_default && apps[0].recommended);
    assert_eq!(apps[1].name, "Editor do usuário");
    assert!(apps[1].path.ends_with("user/applications/editor.desktop"));
    assert!(apps[1].recommended);
    assert!(!apps[2].recommended);
    assert!(file_argument("tool %F"));
    assert!(file_argument("tool %U"));
    assert!(!file_argument("tool %%f"));
}

#[test]
fn gio_launch_passes_special_and_non_utf8_file_names_without_shell_interpolation() {
    if !executable(Path::new("gio")) {
        return;
    }
    let root = tempfile::tempdir().unwrap();
    let capture = root.path().join("capture");
    let script = root.path().join("capture.sh");
    // This fixture launcher only records argv. No real editor or association changes.
    fs::write(
        &script,
        format!(
            "#!/bin/sh\nprintf '%s\\0' \"$@\" >> '{}'\n",
            capture.display()
        ),
    )
    .unwrap();
    let desktop = root.path().join("capture.desktop");
    fs::write(
        &desktop,
        format!(
            "[Desktop Entry]\nType=Application\nName=Capture\nExec=/bin/sh \"{}\" %F\n",
            script.display()
        ),
    )
    .unwrap();
    let app = Application {
        id: "capture.desktop".into(),
        path: desktop,
        name: "Capture".into(),
        description: String::new(),
        recommended: true,
        is_default: false,
    };
    let special = root.path().join("space ' $literal; [test].txt");
    let raw = root
        .path()
        .join(std::ffi::OsString::from_vec(b"raw-\xff.txt".to_vec()));
    for path in [&special, &raw] {
        fs::write(path, "example").unwrap();
        launch(&app, path).unwrap();
    }
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let data = fs::read(&capture).unwrap_or_default();
        let args: Vec<_> = data.split(|b| *b == 0).filter(|p| !p.is_empty()).collect();
        if args.len() == 2 {
            assert!(args.contains(&special.as_os_str().as_bytes()));
            assert!(args.contains(&raw.as_os_str().as_bytes()));
            break;
        }
        assert!(
            Instant::now() < deadline,
            "launcher did not record both paths"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    let missing = Application {
        path: root.path().join("missing.desktop"),
        ..app
    };
    let error = launch(&missing, &special).unwrap_err();
    assert!(error.contains("gio"));
}
