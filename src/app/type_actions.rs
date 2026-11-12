use super::*;
use crate::applications::archives::{self, Extraction};
impl Explorer {
    /// A type-specific operation applies to the entire selection, never just its first item.
    pub(super) fn selected_archives(&self, clicked: &Entry) -> Vec<PathBuf> {
        if self.is_trash()
            || self.is_applications()
            || clicked.is_dir
            || !archives::supported(&clicked.path)
        {
            return vec![];
        }
        if self.selected.is_empty() {
            return vec![clicked.path.clone()];
        }
        if !self.selected.contains(&clicked.path) {
            return vec![];
        }
        let mut paths = Vec::new();
        for entry in self
            .entries
            .iter()
            .filter(|e| self.selected.contains(&e.path))
        {
            if entry.is_dir || !archives::supported(&entry.path) {
                return vec![];
            }
            paths.push(entry.path.clone());
        }
        if paths.len() != self.selected.len() {
            return vec![];
        }
        paths
    }
    pub(super) fn type_actions_menu(&mut self, ui: &mut egui::Ui, entry: &Entry) {
        let paths = self.selected_archives(entry);
        if paths.is_empty() {
            return;
        }
        ui.separator();
        ui.label(
            RichText::new(crate::i18n::tr(if paths.len() == 1 {
                "Arquivo compactado"
            } else {
                "Arquivos compactados"
            }))
            .small()
            .weak(),
        );
        if self
            .menu_item(
                ui,
                Icon::Preview,
                crate::i18n::tr(if paths.len() == 1 {
                    "Ver conteúdo"
                } else {
                    "Ver conteúdo deste arquivo"
                }),
                crate::i18n::tr("Espaço"),
                true,
                false,
            )
            .clicked()
        {
            self.request_preview(entry.path.clone(), ui.ctx());
            ui.close();
        }
        let enabled = !self.loading
            && !crate::sandbox::active()
            && self.active_transfer.is_none()
            && self.active_deletion.is_none()
            && self.active_restore.is_none();
        for (mode, icon, label) in [
            (Extraction::Here, Icon::ExtractHere, "Extrair aqui"),
            (
                Extraction::ChooseDestination,
                Icon::ExtractTo,
                "Extrair em…",
            ),
        ] {
            let response = self.menu_item(ui, icon, crate::i18n::tr(label), "", enabled, false);
            let clicked = response.clicked();
            if crate::sandbox::active() {
                response.on_disabled_hover_text(crate::i18n::tr("A extração pelo gerenciador do sistema ainda não está disponível no Flatpak experimental."));
            }
            if clicked {
                self.extract_archives(paths.clone(), mode, ui.ctx());
                ui.close();
            }
        }
        ui.separator();
    }
    fn extract_archives(&self, paths: Vec<PathBuf>, mode: Extraction, ctx: &egui::Context) {
        let sender = self.event_sender.clone();
        let context = ctx.clone();
        let locale = crate::i18n::current();
        thread::spawn(move || {
            crate::i18n::set_locale(locale);
            let _ = sender.send(Event::ArchiveManagerFinished(archives::extract(
                &paths, mode,
            )));
            context.request_repaint();
        });
    }
}
#[cfg(test)]
mod tests;
