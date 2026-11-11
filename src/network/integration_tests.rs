use super::*;
use std::sync::{Arc, Mutex, mpsc};

fn run_written_helper<T>(mut run: impl FnMut() -> io::Result<T>) -> io::Result<T> {
    // Parallel process tests can inherit a newly written script's write FD
    // briefly before exec closes it. Retry only that transient kernel error.
    let until = std::time::Instant::now() + Duration::from_millis(250);
    loop {
        match run() {
            Err(error)
                if error.kind() == io::ErrorKind::ExecutableFileBusy
                    && std::time::Instant::now() < until =>
            {
                std::thread::sleep(Duration::from_millis(2))
            }
            result => return result,
        }
    }
}

struct FakeKio {
    path: String,
    requests: Arc<Mutex<Vec<String>>>,
}

struct FakeNautilus {
    requests: Arc<Mutex<Vec<String>>>,
}
#[zbus::interface(name = "org.freedesktop.FileManager1")]
impl FakeNautilus {
    fn show_item_properties(&self, uris: Vec<String>, _startup: String) {
        self.requests.lock().unwrap().extend(uris);
    }
}
#[zbus::interface(name = "org.kde.KIOFuse.VFS")]
impl FakeKio {
    #[zbus(name = "mountUrl")]
    fn mount_url(&self, url: String) -> zbus::fdo::Result<String> {
        self.requests.lock().unwrap().push(url.clone());
        if url.contains("failure") {
            return Err(zbus::fdo::Error::Failed(
                "Connection rejected by test server".into(),
            ));
        }
        if url.contains("missing") {
            return Ok(format!("{}/missing", self.path));
        }
        Ok(self.path.clone())
    }
}

