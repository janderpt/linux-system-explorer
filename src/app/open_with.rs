use super::*;
use crate::applications::{self, Application, Applications};

pub(super) struct Chooser {
    path: PathBuf,
    results: Receiver<std::io::Result<Applications>>,
    applications: Vec<Application>,
    mime: Option<String>,
    loading: bool,
    error: Option<String>,
    search: String,
    show_other: bool,
    selected: Option<usize>,
    focus_search: bool,
    always_use: bool,
}

impl Chooser {
    fn new(path: PathBuf, ctx: &egui::Context) -> Self {
        let (sender, results) = mpsc::channel();
        let target = path.clone();
        let context = ctx.clone();
        let locale = crate::i18n::current();
        thread::spawn(move || {
            crate::i18n::set_locale(locale);
            let result = applications::discover(&target);
            let _ = sender.send(result);
            context.request_repaint();
        });
        Self {
            path,
            results,
            applications: Vec::new(),
            mime: None,
            loading: true,
            error: None,
            search: String::new(),
            show_other: false,
            selected: None,
            focus_search: true,
            always_use: false,
        }
    }

    fn poll(&mut self) {
        if let Ok(result) = self.results.try_recv() {
            self.loading = false;
            match result {
                Ok(result) => {
                    self.mime = result.mime;
                    self.show_other = !result.items.iter().any(|app| app.recommended);
                    self.applications = result.items;
                }
                Err(error) => {
                    self.error = Some(crate::i18n::message(
                        "Não foi possível listar os aplicativos: {}",
                        &[error.to_string()],
                    ))
                }
            }
        }
    }

    fn visible(&self) -> Vec<usize> {
        let search = self.search.trim().to_lowercase();
        self.applications
            .iter()
            .enumerate()
            .filter(|(_, app)| {
                (self.show_other || app.recommended)
                    && (search.is_empty()
                        || app.name.to_lowercase().contains(&search)
                        || app.id.to_lowercase().contains(&search)
                        || app.description.to_lowercase().contains(&search))
            })
            .map(|(index, _)| index)
            .collect()
    }
}

impl Explorer {
    pub(super) fn choose_application(&mut self, entry: &Entry, ctx: &egui::Context) {
        if !entry.is_dir && !self.is_trash() {
            if crate::sandbox::active() {
                let path = entry.path.clone();
                let sender = self.event_sender.clone();
                let context = ctx.clone();
                let locale = crate::i18n::current();
                thread::spawn(move || {
                    crate::i18n::set_locale(locale);
                    if let Err(error) = crate::sandbox::open_file(&path, true) {
                        let _ = sender.send(Event::OpenFailed(error));
                        context.request_repaint();
                    }
                });
            } else {
                self.open_with = Some(Chooser::new(entry.path.clone(), ctx));
            }
        }
    }

