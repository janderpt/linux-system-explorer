use super::*;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

pub(super) struct EmptyTrashPrompt {
    pub id: u64,
    pub listing: Option<Listing>,
    pub error: Option<String>,
    focus_cancel: bool,
}

impl Explorer {
    pub(super) fn can_empty_trash(&self) -> bool {
        !self.loading
            && self.dnd.trash_prepare.is_none()
            && !self.close_after_transfer
            && self.empty_trash_prompt.is_none()
            && self.active_empty_trash.is_none()
            && self.active_restore.is_none()
            && self.active_transfer.is_none()
            && self.active_deletion.is_none()
            && self.rename.is_none()
            && self.delete_confirmation.is_none()
    }

    pub(super) fn request_empty_trash(&mut self, ctx: &egui::Context) {
        if !self.can_empty_trash() {
            return;
        }
        self.empty_trash_serial = self.empty_trash_serial.wrapping_add(1);
        let id = self.empty_trash_serial;
        self.empty_trash_prompt = Some(EmptyTrashPrompt {
            id,
            listing: None,
            error: None,
            focus_cancel: true,
        });
        let sender = self.event_sender.clone();
        let context = ctx.clone();
        let locale = crate::i18n::current();
        if let Err(error) = thread::Builder::new()
            .name("trash-empty-preview".into())
            .spawn(move || {
                crate::i18n::set_locale(locale);
                let result = crate::trashbin::read();
                let _ = sender.send(Event::EmptyTrashListed(id, result));
                context.request_repaint();
            })
        {
            self.receive_empty_trash_listing(id, Err(error));
        }
    }

    pub(super) fn receive_empty_trash_listing(
        &mut self,
        id: u64,
        result: std::io::Result<Listing>,
    ) {
        let Some(prompt) = &mut self.empty_trash_prompt else {
            return;
        };
        if prompt.id != id {
            return;
        }
        match result {
            Ok(listing) => {
                prompt.listing = Some(listing);
                prompt.error = None;
            }
            Err(error) => {
                prompt.error = Some(crate::i18n::message(
                    "Não foi possível ler a lixeira: {}",
                    &[error.to_string()],
                ))
            }
        }
    }

    fn start_empty_trash(&mut self, listing: Listing, ctx: &egui::Context) {
        let progress = file_ops::Progress {
            total: listing.entries.len(),
            ..Default::default()
        };
        let cancel = Arc::new(AtomicBool::new(false));
        let worker_cancel = cancel.clone();
        let sender = self.event_sender.clone();
        let context = ctx.clone();
        let locale = crate::i18n::current();
        let worker = thread::Builder::new()
            .name("trash-empty".into())
            .spawn(move || {
                crate::i18n::set_locale(locale);
                let mut last_update = std::time::Instant::now() - std::time::Duration::from_secs(1);
                let mut report =
                    crate::trashbin::purge(listing.entries, worker_cancel, |progress| {
                        if last_update.elapsed() >= std::time::Duration::from_millis(50)
                            || progress.done == progress.total
                        {
                            let _ = sender.send(Event::EmptyTrashProgress(progress));
                            context.request_repaint();
                            last_update = std::time::Instant::now();
                        }
                    });
                if listing.skipped > 0 {
                    report.skipped += listing.skipped;
                    report.errors.push(
                        crate::i18n::tr(
                            "Alguns itens da lixeira não puderam ser listados e foram preservados.",
                        )
                        .into(),
                    );
                }
                let _ = sender.send(Event::EmptyTrashFinished(report));
                context.request_repaint();
            });
        match worker {
            Ok(_) => {
                self.active_empty_trash = Some(trash::ActiveTrashOperation { progress, cancel });
                self.error = None;
            }
            Err(error) => self.report_error(crate::i18n::message(
                "Não foi possível iniciar a exclusão: {error}",
                &[error.to_string()],
            )),
        }
    }

