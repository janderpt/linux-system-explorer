//! New-image operations. Originals are opened read-only; publication never replaces a path.
use crate::filesystem::Entry;
use image::{AnimationDecoder, DynamicImage, ImageDecoder, ImageFormat};
use std::{
    ffi::OsString,
    fs,
    io::{self, Cursor, Read},
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};
const INPUT_LIMIT: u64 = 64 * 1024 * 1024;
const MAX_PIXELS: u64 = 32_000_000;
pub const MAX_EDGE: u32 = 16384;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Png,
    Jpeg,
    Webp,
    Bmp,
    Tiff,
}
impl Format {
    pub const ALL: [Self; 5] = [Self::Png, Self::Jpeg, Self::Webp, Self::Bmp, Self::Tiff];
    pub fn label(self) -> &'static str {
        match self {
            Self::Png => "PNG",
            Self::Jpeg => "JPEG",
            Self::Webp => "WebP",
            Self::Bmp => "BMP",
            Self::Tiff => "TIFF",
        }
    }
    pub fn extension(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpeg => "jpg",
            Self::Webp => "webp",
            Self::Bmp => "bmp",
            Self::Tiff => "tiff",
        }
    }
    fn image_format(self) -> ImageFormat {
        match self {
            Self::Png => ImageFormat::Png,
            Self::Jpeg => ImageFormat::Jpeg,
            Self::Webp => ImageFormat::WebP,
            Self::Bmp => ImageFormat::Bmp,
            Self::Tiff => ImageFormat::Tiff,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rotation {
    Right,
    Left,
    Half,
}
#[derive(Clone, Copy, Debug)]
pub enum Operation {
    Rotate(Rotation),
    Resize {
        width: u32,
        height: u32,
        proportional: bool,
    },
    Convert,
}
impl Operation {
    fn suffix(self) -> &'static str {
        crate::i18n::tr(match self {
            Self::Rotate(_) => " (girada)",
            Self::Resize { .. } => " (redimensionada)",
            Self::Convert => " (convertida)",
        })
    }
}
#[derive(Clone, Copy, Debug)]
pub struct Info {
    pub width: u32,
    pub height: u32,
    pub format: Format,
}
#[derive(Default)]
pub struct Report {
    pub created: Vec<PathBuf>,
    pub errors: Vec<String>,
    pub cancelled: bool,
}
pub fn supported(path: &Path) -> bool {
    matches!(
        crate::preview::Kind::for_path(path),
        crate::preview::Kind::Image | crate::preview::Kind::Svg
    )
}
fn error(text: &str) -> io::Error {
    io::Error::other(text.to_owned())
}
fn check(cancel: &AtomicBool) -> io::Result<()> {
    if cancel.load(Ordering::Acquire) {
        Err(io::ErrorKind::Interrupted.into())
    } else {
        Ok(())
    }
}
fn dimensions(w: u32, h: u32) -> io::Result<()> {
    if w == 0 || h == 0 || w > MAX_EDGE || h > MAX_EDGE || u64::from(w) * u64::from(h) > MAX_PIXELS
    {
        return Err(error(crate::i18n::tr(
            "A imagem deve ter até 32 milhões de pixels e 16384 pixels por lado.",
        )));
    }
    Ok(())
}
fn input(entry: &Entry) -> io::Result<Vec<u8>> {
    if entry.is_dir || entry.trash.is_some() || !supported(&entry.path) {
        return Err(error(crate::i18n::tr(
            "Selecione uma imagem compatível fora da lixeira.",
        )));
    }
    let meta = fs::symlink_metadata(&entry.path)?;
    if meta.ino() != entry.inode || meta.dev() != entry.device {
        return Err(error(crate::i18n::tr(
            "A imagem foi substituída. Atualize a pasta e tente novamente.",
        )));
    }
    let file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(rustix::fs::OFlags::NONBLOCK.bits() as i32)
        .open(&entry.path)?;
    let before = file.metadata()?;
    if !entry.is_symlink && (before.ino() != entry.inode || before.dev() != entry.device) {
        return Err(error(crate::i18n::tr(
            "A imagem foi substituída. Atualize a pasta e tente novamente.",
        )));
    }
    if !before.is_file() || before.len() > INPUT_LIMIT {
        return Err(error(crate::i18n::tr(
            "A imagem deve ser um arquivo regular de até 64 MiB.",
        )));
    }
    let mut bytes = Vec::new();
    (&file).take(INPUT_LIMIT + 1).read_to_end(&mut bytes)?;
    let after = file.metadata()?;
    if bytes.len() as u64 > INPUT_LIMIT
        || before.len() != after.len()
        || before.mtime() != after.mtime()
        || before.mtime_nsec() != after.mtime_nsec()
        || before.ctime() != after.ctime()
        || before.ctime_nsec() != after.ctime_nsec()
    {
        return Err(error(crate::i18n::tr(
            "A imagem mudou durante a leitura. Tente novamente.",
        )));
    }
    Ok(bytes)
}
fn reader(data: &[u8]) -> io::Result<image::ImageReader<Cursor<&[u8]>>> {
    let mut reader = image::ImageReader::new(Cursor::new(data)).with_guessed_format()?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(MAX_EDGE);
    limits.max_image_height = Some(MAX_EDGE);
    limits.max_alloc = Some(256 * 1024 * 1024);
    reader.limits(limits);
    Ok(reader)
}
fn static_image(data: &[u8], format: ImageFormat) -> io::Result<()> {
    let animated = match format {
        ImageFormat::Png => image::codecs::png::PngDecoder::new(Cursor::new(data))
            .map_err(io::Error::other)?
            .is_apng()
            .map_err(io::Error::other)?,
        ImageFormat::WebP => image::codecs::webp::WebPDecoder::new(Cursor::new(data))
            .map_err(io::Error::other)?
            .has_animation(),
        ImageFormat::Gif => {
            let mut decoder =
                image::codecs::gif::GifDecoder::new(Cursor::new(data)).map_err(io::Error::other)?;
            let mut limits = image::Limits::default();
            limits.max_alloc = Some(256 * 1024 * 1024);
            limits.max_image_width = Some(MAX_EDGE);
            limits.max_image_height = Some(MAX_EDGE);
            decoder.set_limits(limits).map_err(io::Error::other)?;
            let mut frames = decoder.into_frames();
            if let Some(frame) = frames.next() {
                frame.map_err(io::Error::other)?;
            }
            frames
                .next()
                .transpose()
                .map_err(io::Error::other)?
                .is_some()
        }
        ImageFormat::Tiff => tiff::decoder::Decoder::new(Cursor::new(data))
            .map_err(io::Error::other)?
            .more_images(),
        _ => false,
    };
    if animated {
        return Err(error(crate::i18n::tr(
            "Esta operação ainda não suporta imagens animadas ou TIFF com várias páginas. O original foi preservado.",
        )));
    }
    Ok(())
}
fn svg(data: &[u8]) -> io::Result<DynamicImage> {
    if data.len() > 2 * 1024 * 1024 {
        return Err(error(crate::i18n::tr("O SVG deve ter até 2 MiB.")));
    }
    let mut options = resvg::usvg::Options::default();
    for font in eframe::egui::FontDefinitions::default()
        .font_data
        .into_values()
    {
        options.fontdb_mut().load_font_data(font.font.to_vec());
    }
    options.font_family = "Ubuntu".into();
    options.fontdb_mut().set_serif_family("Ubuntu");
    options.fontdb_mut().set_sans_serif_family("Ubuntu");
    options.fontdb_mut().set_monospace_family("Hack");
    options.image_href_resolver.resolve_string = Box::new(|_, _| None);
    let tree = resvg::usvg::Tree::from_data(data, &options).map_err(io::Error::other)?;
    let size = tree.size().to_int_size();
    dimensions(size.width(), size.height())?;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(size.width(), size.height())
        .ok_or_else(|| error(crate::i18n::tr("Dimensões de imagem inválidas.")))?;
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::identity(),
        &mut pixmap.as_mut(),
    );
    let rgba = pixmap
        .pixels()
        .iter()
        .flat_map(|p| {
            let p = p.demultiply();
            [p.red(), p.green(), p.blue(), p.alpha()]
        })
        .collect();
    Ok(DynamicImage::ImageRgba8(
        image::RgbaImage::from_raw(size.width(), size.height(), rgba)
            .ok_or_else(|| error(crate::i18n::tr("Dimensões de imagem inválidas.")))?,
    ))
}
pub fn inspect(entry: &Entry) -> io::Result<Info> {
    let data = input(entry)?;
    if crate::preview::Kind::for_path(&entry.path) == crate::preview::Kind::Svg {
        let image = svg(&data)?;
        return Ok(Info {
            width: image.width(),
            height: image.height(),
            format: Format::Png,
        });
    }
    let reader = reader(&data)?;
    let format = reader
        .format()
        .ok_or_else(|| error(crate::i18n::tr("Formato de imagem não reconhecido.")))?;
    let mut decoder = reader.into_decoder().map_err(io::Error::other)?;
    let (mut width, mut height) = decoder.dimensions();
    dimensions(width, height)?;
    if decoder.orientation().map_err(io::Error::other)?.to_exif() >= 5 {
        std::mem::swap(&mut width, &mut height);
    }
    static_image(&data, format)?;
    let format = match format {
        ImageFormat::Jpeg => Format::Jpeg,
        ImageFormat::WebP => Format::Webp,
        ImageFormat::Bmp => Format::Bmp,
        ImageFormat::Tiff => Format::Tiff,
        _ => Format::Png,
    };
    Ok(Info {
        width,
        height,
        format,
    })
}
fn decode(entry: &Entry) -> io::Result<(DynamicImage, Option<Vec<u8>>)> {
    let data = input(entry)?;
    if crate::preview::Kind::for_path(&entry.path) == crate::preview::Kind::Svg {
        return svg(&data).map(|image| (image, None));
    }
    let reader = reader(&data)?;
    let format = reader
        .format()
        .ok_or_else(|| error(crate::i18n::tr("Formato de imagem não reconhecido.")))?;
    let mut decoder = reader.into_decoder().map_err(io::Error::other)?;
    let (w, h) = decoder.dimensions();
    dimensions(w, h)?;
    let orientation = decoder.orientation().map_err(io::Error::other)?;
    static_image(&data, format)?;
    let profile = decoder.icc_profile().map_err(io::Error::other)?;
    let mut image = DynamicImage::from_decoder(decoder).map_err(io::Error::other)?;
    image.apply_orientation(orientation);
    Ok((image, profile))
}
pub fn suggested_name(
    entry: &Entry,
    operation: Operation,
    format: Format,
    index: usize,
) -> OsString {
    let mut name = entry.path.file_stem().unwrap_or_default().to_owned();
    name.push(operation.suffix());
    if index > 1 {
        name.push(format!(" ({index})"));
    }
    name.push(".");
    name.push(format.extension());
    name
}
fn jpeg_background(image: DynamicImage) -> DynamicImage {
    let mut pixels = image.to_rgba8();
    for p in pixels.pixels_mut() {
        let a = u32::from(p[3]);
        for channel in &mut p.0[..3] {
            *channel = ((u32::from(*channel) * a + 255 * (255 - a) + 127) / 255) as u8;
        }
        p[3] = 255;
    }
    DynamicImage::ImageRgba8(pixels).into_rgb8().into()
}
fn create(
    entry: &Entry,
    operation: Operation,
    format: Format,
    quality: u8,
    cancel: &AtomicBool,
) -> io::Result<PathBuf> {
    check(cancel)?;
    if let Operation::Resize { width, height, .. } = operation {
        dimensions(width, height)?;
    }
    let (original, profile) = decode(entry)?;
    check(cancel)?;
    let result = match operation {
        Operation::Rotate(Rotation::Right) => original.rotate90(),
        Operation::Rotate(Rotation::Left) => original.rotate270(),
        Operation::Rotate(Rotation::Half) => original.rotate180(),
        Operation::Resize {
            width,
            height,
            proportional,
        } => {
            if proportional {
                original.resize(width, height, image::imageops::FilterType::Lanczos3)
            } else {
                original.resize_exact(width, height, image::imageops::FilterType::Lanczos3)
            }
        }
        Operation::Convert => original,
    };
    check(cancel)?;
    let parent = entry
        .path
        .parent()
        .ok_or_else(|| error(crate::i18n::tr("Destino inválido")))?;
    let mut temp = tempfile::Builder::new()
        .prefix(".lse-image-")
        .tempfile_in(parent)?;
    match format {
        Format::Jpeg => {
            let image = jpeg_background(result);
            let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(
                temp.as_file_mut(),
                quality.clamp(1, 100),
            );
            if let Some(profile) = profile {
                image::ImageEncoder::set_icc_profile(&mut encoder, profile)
                    .map_err(io::Error::other)?;
            }
            encoder.encode_image(&image).map_err(io::Error::other)?;
        }
        Format::Png => encode(
            &result,
            image::codecs::png::PngEncoder::new(temp.as_file_mut()),
            profile,
        )?,
        Format::Webp => encode(
            &DynamicImage::ImageRgba8(result.to_rgba8()),
            image::codecs::webp::WebPEncoder::new_lossless(temp.as_file_mut()),
            profile,
        )?,
        Format::Tiff => encode(
            &result,
            image::codecs::tiff::TiffEncoder::new(temp.as_file_mut()),
            profile,
        )?,
        Format::Bmp => DynamicImage::ImageRgba8(result.to_rgba8())
            .write_to(temp.as_file_mut(), format.image_format())
            .map_err(io::Error::other)?,
    }
    temp.as_file().sync_all()?;
    check(cancel)?;
    for index in 1..=10000 {
        check(cancel)?;
        let destination = parent.join(suggested_name(entry, operation, format, index));
        match temp.persist_noclobber(&destination) {
            Ok(_) => return Ok(destination),
            Err(error) if error.error.kind() == io::ErrorKind::AlreadyExists => temp = error.file,
            Err(error) => return Err(error.error),
        }
    }
    Err(error(crate::i18n::tr(
        "Não foi possível encontrar um nome livre para a cópia.",
    )))
}
fn encode(
    image: &DynamicImage,
    mut encoder: impl image::ImageEncoder,
    profile: Option<Vec<u8>>,
) -> io::Result<()> {
    if let Some(profile) = profile {
        encoder.set_icc_profile(profile).map_err(io::Error::other)?;
    }
    image.write_with_encoder(encoder).map_err(io::Error::other)
}
pub(crate) fn printable_image(entry: &Entry) -> io::Result<DynamicImage> {
    decode(entry).map(|image| jpeg_background(image.0))
}
pub fn run(
    entries: &[Entry],
    operation: Operation,
    format: Option<Format>,
    quality: u8,
    cancel: &AtomicBool,
    mut progress: impl FnMut(usize, usize),
) -> Report {
    let mut report = Report::default();
    for (index, entry) in entries.iter().enumerate() {
        if cancel.load(Ordering::Acquire) {
            report.cancelled = true;
            break;
        }
        let result = format
            .map(Ok)
            .unwrap_or_else(|| inspect(entry).map(|info| info.format))
            .and_then(|format| create(entry, operation, format, quality, cancel));
        match result {
            Ok(path) => report.created.push(path),
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {
                report.cancelled = true;
                break;
            }
            Err(error) => report.errors.push(format!("{}: {}", entry.name, error)),
        }
        progress(index + 1, entries.len());
    }
    report
}
#[cfg(test)]
mod tests;
