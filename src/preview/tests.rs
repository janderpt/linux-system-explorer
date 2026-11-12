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

pub(crate) fn test_office(path: &Path) {
    let data: &[u8] = match path.extension().unwrap().to_str().unwrap() {
        "docx" => include_bytes!("fixtures/two-pages.docx"),
        "doc" => include_bytes!("fixtures/two-pages.doc"),
        "odt" => include_bytes!("fixtures/two-pages.odt"),
        "xlsx" => include_bytes!("fixtures/two-sheets.xlsx"),
        "xls" => include_bytes!("fixtures/two-sheets.xls"),
        "ods" => include_bytes!("fixtures/two-sheets.ods"),
        "pptx" => include_bytes!("fixtures/two-slides.pptx"),
        "ppt" => include_bytes!("fixtures/two-slides.ppt"),
        "odp" => include_bytes!("fixtures/two-slides.odp"),
        "odg" => include_bytes!("fixtures/two-drawings.odg"),
        "odf" => include_bytes!("fixtures/formula.odf"),
        _ => panic!("unsupported test fixture"),
    };
    std::fs::write(path, data).unwrap();
}

#[test]
fn office_families_render_legacy_microsoft_and_opendocument_files_without_changing_sources() {
    if !office_available() {
        return;
    }
    for (extension, expected) in [
        ("docx", "Second page preview"),
        ("doc", "Second page preview"),
        ("odt", "Second page preview"),
        ("xlsx", "Second sheet preview"),
        ("xls", "Second sheet preview"),
        ("ods", "Second sheet preview"),
        ("pptx", "Second slide preview"),
        ("ppt", "Second slide preview"),
        ("odp", "Second slide preview"),
        ("odg", "Second drawing preview"),
        ("odf", "1"),
    ] {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join(format!("preview with spaces.{extension}"));
        test_office(&source);
        let before = std::fs::read(&source).unwrap();
        let Content::Pdf {
            pixels,
            pages,
            document: Some(document),
            ..
        } = load(&source, 1, &AtomicBool::new(false)).unwrap()
        else {
            panic!("formatted Office preview")
        };
        assert_eq!(pages, if extension == "odf" { 1 } else { 2 });
        let kind = Kind::for_path(&source);
        if kind == Kind::Office(Family::Presentation) {
            assert!(
                pixels.width > pixels.height,
                "landscape slide fixture: {extension}"
            );
        }
        let cached = document.cached_path();
        let output = Command::new("pdftotext")
            .arg(&cached)
            .arg("-")
            .output()
            .unwrap();
        let text = String::from_utf8_lossy(&output.stdout)
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        assert!(text.contains(expected), "{text}");
        if kind == Kind::Office(Family::Spreadsheet) {
            assert!(
                text.contains("Apples") && text.contains("150"),
                "values and formulas: {text}"
            );
        }
        assert!(matches!(
            pdf(&cached, pages, &AtomicBool::new(false)).unwrap(),
            Content::Pdf { page, .. } if page==pages
        ));
        assert_eq!(std::fs::read(&source).unwrap(), before);
        assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 1);
        drop(document);
        assert!(!cached.exists());
    }
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
fn paginated_html_uses_print_styles_instead_of_screen_only_grid_layouts() {
    if !html_available() {
        return;
    }
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("responsive.html");
    std::fs::write(&source, r#"<!doctype html><meta charset="utf-8"><style>
@media screen { .grid { display:grid; grid-template-columns:1fr 1fr } .print {display:none} }
@media print { .grid {display:block} .screen {display:none} }
</style><section class="grid"><article class="grid"><p class="screen">SCREEN_ONLY</p><p class="print">PRINT_LAYOUT</p></article></section>"#).unwrap();
    let before = std::fs::read(&source).unwrap();
    let document = html::convert(&source, &AtomicBool::new(false)).unwrap();
    let output = Command::new("pdftotext")
        .arg(document.cached_path())
        .arg("-")
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(
        text.contains("PRINT_LAYOUT") && !text.contains("SCREEN_ONLY"),
        "{text}"
    );
    assert_eq!(std::fs::read(&source).unwrap(), before);
}

#[test]
fn markdown_presentation_formats_headings_lists_tables_and_code_without_altering_source() {
    if !html_available() {
        return;
    }
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("README.md");
    std::fs::write(&source, "# Formatted Markdown\n\n- First list item\n- Second list item\n\n| Name | Count |\n| --- | ---: |\n| Alpha | 3 |\n\n```rust\nlet value = \"<tag>\";\n```\n\n<script>document.write('NOT_EXECUTED')</script>\n").unwrap();
    let before = std::fs::read(&source).unwrap();
    let document = html::convert(&source, &AtomicBool::new(false)).unwrap();
    let Content::Pdf { pixels, .. } =
        pdf(&document.cached_path(), 1, &AtomicBool::new(false)).unwrap()
    else {
        panic!("formatted Markdown")
    };
    assert!(pixels.height > pixels.width);
    let output = Command::new("pdftotext")
        .arg(document.cached_path())
        .arg("-")
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&output.stdout)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    for expected in ["Formatted Markdown", "First list item", "Alpha", "<tag>"] {
        assert!(text.contains(expected), "{text}");
    }
    assert!(!text.contains("# Formatted Markdown") && !text.contains("NOT_EXECUTED"));
    assert_eq!(std::fs::read(&source).unwrap(), before);
}

