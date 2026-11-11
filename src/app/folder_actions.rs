use super::*;

impl Explorer {
    pub(super) fn open_new_window(&mut self, path: PathBuf, ctx: &egui::Context) {
        let sender = self.event_sender.clone();
        let context = ctx.clone();
        let locale = crate::i18n::current();
        thread::spawn(move || {
            crate::i18n::set_locale(locale);
            if let Err(error) = crate::windows::open_new(&path) {
                let _ = sender.send(Event::OpenFailed(crate::i18n::message(
                    "Não foi possível abrir a nova janela: {error}",
                    &[error.to_string()],
                )));
                context.request_repaint();
            }
        });
    }

    pub(super) fn folder_actions_menu(&mut self, ui: &mut egui::Ui, path: &Path) {
        if self
            .menu_item(
                ui,
                Icon::NewTab,
                crate::i18n::tr("Abrir em nova aba"),
                "",
                true,
                false,
            )
            .clicked()
        {
            self.requested_tabs.push(path.to_owned());
            ui.close();
        }
        if self
            .menu_item(
                ui,
                Icon::NewWindow,
                crate::i18n::tr("Abrir em nova janela"),
                "",
                true,
                false,
            )
            .clicked()
        {
            self.open_new_window(path.to_owned(), ui.ctx());
            ui.close();
        }
        if self
            .menu_item(
                ui,
                Icon::Terminal,
                crate::i18n::tr("Abrir terminal aqui"),
                "",
                true,
                false,
            )
            .clicked()
        {
            self.open_terminal(path.to_owned(), ui.ctx());
            ui.close();
        }
    }
}