    pub(super) fn open_with_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut chooser) = self.open_with.take() else {
            return;
        };
        chooser.poll();
        let mut cancel = false;
        let mut accept = false;
        let mut retry = false;
        let modal = egui::Modal::new(egui::Id::new("open_with_dialog")).show(ctx, |ui| {
            ui.set_width((ctx.content_rect().width() - 70.0).clamp(300.0, 620.0));
            ui.spacing_mut().interact_size.y = 32.0;
            ui.heading(crate::i18n::tr("Abrir com…"));
            ui.add(
                egui::Label::new(
                    chooser
                        .path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy(),
                )
                .truncate(),
            )
            .on_hover_text(chooser.path.display().to_string());
            if let Some(mime) = &chooser.mime {
                ui.weak(mime);
            }
            let search = ui.add(
                egui::TextEdit::singleline(&mut chooser.search)
                    .desired_width(f32::INFINITY)
                    .hint_text(crate::i18n::tr("Buscar aplicativo…")),
            );
            if chooser.focus_search {
                search.request_focus();
                chooser.focus_search = false;
            }
            ui.checkbox(
                &mut chooser.show_other,
                crate::i18n::tr("Mostrar outros aplicativos"),
            );
            let visible = chooser.visible();
            if chooser
                .selected
                .is_some_and(|index| !visible.contains(&index))
            {
                chooser.selected = None;
            }
            ui.separator();
            let height = (ctx.content_rect().height() - 370.0).clamp(110.0, 300.0);
            egui::ScrollArea::vertical()
                .id_salt("open_with_apps")
                .min_scrolled_height(height)
                .max_height(height)
                .show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    if chooser.loading {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label(crate::i18n::tr("Carregando aplicativos…"));
                        });
                    } else if let Some(error) = &chooser.error {
                        ui.colored_label(ui.visuals().error_fg_color, error);
                        retry = ui.button(crate::i18n::tr("Tentar novamente")).clicked();
                    } else if visible.is_empty() {
                        ui.weak(crate::i18n::tr("Nenhum aplicativo encontrado."));
                    }
                    ui.with_layout(Layout::top_down_justified(Align::Min), |ui| {
                        for index in visible {
                            let app = &chooser.applications[index];
                            let badge = if app.is_default {
                                crate::i18n::tr("Padrão")
                            } else if app.recommended {
                                crate::i18n::tr("Recomendado")
                            } else {
                                ""
                            };
                            let response = self
                                .menu_item(
                                    ui,
                                    Icon::OpenWith,
                                    &app.name,
                                    badge,
                                    true,
                                    chooser.selected == Some(index),
                                )
                                .on_hover_text(format!("{}\n{}", app.description, app.id));
                            if response.clicked() {
                                chooser.selected = Some(index);
                            }
                            if response.double_clicked() {
                                chooser.selected = Some(index);
                                accept = true;
                            }
                        }
                    });
                });
            ui.separator();
            ui.add_enabled(
                !chooser.loading
                    && chooser
                        .mime
                        .as_deref()
                        .is_some_and(applications::valid_mime),
                egui::Checkbox::new(
                    &mut chooser.always_use,
                    crate::i18n::tr("Usar sempre este aplicativo para este tipo de arquivo"),
                ),
            )
            .on_disabled_hover_text(crate::i18n::tr(
                "É necessário identificar o tipo MIME do arquivo para salvar a associação.",
            ));
            ui.weak(crate::i18n::tr(if chooser.always_use {
                "A associação padrão do sistema será alterada para este tipo de arquivo."
            } else {
                "Esta escolha vale apenas para esta abertura."
            }));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                accept |= ui
                    .add_enabled(
                        chooser.selected.is_some(),
                        egui::Button::new(crate::i18n::tr("Abrir")),
                    )
                    .clicked();
                cancel = ui.button(crate::i18n::tr("Cancelar")).clicked();
            });
        });
        if cancel || modal.should_close() {
            return;
        }
        if retry {
            self.open_with = Some(Chooser::new(chooser.path, ctx));
        } else if accept {
            if let Some(app) = chooser
                .selected
                .and_then(|index| chooser.applications.get(index))
                .cloned()
            {
                let default_mime = chooser
                    .mime
                    .filter(|mime| chooser.always_use && applications::valid_mime(mime));
                self.open_with_application(app, chooser.path, default_mime, ctx);
            }
        } else {
            self.open_with = Some(chooser);
        }
    }

    fn open_with_application(
        &self,
        app: Application,
        path: PathBuf,
        default_mime: Option<String>,
        ctx: &egui::Context,
    ) {
        let sender = self.event_sender.clone();
        let context = ctx.clone();
        let locale = crate::i18n::current();
        thread::spawn(move || {
            crate::i18n::set_locale(locale);
            let result = default_mime
                .as_deref()
                .map_or(Ok(()), |mime| applications::set_default(&app, mime))
                .and_then(|()| applications::launch(&app, &path));
            if let Err(error) = result {
                let _ = sender.send(Event::OpenFailed(error));
                context.request_repaint();
            }
        });
    }
}

#[cfg(test)]
mod tests;
