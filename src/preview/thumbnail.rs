//! Small, read-only thumbnails. Two decoders at most across all windows/tabs.
use super::*;
use std::os::unix::fs::MetadataExt;
use std::sync::atomic::AtomicUsize;

pub const EDGE: u32 = 256;
static ACTIVE: AtomicUsize = AtomicUsize::new(0);
static OFFICE_ACTIVE: AtomicBool = AtomicBool::new(false);
struct Slot {
    office: bool,
}
impl Slot {
    fn reserve(kind: Kind) -> Option<Self> {
        let office = matches!(kind, Kind::Office(_));
        if office
            && OFFICE_ACTIVE
                .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                .is_err()
        {
            return None;
        }
        let mut active = ACTIVE.load(Ordering::Acquire);
        while active < 2 {
            match ACTIVE.compare_exchange_weak(
                active,
                active + 1,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => return Some(Self { office }),
                Err(current) => active = current,
            }
        }
        if office {
            OFFICE_ACTIVE.store(false, Ordering::Release);
        }
        None
    }
}
impl Drop for Slot {
    fn drop(&mut self) {
        ACTIVE.fetch_sub(1, Ordering::Release);
        if self.office {
            OFFICE_ACTIVE.store(false, Ordering::Release);
        }
    }
}
pub fn supported(kind: Kind) -> bool {
    matches!(
        kind,
        Kind::Image | Kind::Svg | Kind::Pdf | Kind::Video | Kind::Office(_)
    )
}
pub fn start(
    path: PathBuf,
    display_path: PathBuf,
    kind: Kind,
    ctx: &eframe::egui::Context,
) -> Option<Job> {
    if !supported(kind) {
        return None;
    }
    let slot = Slot::reserve(kind)?;
    Some(Job::spawn(ctx, move |cancel| {
        let _slot = slot;
        render(&path, &display_path, kind, cancel).map(Content::Image)
    }))
}
type Signature = (u64, u64, u64, i64, i64, i64, i64);
fn signature(path: &Path) -> io::Result<Signature> {
    let m = path.metadata()?;
    if !m.is_file() {
        return Err(io::Error::from(io::ErrorKind::InvalidInput));
    }
    Ok((
        m.dev(),
        m.ino(),
        m.len(),
        m.mtime(),
        m.mtime_nsec(),
        m.ctime(),
        m.ctime_nsec(),
    ))
}
fn render(path: &Path, display_path: &Path, kind: Kind, cancel: &AtomicBool) -> io::Result<Pixels> {
    let before = signature(path)?;
    check(cancel)?;
    let frame = match kind {
        Kind::Image | Kind::Svg => {
            let limit = if kind == Kind::Svg {
                2 * 1024 * 1024
            } else {
                IMAGE_BYTES
            };
            let data = read_limited(path, limit, cancel)?;
            if data.len() as u64 > limit {
                return Err(io::Error::from(io::ErrorKind::InvalidData));
            }
            if kind == Kind::Svg {
                svg(&data)?
            } else {
                pixels(&data)?
            }
        }
        Kind::Pdf => pdf_first_page(path, cancel)?,
        Kind::Video => media::thumbnail(path, cancel)?,
        Kind::Office(_) => {
            let document = office::thumbnail(path, display_path, cancel)?;
            pdf_first_page(&document.path(), cancel)?
        }
        _ => return Err(io::Error::from(io::ErrorKind::Unsupported)),
    };
    check(cancel)?;
    if signature(path)? != before {
        return Err(io::Error::from(io::ErrorKind::Interrupted));
    }
    let image = image::RgbaImage::from_raw(frame.width, frame.height, frame.rgba)
        .ok_or_else(|| io::Error::from(io::ErrorKind::InvalidData))?;
    let image = if image.width() > EDGE || image.height() > EDGE {
        image::DynamicImage::ImageRgba8(image)
            .thumbnail(EDGE, EDGE)
            .to_rgba8()
    } else {
        image
    };
    Ok(Pixels {
        width: image.width(),
        height: image.height(),
        rgba: image.into_raw(),
    })
}

