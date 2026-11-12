use super::*;

#[test]
fn text_is_bounded_and_binary_or_special_files_are_not_opened_as_text() {
    let root = tempfile::tempdir().unwrap();
    let text = root.path().join("source.rs");
    std::fs::write(&text, "fn main() {}\n").unwrap();
    let cancel = AtomicBool::new(false);
    assert!(
        matches!(load(&text,1,&cancel).unwrap(),Content::Text {text,..} if text.contains("main"))
    );
    std::fs::write(&text, vec![b'a'; TEXT_BYTES as usize + 200]).unwrap();
    assert!(
        matches!(load(&text,1,&cancel).unwrap(),Content::Text {text,truncated:true} if text.len()==TEXT_BYTES as usize)
    );
    let binary = root.path().join("program.bin");
    std::fs::write(&binary, b"\0binary").unwrap();
    assert!(matches!(
        load(&binary, 1, &cancel).unwrap(),
        Content::Unsupported
    ));
    assert!(load(root.path(), 1, &cancel).is_err());
    cancel.store(true, Ordering::Release);
    assert_eq!(
        load(&text, 1, &cancel).err().unwrap().kind(),
        io::ErrorKind::Interrupted
    );
}
#[test]
fn image_and_svg_previews_have_bounded_pixels_and_do_not_change_sources() {
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join("photo.PNG");
    image::RgbaImage::from_pixel(24, 12, image::Rgba([20, 50, 90, 255]))
        .save(&file)
        .unwrap();
    let before = std::fs::read(&file).unwrap();
    let Content::Image(frame) = load(&file, 1, &AtomicBool::new(false)).unwrap() else {
        panic!("image")
    };
    assert_eq!(
        (frame.width, frame.height, frame.rgba.len()),
        (24, 12, 24 * 12 * 4)
    );
    assert_eq!(before, std::fs::read(&file).unwrap());
    let svg = root.path().join("drawing.svg");
    std::fs::write(&svg,r#"<svg xmlns="http://www.w3.org/2000/svg" width="4000" height="2000"><rect width="4000" height="2000" fill="red"/></svg>"#).unwrap();
    let Content::Image(frame) = load(&svg, 1, &AtomicBool::new(false)).unwrap() else {
        panic!("SVG")
    };
    assert_eq!((frame.width, frame.height), (1600, 800));
    assert_eq!(&frame.rgba[..4], &[255, 0, 0, 255]);
}

pub(crate) fn test_pdf(path: &Path) {
    let objects = [
        "<< /Type /Catalog /Pages 2 0 R >>",
        "<< /Type /Pages /Kids [3 0 R 5 0 R] /Count 2 >>",
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 100] /Contents 4 0 R >>",
        "<< /Length 26 >>\nstream\n1 0 0 rg 0 0 200 100 re f\nendstream",
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 100 200] /Contents 6 0 R >>",
        "<< /Length 26 >>\nstream\n0 0 1 rg 0 0 100 200 re f\nendstream",
    ];
    let mut bytes = b"%PDF-1.4\n".to_vec();
    let mut offsets = vec![0];
    for (i, object) in objects.iter().enumerate() {
        offsets.push(bytes.len());
        bytes.extend_from_slice(format!("{} 0 obj\n{}\nendobj\n", i + 1, object).as_bytes());
    }
    let start = bytes.len();
    bytes.extend_from_slice(
        format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).as_bytes(),
    );
    for offset in &offsets[1..] {
        bytes.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    bytes.extend_from_slice(
        format!("trailer\n<< /Size 7 /Root 1 0 R >>\nstartxref\n{start}\n%%EOF\n").as_bytes(),
    );
    std::fs::write(path, bytes).unwrap();
}
#[test]
fn pdf_renderer_handles_real_pages_and_does_not_change_the_document() {
    if Command::new("pdftoppm").arg("-v").output().is_err() {
        assert!(
            std::env::var_os("CI").is_none(),
            "Poppler is required in CI"
        );
        return;
    }
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("document with spaces.pdf");
    test_pdf(&path);
    let before = std::fs::read(&path).unwrap();
    let Content::Pdf {
        pixels,
        page,
        pages,
        ..
    } = pdf(&path, 2, &AtomicBool::new(false)).unwrap()
    else {
        panic!("PDF")
    };
    assert_eq!((page, pages), (2, 2));
    assert!(pixels.height > pixels.width);
    assert_eq!(before, std::fs::read(&path).unwrap());
}

#[test]
fn cancelled_conversion_stops_and_reaps_the_converter() {
    let cancel = Arc::new(AtomicBool::new(false));
    let worker = cancel.clone();
    let start = Instant::now();
    let handle = std::thread::spawn(move || command(Command::new("sleep").arg("10"), &worker));
    std::thread::sleep(Duration::from_millis(30));
    cancel.store(true, Ordering::Release);
    assert_eq!(
        handle.join().unwrap().unwrap_err().kind(),
        io::ErrorKind::Interrupted
    );
    assert!(start.elapsed() < Duration::from_secs(2));
}

