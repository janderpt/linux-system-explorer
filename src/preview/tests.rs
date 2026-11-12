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