fn pdf_first_page(path: &Path, cancel: &AtomicBool) -> io::Result<Pixels> {
    let work = tempfile::tempdir()?;
    let output = work.path().join("first");
    pdf_command(
        Command::new("pdftoppm")
            .args([
                "-f",
                "1",
                "-l",
                "1",
                "-singlefile",
                "-scale-to",
                &EDGE.to_string(),
                "-png",
            ])
            .arg(path)
            .arg(&output),
        cancel,
    )?;
    pixels(&read_limited(
        &output.with_extension("png"),
        IMAGE_BYTES,
        cancel,
    )?)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn image_svg_and_pdf_thumbnails_are_bounded_and_never_change_the_source() {
        let root = tempfile::tempdir().unwrap();
        let image = root.path().join("image.png");
        image::RgbaImage::from_pixel(800, 400, image::Rgba([10, 50, 90, 255]))
            .save(&image)
            .unwrap();
        let svg = root.path().join("drawing.svg");
        std::fs::write(&svg,r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="200"><rect width="100" height="200" fill="red"/></svg>"#).unwrap();
        let pdf = root.path().join("document.pdf");
        crate::preview::tests::test_pdf(&pdf);
        for path in [image, svg, pdf] {
            let before = std::fs::read(&path).unwrap();
            let frame =
                render(&path, &path, Kind::for_path(&path), &AtomicBool::new(false)).unwrap();
            assert!(frame.width <= EDGE && frame.height <= EDGE);
            assert!(frame.width > 0 && frame.height > 0);
            assert_eq!(before, std::fs::read(&path).unwrap());
        }
    }
    #[test]
    fn binary_folders_corrupt_images_and_cancellation_do_not_produce_thumbnails() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("broken.png");
        std::fs::write(&path, b"invalid").unwrap();
        assert!(render(&path, &path, Kind::Image, &AtomicBool::new(false)).is_err());
        assert!(
            render(
                root.path(),
                root.path(),
                Kind::Image,
                &AtomicBool::new(false)
            )
            .is_err()
        );
        assert_eq!(
            render(&path, &path, Kind::Image, &AtomicBool::new(true))
                .err()
                .unwrap()
                .kind(),
            io::ErrorKind::Interrupted
        );
        assert!(!supported(Kind::Audio));
        assert!(!supported(Kind::Unknown));
    }

    #[test]
    fn office_thumbnails_export_only_the_first_page_and_preserve_all_document_families() {
        if !crate::preview::tests::office_available() {
            return;
        }
        let root = tempfile::tempdir().unwrap();
        let cancel = AtomicBool::new(false);
        for extension in [
            "docx", "doc", "odt", "xlsx", "xls", "ods", "pptx", "ppt", "odp", "odg", "odf",
        ] {
            let path = root.path().join(format!("original.{extension}"));
            crate::preview::tests::test_office(&path);
            let before = std::fs::read(&path).unwrap();
            let document = office::thumbnail(&path, &path, &cancel).unwrap();
            let pdf = document.path();
            let info = Command::new("pdfinfo").arg(&pdf).output().unwrap();
            assert!(info.status.success());
            let info = String::from_utf8_lossy(&info.stdout);
            assert!(
                info.lines().any(|line| {
                    line.strip_prefix("Pages:")
                        .is_some_and(|count| count.trim() == "1")
                }),
                "{extension}: {info}"
            );
            let frame = pdf_first_page(&pdf, &cancel).unwrap();
            assert!(frame.width > 0 && frame.height > 0);
            assert!(frame.width <= EDGE && frame.height <= EDGE, "{extension}");
            assert_eq!(std::fs::read(&path).unwrap(), before);
            drop(document);
            assert!(
                !pdf.exists(),
                "temporary PDF is released after rasterization"
            );
        }
        // Trash storage may not retain the source extension; use its display name.
        let path = root.path().join("trashed-file");
        crate::preview::tests::test_docx(&path);
        let display = Path::new("original.docx");
        let kind = Kind::for_path(display);
        let frame = render(&path, display, kind, &cancel).unwrap();
        assert!(frame.height > frame.width && frame.height <= EDGE);
        assert_eq!(
            render(&path, display, kind, &AtomicBool::new(true))
                .err()
                .unwrap()
                .kind(),
            io::ErrorKind::Interrupted
        );
    }
}
