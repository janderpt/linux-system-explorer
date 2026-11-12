//! Archive operations delegated to installed desktop archivers, never a shell.
use super::*;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Extraction {
    Here,
    ChooseDestination,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Backend {
    Ark,
    FileRoller,
    Engrampa,
}
impl Backend {
    fn program(self) -> &'static str {
        match self {
            Self::Ark => "ark",
            Self::FileRoller => "file-roller",
            Self::Engrampa => "engrampa",
        }
    }
    fn command(self, mode: Extraction, paths: &[PathBuf]) -> Command {
        let mut command = Command::new(self.program());
        match (self, mode) {
            (Self::Ark, Extraction::Here) => {
                command.args(["--batch", "--autodestination", "--autosubfolder"]);
            }
            (Self::Ark, Extraction::ChooseDestination) => {
                command.args(["--batch", "--dialog"]);
            }
            (_, Extraction::Here) => {
                command.arg("--extract-here");
            }
            (_, Extraction::ChooseDestination) => {
                command.arg("--extract");
            }
        }
        command
            .arg("--")
            .args(paths)
            .current_dir(paths[0].parent().unwrap_or(Path::new("/")));
        command
    }
}
fn choose(desktop: &str, installed: impl Fn(&str) -> bool) -> Option<Backend> {
    let desktop = desktop
        .split(':')
        .find(|s| matches!(*s, "KDE" | "GNOME" | "MATE" | "XFCE"));
    let order = match desktop {
        Some("KDE") => [Backend::Ark, Backend::FileRoller, Backend::Engrampa],
        Some("MATE") => [Backend::Engrampa, Backend::FileRoller, Backend::Ark],
        _ => [Backend::FileRoller, Backend::Engrampa, Backend::Ark],
    };
    order.into_iter().find(|b| installed(b.program()))
}
pub fn supported(path: &Path) -> bool {
    crate::preview::Kind::for_path(path) == crate::preview::Kind::Archive
}
pub fn extract(paths: &[PathBuf], mode: Extraction) -> Result<(), String> {
    if crate::sandbox::active() {
        return Err(crate::i18n::tr("A extração pelo gerenciador do sistema ainda não está disponível no Flatpak experimental.").into());
    }
    if paths.is_empty()
        || paths
            .iter()
            .any(|p| !p.is_absolute() || !p.is_file() || !supported(p))
    {
        return Err(crate::i18n::tr(
            "Selecione apenas arquivos compactados existentes para extrair.",
        )
        .into());
    }
    let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
    let backend = choose(&desktop, |p| executable(Path::new(p))).ok_or_else(|| crate::i18n::tr("Nenhum gerenciador de compactados compatível foi encontrado. Instale o Ark, File Roller ou Engrampa.").to_owned())?;
    match run(&mut backend.command(mode, paths)) {
        Ok((status, _)) if status.success() => Ok(()),
        Ok((status, diagnostic)) => Err(crate::i18n::message(
            "O gerenciador de compactados retornou um erro: {}",
            &[if diagnostic.is_empty() {
                status.to_string()
            } else {
                diagnostic
            }],
        )),
        Err(error) => Err(crate::i18n::message(
            "Não foi possível abrir o gerenciador de compactados: {}",
            &[error.to_string()],
        )),
    }
}
#[cfg(test)]
mod tests;
