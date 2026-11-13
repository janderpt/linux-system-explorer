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
                .unwrap()
                .name("org.kde.KIOFuse")
                .unwrap()
                .name("org.gnome.Nautilus")
                .unwrap()
                .serve_at("/org/kde/KIOFuse", fake)
                .unwrap()
                .serve_at("/org/freedesktop/FileManager1", nautilus)
                .unwrap()
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
fn ftp_uses_native_gvfs_authentication_on_kde_without_changing_other_protocols() {
    for kde in [false, true] {
        for kio in [false, true] {
            assert_eq!(
                choose_mount_backend("ftp://user@server/", kde, kio, true),
                Some(Backend::Gvfs)
            );
        }
    }
    assert_eq!(
        choose_mount_backend("ftp://server/", true, true, false),
        Some(Backend::Kio)
    );
    assert_eq!(
        choose_mount_backend("ftp://server/", false, false, false),
        None
    );
    for uri in ["smb://server/share", "sftp://server/", "dav://server/"] {
        assert_eq!(
            choose_mount_backend(uri, true, true, true),
            Some(Backend::Kio)
        );
        assert_eq!(
            choose_mount_backend(uri, false, true, true),
            Some(Backend::Gvfs)
        );
    }
}

#[test]
fn ftp_authentication_errors_and_cancellation_do_not_trigger_a_second_dialog() {
    let uri = remote_url("FTP://user@server/folder%20name").unwrap();
    let backend = choose_mount_backend(&uri, true, true, true).unwrap();
    for kind in [io::ErrorKind::PermissionDenied, io::ErrorKind::Interrupted] {
        let mut calls = Vec::new();
        let result =
            connect_with_fallback(&uri, backend, true, &Cancellation::default(), |backend| {
                calls.push(backend);
                Err(io::Error::from(kind))
            });
        assert_eq!(result.unwrap_err().kind(), kind);
        assert_eq!(calls, [Backend::Gvfs]);
    }
}

