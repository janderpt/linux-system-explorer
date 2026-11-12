//! Read-only preview engines. GUI state lives separately in app::preview.
use std::{
    fs::File,
    io::{self, Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};
pub mod archive;
mod html;
pub mod media;
mod office;
pub mod thumbnail;
pub use office::{Document, Family};

const IMAGE_BYTES: u64 = 64 * 1024 * 1024;
const TEXT_BYTES: u64 = 128 * 1024;
pub const MAX_DIMENSION: u32 = 1600;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Image,
    Svg,
    Pdf,
    Archive,
    Office(Family),
    Html,
    Markdown,
    Text,
    Audio,
    Video,
    Unknown,
}
impl Kind {
    pub fn for_path(path: &Path) -> Self {
        match path
            .extension()
            .and_then(|x| x.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase()
            .as_str()
        {
            "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "tif" | "tiff" | "ico" => Self::Image,
            "zip" | "zipx" | "tar" | "gz" | "bz2" | "xz" | "zst" | "lz4" | "lzma" | "lz"
            | "tgz" | "tbz" | "tbz2" | "txz" | "tzst" | "7z" | "rar" | "cab" | "jar" | "war"
            | "apk" => Self::Archive,
            "svg" => Self::Svg,
            "pdf" => Self::Pdf,
            "html" | "htm" | "xhtml" => Self::Html,
            "md" | "markdown" => Self::Markdown,
            "mp3" | "wav" | "flac" | "ogg" | "oga" | "opus" | "m4a" | "aac" | "wma" | "aif"
            | "aiff" => Self::Audio,
            "mp4" | "mkv" | "webm" | "avi" | "mov" | "m4v" | "mpg" | "mpeg" | "ogv" | "wmv"
            | "flv" | "3gp" => Self::Video,
            "txt" | "log" | "csv" | "tsv" | "rs" | "py" | "js" | "ts" | "jsx" | "tsx" | "c"
            | "cpp" | "h" | "hpp" | "java" | "go" | "sh" | "json" | "xml" | "css" | "toml"
            | "yaml" | "yml" | "ini" | "conf" | "sql" | "rst" => Self::Text,
            extension => Family::for_extension(extension)
                .map(Self::Office)
                .unwrap_or(Self::Unknown),
        }
    }
}

pub struct Pixels {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}
pub enum Content {
    Archive(archive::Listing),
    Image(Pixels),
    Pdf {
        pixels: Pixels,
        page: u32,
        pages: u32,
        document: Option<Document>,
    },
    Text {
        text: String,
        truncated: bool,
    },
    Unsupported,
}

pub struct Job {
    result: mpsc::Receiver<io::Result<Content>>,
    cancelled: Arc<AtomicBool>,
}
impl Job {
    #[cfg(test)]
    pub(crate) fn pending() -> (Self, mpsc::Sender<io::Result<Content>>) {
        let (send, result) = mpsc::channel();
        (
            Self {
                result,
                cancelled: Arc::new(AtomicBool::new(false)),
            },
            send,
        )
    }
    pub fn start(path: PathBuf, page: u32, ctx: &eframe::egui::Context) -> Self {
        Self::spawn(ctx, move |cancel| load(&path, page, cancel))
    }
    pub fn start_document(document: Document, page: u32, ctx: &eframe::egui::Context) -> Self {
        // The closure owns the cache until the converter/page read exits.
        Self::spawn(ctx, move |cancel| pdf(&document.path(), page, cancel))
    }
    pub fn start_markup(path: PathBuf, ctx: &eframe::egui::Context) -> Self {
        Self::spawn(ctx, move |cancel| {
            converted_content(html::convert(&path, cancel)?, 1, cancel)
        })
    }
    fn spawn(
        ctx: &eframe::egui::Context,
        render: impl FnOnce(&AtomicBool) -> io::Result<Content> + Send + 'static,
    ) -> Self {
        let (send, result) = mpsc::channel();
        let cancelled = Arc::new(AtomicBool::new(false));
        let cancel = cancelled.clone();
        let context = ctx.clone();
        let locale = crate::i18n::current();
        std::thread::spawn(move || {
            crate::i18n::set_locale(locale);
            let value = render(&cancel);
            if !cancel.load(Ordering::Acquire) {
                let _ = send.send(value);
                context.request_repaint();
            }
        });
        Self { result, cancelled }
    }
    pub fn poll(&self) -> Option<io::Result<Content>> {
        match self.result.try_recv() {
            Ok(result) => Some(result),
            Err(mpsc::TryRecvError::Empty) => None,
            Err(mpsc::TryRecvError::Disconnected) => Some(Err(io::Error::other(crate::i18n::tr(
                "A pré-visualização foi interrompida.",
            )))),
        }
    }
}
impl Drop for Job {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Release);
    }
}

