use super::*;

pub(super) struct Dialog {
    icon: egui::TextureHandle,
}

impl Dialog {
    fn new(ctx: &egui::Context) -> Self {
        let icon = crate::system_style::application_icon();
        Self {
            icon: ctx.load_texture(
                "about_application_icon",
                egui::ColorImage::from_rgba_unmultiplied(
                    [icon.width as usize, icon.height as usize],
                    &icon.rgba,
                ),
                egui::TextureOptions::LINEAR,
            ),
        }
    }

    fn show(&self, ctx: &egui::Context) -> bool {
        let mut close = false;
        let modal = egui::Modal::new(egui::Id::new("about_dialog")).show(ctx, |ui| {
            ui.set_width((ctx.content_rect().width() - 64.0).clamp(280.0, 480.0));
            ui.spacing_mut().interact_size.y = 32.0;
            ui.heading(crate::i18n::tr("Sobre"));
            ui.separator();
            egui::ScrollArea::vertical()
                .id_salt("about_content")
                .max_height((ctx.content_rect().height() - 160.0).max(120.0))
                .show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    ui.vertical_centered(|ui| {
                        ui.add_space(8.0);
                        ui.add(egui::Image::new(&self.icon).fit_to_exact_size(Vec2::splat(88.0)));
                        ui.add_space(8.0);
                        ui.heading("Linux System Explorer");
                        ui.label(crate::i18n::tr(
                            "Um explorador de arquivos para Linux inspirado no Windows Explorer",
                        ));
                    });
                    ui.add_space(16.0);
                    egui::Grid::new("about_metadata")
                        .spacing([20.0, 10.0])
                        .show(ui, |ui| {
                            for (label, value) in [
                                (
                                    crate::i18n::tr("Autoria"),
                                    env!("CARGO_PKG_AUTHORS").to_owned(),
                                ),
                                (
                                    crate::i18n::tr("Versão"),
                                    env!("CARGO_PKG_VERSION").to_owned(),
                                ),
                                (crate::i18n::tr("Publicado em"), publication_date()),
                                (
                                    crate::i18n::tr("Licença"),
                                    env!("CARGO_PKG_LICENSE").to_owned(),
                                ),
                            ] {
                                ui.weak(label);
                                ui.label(value);
                                ui.end_row();
                            }
                        });
                    ui.add_space(16.0);
                    let repository = env!("CARGO_PKG_REPOSITORY");
                    ui.hyperlink_to(crate::i18n::tr("Código-fonte"), repository);
                    ui.hyperlink_to(
                        crate::i18n::tr("Versões e downloads"),
                        format!("{repository}/releases"),
                    );
                    ui.hyperlink_to(
                        crate::i18n::tr("Relatar um problema"),
                        format!("{repository}/issues"),
                    );
                    ui.hyperlink_to(
                        crate::i18n::tr("Licença do projeto"),
                        format!("{repository}/blob/main/LICENSE"),
                    );
                    ui.add_space(8.0);
                });
            ui.separator();
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                close = ui.button(crate::i18n::tr("Fechar")).clicked();
            });
        });
        close || modal.should_close()
    }
}

fn publication_date() -> String {
    let date = chrono::NaiveDate::parse_from_str(env!("LSE_RELEASE_DATE"), "%Y-%m-%d")
        .expect("invalid packaging/release-date");
    date.format(match crate::i18n::current() {
        crate::i18n::Locale::Portuguese => "%d/%m/%Y",
        crate::i18n::Locale::English => "%Y-%m-%d",
    })
    .to_string()
}

impl Explorer {
    pub(super) fn about_button(&mut self, ui: &mut egui::Ui) -> egui::Response {
        let response = self.icon_button(ui, Icon::About, crate::i18n::tr("Sobre"));
        if response.clicked() {
            self.about = Some(Dialog::new(ui.ctx()));
        }
        response
    }

    pub(super) fn about_dialog(&mut self, ctx: &egui::Context) {
        if self.about.as_ref().is_some_and(|dialog| dialog.show(ctx)) {
            self.about = None;
        }
    }
}
