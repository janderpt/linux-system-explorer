use super::*;
use std::{io::Read, process::Command, sync::mpsc};
struct Portal {
    calls: Arc<Mutex<Vec<String>>>,
    closed: Arc<AtomicBool>,
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
#[zbus::interface(name = "org.freedesktop.portal.Print")]
impl Portal {
    async fn print(
        &self,
        _parent: &str,
        title: &str,
        fd: zbus::OwnedFd,
        options: HashMap<String, OwnedVariant>,
        #[zbus(header)] header: zbus::message::Header<'_>,
        #[zbus(connection)] connection: &zbus::Connection,
    ) -> zbus::fdo::Result<OwnedObjectPath> {
        assert!(
            !options.contains_key("token"),
            "sem token: o sistema deve apresentar diálogo"
        );
        assert!(bool::try_from(options.get("modal").unwrap()).unwrap());
        let mut file = File::from(std::os::fd::OwnedFd::from(fd));
        let flags = rustix::fs::fcntl_getfl(&file).unwrap();
        assert!(!flags.intersects(rustix::fs::OFlags::WRONLY | rustix::fs::OFlags::RDWR));
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes).unwrap();
        assert!(bytes.starts_with(b"%PDF-"));
        self.calls.lock().unwrap().push(title.to_owned());
        let sender = header.sender().unwrap();
        let token = <&str>::try_from(options.get("handle_token").unwrap()).unwrap();
        let path = format!(
            "/org/freedesktop/portal/desktop/request/{}/{}",
            sender.as_str().trim_start_matches(':').replace('.', "_"),
            token
        );
        if title == "hold" {
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
            let code = match title {
                "cancel" => 1u32,
                "failure" => 2,
                _ => 0,
            };
            connection
                .emit_signal(
                    Some(sender),
                    path.as_str(),
                    "org.freedesktop.portal.Request",
                    "Response",
                    &(code, HashMap::<String, OwnedVariant>::new()),
                )
                .await
                .unwrap();
        }
        Ok(OwnedObjectPath::try_from(path).unwrap())
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
    let (ready, received) = mpsc::channel();
    let (stop, stopped) = mpsc::channel();
    let service = Portal {
        calls: calls.clone(),
        closed: closed.clone(),
    };
    let worker = std::thread::spawn(move || {
        let connection = zbus::block_on(
            zbus::connection::Builder::session()
                .name("org.freedesktop.portal.Desktop")
                .serve_at("/org/freedesktop/portal/desktop", service)
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
    let token = Arc::new(AtomicBool::new(false));
    for (title, expected) in [
        ("success", Some(true)),
        ("cancel", Some(false)),
        ("failure", None),
    ] {
        let file = readonly(&path).unwrap();
        let result = portal(&file, title, &token);
        match expected {
            Some(value) => assert_eq!(result.unwrap(), value),
            None => assert!(result.is_err()),
        }
    }
    let cancel = token.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(100));
        cancel.store(true, Ordering::Release);
    });
    assert!(!portal(&readonly(&path).unwrap(), "hold", &token).unwrap());
    assert!(closed.load(Ordering::Acquire));
    assert_eq!(calls.lock().unwrap().len(), 4);
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