#[test]
fn html_render_failures_use_a_short_error_instead_of_a_traceback() {
    const CHILD: &str = "LSE_TEST_HTML_RENDER_ERROR";
    if std::env::var_os(CHILD).is_none() {
        let fake = tempfile::tempdir().unwrap();
        std::fs::write(fake.path().join("weasyprint.py"),"class HTML:\n def __init__(self, *args, **kwargs): raise AssertionError('layout assertion')\ndef default_url_fetcher(*args, **kwargs): pass\n").unwrap();
        assert!(
            Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "preview::tests::html_render_failures_use_a_short_error_instead_of_a_traceback",
                    "--nocapture"
                ])
                .env(CHILD, "1")
                .env("PYTHONPATH", fake.path())
                .status()
                .unwrap()
                .success()
        );
        return;
    }
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("index.html");
    std::fs::write(&source, "<h1>content</h1>").unwrap();
    let error = html::convert(&source, &AtomicBool::new(false))
        .unwrap_err()
        .to_string();
    assert!(error.contains("código-fonte"), "{error}");
    assert!(
        !error.contains("Traceback")
            && !error.contains("AssertionError")
            && !error.contains("LSE_HTML"),
        "{error}"
    );
}

#[test]
fn missing_preview_tools_only_disable_their_corresponding_renderer() {
    const CHILD: &str = "LSE_TEST_NO_PREVIEW_TOOLS";
    if std::env::var_os(CHILD).is_some() {
        let root = tempfile::tempdir().unwrap();
        for extension in [
            "docx", "doc", "odt", "xlsx", "xls", "ods", "pptx", "ppt", "odp", "odg", "odf",
        ] {
            let source = root.path().join(format!("document.{extension}"));
            test_office(&source);
            assert_eq!(
                office::convert(&source, &AtomicBool::new(false))
                    .unwrap_err()
                    .kind(),
                io::ErrorKind::Unsupported
            );
            assert_eq!(
                office::thumbnail(&source, &source, &AtomicBool::new(false))
                    .unwrap_err()
                    .kind(),
                io::ErrorKind::Unsupported
            );
        }
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
        let markdown = root.path().join("README.md");
        std::fs::write(&markdown, "# source").unwrap();
        assert_eq!(
            html::convert(&markdown, &AtomicBool::new(false))
                .unwrap_err()
                .kind(),
            io::ErrorKind::Unsupported
        );
        assert!(matches!(
            load(&markdown, 1, &AtomicBool::new(false)).unwrap(),
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

#[test]
fn archives_use_header_listing_instead_of_text_probing() {
    for name in [
        "data.ZIP",
        "backup.tar.gz",
        "backup.tar.xz",
        "data.7z",
        "data.rar",
        "data.tgz",
        "data.zst",
    ] {
        assert_eq!(Kind::for_path(Path::new(name)), Kind::Archive);
    }
    // Office files remain formatted documents even though their containers are ZIP.
    assert_eq!(
        Kind::for_path(Path::new("data.docx")),
        Kind::Office(Family::Text)
    );
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join("data.zip");
    let bytes = include_bytes!("fixtures/two-pages.docx");
    std::fs::write(&file, bytes).unwrap();
    let Content::Archive(listing) = load(&file, 1, &AtomicBool::new(false)).unwrap() else {
        panic!("archive")
    };
    assert!(
        listing
            .entries
            .iter()
            .any(|entry| entry.path == "word/document.xml"
                && entry.size.is_some_and(|size| size > 0))
    );
    assert!(!listing.truncated);
    assert_eq!(std::fs::read(&file).unwrap(), bytes);
    assert_eq!(
        std::fs::read_dir(root.path()).unwrap().count(),
        1,
        "no extraction"
    );
}
