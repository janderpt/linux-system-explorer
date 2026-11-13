use super::*;
use std::{collections::BTreeMap, io::Read, process::Command, sync::mpsc};
struct Portal {
    calls: Arc<Mutex<Vec<String>>>,
    closed: Arc<AtomicBool>,
    issued: Mutex<BTreeMap<u32, (String, String)>>,
    serial: AtomicU64,
    cancel: Arc<AtomicBool>,
}
struct Request {
    closed: Arc<AtomicBool>,
}
#[zbus::interface(name = "org.freedesktop.portal.Request")]
impl Request {
    fn close(&self) {
        self.closed.store(true, Ordering::Release);
    }
}
impl Portal {
    async fn response(
        &self,
        title: &str,
        options: &HashMap<String, OwnedVariant>,
        header: zbus::message::Header<'_>,
        connection: &zbus::Connection,
        prepare: bool,
        results: HashMap<String, OwnedVariant>,
    ) -> zbus::fdo::Result<OwnedObjectPath> {
        let sender = header.sender().unwrap();
        let token = <&str>::try_from(options.get("handle_token").unwrap()).unwrap();
        let path = format!(
            "/org/freedesktop/portal/desktop/request/{}/{}",
            sender.as_str().trim_start_matches(':').replace('.', "_"),
            token
        );
        if (prepare && title == "hold") || (!prepare && title == "hold-send") {
            connection
                .object_server()
                .at(
                    path.as_str(),
                    Request {
                        closed: self.closed.clone(),
                    },
                )
                .await
                .unwrap();
        } else {
            let code = match (prepare, title) {
                (true, "cancel") => 1u32,
                (true, "failure") | (false, "send-failure") => 2,
                _ => 0,
            };
            if prepare && title == "race-cancel" {
                self.cancel.store(true, Ordering::Release);
            }
            connection
                .emit_signal(
                    Some(sender.as_str()),
                    path.as_str(),
                    "org.freedesktop.portal.Request",
                    "Response",
                    &(code, results),
                )
                .await
                .unwrap();
        }
        Ok(OwnedObjectPath::try_from(path).unwrap())
    }
}
#[zbus::interface(name = "org.freedesktop.portal.Print")]
impl Portal {
    // The fixed portal D-Bus signature also injects the message header and connection.
    #[allow(clippy::too_many_arguments)]
    async fn prepare_print(
        &self,
        _parent: &str,
        title: &str,
        _settings: HashMap<String, OwnedVariant>,
        _page_setup: HashMap<String, OwnedVariant>,
        options: HashMap<String, OwnedVariant>,
        #[zbus(header)] header: zbus::message::Header<'_>,
        #[zbus(connection)] connection: &zbus::Connection,
    ) -> zbus::fdo::Result<OwnedObjectPath> {
        assert!(
            !options.contains_key("token"),
            "cada trabalho precisa de confirmação nova"
        );
        assert!(bool::try_from(options.get("modal").unwrap()).unwrap());
        self.calls.lock().unwrap().push(format!("prepare:{title}"));
        let mut results = HashMap::new();
        if title == "invalid-token" {
            results.insert(
                "token".into(),
                OwnedVariant::from(zbus::zvariant::Str::from("invalid")),
            );
        } else if !matches!(title, "missing-token" | "cancel" | "failure" | "hold") {
            let token = self.serial.fetch_add(1, Ordering::Relaxed) as u32;
            self.issued.lock().unwrap().insert(
                token,
                (header.sender().unwrap().to_string(), title.to_owned()),
            );
            results.insert("token".into(), OwnedVariant::from(token));
        }
        self.response(title, &options, header, connection, true, results)
            .await
    }
    async fn print(
        &self,
        _parent: &str,
        title: &str,
        fd: zbus::zvariant::OwnedFd,
        options: HashMap<String, OwnedVariant>,
        #[zbus(header)] header: zbus::message::Header<'_>,
        #[zbus(connection)] connection: &zbus::Connection,
    ) -> zbus::fdo::Result<OwnedObjectPath> {
        let token = u32::try_from(
            options
                .get("token")
                .expect("KDE exige o token de PreparePrint"),
        )
        .unwrap();
        assert_eq!(
            self.issued.lock().unwrap().remove(&token),
            Some((header.sender().unwrap().to_string(), title.to_owned())),
            "não reutilizar autorização de outro trabalho"
        );
        let mut file = File::from(std::os::fd::OwnedFd::from(fd));
        let flags = rustix::fs::fcntl_getfl(&file).unwrap();
        assert!(!flags.intersects(rustix::fs::OFlags::WRONLY | rustix::fs::OFlags::RDWR));
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes).unwrap();
        assert!(bytes.starts_with(b"%PDF-"));
        self.calls.lock().unwrap().push(format!("submit:{title}"));
        self.response(title, &options, header, connection, false, HashMap::new())
            .await
    }
}
#[test]
fn print_portal_requires_confirmation_passes_read_only_pdf_and_handles_cancellation_without_printing()
 {
    if std::env::var_os("LSE_PRINT_BUS_TEST").is_none() {
        let result=Command::new("dbus-run-session").arg("--").arg(std::env::current_exe().unwrap()).args(["--exact","printing::tests::print_portal_requires_confirmation_passes_read_only_pdf_and_handles_cancellation_without_printing","--nocapture"]).env("LSE_PRINT_BUS_TEST","1").output().unwrap();
        assert!(
            result.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
        return;
    }
    let calls = Arc::new(Mutex::new(Vec::new()));
    let closed = Arc::new(AtomicBool::new(false));
    let token = Arc::new(AtomicBool::new(false));
    let (ready, received) = mpsc::channel();
    let (stop, stopped) = mpsc::channel();
    let service = Portal {
        calls: calls.clone(),
        closed: closed.clone(),
        issued: Mutex::new(BTreeMap::new()),
        serial: AtomicU64::new(42),
        cancel: token.clone(),
    };
    let worker = std::thread::spawn(move || {
        let connection = zbus::block_on(
            crate::sandbox::tests::portal_service_builder()
                .name("org.freedesktop.portal.Desktop")
                .unwrap()
                .serve_at("/org/freedesktop/portal/desktop", service)
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
    let path = root.path().join("source.pdf");
    std::fs::write(&path, b"%PDF-1.4\nmock-only").unwrap();
    for (title, expected) in [
        ("success", Some(true)),
        ("cancel", Some(false)),
        ("failure", None),
        ("missing-token", None),
        ("invalid-token", None),
        ("send-failure", None),
        ("success", Some(true)),
        ("race-cancel", Some(false)),
    ] {
        token.store(false, Ordering::Release);
        let before = calls.lock().unwrap().len();
        let result = portal(&readonly(&path).unwrap(), title, &token);
        match expected {
            Some(value) => assert_eq!(result.unwrap(), value),
            None => assert!(result.is_err()),
        }
        let log = calls.lock().unwrap();
        assert_eq!(log[before], format!("prepare:{title}"));
        if matches!(
            title,
            "cancel" | "failure" | "missing-token" | "invalid-token" | "race-cancel"
        ) {
            assert_eq!(
                log.len(),
                before + 1,
                "falha ou cancelamento na preparação nunca pode chamar Print"
            );
        } else {
            assert_eq!(log[before + 1], format!("submit:{title}"));
        }
    }
    for title in ["hold", "hold-send"] {
        token.store(false, Ordering::Release);
        closed.store(false, Ordering::Release);
        let cancel = token.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(100));
            cancel.store(true, Ordering::Release);
        });
        assert!(!portal(&readonly(&path).unwrap(), title, &token).unwrap());
        assert!(closed.load(Ordering::Acquire));
    }
    assert_eq!(std::fs::read(path).unwrap(), b"%PDF-1.4\nmock-only");
    stop.send(()).unwrap();
    worker.join().unwrap();
}
#[test]
fn printable_types_prepare_and_merge_without_touching_sources() {
    use std::fs;
    let root = tempfile::tempdir().unwrap();
    for (name, expected) in [
        ("image.png", true),
        ("report.pdf", true),
        ("word.docx", true),
        ("sheet.xlsx", true),
        ("slides.pptx", true),
        ("page.html", true),
        ("readme.md", true),
        ("text.txt", true),
        ("archive.zip", false),
        ("video.mp4", false),
        ("app.exe", false),
    ] {
        let path = root.path().join(name);
        fs::write(&path, "fixture").unwrap();
        let entry = crate::filesystem::read_entry(&path).unwrap();
        assert_eq!(supported(&entry), expected);
    }
    if !crate::preview::tests::html_available() {
        return;
    }
    let image = root.path().join("image.png");
    image::RgbaImage::from_pixel(16, 8, image::Rgba([30, 50, 90, 255]))
        .save(&image)
        .unwrap();
    let text = root.path().join("text.txt");
    fs::write(&text, "texto <script>não executar</script> & fim").unwrap();
    let cancel = AtomicBool::new(false);
    let mut documents = Vec::new();
    for path in [&image, &text] {
        let before = fs::read(path).unwrap();
        let entry = crate::filesystem::read_entry(path).unwrap();
        let pdf = print::prepare(&entry, &cancel).unwrap();
        assert!(fs::read(&pdf.path).unwrap().starts_with(b"%PDF-"));
        assert_eq!(fs::read(path).unwrap(), before);
        documents.push(pdf);
    }
    let merged = print::merge(&documents, &cancel).unwrap();
    let info = Command::new("pdfinfo").arg(&merged.path).output().unwrap();
    assert!(info.status.success());
    assert!(
        String::from_utf8_lossy(&info.stdout)
            .lines()
            .any(|line| line.starts_with("Pages:") && line.ends_with('2'))
    );
    let output = Command::new("pdftotext")
        .arg(&documents[1].path)
        .arg("-")
        .output()
        .unwrap();
    assert!(String::from_utf8_lossy(&output.stdout).contains("<script>"));
}

/// Manual integration smoke test. This test only calls PreparePrint, so even
/// accepting the system dialog never submits a document or prints a page.
#[test]
#[ignore = "requires the user's graphical session; opens only the print-settings dialog"]
fn native_print_dialog_prepares_without_sending_document() {
    let cancel = Arc::new(AtomicBool::new(false));
    let timeout_cancel = cancel.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(45));
        timeout_cancel.store(true, Ordering::Release);
    });
    let file = tempfile::tempfile().unwrap();
    let result = zbus::block_on(async {
        let connection = zbus::connection::Builder::session()
            .unwrap()
            .method_timeout(Duration::from_secs(15))
            .build()
            .await?;
        portal_request(
            &connection,
            &file,
            "Linux System Explorer — teste do diálogo (nenhum documento será enviado)",
            &cancel,
            Phase::Prepare,
        )
        .await
    })
    .expect("the real desktop must open print settings instead of rejecting the request");
    println!(
        "Native PreparePrint completed: {}; Print was never called.",
        if result.is_some() {
            "settings accepted"
        } else {
            "dialog cancelled"
        }
    );
}
