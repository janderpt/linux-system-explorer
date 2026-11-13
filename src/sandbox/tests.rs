use super::*;
use std::{
    io::Read,
    process::Command,
    sync::{Arc, Mutex, mpsc},
};

type Calls = Arc<Mutex<Vec<(bool, Vec<u8>)>>>;
struct Portal {
    calls: Calls,
}
#[zbus::interface(name = "org.freedesktop.portal.OpenURI")]
impl Portal {
    async fn open_file(
        &self,
        _parent: &str,
        fd: zbus::zvariant::OwnedFd,
        options: HashMap<String, OwnedVariant>,
        #[zbus(header)] header: zbus::message::Header<'_>,
        #[zbus(connection)] connection: &zbus::Connection,
    ) -> zbus::fdo::Result<OwnedObjectPath> {
        let sender = header.sender().unwrap();
        let token = <&str>::try_from(options.get("handle_token").unwrap()).unwrap();
        let ask = bool::try_from(options.get("ask").unwrap()).unwrap();
        assert!(bool::try_from(options.get("writable").unwrap()).unwrap());
        let mut contents = Vec::new();
        File::from(std::os::fd::OwnedFd::from(fd))
            .read_to_end(&mut contents)
            .unwrap();
        let code = u32::from(contents[0]);
        self.calls.lock().unwrap().push((ask, contents));
        let path = format!(
            "/org/freedesktop/portal/desktop/request/{}/{}",
            sender.as_str().trim_start_matches(':').replace('.', "_"),
            token
        );
        connection
            .emit_signal(
                Some(sender.as_str()),
                path.as_str(),
                "org.freedesktop.portal.Request",
                "Response",
                &(code, HashMap::<String, OwnedVariant>::new()),
            )
            .await
            .unwrap();
        Ok(OwnedObjectPath::try_from(path).unwrap())
    }
}

#[test]
fn portal_opens_an_fd_supports_chooser_cancellation_and_failure_on_an_isolated_bus() {
    const CHILD: &str = "LSE_TEST_PORTAL_BUS";
    if std::env::var_os(CHILD).is_none() {
        assert!(Command::new("dbus-run-session").arg("--").arg(std::env::current_exe().unwrap()).args(["--exact", "sandbox::tests::portal_opens_an_fd_supports_chooser_cancellation_and_failure_on_an_isolated_bus", "--nocapture"]).env(CHILD,"1").env("FLATPAK_ID", crate::desktop::APP_ID).status().unwrap().success());
        return;
    }
    let calls = Arc::new(Mutex::new(Vec::new()));
    let portal = Portal {
        calls: calls.clone(),
    };
    let (ready, received) = mpsc::channel();
    let (stop, stopped) = mpsc::channel();
    let worker = std::thread::spawn(move || {
        let connection = zbus::block_on(
            portal_service_builder()
                .name("org.freedesktop.portal.Desktop")
                .unwrap()
                .serve_at("/org/freedesktop/portal/desktop", portal)
                .unwrap()
                .build(),
        )
        .unwrap();
        ready.send(()).unwrap();
        stopped.recv().unwrap();
        drop(connection);
    });
    received.recv_timeout(Duration::from_secs(5)).unwrap();
    let root = tempfile::tempdir().unwrap();
    use std::os::unix::ffi::OsStringExt;
    let path = root.path().join(std::ffi::OsString::from_vec(
        b"file ' $(literal)-\xff.txt".to_vec(),
    ));
    for (code, ask) in [(0, false), (0, true), (1, true), (2, false)] {
        std::fs::write(&path, [code, 42]).unwrap();
        let result = if ask {
            open_file(&path, true)
        } else {
            crate::applications::open_default(&path)
        };
        assert_eq!(result.is_ok(), code != 2);
    }
    assert!(open_file(root.path(), true).is_err());
    assert_eq!(
        *calls.lock().unwrap(),
        vec![
            (false, vec![0, 42]),
            (true, vec![0, 42]),
            (true, vec![1, 42]),
            (false, vec![2, 42])
        ]
    );
    // Multiple windows/Flatpak instances must not depend on distinct host PIDs.
    let ctx = eframe::egui::Context::default();
    let first = crate::file_manager::window::Window::attach(&ctx).unwrap();
    let second = crate::file_manager::window::Window::attach(&ctx).unwrap();
    drop((first, second));
    stop.send(()).unwrap();
    worker.join().unwrap();
}

// Only the client goes through the filtered proxy in compatibility tests. The
// mock portal stays on the private host bus, like a real desktop portal.
pub(crate) fn portal_service_builder() -> zbus::connection::Builder<'static> {
    match std::env::var("LSE_TEST_DBUS_SERVICE_ADDRESS") {
        Ok(address) => zbus::connection::Builder::address(address.as_str()).unwrap(),
        Err(_) => zbus::connection::Builder::session().unwrap(),
    }
}

#[test]
#[ignore = "requires the real Flatpak runtime and externally generated fixture directory"]
fn flatpak_runtime_exercises_renderers_file_operations_and_private_preferences() {
    assert!(Path::new("/.flatpak-info").is_file());
    let fixtures = std::env::var_os("LSE_FLATPAK_FIXTURES")
        .map(std::path::PathBuf::from)
        .expect("fixture directory");
    let ctx = eframe::egui::Context::default();
    let first = crate::file_manager::window::Window::attach(&ctx).unwrap();
    let second = crate::file_manager::window::Window::attach(&ctx).unwrap();
    drop((first, second));
    for name in ["sample.png", "sample.webm"] {
        eprintln!("Checking runtime thumbnail: {name}");
        let path = fixtures.join(name);
        let job = crate::preview::thumbnail::start(
            path.clone(),
            path.clone(),
            crate::preview::Kind::for_path(&path),
            &ctx,
        )
        .unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(90);
        loop {
            if let Some(result) = job.poll() {
                let crate::preview::Content::Image(frame) = result.unwrap() else {
                    panic!("thumbnail")
                };
                assert!(
                    frame.width > 0
                        && frame.height > 0
                        && frame.width <= 256
                        && frame.height <= 256
                );
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "runtime thumbnail did not finish: {name}"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    let job = crate::preview::Job::start(fixtures.join("sample.zip"), 1, &ctx);
    let deadline = std::time::Instant::now() + Duration::from_secs(15);
    loop {
        if let Some(result) = job.poll() {
            assert!(
                matches!(result.unwrap(), crate::preview::Content::Archive(list) if list.entries.iter().any(|entry| entry.path=="sample.txt"))
            );
            break;
        }
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(5));
    }
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("trash-roundtrip.txt");
    std::fs::write(&path, "preserve this fixture").unwrap();
    ::trash::delete(&path).unwrap();
    assert!(!path.exists());
    let item = ::trash::os_limited::list()
        .unwrap()
        .into_iter()
        .find(|item| item.original_parent == root.path() && item.name == "trash-roundtrip.txt")
        .unwrap();
    ::trash::os_limited::restore_all([item]).unwrap();
    assert_eq!(
        std::fs::read_to_string(path).unwrap(),
        "preserve this fixture"
    );
    let config = std::env::var_os("XDG_CONFIG_HOME").expect("Flatpak config home");
    assert!(
        crate::settings::Preferences::path()
            .unwrap()
            .starts_with(config)
    );
    assert_eq!(
        crate::network::connect_cancellable("smb://invalid/share", &Default::default())
            .unwrap_err()
            .kind(),
        io::ErrorKind::Unsupported
    );
}