fn check(cancel: &AtomicBool) -> io::Result<()> {
    if cancel.load(Ordering::Acquire) {
        Err(io::Error::from(io::ErrorKind::Interrupted))
    } else {
        Ok(())
    }
}
fn regular(path: &Path) -> io::Result<()> {
    if !path.metadata()?.is_file() {
        return Err(io::Error::other(crate::i18n::tr(
            "Selecione um arquivo regular para pré-visualizar.",
        )));
    }
    Ok(())
}
fn read_limited(path: &Path, limit: u64, cancel: &AtomicBool) -> io::Result<Vec<u8>> {
    regular(path)?;
    let source = File::from(rustix::fs::open(
        path,
        rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::NONBLOCK | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::empty(),
    )?);
    // Recheck the opened object: a regular path may have been replaced after stat.
    if !source.metadata()?.is_file() {
        return Err(io::Error::from(io::ErrorKind::InvalidInput));
    }
    let mut file = source.take(limit + 1);
    let mut data = Vec::new();
    let mut chunk = [0_u8; 16384];
    loop {
        check(cancel)?;
        let size = file.read(&mut chunk)?;
        if size == 0 {
            break;
        }
        data.extend_from_slice(&chunk[..size]);
    }
    Ok(data)
}
fn pixels(data: &[u8]) -> io::Result<Pixels> {
    let mut reader = image::ImageReader::new(io::Cursor::new(data)).with_guessed_format()?;
    let mut limits = image::Limits::default();
    limits.max_alloc = Some(128 * 1024 * 1024);
    limits.max_image_width = Some(16384);
    limits.max_image_height = Some(16384);
    reader.limits(limits);
    let image = reader.decode().map_err(io::Error::other)?;
    let image = if image.width() > MAX_DIMENSION || image.height() > MAX_DIMENSION {
        image.thumbnail(MAX_DIMENSION, MAX_DIMENSION)
    } else {
        image
    }
    .to_rgba8();
    Ok(Pixels {
        width: image.width(),
        height: image.height(),
        rgba: image.into_raw(),
    })
}
fn load(path: &Path, page: u32, cancel: &AtomicBool) -> io::Result<Content> {
    regular(path)?;
    check(cancel)?;
    match Kind::for_path(path) {
        Kind::Pdf => pdf(path, page, cancel),
        Kind::Archive => archive::load(path, cancel),
        Kind::Office(_) => converted_content(office::convert(path, cancel)?, page, cancel),
        Kind::Image | Kind::Svg => {
            let data = read_limited(path, IMAGE_BYTES, cancel)?;
            if data.len() as u64 > IMAGE_BYTES {
                return Err(io::Error::other(crate::i18n::tr(
                    "A imagem excede o limite de tamanho da pré-visualização.",
                )));
            }
            let image = if Kind::for_path(path) == Kind::Svg {
                svg(&data)?
            } else {
                pixels(&data)?
            };
            check(cancel)?;
            Ok(Content::Image(image))
        }
        Kind::Text | Kind::Html | Kind::Markdown | Kind::Unknown => {
            let data = read_limited(path, TEXT_BYTES, cancel)?;
            if data.contains(&0)
                || Kind::for_path(path) == Kind::Unknown && std::str::from_utf8(&data).is_err()
            {
                return Ok(Content::Unsupported);
            }
            let truncated = data.len() as u64 > TEXT_BYTES;
            let data = &data[..data.len().min(TEXT_BYTES as usize)];
            Ok(Content::Text {
                text: String::from_utf8_lossy(data).into_owned(),
                truncated,
            })
        }
        _ => Ok(Content::Unsupported),
    }
}
fn converted_content(converted: Document, page: u32, cancel: &AtomicBool) -> io::Result<Content> {
    let mut content = pdf(&converted.path(), page, cancel)?;
    if let Content::Pdf { document, .. } = &mut content {
        *document = Some(converted);
    }
    Ok(content)
}
fn svg(data: &[u8]) -> io::Result<Pixels> {
    let mut options = resvg::usvg::Options::default();
    // Reuse the application's bundled text fonts rather than depending on a
    // particular desktop's installed font collection.
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
    // A preview must not resolve external paths referenced by an SVG.
    options.image_href_resolver.resolve_string = Box::new(|_, _| None);
    let tree = resvg::usvg::Tree::from_data(data, &options).map_err(io::Error::other)?;
    let size = tree.size();
    let scale = (MAX_DIMENSION as f32 / size.width().max(size.height())).min(1.0);
    let width = (size.width() * scale).ceil().max(1.0) as u32;
    let height = (size.height() * scale).ceil().max(1.0) as u32;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(width, height)
        .ok_or_else(|| io::Error::other("Invalid SVG dimensions"))?;
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_scale(scale, scale),
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
    Ok(Pixels {
        width,
        height,
        rgba,
    })
}
fn command(command: &mut Command, cancel: &AtomicBool) -> io::Result<Vec<u8>> {
    command_with_timeout(command, cancel, Duration::from_secs(25))
}
fn command_with_timeout(
    command: &mut Command,
    cancel: &AtomicBool,
    timeout: Duration,
) -> io::Result<Vec<u8>> {
    use std::os::unix::process::CommandExt;
    let mut out = tempfile::tempfile()?;
    let mut err = tempfile::tempfile()?;
    let mut child = command
        .process_group(0)
        .stdin(Stdio::null())
        .stdout(out.try_clone()?)
        .stderr(err.try_clone()?)
        .env("LC_ALL", "C")
        .spawn()?;
    let deadline = Instant::now() + timeout;
    loop {
        if cancel.load(Ordering::Acquire) || Instant::now() >= deadline {
            if let Some(pid) = rustix::process::Pid::from_raw(child.id() as i32) {
                let _ = rustix::process::kill_process_group(pid, rustix::process::Signal::KILL);
            }
            let _ = child.kill();
            let _ = child.wait();
            check(cancel)?;
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                crate::i18n::tr("A pré-visualização excedeu o tempo limite."),
            ));
        }
        if let Some(status) = child.try_wait()? {
            if !status.success() {
                err.seek(SeekFrom::Start(0))?;
                let mut text = String::new();
                err.take(4096).read_to_string(&mut text)?;
                return Err(io::Error::other(text.trim().to_owned()));
            }
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    out.seek(SeekFrom::Start(0))?;
    let mut bytes = Vec::new();
    out.take(64 * 1024).read_to_end(&mut bytes)?;
    Ok(bytes)
}
fn pdf(path: &Path, page: u32, cancel: &AtomicBool) -> io::Result<Content> {
    let info = pdf_command(Command::new("pdfinfo").arg(path), cancel)?;
    let pages = String::from_utf8_lossy(&info)
        .lines()
        .find_map(|line| {
            line.strip_prefix("Pages:")
                .and_then(|n| n.trim().parse::<u32>().ok())
        })
        .filter(|n| *n > 0)
        .ok_or_else(|| {
            io::Error::other(crate::i18n::tr(
                "Não foi possível determinar as páginas do PDF.",
            ))
        })?;
    let page = page.clamp(1, pages);
    let work = tempfile::tempdir()?;
    let prefix = work.path().join("page");
    pdf_command(
        Command::new("pdftoppm")
            .args([
                "-f",
                &page.to_string(),
                "-l",
                &page.to_string(),
                "-singlefile",
                "-scale-to",
                &MAX_DIMENSION.to_string(),
                "-png",
            ])
            .arg(path)
            .arg(&prefix),
        cancel,
    )?;
    let data = read_limited(&prefix.with_extension("png"), IMAGE_BYTES, cancel)?;
    Ok(Content::Pdf {
        pixels: pixels(&data)?,
        page,
        pages,
        document: None,
    })
}

fn pdf_command(program: &mut Command, cancel: &AtomicBool) -> io::Result<Vec<u8>> {
    command(program, cancel).map_err(|error| {
        if error.kind() == io::ErrorKind::NotFound {
            io::Error::new(
                io::ErrorKind::Unsupported,
                crate::i18n::tr("O preview de PDF requer Poppler (poppler-utils)."),
            )
        } else {
            error
        }
    })
}

#[cfg(test)]
pub(crate) mod tests;