#[test]
fn cancelling_a_converter_also_stops_its_helper_process() {
    let root = tempfile::tempdir().unwrap();
    let pidfile = root.path().join("helper.pid");
    let worker_pidfile = pidfile.clone();
    let cancel = Arc::new(AtomicBool::new(false));
    let worker_cancel = cancel.clone();
    let worker = std::thread::spawn(move || {
        command(
            Command::new("sh")
                .args([
                    "-c",
                    "sleep 30 & helper=$!; echo \"$helper\" > \"$1\"; wait",
                    "preview-test",
                ])
                .arg(worker_pidfile),
            &worker_cancel,
        )
    });
    let deadline = Instant::now() + Duration::from_secs(3);
    let helper: u32 = loop {
        if let Ok(pid) = std::fs::read_to_string(&pidfile)
            && let Ok(pid) = pid.trim().parse()
        {
            break pid;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(5));
    };
    cancel.store(true, Ordering::Release);
    assert_eq!(
        worker.join().unwrap().unwrap_err().kind(),
        io::ErrorKind::Interrupted
    );
    loop {
        let stopped = std::fs::read_to_string(format!("/proc/{helper}/stat"))
            .map(|stat| {
                stat.split_once(") ")
                    .is_some_and(|(_, rest)| rest.starts_with('Z'))
            })
            .unwrap_or(true);
        if stopped {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "converter helper remained running after cancellation"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

pub(crate) fn office_available() -> bool {
    let available = ["libreoffice", "soffice"].into_iter().any(|program| {
        Command::new(program)
            .arg("--version")
            .output()
            .is_ok_and(|out| out.status.success())
    });
    assert!(
        available || std::env::var_os("CI").is_none(),
        "LibreOffice Writer is required in CI"
    );
    available
}

pub(crate) fn test_docx(path: &Path) {
    std::fs::write(path, include_bytes!("fixtures/two-pages.docx")).unwrap();
}

#[test]
fn docx_conversion_preserves_formatted_pages_source_and_releases_its_private_cache() {
    if !office_available() {
        return;
    }
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("á source with spaces.docx");
    test_docx(&source);
    let before = std::fs::read(&source).unwrap();
    let Content::Pdf {
        pixels,
        page,
        pages,
        document: Some(document),
    } = load(&source, 1, &AtomicBool::new(false)).unwrap()
    else {
        panic!("formatted DOCX")
    };
    assert_eq!((page, pages), (1, 2));
    assert!(pixels.height > pixels.width);
    let cached = document.cached_path();
    let text = Command::new("pdftotext")
        .arg(&cached)
        .arg("-")
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&text.stdout);
    assert!(
        text.contains("Formatted DOCX preview")
            && text.contains("Formatting retained")
            && text.contains("Second page preview")
    );
    assert!(cached.is_file());
    assert!(matches!(
        pdf(&cached, 2, &AtomicBool::new(false)).unwrap(),
        Content::Pdf {
            page: 2,
            pages: 2,
            ..
        }
    ));
    assert_eq!(std::fs::read(&source).unwrap(), before);
    assert_eq!(
        std::fs::read_dir(root.path()).unwrap().count(),
        1,
        "no source lock or output files beside the original"
    );
    drop(document);
    assert!(!cached.exists());
}

pub(crate) fn html_available() -> bool {
    let available = Command::new("python3")
        .args(["-c", "import weasyprint"])
        .output()
        .is_ok_and(|o| o.status.success());
    assert!(
        available || std::env::var_os("CI").is_none(),
        "WeasyPrint is required in CI"
    );
    available
}

#[test]
fn html_presentation_loads_local_css_without_javascript_or_network_requests() {
    if !html_available() {
        return;
    }
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("index.html");
    let server = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    server.set_nonblocking(true).unwrap();
    std::fs::write(root.path().join("style.css"), "@page {size: 400px 300px; margin: 10px} h1::before {content: 'LOCAL_CSS '} h1 {color: blue}").unwrap();
    std::fs::write(&source, format!("<!doctype html><meta charset=utf-8><link rel=stylesheet href=style.css><h1>Static HTML preview</h1><script>document.write('SCRIPT_EXECUTED')</script><img src='http://{}/remote.png'>",server.local_addr().unwrap())).unwrap();
    let before = std::fs::read(&source).unwrap();
    let document = html::convert(&source, &AtomicBool::new(false)).unwrap();
    let pdf_path = document.cached_path();
    let Content::Pdf { pixels, .. } = pdf(&pdf_path, 1, &AtomicBool::new(false)).unwrap() else {
        panic!("HTML presentation")
    };
    assert!(
        pixels.width > pixels.height,
        "local CSS page dimensions are respected"
    );
    let text = Command::new("pdftotext")
        .arg(&pdf_path)
        .arg("-")
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&text.stdout)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    assert!(
        text.contains("LOCAL_CSS") && text.contains("Static HTML preview"),
        "{text}"
    );
    assert!(!text.contains("SCRIPT_EXECUTED"));
    assert_eq!(
        server.accept().unwrap_err().kind(),
        io::ErrorKind::WouldBlock
    );
    assert_eq!(std::fs::read(&source).unwrap(), before);
    drop(document);
    assert!(!pdf_path.exists());
}

#[test]
fn missing_preview_tools_only_disable_their_corresponding_renderer() {
    const CHILD: &str = "LSE_TEST_NO_PREVIEW_TOOLS";
    if std::env::var_os(CHILD).is_some() {
        let root = tempfile::tempdir().unwrap();
        let docx = root.path().join("document.docx");
        test_docx(&docx);
        assert_eq!(
            office::convert(&docx, &AtomicBool::new(false))
                .unwrap_err()
                .kind(),
            io::ErrorKind::Unsupported
        );
        let html = root.path().join("index.html");
        std::fs::write(&html, "<h1>source</h1>").unwrap();
        assert_eq!(
            html::convert(&html, &AtomicBool::new(false))
                .unwrap_err()
                .kind(),
            io::ErrorKind::Unsupported
        );
        assert!(matches!(
            load(&html, 1, &AtomicBool::new(false)).unwrap(),
            Content::Text { .. }
        ));
        return;
    }
    let empty = tempfile::tempdir().unwrap();
    assert!(
        Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "preview::tests::missing_preview_tools_only_disable_their_corresponding_renderer",
                "--nocapture"
            ])
            .env(CHILD, "1")
            .env("PATH", empty.path())
            .status()
            .unwrap()
            .success()
    );
}
