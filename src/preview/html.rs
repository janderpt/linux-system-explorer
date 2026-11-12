//! Static HTML/CSS and Markdown presentation; source stays available without this runtime.
use super::{Document, Kind, check, command, read_limited};
use std::{
    fs, io,
    path::Path,
    process::Command,
    sync::{Arc, atomic::AtomicBool},
};

pub(super) fn convert(path: &Path, cancel: &AtomicBool) -> io::Result<Document> {
    const MAX_HTML_BYTES: u64 = 8 * 1024 * 1024;
    let data = read_limited(path, MAX_HTML_BYTES, cancel)?;
    if data.len() as u64 > MAX_HTML_BYTES {
        return Err(io::Error::other(crate::i18n::tr(
            "O documento excede o limite de tamanho da pré-visualização.",
        )));
    }
    let work = Arc::new(tempfile::tempdir()?);
    let snapshot = work.path().join("source.html");
    if Kind::for_path(path) == Kind::Markdown {
        fs::write(&snapshot, markdown(&data))?;
    } else {
        fs::write(&snapshot, data)?;
    }
    let document = Document::new(work.clone());
    check(cancel)?;
    command(
        Command::new("python3")
            .arg("-c")
            .arg(include_str!("html_renderer.py"))
            .arg(&snapshot)
            .arg(path)
            .arg(document.path())
            .current_dir(work.path()),
        cancel,
    )
    .map_err(|error| {
        if error.kind() == io::ErrorKind::NotFound
            || error.to_string().contains("LSE_HTML_RUNTIME_MISSING")
        {
            io::Error::new(
                io::ErrorKind::Unsupported,
                crate::i18n::tr(
                    "A apresentação requer WeasyPrint. O código-fonte continua disponível.",
                ),
            )
        } else if error.to_string().contains("LSE_HTML_RENDER_FAILED")
            || fs::read_to_string(document.path().with_extension("error"))
                .is_ok_and(|code| code == "render-failed")
        {
            io::Error::other(crate::i18n::tr(if Kind::for_path(path) == Kind::Markdown {
                "Não foi possível apresentar este Markdown. O código-fonte continua disponível."
            } else {
                "Não foi possível apresentar este HTML. O código-fonte continua disponível."
            }))
        } else {
            error
        }
    })?;
    check(cancel)?;
    Ok(document)
}

fn markdown(data: &[u8]) -> String {
    use pulldown_cmark::{Options, Parser, html::push_html};
    let source = String::from_utf8_lossy(data);
    let options = Options::ENABLE_TABLES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_FOOTNOTES;
    let mut output = String::from(
        r#"<!doctype html><meta charset="utf-8"><style>
@page { size:A4; margin:18mm }
body { font-family:sans-serif; font-size:11pt; line-height:1.5; color:#222 }
h1,h2,h3 { line-height:1.2; break-after:avoid }
pre { white-space:pre-wrap; overflow-wrap:anywhere; padding:10px; background:#f1f3f5 }
code { font-family:monospace; font-size:0.9em }
table { border-collapse:collapse; width:100% }
th,td { border:1px solid #bbb; padding:5px 8px; text-align:left }
th { background:#edf0f4 }
blockquote { border-left:3px solid #ccc; margin-left:0; padding-left:1em; color:#555 }
img { max-width:100%; height:auto }
</style><main>"#,
    );
    push_html(&mut output, Parser::new_ext(&source, options));
    output.push_str("</main>");
    output
}