#[test]
fn kio_connection_uses_the_desktop_api_and_validates_results_in_an_isolated_bus() {
    if std::env::var_os("LSE_NETWORK_PRIVATE_BUS").is_none() {
        let status=Command::new("dbus-run-session").arg("--").arg(std::env::current_exe().unwrap()).args(["--exact","network::integration_tests::kio_connection_uses_the_desktop_api_and_validates_results_in_an_isolated_bus","--nocapture"]).env("LSE_NETWORK_PRIVATE_BUS","1").env("XDG_CURRENT_DESKTOP","KDE").status().unwrap();
        assert!(status.success());
        return;
    }
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir(root.path().join("Docs")).unwrap();
    std::fs::create_dir(root.path().join("Shared folder")).unwrap();
    std::fs::write(root.path().join("not-a-share.txt"), "test").unwrap();
    let requests = Arc::new(Mutex::new(vec![]));
    let properties = Arc::new(Mutex::new(vec![]));
    let nautilus = FakeNautilus {
        requests: properties.clone(),
    };
    let fake = FakeKio {
        path: root.path().to_string_lossy().into_owned(),
        requests: requests.clone(),
    };
    let (ready_sender, ready) = mpsc::channel();
    let (stop, stopped) = mpsc::channel();
    let worker = std::thread::spawn(move || {
        let connection = zbus::block_on(
            zbus::connection::Builder::session()
                .name("org.kde.KIOFuse")
                .name("org.gnome.Nautilus")
                .serve_at("/org/kde/KIOFuse", fake)
                .serve_at("/org/freedesktop/FileManager1", nautilus)
                .build(),
        )
        .unwrap();
        ready_sender.send(()).unwrap();
        let _ = stopped.recv();
        drop(connection);
    });
    ready.recv_timeout(Duration::from_secs(5)).unwrap();
    let shares = browse_server(r"\\server\", &Cancellation::default()).unwrap();
    assert_eq!(
        shares.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(),
        vec!["Docs", "Shared folder"]
    );
    assert_eq!(shares[1].url, "smb://server/Shared%20folder");
    assert_eq!(connect(&shares[0].url).unwrap(), root.path());
    assert_eq!(connect("smb://server/share").unwrap(), root.path());
    assert!(
        connect("smb://server/failure")
            .unwrap_err()
            .to_string()
            .contains("Connection rejected")
    );
    assert!(connect("smb://server/missing").is_err());
    let folder = sharing_uri(root.path()).unwrap();
    configure_nautilus(&folder).unwrap();
    assert_eq!(*properties.lock().unwrap(), vec![folder.to_string()]);
    assert_eq!(
        *requests.lock().unwrap(),
        vec![
            "smb://server/",
            "smb://server/Docs",
            "smb://server/share",
            "smb://server/failure",
            "smb://server/missing"
        ]
    );
    stop.send(()).unwrap();
    worker.join().unwrap();
}

#[test]
fn native_sharing_launcher_passes_one_literal_folder_uri_to_the_system_tool() {
    use std::os::unix::fs::PermissionsExt;
    let root = tempfile::tempdir().unwrap();
    let folder = root.path().join("folder # ' $(ignored)");
    std::fs::create_dir(&folder).unwrap();
    let helper = root.path().join("kioclient-test");
    std::fs::write(&helper, "#!/bin/sh\nprintf '%s\\n' \"$@\" > \"$0.args\"\n").unwrap();
    std::fs::set_permissions(&helper, std::fs::Permissions::from_mode(0o700)).unwrap();
    let uri = sharing_uri(&folder).unwrap();
    run_written_helper(|| run_sharing_program(&helper, &uri)).unwrap();
    let args = std::fs::read_to_string(helper.with_extension("args")).unwrap();
    assert_eq!(args, format!("openProperties\n{uri}\n"));
}

#[test]
fn gvfs_helper_has_a_separate_process_and_preserves_non_utf8_mount_paths() {
    use std::os::unix::{ffi::OsStringExt, fs::PermissionsExt};
    let root = tempfile::tempdir().unwrap();
    let mount = root
        .path()
        .join(std::ffi::OsString::from_vec(b"share-\xff".to_vec()));
    std::fs::create_dir(&mount).unwrap();
    let expected = serde_json::to_string(&crate::settings::folder_key(&mount)).unwrap();
    let helper = root.path().join("gio-mount-test");
    // The helper prints a fixed JSON response; URI input remains a literal argument.
    std::fs::write(&helper,format!("#!/bin/sh\nprintf '%s\\n' \"$@\" > \"$0.args\"\ncat <<'END_MOUNT_PATH'\n{expected}\nEND_MOUNT_PATH\n")).unwrap();
    std::fs::set_permissions(&helper, std::fs::Permissions::from_mode(0o700)).unwrap();
    assert_eq!(
        run_written_helper(|| connect_gvfs_with(
            &helper,
            "smb://server/share",
            &Cancellation::default()
        ))
        .unwrap(),
        mount
    );
    assert_eq!(
        std::fs::read_to_string(helper.with_extension("args")).unwrap(),
        "--mount-network\nsmb://server/share\n"
    );
    assert_eq!(choose_backend(false, true, true), Some(Backend::Gvfs));
    assert_eq!(choose_backend(true, true, true), Some(Backend::Kio));
    assert_eq!(choose_backend(false, true, false), Some(Backend::Kio));
    assert_eq!(choose_backend(true, false, true), Some(Backend::Gvfs));
    assert_eq!(choose_backend(false, false, false), None);
}

#[test]
fn cancelling_a_gvfs_connection_stops_and_reaps_its_helper() {
    use std::os::unix::fs::PermissionsExt;
    let root = tempfile::tempdir().unwrap();
    let helper = root.path().join("waiting-mount");
    std::fs::write(&helper, "#!/bin/sh\nexec sleep 10\n").unwrap();
    std::fs::set_permissions(&helper, std::fs::Permissions::from_mode(0o700)).unwrap();
    let cancel = Arc::new(Cancellation::default());
    let worker_cancel = cancel.clone();
    let started = std::time::Instant::now();
    let worker = std::thread::spawn(move || {
        run_written_helper(|| connect_gvfs_with(&helper, "smb://server/share", &worker_cancel))
    });
    std::thread::sleep(Duration::from_millis(50));
    cancel.cancel();
    assert_eq!(
        worker.join().unwrap().unwrap_err().kind(),
        io::ErrorKind::Interrupted
    );
    assert!(started.elapsed() < Duration::from_secs(2));
}

#[test]
fn browse_helper_drains_large_output_and_keeps_the_uri_as_a_literal_argument() {
    use std::os::unix::fs::PermissionsExt;
    let root = tempfile::tempdir().unwrap();
    let helper = root.path().join("browse-helper");
    std::fs::write(&helper,"#!/usr/bin/python3\nimport json,sys\nfrom pathlib import Path\nPath(sys.argv[0]+'.args').write_text('\\n'.join(sys.argv[1:]))\nsys.stderr.write('x'*200000)\nprint(json.dumps(['Share-'+str(i)+'x'*100 for i in range(1000)]))\n").unwrap();
    std::fs::set_permissions(&helper, std::fs::Permissions::from_mode(0o700)).unwrap();
    let output = run_written_helper(|| {
        run_gvfs_helper(
            &helper,
            "--browse-network",
            "smb://server/",
            &Cancellation::default(),
        )
    })
    .unwrap();
    let names: Vec<String> = serde_json::from_slice(&output).unwrap();
    assert_eq!(names.len(), 1000);
    assert_eq!(
        std::fs::read_to_string(helper.with_extension("args")).unwrap(),
        "--browse-network\nsmb://server/"
    );
}

#[test]
#[ignore = "Explicit read-only SMB integration; requires LSE_TEST_SMB_SERVER"]
fn live_smb_server_lists_names_without_opening_share_contents() {
    let server = std::env::var("LSE_TEST_SMB_SERVER")
        .expect("Set LSE_TEST_SMB_SERVER to an authorized SMB server root");
    let normalized = remote_url(&server).unwrap();
    assert!(is_smb_server(&normalized));
    let shares = browse_server(&server, &Cancellation::default()).unwrap();
    let host = Url::parse(&normalized).unwrap();
    for share in &shares {
        let target = Url::parse(&share.url).unwrap();
        assert_eq!(target.host_str(), host.host_str());
        assert!(target.password().is_none());
        println!("{}", share.name);
    }
    assert!(
        !shares.is_empty(),
        "Expected shares from the explicitly selected test server"
    );
}
