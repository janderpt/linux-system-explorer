use super::*;
use std::{
    fs,
    os::unix::{ffi::OsStringExt, fs::PermissionsExt},
    time::Instant,
};

fn uri(path: &Path) -> String {
    Url::from_file_path(path).unwrap().to_string()
}

#[test]
fn groups_items_by_parent_and_decodes_special_file_names_without_a_shell() {
    let a = PathBuf::from("/tmp/one space/á#quote'$.pdf");
    let b = PathBuf::from("/tmp/one space/other.docx");
    let c = PathBuf::from("/tmp/two/-file");
    let requests = plan(Operation::Items, &[uri(&a), uri(&b), uri(&c), uri(&a)]).unwrap();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0].folder, a.parent().unwrap());
    assert_eq!(requests[0].selected, vec![a.clone(), b.clone()]);
    assert_eq!(
        requests[0].arguments(),
        vec![OsString::from("--select"), a.into(), b.into()]
    );
    assert_eq!(requests[1].selected, vec![c]);
    let raw = PathBuf::from(OsString::from_vec(b"/tmp/bytes/\xff-file".to_vec()));
    assert_eq!(
        plan(Operation::Items, &[uri(&raw)]).unwrap()[0].selected,
        vec![raw]
    );
    assert!(plan(Operation::Items, &[]).unwrap().is_empty());
    assert_eq!(
        plan(Operation::Folders, &["file://localhost/tmp".into()]).unwrap()[0].folder,
        Path::new("/tmp")
    );
}

#[test]
fn rejects_remote_malformed_or_excessive_requests_before_launching_anything() {
    for invalid in [
        "https://example.org/file",
        "smb://server/share/a",
        "file://remote/tmp/a",
        "file:///tmp/a?query",
        "file:///tmp/a#fragment",
        "file:///tmp/a%00b",
        "not a URI",
    ] {
        assert!(
            plan(
                Operation::Items,
                &["file:///tmp/good".into(), invalid.into()]
            )
            .is_err(),
            "{invalid}"
        );
    }
    assert!(plan(Operation::Items, &vec!["file:///tmp/a".into(); 1025]).is_err());
    let folders: Vec<_> = (0..33)
        .map(|index| format!("file:///tmp/folder-{index}"))
        .collect();
    assert!(plan(Operation::Folders, &folders).is_err());
    let properties = plan(
        Operation::Properties,
        &["file:///tmp/a".into(), "file:///tmp/b".into()],
    )
    .unwrap();
    assert_eq!(properties.len(), 2);
    assert!(properties.iter().all(|r| r.properties));
}

