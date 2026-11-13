use super::*;

fn request() -> OpenRequest {
    OpenRequest {
        folder: "/tmp/a".into(),
        selected: vec!["/tmp/a/file".into()],
        properties: false,
    }
}

#[test]
fn closed_timed_out_or_mismatched_windows_cannot_claim_requests() {
    let (queued, reply) = Request::for_test(request());
    assert!(queued.accept(Path::new("/tmp/another")).is_none());
    assert!(!reply.recv().unwrap());
    let (mut queued, reply) = Request::for_test(request());
    queued.deadline = Instant::now() - Duration::from_secs(1);
    assert!(queued.accept(Path::new("/tmp/a")).is_none());
    assert!(!reply.recv().unwrap());
    let (queued, reply) = Request::for_test(request());
    drop(reply);
    assert!(queued.accept(Path::new("/tmp/a")).is_none());
    let (queued, reply) = Request::for_test(request());
    let (accepted, token) = queued.accept(Path::new("/tmp/a")).unwrap();
    assert!(reply.recv().unwrap());
    assert_eq!(accepted, request());
    assert_eq!(token, "test-token");
}

#[test]
fn window_reuse_preserves_selection_and_proxy_restricts_ungranted_names() {
    const TEST: &str = "file_manager::window::tests::window_reuse_preserves_selection_and_proxy_restricts_ungranted_names";
    if std::env::var_os("LSE_WINDOW_BUS_TEST").is_none() {
        assert!(
            Command::new("dbus-run-session")
                .arg("--")
                .arg(std::env::current_exe().unwrap())
                .args(["--exact", TEST, "--nocapture"])
                .env("LSE_WINDOW_BUS_TEST", "1")
                .status()
                .unwrap()
                .success()
        );
        return;
    }
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("selected file.txt");
    std::fs::write(&path, "fixture").unwrap();
    let request = OpenRequest {
        folder: root.path().to_owned(),
        selected: vec![path],
        properties: false,
    };
    let expected = OpenRequest {
        folder: request.folder.clone(),
        selected: request.selected.clone(),
        properties: request.properties,
    };
    let (ready, received) = mpsc::sync_channel(1);
    let (stop, stopped) = mpsc::sync_channel(1);
    let worker = std::thread::spawn(move || {
        let window = Window::attach(&eframe::egui::Context::default()).unwrap();
        let second = Window::attach(&eframe::egui::Context::default()).unwrap();
        ready.send(()).unwrap();
        let deadline = Instant::now() + Duration::from_secs(8);
        loop {
            if let Some(queued) = second.pending().into_iter().chain(window.pending()).next() {
                let (actual, token) = queued.accept(&expected.folder).unwrap();
                assert_eq!(actual, expected);
                assert_eq!(token, "activation-token");
                stopped.recv_timeout(Duration::from_secs(8)).unwrap();
                return;
            }
            assert!(Instant::now() < deadline, "window did not receive Reveal");
            std::thread::sleep(Duration::from_millis(5));
        }
    });
    received.recv_timeout(Duration::from_secs(5)).unwrap();
    zbus::block_on(async {
        let connection = zbus::Connection::session().await.unwrap();
        if std::env::var_os("LSE_TEST_FILTERED_DBUS").is_some() {
            assert!(
                connection
                    .request_name("org.example.NotGranted")
                    .await
                    .is_err()
            );
            assert!(
                connection
                    .request_name("org.freedesktop.portal.Desktop")
                    .await
                    .is_err()
            );
            // No unrestricted host execution/service access was added for compatibility.
            assert!(
                connection
                    .call_method(
                        Some("org.freedesktop.Flatpak"),
                        "/org/freedesktop/Flatpak",
                        Some("org.freedesktop.DBus.Peer"),
                        "Ping",
                        &(),
                    )
                    .await
                    .is_err()
            );
        }
        assert!(reuse(&connection, &request, "activation-token").await);
    });
    stop.send(()).unwrap();
    worker.join().unwrap();
}
