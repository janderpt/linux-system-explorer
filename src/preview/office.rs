//! Office previews use an isolated, disposable LibreOffice profile and source copy.
use super::{check, command_with_timeout, regular};
use std::{
    fs,
    io::{self, Read, Write},
    path::{Path, PathBuf},
    process::Command,
    sync::{Arc, atomic::AtomicBool},
    time::Duration,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Family {
    Text,
    Spreadsheet,
    Presentation,
    Drawing,
    Formula,
}
impl Family {
    pub(super) fn for_extension(extension: &str) -> Option<Self> {
        Some(match extension {
            "doc" | "docx" | "docm" | "dot" | "dotx" | "dotm" | "odt" | "ott" | "fodt" | "rtf" => {
                Self::Text
            }
            "xls" | "xlsx" | "xlsm" | "xlsb" | "xlt" | "xltx" | "xltm" | "ods" | "ots" | "fods" => {
                Self::Spreadsheet
            }
            "ppt" | "pptx" | "pptm" | "pot" | "potx" | "potm" | "pps" | "ppsx" | "ppsm" | "odp"
            | "otp" | "fodp" => Self::Presentation,
            "odg" | "otg" | "fodg" => Self::Drawing,
            "odf" => Self::Formula,
            _ => return None,
        })
    }
    fn filter(self) -> &'static str {
        match self {
            Self::Text => "pdf:writer_pdf_Export",
            Self::Spreadsheet => "pdf:calc_pdf_Export",
            Self::Presentation => "pdf:impress_pdf_Export",
            Self::Drawing => "pdf:draw_pdf_Export",
            Self::Formula => "pdf:math_pdf_Export",
        }
    }
    fn required(self) -> &'static str {
        match self {
            Self::Text => "O preview de documentos de texto requer LibreOffice Writer.",
            Self::Spreadsheet => "O preview de planilhas requer LibreOffice Calc.",
            Self::Presentation => "O preview de apresentações requer LibreOffice Impress.",
            Self::Drawing => "O preview de desenhos requer LibreOffice Draw.",
            Self::Formula => "O preview de fórmulas requer LibreOffice Math.",
        }
    }
}

const MAX_DOCUMENT_BYTES: u64 = 256 * 1024 * 1024;

fn too_large(limit: u64) -> io::Error {
    io::Error::other(crate::i18n::message(
        "O documento excede o limite de {} MiB da pré-visualização.",
        &[(limit / 1024 / 1024).to_string()],
    ))
}

fn copy_source(path: &Path, destination: &Path, limit: u64, cancel: &AtomicBool) -> io::Result<()> {
    regular(path)?;
    let input = fs::File::open(path)?;
    if input.metadata()?.len() > limit {
        return Err(too_large(limit));
    }
    let mut input = input.take(limit + 1);
    let mut output = fs::File::create(destination)?;
    let mut total = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        check(cancel)?;
        let bytes = input.read(&mut buffer)?;
        if bytes == 0 {
            break;
        }
        total += bytes as u64;
        if total > limit {
            return Err(too_large(limit));
        }
        output.write_all(&buffer[..bytes])?;
    }
    Ok(())
}

#[derive(Clone, Debug)]
pub struct Document {
    work: Arc<tempfile::TempDir>,
}
impl Document {
    pub(super) fn new(work: Arc<tempfile::TempDir>) -> Self {
        Self { work }
    }
    pub(super) fn path(&self) -> PathBuf {
        self.work.path().join("document.pdf")
    }
    #[cfg(test)]
    pub(crate) fn cached_path(&self) -> PathBuf {
        self.path()
    }
}