#[test]
fn dispatches_real_dbus_calls_in_a_private_session() {
    let Some(root) = std::env::var_os("LSE_PRIVATE_BUS_TEST") else {
        let root = tempfile::tempdir().unwrap();
        let result = Command::new("dbus-run-session")
            .arg("--")
            .arg(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "file_manager::tests::dispatches_real_dbus_calls_in_a_private_session",
                "--nocapture",
            ])
            .env("LSE_PRIVATE_BUS_TEST", root.path())
            .output()
            .expect("dbus-run-session is required for this integration test");
        assert!(
            result.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
        return;
    };
    let root = PathBuf::from(root);
    let log = root.join("requests");
    fs::create_dir(&log).unwrap();
    let launcher = root.join("fake-explorer");
    fs::write(&launcher, format!(r#"#!/usr/bin/env python3
import os,sys,json,tempfile
with tempfile.NamedTemporaryFile(mode='w',dir={:?},delete=False) as f:
 json.dump({{'args':[os.fsencode(a).hex() for a in sys.argv[1:]],'startup':os.environ.get('DESKTOP_STARTUP_ID'),'activation':os.environ.get('XDG_ACTIVATION_TOKEN')}},f)
"#, log.to_str().unwrap())).unwrap();
    fs::set_permissions(&launcher, fs::Permissions::from_mode(0o755)).unwrap();
    let (_service, stopped) = start(&launcher).unwrap();
    assert!(
        start(&launcher).is_err(),
        "never silently queue behind or steal another file manager's name"
    );
    let client = zbus::block_on(zbus::Connection::session()).unwrap();
    let call = |method: &str, paths: Vec<String>, startup: &str| {
        zbus::block_on(client.call_method(
            Some(BUS_NAME),
            OBJECT_PATH,
            Some(BUS_NAME),
            method,
            &(paths, startup.to_owned()),
        ))
    };
    call(
        "ShowItems",
        vec![
            "file:///tmp/a%20folder/report%23.pdf".into(),
            "file:///tmp/a%20folder/other.docx".into(),
        ],
        "startup-token",
    )
    .unwrap();
    call("ShowFolders", vec!["file:///tmp/folder".into()], "").unwrap();
    call("ShowItemProperties", vec!["file:///tmp/item".into()], "").unwrap();
    assert!(
        call(
            "ShowItems",
            vec!["https://example.org/not-local".into()],
            ""
        )
        .is_err()
    );
    let deadline = Instant::now() + Duration::from_secs(5);
    let records = loop {
        let records: Vec<serde_json::Value> = fs::read_dir(&log)
            .unwrap()
            .filter_map(|entry| {
                let bytes = fs::read(entry.ok()?.path()).ok()?;
                serde_json::from_slice(&bytes).ok()
            })
            .collect();
        if records.len() == 3 {
            break records;
        }
        assert!(
            Instant::now() < deadline,
            "launcher did not receive all D-Bus requests"
        );
        thread::sleep(Duration::from_millis(10));
    };
    let hex = |text: &str| {
        text.as_bytes()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    };
    let selected = records
        .iter()
        .find(|r| r["args"][0] == hex("--select"))
        .unwrap();
    assert_eq!(
        selected["args"],
        serde_json::json!([
            hex("--select"),
            hex("/tmp/a folder/report#.pdf"),
            hex("/tmp/a folder/other.docx")
        ])
    );
    assert_eq!(selected["startup"], "startup-token");
    assert_eq!(selected["activation"], "startup-token");
    let folder = records.iter().find(|r| r["args"][0] == hex("--")).unwrap();
    assert_eq!(
        folder["args"],
        serde_json::json!([hex("--"), hex("/tmp/folder")])
    );
    assert!(folder["startup"].is_null());
    // A real per-window endpoint must suppress process creation and receive the
    // original token and complete selection, including non-UTF-8 names.
    let window = window::Window::with_name(
        &eframe::egui::Context::default(),
        "io.github.janderpt.LinuxSystemExplorer.Window1.p1",
    )
    .unwrap();
    let current = std::sync::Arc::new(std::sync::Mutex::new(PathBuf::from("/tmp/a folder")));
    let (accepted_tx, accepted_rx) = mpsc::channel();
    let (stop_tx, stop_rx) = mpsc::channel();
    let worker_current = current.clone();
    let worker = thread::spawn(move || {
        while stop_rx.try_recv().is_err() {
            for queued in window.pending() {
                if let Some(request) = queued.accept(&worker_current.lock().unwrap()) {
                    accepted_tx.send(request).unwrap();
                }
            }
            thread::sleep(Duration::from_millis(1));
        }
    });
    let raw = PathBuf::from(OsString::from_vec(b"/tmp/a folder/raw-\xff.pdf".to_vec()));
    call(
        "ShowItems",
        vec!["file:///tmp/a%20folder/report%23.pdf".into(), uri(&raw)],
        "reuse-token",
    )
    .unwrap();
    let (reused, token) = accepted_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    assert_eq!(
        reused.selected,
        vec![PathBuf::from("/tmp/a folder/report#.pdf"), raw]
    );
    assert_eq!(token, "reuse-token");
    call(
        "ShowFolders",
        vec!["file:///tmp/a%20folder".into()],
        "folder-token",
    )
    .unwrap();
    assert!(
        accepted_rx
            .recv_timeout(Duration::from_secs(2))
            .unwrap()
            .0
            .selected
            .is_empty()
    );
    assert_eq!(
        fs::read_dir(&log).unwrap().count(),
        3,
        "reused windows must not launch a new GUI"
    );
    // Navigating the window invalidates its previous folder immediately.
    *current.lock().unwrap() = PathBuf::from("/tmp/elsewhere");
    let request = OpenRequest {
        folder: "/tmp/a folder".into(),
        selected: vec![],
        properties: false,
    };
    assert!(!zbus::block_on(window::reuse(&client, &request, "")));
    stop_tx.send(()).unwrap();
    worker.join().unwrap();
    assert!(
        !zbus::block_on(window::reuse(&client, &request, "")),
        "closed windows must disappear from discovery"
    );
    zbus::block_on(client.call_method(Some(BUS_NAME), OBJECT_PATH, Some(CONTROL), "Quit", &()))
        .unwrap();
    stopped.recv_timeout(Duration::from_secs(2)).unwrap();
}