#[test]
fn ftp_helper_receives_the_username_and_path_without_shell_interpretation() {
    use std::os::unix::fs::PermissionsExt;
    let root = tempfile::tempdir().unwrap();
    let helper = root.path().join("ftp-helper");
    let expected = serde_json::to_string(&crate::settings::folder_key(root.path())).unwrap();
    std::fs::write(
        &helper,
        format!("#!/bin/sh\nprintf '%s\\n' \"$@\" > \"$0.args\"\ncat <<'END_MOUNT_PATH'\n{expected}\nEND_MOUNT_PATH\n"),
    )
    .unwrap();
    std::fs::set_permissions(&helper, std::fs::Permissions::from_mode(0o700)).unwrap();
    let uri = remote_url("ftp://user%40domain@server/folder%20%24%28ignored%29/").unwrap();
    let mount =
        run_written_helper(|| connect_gvfs_with(&helper, &uri, &Cancellation::default())).unwrap();
    assert_eq!(mount, root.path());
    assert_eq!(
        std::fs::read_to_string(helper.with_extension("args")).unwrap(),
        format!("--mount-network\n{uri}\n")
    );
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

#[test]
fn unreadable_kio_mount_retries_with_temporary_authenticated_mount_before_saving() {
    let root = tempfile::tempdir().unwrap();
    let mut calls = Vec::new();
    let result = connect_with_fallback(
        "smb://server/share",
        Backend::Kio,
        true,
        &Cancellation::default(),
        |backend| {
            calls.push(backend);
            if backend == Backend::Kio {
                Ok(root.path().join("unreadable-mount"))
            } else {
                Ok(root.path().into())
            }
        },
    )
    .unwrap();
    assert_eq!(result, root.path());
    assert_eq!(calls, vec![Backend::Kio, Backend::Gvfs]);
    assert!(std::fs::read_dir(result).unwrap().next().is_none());
}
#[test]
fn a_readable_primary_mount_and_cancellation_do_not_start_another_authentication() {
    let root = tempfile::tempdir().unwrap();
    let mut calls = Vec::new();
    connect_with_fallback(
        "smb://server/share",
        Backend::Kio,
        true,
        &Cancellation::default(),
        |backend| {
            calls.push(backend);
            Ok(root.path().into())
        },
    )
    .unwrap();
    assert_eq!(calls, vec![Backend::Kio]);
    calls.clear();
    let cancel = Cancellation::default();
    let result = connect_with_fallback(
        "smb://server/share",
        Backend::Kio,
        true,
        &cancel,
        |backend| {
            calls.push(backend);
            cancel.cancel();
            Err(io::Error::from(io::ErrorKind::PermissionDenied))
        },
    );
    assert_eq!(result.unwrap_err().kind(), io::ErrorKind::Interrupted);
    assert_eq!(calls, vec![Backend::Kio]);
}
#[test]
fn fallback_requires_an_smb_target_and_an_available_service_and_reports_both_failures() {
    for (uri, available, expected) in [
        ("sftp://server/share", true, 1),
        ("smb://server/share", false, 1),
        ("smb://server/share", true, 2),
    ] {
        let mut calls = Vec::new();
        let error = connect_with_fallback(
            uri,
            Backend::Kio,
            available,
            &Cancellation::default(),
            |backend| {
                calls.push(backend);
                Err(io::Error::other(if backend == Backend::Kio {
                    "KIO denied"
                } else {
                    "Temporary mount denied"
                }))
            },
        )
        .unwrap_err();
        assert_eq!(calls.len(), expected);
        assert!(error.to_string().contains("KIO denied"));
        if expected == 2 {
            assert!(error.to_string().contains("Temporary mount denied"));
        }
    }
}

#[test]
fn server_browsing_retries_after_kio_denial_and_preserves_user_in_selected_share_urls() {
    let mut calls = Vec::new();
    let shares = browse_with_fallback(
        "smb://Administrador@server/",
        Backend::Kio,
        true,
        &Cancellation::default(),
        |backend| {
            calls.push(backend);
            match backend {
                Backend::Kio => Err(io::Error::from(io::ErrorKind::PermissionDenied)),
                Backend::Gvfs => Ok(vec!["D".into(), "Shared folder #1".into()]),
            }
        },
    )
    .unwrap();
    assert_eq!(calls, vec![Backend::Kio, Backend::Gvfs]);
    assert_eq!(shares[0].url, "smb://Administrador@server/D");
    assert_eq!(
        shares[1].url,
        "smb://Administrador@server/Shared%20folder%20%231"
    );
}

#[test]
fn server_browsing_respects_cancellation_service_availability_and_successful_empty_lists() {
    for (available, cancel_after_first, success, expected_calls) in [
        (false, false, false, 1),
        (true, true, false, 1),
        (true, false, true, 1),
        (true, false, false, 2),
    ] {
        let cancel = Cancellation::default();
        let mut calls = Vec::new();
        let result = browse_with_fallback(
            "smb://Administrador@server/",
            Backend::Kio,
            available,
            &cancel,
            |backend| {
                calls.push(backend);
                if cancel_after_first {
                    cancel.cancel();
                }
                if success {
                    Ok(Vec::new())
                } else {
                    Err(io::Error::other(match backend {
                        Backend::Kio => "KIO listing denied",
                        Backend::Gvfs => "GVfs listing denied",
                    }))
                }
            },
        );
        assert_eq!(calls.len(), expected_calls);
        if success {
            assert!(result.unwrap().is_empty());
        } else if cancel_after_first {
            assert_eq!(result.unwrap_err().kind(), io::ErrorKind::Interrupted);
        } else {
            let error = result.unwrap_err().to_string();
            assert!(error.contains("KIO listing denied"));
            assert_eq!(error.contains("GVfs listing denied"), available);
        }
    }
}
