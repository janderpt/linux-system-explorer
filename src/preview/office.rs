//! DOCX conversion uses an isolated, disposable LibreOffice profile and source copy.
use super::{check, command, read_limited};
use std::{
    fs, io,
    path::{Path, PathBuf},
    process::Command,
    sync::{Arc, atomic::AtomicBool},
};

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
    const MAX_DOCUMENT_BYTES: u64 = 64 * 1024 * 1024;
    let data = read_limited(path, MAX_DOCUMENT_BYTES, cancel)?;
    if data.len() as u64 > MAX_DOCUMENT_BYTES {
        return Err(io::Error::other(crate::i18n::tr(
            "O documento excede o limite de tamanho da pré-visualização.",
        )));
    }
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
</oor:items>"#,
    )?;
    let source = work.path().join("document.docx");
    fs::write(&source, data)?;
    let profile = url::Url::from_directory_path(&profile)
        .map_err(|_| io::Error::other("Invalid LibreOffice profile path"))?;
    check(cancel)?;
    let mut installed = false;
    for executable in ["libreoffice", "soffice"] {
        let result = command(
            Command::new(executable)
                .arg(format!("-env:UserInstallation={profile}"))
                .args([
                    "--headless",
                    "--nologo",
                    "--nodefault",
                    "--norestore",
                    "--convert-to",
                    "pdf:writer_pdf_Export",
                    "--outdir",
                ])
                .arg(work.path())
                .arg(&source)
                .env("SAL_USE_VCLPLUGIN", "svp"),
            cancel,
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
            crate::i18n::tr("O preview de DOCX requer LibreOffice Writer."),
        ));
    }
    check(cancel)?;
    let document = Document { work };
    if !document.path().is_file() {
        return Err(io::Error::other(crate::i18n::tr(
            "Não foi possível converter o DOCX para pré-visualização.",
        )));
    }
    Ok(document)
}
