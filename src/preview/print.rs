//! Prepare printable content without changing the source or invoking application macros.
use super::*;
use crate::filesystem::Entry;
pub struct Prepared {
    pub path: PathBuf,
    _document: Option<Document>,
}
pub fn supported(entry: &Entry) -> bool {
    !entry.is_dir
        && entry.trash.is_none()
        && matches!(
            Kind::for_path(&entry.path),
            Kind::Image
                | Kind::Svg
                | Kind::Pdf
                | Kind::Office(_)
                | Kind::Html
                | Kind::Markdown
                | Kind::Text
        )
}
fn prepared(document: Document) -> Prepared {
    Prepared {
        path: document.path(),
        _document: Some(document),
    }
}
pub fn prepare(entry: &Entry, cancel: &AtomicBool) -> io::Result<Prepared> {
    if !supported(entry) {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            crate::i18n::tr("Este tipo de arquivo não oferece impressão."),
        ));
    }
    check(cancel)?;
    let meta = std::fs::metadata(&entry.path)?;
    if !meta.is_file() || meta.len() > 256 * 1024 * 1024 {
        return Err(io::Error::other(crate::i18n::tr(
            "O arquivo para impressão deve ser regular e ter até 256 MiB.",
        )));
    }
    match Kind::for_path(&entry.path) {
        Kind::Pdf => {
            let data = read_limited(&entry.path, 8, cancel)?;
            if !data.starts_with(b"%PDF-") {
                return Err(io::Error::other(crate::i18n::tr(
                    "O arquivo não contém um PDF válido.",
                )));
            }
            Ok(Prepared {
                path: entry.path.clone(),
                _document: None,
            })
        }
        Kind::Office(_) => office::convert(&entry.path, cancel).map(prepared),
        Kind::Html | Kind::Markdown => html::convert(&entry.path, cancel).map(prepared),
        Kind::Image | Kind::Svg | Kind::Text => {
            let work = tempfile::tempdir()?;
            let source = work.path().join("print.html");
            let body = if Kind::for_path(&entry.path) == Kind::Text {
                let bytes = read_limited(&entry.path, 1024 * 1024, cancel)?;
                if bytes.len() > 1024 * 1024 {
                    return Err(io::Error::other(crate::i18n::tr(
                        "A impressão de texto está limitada a 1 MiB.",
                    )));
                }
                let text = std::str::from_utf8(&bytes).map_err(|_| {
                    io::Error::other(crate::i18n::tr(
                        "A impressão de texto requer codificação UTF-8.",
                    ))
                })?;
                format!(
                    "<pre>{}</pre>",
                    text.replace('&', "&amp;")
                        .replace('<', "&lt;")
                        .replace('>', "&gt;")
                )
            } else {
                let image = crate::image_ops::printable_image(entry)?.thumbnail(3508, 3508);
                let mut output = std::fs::File::create(work.path().join("image.jpg"))?;
                image::codecs::jpeg::JpegEncoder::new_with_quality(&mut output, 90)
                    .encode_image(&image)
                    .map_err(io::Error::other)?;
                if output.metadata()?.len() > 16 * 1024 * 1024 {
                    return Err(io::Error::other(crate::i18n::tr(
                        "A imagem excede o limite de preparação para impressão.",
                    )));
                }
                "<img src='image.jpg'>".to_owned()
            };
            std::fs::write(
                &source,
                format!(
                    "<!doctype html><meta charset='utf-8'><style>@page{{size:A4;margin:15mm}}body{{margin:0;font:10pt monospace}}pre{{white-space:pre-wrap;overflow-wrap:anywhere}}img{{display:block;max-width:180mm;max-height:267mm;margin:auto}}</style>{body}"
                ),
            )?;
            check(cancel)?;
            html::convert(&source, cancel).map(prepared)
        }
        _ => unreachable!(),
    }
}
pub fn merge(documents: &[Prepared], cancel: &AtomicBool) -> io::Result<Prepared> {
    let work = Arc::new(tempfile::tempdir()?);
    let document = Document::new(work);
    command(
        Command::new("pdfunite")
            .args(documents.iter().map(|p| &p.path))
            .arg(document.path()),
        cancel,
    )
    .map_err(|error| {
        if error.kind() == io::ErrorKind::NotFound {
            io::Error::new(
                io::ErrorKind::Unsupported,
                crate::i18n::tr("A impressão de vários arquivos requer pdfunite (Poppler)."),
            )
        } else {
            error
        }
    })?;
    Ok(prepared(document))
}
