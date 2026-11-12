use super::*;
use crate::preview::{
    Content, Job, Kind, Pixels,
    media::{Control, Session},
};

pub(super) struct Preview {
    path: PathBuf,
    kind: Kind,
    job: Option<Job>,
    content: Option<Content>,
    texture: Option<egui::TextureHandle>,
    media: Option<Session>,
    page: u32,
    pages: u32,
    zoom: f32,
    volume: f64,
    playhead: f64,
    seeking: bool,
    failed: bool,
}
impl Preview {
    fn new(path: PathBuf, ctx: &egui::Context) -> Self {
        let kind = Kind::for_path(&path);
        let media =
            matches!(kind, Kind::Audio | Kind::Video).then(|| Session::start(path.clone(), ctx));
        let job = media.is_none().then(|| Job::start(path.clone(), 1, ctx));
        Self {
            path,
            kind,
            job,
            media,
            content: None,
            texture: None,
            page: 1,
            pages: 1,
            zoom: 1.0,
            volume: 0.7,
            playhead: 0.0,
            seeking: false,
            failed: false,
        }
    }
    fn image(&mut self, frame: Pixels, ctx: &egui::Context) {
        let image = egui::ColorImage::from_rgba_unmultiplied(
            [frame.width as usize, frame.height as usize],
            &frame.rgba,
        );
        if let Some(texture) = &mut self.texture {
            texture.set(image, egui::TextureOptions::LINEAR);
        } else {
            self.texture =
                Some(ctx.load_texture("file_preview", image, egui::TextureOptions::LINEAR));
        }
    }
    fn load_page(&mut self, page: u32, ctx: &egui::Context) {
        self.page = page.clamp(1, self.pages);
        self.job = Some(Job::start(self.path.clone(), self.page, ctx));
        self.texture = None;
        self.content = None;
        self.failed = false;
    }
    fn poll(&mut self, ctx: &egui::Context) -> Option<String> {
        if let Some(result) = self.job.as_ref().and_then(Job::poll) {
            self.job = None;
            match result {
                Ok(content) => match content {
                    Content::Image(frame) => self.image(frame, ctx),
                    Content::Pdf {
                        pixels,
                        page,
                        pages,
                    } => {
                        self.page = page;
                        self.pages = pages;
                        self.image(pixels, ctx);
                    }
                    other => self.content = Some(other),
                },
                Err(error) => {
                    self.failed = true;
                    return Some(error.to_string());
                }
            }
        }
        None
    }
    fn picture(&self, ui: &mut egui::Ui, height: f32) {
        let Some(texture) = &self.texture else { return };
        let size = texture.size_vec2();
        let fit = (ui.available_width() / size.x)
            .min(height / size.y)
            .min(1.0);
        let shown = size * fit * self.zoom;
        egui::ScrollArea::both()
            .id_salt("preview_image_scroll")
            .auto_shrink([false, false])
            .max_height(height)
            .show(ui, |ui| {
                ui.add(egui::Image::new((texture.id(), shown)));
            });
    }
    fn body(&mut self, ui: &mut egui::Ui, height: f32, ctx: &egui::Context) -> Option<String> {
        if self.media.is_some() {
            let state = self.media.as_ref().unwrap().poll();
            if let Some(frame) = state.frame {
                self.image(frame, ctx);
            }
            let error = state.error;
            if error.is_some() {
                self.failed = true;
            }
            if !self.seeking {
                self.playhead = state.position;
            }
            let picture_height = (height - 58.0).max(80.0);
            if self.texture.is_some() {
                self.picture(ui, picture_height);
            } else {
                ui.allocate_ui_with_layout(
                    Vec2::new(ui.available_width(), picture_height),
                    Layout::top_down(Align::Center),
                    |ui| {
                        ui.add_space((picture_height * 0.35).max(0.0));
                        ui.label(
                            RichText::new(if self.kind == Kind::Audio {
                                crate::i18n::tr("Áudio")
                            } else {
                                crate::i18n::tr("Vídeo")
                            })
                            .size(28.0),
                        );
                        if self.failed {
                            ui.label(crate::i18n::tr("Não há pré-visualização disponível."));
                        } else if self.kind == Kind::Video {
                            ui.spinner();
                        }
                    },
                );
            }
            let media = self.media.as_ref().unwrap();
            ui.add_enabled_ui(!self.failed, |ui| {
                ui.horizontal(|ui| {
                    if ui
                        .button(if state.playing {
                            crate::i18n::tr("Pausar")
                        } else {
                            crate::i18n::tr("Reproduzir")
                        })
                        .clicked()
                    {
                        media.control(Control::Playing(!state.playing));
                    }
                    ui.label(format!(
                        "{} / {}",
                        clock(self.playhead),
                        clock(state.duration)
                    ));
                    let width = (ui.available_width() - 180.0).max(80.0);
                    let response = ui.add_sized(
                        [width, 20.0],
                        egui::Slider::new(&mut self.playhead, 0.0..=state.duration.max(0.01))
                            .show_value(false),
                    );
                    self.seeking = response.dragged();
                    if response.drag_stopped() || response.changed() && !response.dragged() {
                        media.control(Control::Seek(self.playhead));
                    }
                    ui.label(crate::i18n::tr("Volume"));
                    if ui
                        .add_sized(
                            [90.0, 20.0],
                            egui::Slider::new(&mut self.volume, 0.0..=1.0).show_value(false),
                        )
                        .changed()
                    {
                        media.control(Control::Volume(self.volume));
                    }
                });
            });
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
            return error;
        }
        if self.kind == Kind::Pdf {
            ui.horizontal(|ui| {
                let previous = ui
                    .add_enabled(
                        self.page > 1 && self.job.is_none(),
                        egui::Button::new(crate::i18n::tr("Página anterior")),
                    )
                    .clicked();
                ui.label(crate::i18n::message(
                    "Página {} de {}",
                    &[self.page.to_string(), self.pages.to_string()],
                ));
                let next = ui
                    .add_enabled(
                        self.page < self.pages && self.job.is_none(),
                        egui::Button::new(crate::i18n::tr("Próxima página")),
                    )
                    .clicked();
                if previous {
                    self.load_page(self.page - 1, ctx);
                } else if next {
                    self.load_page(self.page + 1, ctx);
                }
            });
        }
        if self.texture.is_some() {
            ui.horizontal(|ui| {
                if ui.button("−").clicked() {
                    self.zoom = (self.zoom / 1.25).max(0.25);
                }
                if ui.button(crate::i18n::tr("Ajustar à janela")).clicked() {
                    self.zoom = 1.0;
                }
                if ui.button("+").clicked() {
                    self.zoom = (self.zoom * 1.25).min(8.0);
                }
            });
            self.picture(ui, (height - 64.0).max(80.0));
        } else if let Some(Content::Text { text, truncated }) = &self.content {
            if *truncated {
                ui.weak(crate::i18n::tr("Exibindo apenas o início do arquivo."));
            }
            egui::ScrollArea::both()
                .id_salt("preview_text_scroll")
                .auto_shrink([false, false])
                .max_height(height - 24.0)
                .show(ui, |ui| {
                    ui.add(egui::Label::new(RichText::new(text).monospace()).selectable(true));
                });
        } else {
            ui.allocate_ui_with_layout(
                Vec2::new(ui.available_width(), height),
                Layout::top_down(Align::Center),
                |ui| {
                    ui.add_space(height * 0.35);
                    if self.job.is_some() {
                        ui.spinner();
                        ui.label(crate::i18n::tr("Carregando pré-visualização…"));
                    } else {
                        ui.label(crate::i18n::tr("Não há pré-visualização disponível."));
                    }
                },
            );
        }
        None
    }
}
fn clock(seconds: f64) -> String {
    let seconds = seconds.max(0.0) as u64;
    if seconds >= 3600 {
        format!(
            "{}:{:02}:{:02}",
            seconds / 3600,
            seconds / 60 % 60,
            seconds % 60
        )
    } else {
        format!("{}:{:02}", seconds / 60, seconds % 60)
    }
}
impl Explorer {
    pub(super) fn preview_selected(&mut self, ctx: &egui::Context) {
        let path = self
            .selection_cursor
            .as_ref()
            .filter(|path| self.selected.contains(*path))
            .or_else(|| self.selected.iter().next())
            .cloned();
        if let Some(path) = path {
            self.request_preview(path, ctx);
        }
    }
    pub(super) fn request_preview(&mut self, path: PathBuf, ctx: &egui::Context) {
        if self
            .entries
            .iter()
            .any(|entry| entry.path == path && !entry.is_dir)
        {
            self.preview = Some(Preview::new(path, ctx));
            ctx.request_repaint();
        }
    }
    pub(super) fn preview_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut preview) = self.preview.take() else {
            return;
        };
        if let Some(error) = preview.poll(ctx) {
            self.report_error(crate::i18n::message(
                "Não foi possível pré-visualizar o arquivo: {}",
                &[error],
            ));
        }
        let mut close = ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Escape));
        let mut open = false;
        let mut media_error = None;
        let width = (ctx.content_rect().width() - 100.0).clamp(280.0, 1040.0);
        let height = (ctx.content_rect().height() - 230.0).clamp(160.0, 600.0);
        let response =
            egui::Modal::new(egui::Id::new(("file_preview", self.tab_id))).show(ctx, |ui| {
                ui.set_width(width);
                ui.horizontal(|ui| {
                    ui.add_sized(
                        [width - 60.0, 28.0],
                        egui::Label::new(
                            RichText::new(
                                preview
                                    .path
                                    .file_name()
                                    .unwrap_or_default()
                                    .to_string_lossy(),
                            )
                            .heading(),
                        )
                        .truncate(),
                    );
                    close |= ui
                        .button("×")
                        .on_hover_text(crate::i18n::tr("Fechar pré-visualização · Esc"))
                        .clicked();
                });
                ui.add(
                    egui::Label::new(
                        RichText::new(self.display_address(&preview.path))
                            .small()
                            .weak(),
                    )
                    .truncate(),
                );
                ui.separator();
                media_error = preview.body(ui, height, ctx);
                ui.separator();
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    close |= ui.button(crate::i18n::tr("Fechar")).clicked();
                    open = ui
                        .button(crate::i18n::tr("Abrir no aplicativo padrão"))
                        .clicked();
                });
            });
        if let Some(error) = media_error {
            self.report_error(crate::i18n::message(
                "Não foi possível reproduzir este arquivo: {}",
                &[error],
            ));
        }
        if open {
            if let Some(entry) = self
                .entries
                .iter()
                .find(|entry| entry.path == preview.path)
                .cloned()
            {
                self.open(&entry, ctx);
            }
            return;
        }
        if !close && !response.should_close() {
            self.preview = Some(preview);
        }
    }
}

#[cfg(test)]
mod tests;