    pub(super) fn finish_empty_trash(&mut self, report: crate::trashbin::Report) {
        let removed = |path: &Path| report.completed.iter().any(|root| path.starts_with(root));
        self.selected.retain(|path| !removed(path));
        if self
            .selection_anchor
            .as_ref()
            .is_some_and(|path| removed(path))
        {
            self.selection_anchor = None;
        }
        if self
            .selection_cursor
            .as_ref()
            .is_some_and(|path| removed(path))
        {
            self.selection_cursor = None;
            self.reveal_selection = false;
        }
        if let Some(clipboard) = &mut self.file_clipboard {
            clipboard.paths.retain(|path| !removed(path));
            if clipboard.paths.is_empty() {
                self.file_clipboard = None;
            }
        }
        self.post_operation_notice(
            format!(
                "{}{}",
                crate::i18n::plural(
                    "{} item excluído permanentemente.",
                    "{} itens excluídos permanentemente.",
                    report.completed.len()
                ),
                if report.cancelled {
                    crate::i18n::tr(" Operação interrompida entre os itens.")
                } else {
                    ""
                }
            ),
            &report.errors,
            report.cancelled,
        );
        if !report.errors.is_empty() {
            self.error = Some(crate::i18n::message(
                "Falhas ao esvaziar a lixeira:\n{}",
                &[report
                    .errors
                    .iter()
                    .take(3)
                    .cloned()
                    .collect::<Vec<_>>()
                    .join("\n")],
            ));
        }
        self.active_empty_trash = None;
        self.refresh_after_transfer = true;
    }

    pub(super) fn cancel_empty_trash(&self) {
        if let Some(operation) = &self.active_empty_trash {
            operation.cancel.store(true, Ordering::Relaxed);
        }
    }

    pub(super) fn empty_trash_status(&mut self, ui: &mut egui::Ui) {
        let Some(operation) = &self.active_empty_trash else {
            return;
        };
        let text = crate::i18n::message(
            "Esvaziando lixeira {}/{} · {}",
            &[
                operation.progress.done.to_string(),
                operation.progress.total.to_string(),
                operation.progress.current.display().to_string(),
            ],
        );
        ui.horizontal_wrapped(|ui| {
            ui.spinner();
            ui.label(text);
            if ui
                .button(crate::i18n::tr("Interromper após este item"))
                .clicked()
            {
                self.cancel_empty_trash();
            }
        });
    }

    pub(super) fn empty_trash_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut prompt) = self.empty_trash_prompt.take() else {
            return;
        };
        let mut confirm = false;
        let mut cancel = false;
        let modal = egui::Modal::new(egui::Id::new("empty_trash_confirmation")).show(ctx, |ui| {
            ui.spacing_mut().interact_size.y = 32.0;
            ui.set_width((ctx.content_rect().width() - 80.0).clamp(350.0, 620.0));
            ui.heading(crate::i18n::tr("Esvaziar lixeira?"));
            ui.label(crate::i18n::tr("Os itens listados serão excluídos permanentemente, incluindo o conteúdo das pastas."));
            ui.label(crate::i18n::tr("Esta ação não pode ser desfeita pela lixeira."));
            let entries = prompt.listing.as_ref().map(|listing| listing.entries.as_slice()).unwrap_or_default();
            ui.label(if prompt.listing.is_some() {
                crate::i18n::plural("{} item", "{} itens", entries.len())
            } else { crate::i18n::tr("Carregando lixeira…").to_owned() });
            empty_trash_items(ui, &prompt);
            if prompt.listing.as_ref().is_some_and(|listing| listing.skipped > 0) {
                ui.label(crate::i18n::tr("Alguns itens da lixeira não puderam ser listados e foram preservados."));
            }
            ui.separator();
            egui::Frame::new()
                .fill(ui.visuals().faint_bg_color)
                .corner_radius(4.0)
                .inner_margin(egui::Margin::symmetric(12, 10))
                .show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    ui.spacing_mut().item_spacing.x = 10.0;
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let action = egui::Button::new(
                            RichText::new(crate::i18n::tr("Esvaziar lixeira")).color(Color32::WHITE),
                        ).fill(Color32::from_rgb(182, 45, 45)).min_size(Vec2::new(190.0, 36.0));
                        confirm = ui.add_enabled(!entries.is_empty(), action).clicked();
                        let button = ui.add(
                            egui::Button::new(crate::i18n::tr("Cancelar")).min_size(Vec2::new(96.0, 36.0)),
                        );
                        if prompt.focus_cancel { button.request_focus(); prompt.focus_cancel = false; }
                        cancel = button.clicked();
                    });
                });
        });
        if cancel || modal.should_close() {
            return;
        }
        if confirm && let Some(listing) = prompt.listing {
            self.start_empty_trash(listing, ctx);
        } else {
            self.empty_trash_prompt = Some(prompt);
        }
    }
}

