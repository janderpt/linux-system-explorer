use super::*;
use crate::preview::{
    Content, Document, Family, Job, Kind, Pixels,
    media::{Control, Session, Snapshot},
};

pub(super) fn consume_space(ctx: &egui::Context) -> bool {
    ctx.input_mut(|i| {
        let pressed = i.events.iter().any(|event| {
            matches!(
                event,
                egui::Event::Key {
                    key: Key::Space,
                    pressed: true,
                    repeat: false,
                    ..
                }
            )
        });
        pressed && i.consume_key(egui::Modifiers::NONE, Key::Space)
    })
}

#[derive(Clone, Copy, PartialEq)]
enum MarkupView {
    Source,
    Presentation,
}

pub(super) struct Preview {
    path: PathBuf,
    kind: Kind,
    job: Option<Job>,
    content: Option<Content>,
    document: Option<Document>,
    markup_view: MarkupView,
    markup_source: Option<(String, bool)>,
    texture: Option<egui::TextureHandle>,
    media: Option<Session>,
    playback: Snapshot,
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
            playback: Snapshot::default(),
            content: None,
            document: None,
            markup_view: MarkupView::Source,
            markup_source: None,
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
        self.job = Some(if let Some(document) = &self.document {
            Job::start_document(document.clone(), self.page, ctx)
        } else {
            Job::start(self.path.clone(), self.page, ctx)
        });
        self.texture = None;
        self.content = None;
        self.failed = false;
    }
    fn switch_markup_view(&mut self, view: MarkupView, ctx: &egui::Context) {
        if self.markup_view == view {
            return;
        }
        self.markup_view = view;
        self.job = None;
        self.content = None;
        self.texture = None;
        self.failed = false;
        match view {
            MarkupView::Source => {
                if let Some((text, truncated)) = &self.markup_source {
                    self.content = Some(Content::Text {
                        text: text.clone(),
                        truncated: *truncated,
                    });
                } else {
                    self.job = Some(Job::start(self.path.clone(), 1, ctx));
                }
            }
            MarkupView::Presentation => {
                if self.document.is_some() {
                    self.load_page(self.page, ctx);
                } else {
                    self.job = Some(Job::start_markup(self.path.clone(), ctx));
                }
            }
        }
        ctx.request_repaint();
    }
    fn poll(&mut self, ctx: &egui::Context) -> Option<std::io::Error> {
        if let Some(mut state) = self.media.as_ref().map(Session::poll) {
            if let Some(frame) = state.frame.take() {
                self.image(frame, ctx);
            }
            if !self.seeking {
                self.playhead = state.position;
            }
            self.failed |= state.error.is_some();
            self.playback = state;
        }
        if let Some(result) = self.job.as_ref().and_then(Job::poll) {
            self.job = None;
            match result {
                Ok(content) => match content {
                    Content::Image(frame) => self.image(frame, ctx),
                    Content::Pdf {
                        pixels,
                        page,
                        pages,
                        document,
                    } => {
                        self.page = page;
                        self.pages = pages;
                        if document.is_some() {
                            self.document = document;
                        }
                        self.image(pixels, ctx);
                    }
                    Content::Text { text, truncated } => {
                        if matches!(self.kind, Kind::Html | Kind::Markdown) {
                            self.markup_source = Some((text.clone(), truncated));
                        }
                        self.content = Some(Content::Text { text, truncated });
                    }
                    other => self.content = Some(other),
                },
                Err(error) => {
                    self.failed = true;
                    return Some(error);
                }
            }
        }
        None
    }
    fn popup_size(&self, ctx: &egui::Context) -> Vec2 {
        let max_width = (ctx.content_rect().width() - 100.0).clamp(280.0, 1040.0);
        let max_height = (ctx.content_rect().height() - 230.0).clamp(160.0, 600.0);
        if self.kind != Kind::Video {
            return Vec2::new(max_width, max_height);
        }
        let aspect = self.texture.as_ref().map_or(16.0 / 9.0, |t| {
            let size = t.size_vec2();
            size.x / size.y
        });
        let reserve = controls_height(((max_height - 58.0).max(80.0) * aspect).min(max_width));
        let picture_height = (max_height - reserve).max(80.0).min(max_width / aspect);
        let width = (picture_height * aspect).max(220.0).min(max_width);
        Vec2::new(width, picture_height + controls_height(width))
    }
    fn picture(&self, ui: &mut egui::Ui, height: f32) {
        let Some(texture) = &self.texture else { return };
        let size = texture.size_vec2();
        let fit = (ui.available_width() / size.x).min(height / size.y);
        let shown = size * fit * self.zoom;
        egui::ScrollArea::both()
            .id_salt("preview_image_scroll")
            .auto_shrink([false, false])
            .max_height(height)
            .show(ui, |ui| {
                let canvas = Vec2::new(ui.available_width().max(shown.x), height.max(shown.y));
                let (rect, _) = ui.allocate_exact_size(canvas, Sense::hover());
                egui::Image::new((texture.id(), shown))
                    .paint_at(ui, egui::Rect::from_center_size(rect.center(), shown));
            });
    }
    fn body(&mut self, ui: &mut egui::Ui, height: f32, ctx: &egui::Context) -> Option<String> {
        if matches!(self.kind, Kind::Html | Kind::Markdown) {
            let mut view = self.markup_view;
            ui.horizontal(|ui| {
                ui.selectable_value(&mut view, MarkupView::Source, crate::i18n::tr("Fonte"));
                ui.selectable_value(
                    &mut view,
                    MarkupView::Presentation,
                    crate::i18n::tr("Apresentação"),
                );
            });
            self.switch_markup_view(view, ctx);
        }
        if let Some(media) = &self.media {
            let error = self.playback.error.take();
            let (playing, duration) = (self.playback.playing, self.playback.duration);
            let compact = ui.available_width() < 640.0;
            let picture_height = (height - controls_height(ui.available_width())).max(80.0);
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
            ui.add_enabled_ui(!self.failed, |ui| {
                ui.horizontal(|ui| {
                    if ui
                        .button(if playing {
                            crate::i18n::tr("Pausar")
                        } else {
                            crate::i18n::tr("Reproduzir")
                        })
                        .clicked()
                    {
                        media.control(Control::Playing(!playing));
                    }
                    ui.label(format!("{} / {}", clock(self.playhead), clock(duration)));
                    if !compact {
                        seek_slider(
                            ui,
                            &mut self.playhead,
                            &mut self.seeking,
                            duration,
                            media,
                            (ui.available_width() - 180.0).max(80.0),
                        );
                        volume_slider(ui, &mut self.volume, media, Some(90.0));
                    }
                });
                if compact {
                    seek_slider(
                        ui,
                        &mut self.playhead,
                        &mut self.seeking,
                        duration,
                        media,
                        ui.available_width(),
                    );
                    ui.horizontal(|ui| volume_slider(ui, &mut self.volume, media, None));
                }
            });
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
            return error;
        }
        let paginated = matches!(self.kind, Kind::Pdf | Kind::Office(_))
            || matches!(self.kind, Kind::Html | Kind::Markdown)
                && self.markup_view == MarkupView::Presentation;
        if paginated || self.texture.is_some() {
            ui.horizontal(|ui| {
                if paginated {
                    let previous = ui
                        .add_enabled(
                            self.page > 1 && self.job.is_none(),
                            egui::Button::new(crate::i18n::tr(
                                if self.kind == Kind::Office(Family::Presentation) {
                                    "Slide anterior"
                                } else {
                                    "Página anterior"
                                },
                            )),
                        )
                        .clicked();
                    ui.label(crate::i18n::message(
                        if self.kind == Kind::Office(Family::Presentation) {
                            "Slide {} de {}"
                        } else {
                            "Página {} de {}"
                        },
                        &[self.page.to_string(), self.pages.to_string()],
                    ));
                    let next = ui
                        .add_enabled(
                            self.page < self.pages && self.job.is_none(),
                            egui::Button::new(crate::i18n::tr(
                                if self.kind == Kind::Office(Family::Presentation) {
                                    "Próximo slide"
                                } else {
                                    "Próxima página"
                                },
                            )),
                        )
                        .clicked();
                    if previous {
                        self.load_page(self.page - 1, ctx);
                    } else if next {
                        self.load_page(self.page + 1, ctx);
                    }
                    ui.add_space(12.0);
                }
                ui.add_enabled_ui(self.texture.is_some(), |ui| {
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
            });
        }
        if self.texture.is_some() {
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
fn controls_height(width: f32) -> f32 {
    if width < 640.0 { 112.0 } else { 58.0 }
}
fn seek_slider(
    ui: &mut egui::Ui,
    playhead: &mut f64,
    seeking: &mut bool,
    duration: f64,
    media: &Session,
    width: f32,
) {
    let response = ui
        .scope(|ui| {
            ui.spacing_mut().slider_width = width.max(40.0);
            ui.add(egui::Slider::new(playhead, 0.0..=duration.max(0.01)).show_value(false))
        })
        .inner;
    *seeking = response.dragged();
    if response.drag_stopped() || response.changed() && !response.dragged() {
        media.control(Control::Seek(*playhead));
    }
}
fn volume_slider(ui: &mut egui::Ui, volume: &mut f64, media: &Session, width: Option<f32>) {
    ui.label(crate::i18n::tr("Volume"));
    let response = ui
        .scope(|ui| {
            ui.spacing_mut().slider_width = width.unwrap_or_else(|| ui.available_width()).max(40.0);
            ui.add(egui::Slider::new(volume, 0.0..=1.0).show_value(false))
        })
        .inner;
    if response.changed() {
        media.control(Control::Volume(*volume));
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
            if error.kind() == std::io::ErrorKind::Unsupported {
                self.post_typed_notice(toast::NoticeKind::Warning, error.to_string());
            } else {
                self.report_error(crate::i18n::message(
                    "Não foi possível pré-visualizar o arquivo: {}",
                    &[error.to_string()],
                ));
            }
            if matches!(preview.kind, Kind::Html | Kind::Markdown)
                && preview.markup_view == MarkupView::Presentation
            {
                preview.switch_markup_view(MarkupView::Source, ctx);
            } else {
                return;
            }
        }
        if let Some(error) = preview.playback.error.take() {
            self.report_error(crate::i18n::message(
                "Não foi possível reproduzir este arquivo: {}",
                &[error],
            ));
            return;
        }
        if matches!(&preview.content, Some(Content::Unsupported)) {
            self.post_typed_notice(
                toast::NoticeKind::Warning,
                crate::i18n::message(
                    "Pré-visualização não disponível para este tipo de arquivo: {}",
                    &[preview
                        .path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned()],
                ),
            );
            return;
        }
        let mut close = ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Escape))
            || consume_space(ctx);
        if close {
            return;
        }
        // Probe unclassified files as bounded text before showing any modal.
        // Unsupported binaries must never flash an empty preview popup.
        if preview.kind == Kind::Unknown && preview.job.is_some() {
            self.preview = Some(preview);
            ctx.request_repaint_after(std::time::Duration::from_millis(20));
            return;
        }
        let mut open = false;
        let mut media_error = None;
        let Vec2 {
            x: width,
            y: height,
        } = preview.popup_size(ctx);
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
                        .on_hover_text(crate::i18n::tr("Fechar pré-visualização · Espaço / Esc"))
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
