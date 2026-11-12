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
    pub(super) fn selected_images(&self, clicked: &Entry) -> Vec<Entry> {
        if self.is_trash()
            || self.is_applications()
            || clicked.is_dir
            || !crate::image_ops::supported(&clicked.path)
        {
            return vec![];
        }
        if self.selected.is_empty() {
            return vec![clicked.clone()];
        }
        if !self.selected.contains(&clicked.path) {
            return vec![];
        }
        let mut entries: Vec<_> = self
            .entries
            .iter()
            .filter(|e| self.selected.contains(&e.path))
            .cloned()
            .collect();
        if entries.len() != self.selected.len()
            || entries
                .iter()
                .any(|e| e.is_dir || !crate::image_ops::supported(&e.path))
        {
            return vec![];
        }
        entries.sort_by_key(|e| (e.path != clicked.path, e.path.clone()));
        entries
    }
    pub(super) fn type_actions_menu(&mut self, ui: &mut egui::Ui, entry: &Entry) {
        self.image_actions_menu(ui, entry);
        self.print_menu(ui, entry);
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
    pub(super) fn print_menu(&mut self, ui: &mut egui::Ui, clicked: &Entry) {
        if self.is_trash() || self.is_applications() || !crate::printing::supported(clicked) {
            return;
        }
        let entries: Vec<_> = if self.selected.is_empty() {
            vec![clicked.clone()]
        } else {
            filesystem::visible_entries(&self.entries, "", true, self.sort, self.descending)
                .into_iter()
                .map(|i| &self.entries[i])
                .filter(|e| self.selected.contains(&e.path))
                .cloned()
                .collect()
        };
        if entries.is_empty()
            || (!self.selected.is_empty()
                && (!self.selected.contains(&clicked.path) || entries.len() != self.selected.len()))
            || entries.iter().any(|e| !crate::printing::supported(e))
        {
            return;
        }
        ui.separator();
        ui.label(RichText::new(crate::i18n::tr("Impressão")).small().weak());
        if self
            .menu_item(
                ui,
                Icon::Print,
                crate::i18n::tr(if entries.len() == 1 {
                    "Imprimir…"
                } else {
                    "Imprimir seleção…"
                }),
                "",
                !self.loading && self.print_cancel.is_none(),
                false,
            )
            .clicked()
        {
            self.request_print(entries, ui.ctx());
            ui.close();
        }
        ui.separator();
    }
    pub(super) fn cancel_print(&self) {
        if let Some(cancel) = &self.print_cancel {
            cancel.store(true, std::sync::atomic::Ordering::Release);
        }
    }
    fn request_print(&mut self, entries: Vec<Entry>, ctx: &egui::Context) {
        if self.print_cancel.is_some() {
            return;
        }
        let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let worker_cancel = cancel.clone();
        let sender = self.event_sender.clone();
        let context = ctx.clone();
        let locale = crate::i18n::current();
        thread::spawn(move || {
            crate::i18n::set_locale(locale);
            let result = crate::printing::run(&entries, &worker_cancel);
            let _ = sender.send(Event::PrintFinished(result));
            context.request_repaint();
        });
        self.print_cancel = Some(cancel);
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
