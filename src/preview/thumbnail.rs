//! Small, read-only thumbnails. Two decoders at most across all windows/tabs.
use super::*;
use std::os::unix::fs::MetadataExt;
use std::sync::atomic::AtomicUsize;

pub const EDGE: u32 = 256;
static ACTIVE: AtomicUsize = AtomicUsize::new(0);
struct Slot;
impl Slot {
    fn reserve() -> Option<Self> {
        let mut active = ACTIVE.load(Ordering::Acquire);
        while active < 2 {
            match ACTIVE.compare_exchange_weak(
                active,
                active + 1,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => return Some(Self),
                Err(current) => active = current,
            }
        }
        None
    }
}
impl Drop for Slot {
    fn drop(&mut self) {
        ACTIVE.fetch_sub(1, Ordering::Release);
    }
}
pub fn supported(kind: Kind) -> bool {
    matches!(kind, Kind::Image | Kind::Svg | Kind::Pdf)
}
pub fn start(path: PathBuf, kind: Kind, ctx: &eframe::egui::Context) -> Option<Job> {
    if !supported(kind) {
        return None;
    }
    let slot = Slot::reserve()?;
    Some(Job::spawn(ctx, move |cancel| {
        let _slot = slot;
        render(&path, kind, cancel).map(Content::Image)
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
fn render(path: &Path, kind: Kind, cancel: &AtomicBool) -> io::Result<Pixels> {
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
        Kind::Pdf => {
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
            )?)?
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
            let frame = render(&path, Kind::for_path(&path), &AtomicBool::new(false)).unwrap();
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
        assert!(render(&path, Kind::Image, &AtomicBool::new(false)).is_err());
        assert!(render(root.path(), Kind::Image, &AtomicBool::new(false)).is_err());
        assert_eq!(
            render(&path, Kind::Image, &AtomicBool::new(true))
                .err()
                .unwrap()
                .kind(),
            io::ErrorKind::Interrupted
        );
        assert!(!supported(Kind::Office(Family::Text)));
        assert!(!supported(Kind::Video));
    }
}
