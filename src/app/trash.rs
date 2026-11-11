use super::*;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

pub(super) struct ActiveTrashOperation {
    pub progress: file_ops::Progress,
    pub cancel: Arc<AtomicBool>,
}

impl Explorer {
    pub(super) fn is_trash(&self) -> bool {
        crate::trashbin::is_location(&self.current)
    }

    pub(super) fn can_restore(&self) -> bool {
        self.is_trash()
            && self.dnd.trash_prepare.is_none()
            && !self.loading
            && !self.close_after_transfer
            && !self.selected.is_empty()
            && self.active_transfer.is_none()
            && self.active_deletion.is_none()
            && self.active_restore.is_none()
            && self.active_empty_trash.is_none()
            && self.rename.is_none()
            && self.empty_trash_prompt.is_none()
    }

    pub(super) fn restore_selected(&mut self, ctx: &egui::Context) {
        if !self.can_restore() {
            return;
        }
        let entries: Vec<_> = filesystem::visible_entries(
            &self.entries,
            &self.query,
            self.hidden_visible(),
            self.sort,
            self.descending,
        )
        .into_iter()
        .map(|index| &self.entries[index])
        .filter(|entry| self.selected.contains(&entry.path))
        .cloned()
        .collect();
        if entries.is_empty() {
            return;
        }
        let cancel = Arc::new(AtomicBool::new(false));
        let worker_cancel = cancel.clone();
        let progress = file_ops::Progress {
            total: entries.len(),
            ..Default::default()
        };
        let sender = self.event_sender.clone();
        let context = ctx.clone();
        let locale = crate::i18n::current();
        let worker = thread::Builder::new()
            .name("trash-restore".into())
            .spawn(move || {
                crate::i18n::set_locale(locale);
                let conflict_sender = sender.clone();
                let conflict_context = context.clone();
                let mut last_update = std::time::Instant::now() - std::time::Duration::from_secs(1);
                let report = crate::trashbin::restore(
                    entries,
                    worker_cancel,
                    move |conflict| {
                        let (reply, answer) = mpsc::channel();
                        let prompt = transfers::ConflictPrompt {
                            conflict,
                            reply,
                            apply_all: false,
                        };
                        if conflict_sender
                            .send(Event::TransferConflict(Box::new(prompt)))
                            .is_ok()
                        {
                            conflict_context.request_repaint();
                            if let Ok(decision) = answer.recv() {
                                return decision;
                            }
                        }
                        file_ops::Decision {
                            choice: file_ops::Choice::Cancel,
                            apply_all: false,
                        }
                    },
                    |progress| {
                        if last_update.elapsed() >= std::time::Duration::from_millis(50)
                            || progress.done == progress.total
                        {
                            let _ = sender.send(Event::RestoreProgress(progress));
                            context.request_repaint();
                            last_update = std::time::Instant::now();
                        }
                    },
                );
                let _ = sender.send(Event::RestoreFinished(report));
                context.request_repaint();
            });
        match worker {
            Ok(_) => {
                self.active_restore = Some(ActiveTrashOperation { progress, cancel });
                self.error = None;
            }
            Err(error) => self.report_error(crate::i18n::message(
                "Não foi possível iniciar a restauração: {}",
                &[error.to_string()],
            )),
        }
    }

    pub(super) fn finish_restore(&mut self, report: crate::trashbin::Report) {
        self.selected
            .retain(|path| !report.completed.contains(path));
        if self
            .selection_anchor
            .as_ref()
            .is_some_and(|path| report.completed.contains(path))
        {
            self.selection_anchor = None;
        }
        if self
            .selection_cursor
            .as_ref()
            .is_some_and(|path| report.completed.contains(path))
        {
            self.selection_cursor = None;
            self.reveal_selection = false;
        }
        self.post_operation_notice(
            format!(
                "{} {}{}",
                crate::i18n::plural(
                    "{} item restaurado.",
                    "{} itens restaurados.",
                    report.completed.len()
                ),
                crate::i18n::plural("{} item ignorado.", "{} itens ignorados.", report.skipped),
                if report.cancelled {
                    crate::i18n::tr(" Restauração cancelada.")
                } else {
                    ""
                }
            ),
            &report.errors,
            report.cancelled || report.skipped > 0,
        );
        if !report.errors.is_empty() {
            self.error = Some(crate::i18n::message(
                "Falhas na restauração:\n{}",
                &[report
                    .errors
                    .iter()
                    .take(3)
                    .cloned()
                    .collect::<Vec<_>>()
                    .join("\n")],
            ));
        }
        self.active_restore = None;
        self.refresh_after_transfer = true;
    }

    pub(super) fn cancel_restore(&mut self) {
        if let Some(restore) = &self.active_restore {
            restore.cancel.store(true, Ordering::Relaxed);
        }
        self.cancel_transfer();
    }

    pub(super) fn restore_status(&mut self, ui: &mut egui::Ui) {
        let Some(restore) = &self.active_restore else {
            return;
        };
        let text = crate::i18n::message(
            "Restaurando {}/{} · {}",
            &[
                restore.progress.done.to_string(),
                restore.progress.total.to_string(),
                restore.progress.current.display().to_string(),
            ],
        );
        ui.horizontal_wrapped(|ui| {
            ui.spinner();
            ui.label(text);
            if ui.button(crate::i18n::tr("Cancelar restauração")).clicked() {
                self.cancel_restore();
            }
        });
    }

    pub(super) fn trash_context_menu(&mut self, ui: &mut egui::Ui, entry: &Entry) {
        if self
            .menu_item(
                ui,
                Icon::Restore,
                crate::i18n::tr("Restaurar"),
                "",
                self.can_restore(),
                false,
            )
            .clicked()
        {
            self.restore_selected(ui.ctx());
            ui.close();
        }
        self.deletion_menu(ui);
        ui.separator();
        if self
            .menu_item(
                ui,
                Icon::CopyPath,
                crate::i18n::tr("Copiar caminho original"),
                "",
                true,
                false,
            )
            .clicked()
        {
            let paths = filesystem::visible_entries(
                &self.entries,
                &self.query,
                self.hidden_visible(),
                self.sort,
                self.descending,
            )
            .into_iter()
            .map(|i| &self.entries[i])
            .filter(|entry| self.selected.contains(&entry.path))
            .filter_map(|entry| entry.trash.as_ref())
            .map(|record| record.original_path.display().to_string())
            .collect::<Vec<_>>()
            .join("\n");
            self.file_clipboard = None;
            ui.ctx().copy_text(paths);
            ui.close();
        }
        if self
            .menu_item(
                ui,
                Icon::Properties,
                crate::i18n::tr("Propriedades"),
                "",
                true,
                false,
            )
            .clicked()
        {
            self.properties = Some(entry.clone());
            ui.close();
        }
    }

    pub(super) fn default_current_layout(&self) -> FolderLayout {
        if self.is_trash() {
            FolderLayout {
                view: View::Details,
                columns: vec![
                    Column::Name,
                    Column::OriginalPath,
                    Column::Deleted,
                    Column::Kind,
                    Column::Size,
                ],
                sort: Column::Deleted,
                descending: true,
                column_widths: BTreeMap::new(),
                ..self.default_layout.clone()
            }
        } else {
            self.default_layout.clone()
        }
    }
}
