use super::*;
use std::{
    fs,
    os::unix::{ffi::OsStringExt, fs::symlink},
};
fn fixture(root: &Path, name: &std::ffi::OsStr) -> Entry {
    let path = root.join(name);
    let pixels = image::RgbaImage::from_fn(8, 4, |x, y| {
        image::Rgba([x as u8 * 25, y as u8 * 50, 70, if x == 0 { 0 } else { 255 }])
    });
    pixels.save_with_format(&path, ImageFormat::Png).unwrap();
    crate::filesystem::read_entry(&path).unwrap()
}
fn execute(entry: &Entry, operation: Operation, format: Option<Format>) -> PathBuf {
    let report = run(
        std::slice::from_ref(entry),
        operation,
        format,
        95,
        &AtomicBool::new(false),
        |_, _| {},
    );
    assert!(report.errors.is_empty(), "{:?}", report.errors);
    assert_eq!(report.created.len(), 1);
    report.created[0].clone()
}
#[test]
fn rotations_resize_and_all_output_formats_create_valid_copies_without_modifying_original() {
    let root = tempfile::tempdir().unwrap();
    let entry = fixture(root.path(), std::ffi::OsStr::new("source.png"));
    let before = fs::read(&entry.path).unwrap();
    let original = image::open(&entry.path).unwrap().to_rgba8();
    for rotation in [Rotation::Right, Rotation::Left, Rotation::Half] {
        let out = execute(&entry, Operation::Rotate(rotation), None);
        let transformed = image::open(out).unwrap().to_rgba8();
        let expected = match rotation {
            Rotation::Right => image::imageops::rotate90(&original),
            Rotation::Left => image::imageops::rotate270(&original),
            Rotation::Half => image::imageops::rotate180(&original),
        };
        assert_eq!(transformed, expected);
    }
    for proportional in [true, false] {
        let out = execute(
            &entry,
            Operation::Resize {
                width: 4,
                height: 4,
                proportional,
            },
            None,
        );
        let output = image::open(out).unwrap();
        assert_eq!(
            (output.width(), output.height()),
            if proportional { (4, 2) } else { (4, 4) }
        );
    }
    for format in Format::ALL {
        let out = execute(&entry, Operation::Convert, Some(format));
        let decoder = reader(&fs::read(&out).unwrap()).unwrap().format();
        assert_eq!(decoder, Some(format.image_format()));
        let output = image::open(out).unwrap();
        assert_eq!((output.width(), output.height()), (8, 4));
        if format == Format::Jpeg {
            let pixel = output.to_rgb8().get_pixel(0, 0).0;
            assert!(
                pixel.iter().all(|&c| c > 225),
                "JPEG preenche transparência com branco: {pixel:?}"
            );
        } else if format == Format::Png {
            assert_eq!(output.to_rgba8().get_pixel(0, 0)[3], 0);
        }
    }
    assert_eq!(fs::read(&entry.path).unwrap(), before);
}
#[test]
fn conflicts_symlinks_raw_names_and_concurrent_copies_never_replace_any_existing_path() {
    let root = tempfile::tempdir().unwrap();
    let entry = fixture(root.path(), std::ffi::OsStr::new("source.png"));
    let op = Operation::Convert;
    let first = root.path().join(suggested_name(&entry, op, Format::Png, 1));
    let second = root.path().join(suggested_name(&entry, op, Format::Png, 2));
    fs::write(&first, "existing").unwrap();
    symlink(&entry.path, &second).unwrap();
    let handles: Vec<_> = (0..2)
        .map(|_| {
            let entry = entry.clone();
            std::thread::spawn(move || execute(&entry, op, Some(Format::Png)))
        })
        .collect();
    let a = handles
        .into_iter()
        .map(|h| h.join().unwrap())
        .collect::<Vec<_>>();
    assert_ne!(a[0], a[1]);
    assert_eq!(fs::read_to_string(first).unwrap(), "existing");
    assert!(fs::symlink_metadata(second).unwrap().is_symlink());
    let raw = fixture(root.path(), &OsString::from_vec(b"raw-\xff.png".to_vec()));
    let result = execute(&raw, op, Some(Format::Png));
    assert!(
        result
            .file_name()
            .unwrap()
            .as_encoded_bytes()
            .starts_with(b"raw-\xff")
    );
    assert!(
        !fs::read_dir(root.path())
            .unwrap()
            .flatten()
            .any(|e| e.file_name().as_encoded_bytes().starts_with(b".lse-image-"))
    );
}
#[test]
fn invalid_dimensions_corrupt_images_and_cancellation_leave_no_incomplete_outputs() {
    let root = tempfile::tempdir().unwrap();
    let entry = fixture(root.path(), std::ffi::OsStr::new("source.png"));
    let before = fs::read(&entry.path).unwrap();
    for (w, h) in [(0, 1), (16385, 1), (10000, 10000)] {
        let report = run(
            std::slice::from_ref(&entry),
            Operation::Resize {
                width: w,
                height: h,
                proportional: false,
            },
            None,
            90,
            &AtomicBool::new(false),
            |_, _| {},
        );
        assert!(report.created.is_empty());
        assert_eq!(report.errors.len(), 1);
    }
    let token = AtomicBool::new(true);
    let report = run(
        std::slice::from_ref(&entry),
        Operation::Convert,
        None,
        90,
        &token,
        |_, _| {},
    );
    assert!(report.cancelled && report.created.is_empty());
    token.store(false, Ordering::Release);
    let report = run(
        &[entry.clone(), entry.clone()],
        Operation::Convert,
        None,
        90,
        &token,
        |_, _| token.store(true, Ordering::Release),
    );
    assert_eq!(report.created.len(), 1);
    assert!(report.cancelled);
    let corrupt = root.path().join("corrupt.png");
    fs::write(&corrupt, b"not an image").unwrap();
    assert!(inspect(&crate::filesystem::read_entry(&corrupt).unwrap()).is_err());
    assert_eq!(fs::read(&entry.path).unwrap(), before);
    assert!(
        !fs::read_dir(root.path())
            .unwrap()
            .flatten()
            .any(|e| e.file_name().as_encoded_bytes().starts_with(b".lse-image-"))
    );
}
#[test]
fn animated_gif_and_multipage_tiff_are_rejected_instead_of_silently_losing_frames_or_pages() {
    let root = tempfile::tempdir().unwrap();
    let gif = root.path().join("animated.gif");
    {
        let mut encoder = image::codecs::gif::GifEncoder::new(fs::File::create(&gif).unwrap());
        for _ in 0..2 {
            encoder
                .encode_frame(image::Frame::new(image::RgbaImage::from_pixel(
                    2,
                    2,
                    image::Rgba([2, 3, 4, 255]),
                )))
                .unwrap();
        }
    }
    let tiff = root.path().join("pages.tiff");
    {
        let mut encoder =
            tiff::encoder::TiffEncoder::new(fs::File::create(&tiff).unwrap()).unwrap();
        for _ in 0..2 {
            encoder
                .write_image::<tiff::encoder::colortype::RGB8>(2, 1, &[1, 2, 3, 4, 5, 6])
                .unwrap();
        }
    }
    for path in [gif, tiff] {
        let before = fs::read(&path).unwrap();
        let entry = crate::filesystem::read_entry(&path).unwrap();
        assert!(
            inspect(&entry)
                .unwrap_err()
                .to_string()
                .contains("animadas")
        );
        let report = run(
            &[entry],
            Operation::Convert,
            Some(Format::Png),
            90,
            &AtomicBool::new(false),
            |_, _| {},
        );
        assert!(report.created.is_empty());
        assert_eq!(fs::read(path).unwrap(), before);
    }
}
#[test]
fn svg_is_rasterized_and_jpeg_exif_orientation_is_applied_once_before_transforming() {
    let root = tempfile::tempdir().unwrap();
    let svg = root.path().join("drawing.svg");
    fs::write(&svg,"<svg xmlns='http://www.w3.org/2000/svg' width='10' height='20'><rect width='10' height='20' fill='red'/></svg>").unwrap();
    let entry = crate::filesystem::read_entry(&svg).unwrap();
    let output = image::open(execute(&entry, Operation::Rotate(Rotation::Right), None)).unwrap();
    assert_eq!((output.width(), output.height()), (20, 10));
    let jpeg = root.path().join("orientation.jpg");
    {
        let mut encoder = image::codecs::jpeg::JpegEncoder::new(fs::File::create(&jpeg).unwrap());
        image::ImageEncoder::set_exif_metadata(
            &mut encoder,
            vec![
                b'I', b'I', 42, 0, 8, 0, 0, 0, 1, 0, 0x12, 1, 3, 0, 1, 0, 0, 0, 6, 0, 0, 0, 0, 0,
                0, 0,
            ],
        )
        .unwrap();
        encoder
            .encode_image(&DynamicImage::ImageRgb8(image::RgbImage::new(8, 4)))
            .unwrap();
    }
    let entry = crate::filesystem::read_entry(&jpeg).unwrap();
    let info = inspect(&entry).unwrap();
    assert_eq!((info.width, info.height), (4, 8));
    let out = execute(
        &entry,
        Operation::Rotate(Rotation::Right),
        Some(Format::Png),
    );
    let out = image::open(out).unwrap();
    assert_eq!((out.width(), out.height()), (8, 4));
}