pub(super) fn convert(path: &Path, cancel: &AtomicBool) -> io::Result<Document> {
    let family = match super::Kind::for_path(path) {
        super::Kind::Office(family) => family,
        _ => {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "Unsupported Office format",
            ));
        }
    };
    let required = family.required();
    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let work = Arc::new(tempfile::tempdir()?);
    let profile = work.path().join("profile");
    let user = profile.join("user");
    fs::create_dir_all(&user)?;
    fs::write(
        user.join("registrymodifications.xcu"),
        r#"<?xml version="1.0" encoding="UTF-8"?>
<oor:items xmlns:oor="http://openoffice.org/2001/registry">
<item oor:path="/org.openoffice.Office.Common/Security/Scripting">
<prop oor:name="MacroSecurityLevel" oor:op="fuse"><value>3</value></prop>
<prop oor:name="DisableMacrosExecution" oor:op="fuse"><value>true</value></prop>
<prop oor:name="DisableActiveContent" oor:op="fuse"><value>true</value></prop>
<prop oor:name="BlockUntrustedRefererLinks" oor:op="fuse"><value>true</value></prop>
</item>
<item oor:path="/org.openoffice.Office.Writer/Content/Update">
<prop oor:name="Link" oor:op="fuse"><value>2</value></prop>
</item>
<item oor:path="/org.openoffice.Office.Calc/Content/Update">
<prop oor:name="Link" oor:op="fuse"><value>1</value></prop>
</item>
</oor:items>"#,
    )?;
    let source = work.path().join(format!("document.{extension}"));
    // Stream the private snapshot rather than allocating the whole presentation.
    copy_source(path, &source, MAX_DOCUMENT_BYTES, cancel)?;
    let profile = url::Url::from_directory_path(&profile)
        .map_err(|_| io::Error::other("Invalid LibreOffice profile path"))?;
    check(cancel)?;
    let mut installed = false;
    for executable in ["libreoffice", "soffice"] {
        let result = command_with_timeout(
            Command::new(executable)
                .arg(format!("-env:UserInstallation={profile}"))
                .args([
                    "--headless",
                    "--nologo",
                    "--nodefault",
                    "--norestore",
                    "--convert-to",
                ])
                .arg(family.filter())
                .arg("--outdir")
                .arg(work.path())
                .arg(&source)
                .env("SAL_USE_VCLPLUGIN", "svp"),
            cancel,
            Duration::from_secs(120),
        );
        match result {
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            other => {
                other?;
                installed = true;
                break;
            }
        }
    }
    if !installed {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            crate::i18n::tr(required),
        ));
    }
    check(cancel)?;
    let document = Document { work };
    if !document.path().is_file() {
        return Err(io::Error::other(crate::i18n::message(
            "Não foi possível converter o documento para pré-visualização. {}",
            &[crate::i18n::tr(required).to_owned()],
        )));
    }
    Ok(document)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::preview::Kind;

    #[test]
    fn legacy_opendocument_and_template_extensions_share_the_same_preview_family() {
        for (alias, modern) in [
            ("report.DOC", "report.docx"),
            ("text.ODT", "report.docx"),
            ("template.OTT", "report.docx"),
            ("flat.FODT", "report.docx"),
            ("budget.XLS", "budget.xlsx"),
            ("table.ODS", "budget.xlsx"),
            ("template.OTS", "budget.xlsx"),
            ("flat.FODS", "budget.xlsx"),
            ("slides.PPT", "slides.pptx"),
            ("slides.ODP", "slides.pptx"),
            ("template.OTP", "slides.pptx"),
            ("flat.FODP", "slides.pptx"),
        ] {
            let kind = Kind::for_path(Path::new(alias));
            assert!(matches!(kind, Kind::Office(_)));
            assert_eq!(kind, Kind::for_path(Path::new(modern)));
        }
        assert_eq!(Kind::for_path(Path::new("data.csv")), Kind::Text);
        assert_eq!(Kind::for_path(Path::new("index.html")), Kind::Html);
        assert_eq!(Kind::for_path(Path::new("data.unknown")), Kind::Unknown);
    }

    #[test]
    fn office_snapshots_stream_above_the_old_limit_and_still_enforce_size_and_cancellation() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("large.pptx");
        let mut file = fs::File::create(&source).unwrap();
        file.write_all(b"large-input").unwrap();
        let length = 65 * 1024 * 1024;
        file.set_len(length).unwrap();
        let copy = root.path().join("snapshot.pptx");
        copy_source(&source, &copy, MAX_DOCUMENT_BYTES, &AtomicBool::new(false)).unwrap();
        assert_eq!(fs::metadata(&copy).unwrap().len(), length);
        let mut header = [0; 11];
        fs::File::open(&copy)
            .unwrap()
            .read_exact(&mut header)
            .unwrap();
        assert_eq!(&header, b"large-input");
        assert_eq!(
            copy_source(
                &source,
                &root.path().join("cancelled"),
                MAX_DOCUMENT_BYTES,
                &AtomicBool::new(true)
            )
            .unwrap_err()
            .kind(),
            io::ErrorKind::Interrupted
        );
        file.set_len(MAX_DOCUMENT_BYTES + 1).unwrap();
        let rejected = root.path().join("too-large");
        assert!(
            copy_source(
                &source,
                &rejected,
                MAX_DOCUMENT_BYTES,
                &AtomicBool::new(false)
            )
            .is_err()
        );
        assert!(!rejected.exists());
    }
}