fn empty_trash_items(ui: &mut egui::Ui, prompt: &EmptyTrashPrompt) {
    let entries = prompt
        .listing
        .as_ref()
        .map(|listing| listing.entries.as_slice())
        .unwrap_or_default();
    egui::Frame::new()
        .fill(ui.visuals().extreme_bg_color)
        .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
        .corner_radius(4.0)
        .inner_margin(8.0)
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 4.0;
            egui::ScrollArea::vertical()
                .id_salt(prompt.id)
                .min_scrolled_height(230.0)
                .max_height(230.0)
                .auto_shrink([false, false])
                .show_rows(ui, 28.0, entries.len().max(1), |ui, rows| {
                    if let Some(error) = &prompt.error {
                        ui.colored_label(ui.visuals().error_fg_color, error);
                    } else if prompt.listing.is_none() {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label(crate::i18n::tr("Carregando lixeira…"));
                        });
                    } else if entries.is_empty() {
                        ui.label(
                            if prompt
                                .listing
                                .as_ref()
                                .is_some_and(|listing| listing.skipped > 0)
                            {
                                crate::i18n::tr("Nenhum item disponível para exclusão.")
                            } else {
                                crate::i18n::tr("A lixeira está vazia")
                            },
                        );
                    } else {
                        for row in rows {
                            let entry = &entries[row];
                            ui.allocate_ui_with_layout(
                                Vec2::new(ui.available_width(), 28.0),
                                Layout::left_to_right(Align::Center),
                                |ui| {
                                    ui.add(egui::Label::new(&entry.name).truncate())
                                        .on_hover_text(
                                            entry
                                                .trash
                                                .as_ref()
                                                .map(|record| {
                                                    record.original_path.display().to_string()
                                                })
                                                .unwrap_or_default(),
                                        );
                                },
                            );
                        }
                    }
                });
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stale_listing_results_never_replace_a_newer_confirmation_or_reopen_a_cancelled_one() {
        let ctx = egui::Context::default();
        let mut app = Explorer::with_context(&ctx, PathBuf::from("/"));
        app.empty_trash_prompt = Some(EmptyTrashPrompt {
            id: 2,
            listing: None,
            error: None,
            focus_cancel: true,
        });
        let listing = || Listing {
            path: crate::trashbin::location(),
            entries: Vec::new(),
            skipped: 0,
        };
        app.receive_empty_trash_listing(1, Ok(listing()));
        assert!(app.empty_trash_prompt.as_ref().unwrap().listing.is_none());
        app.receive_empty_trash_listing(2, Ok(listing()));
        assert!(app.empty_trash_prompt.as_ref().unwrap().listing.is_some());
        app.empty_trash_prompt = None;
        app.receive_empty_trash_listing(2, Ok(listing()));
        assert!(app.empty_trash_prompt.is_none() && app.active_empty_trash.is_none());
    }
}
