use super::*;
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

pub(super) struct ActiveTransfer {
    pub progress: file_ops::Progress,
    pub clipboard_id: u64,
    pub mode: file_ops::Mode,
    cancel: Arc<AtomicBool>,
}

pub(super) struct ConflictPrompt {
    pub(super) conflict: file_ops::Conflict,
    pub(super) reply: Sender<file_ops::Decision>,
    pub(super) apply_all: bool,
}

impl Explorer {
    pub(super) fn cut_opacity(&self, path: &Path) -> f32 {
        if self.file_clipboard.as_ref().is_some_and(|clipboard| {
            clipboard.mode == file_ops::Mode::Cut && clipboard.paths.iter().any(|p| p == path)
        }) {
            0.45
        } else {
            1.0
        }
    }

    fn cancel_pending_cut(&mut self, ctx: &egui::Context) -> bool {
        let Some(clipboard) = &self.file_clipboard else {
            return false;
        };
        if clipboard.mode != file_ops::Mode::Cut
            || self.cut_transfer_running
            || self.active_transfer.as_ref().is_some_and(|job| {
                job.clipboard_id == clipboard.id && job.mode == file_ops::Mode::Cut
            })
        {
            return false;
        }
        self.file_clipboard = None;
        self.file_clipboard_text.clear();
        self.clipboard_serial = self.clipboard_serial.wrapping_add(1);
        ctx.request_repaint();
        true
    }

    pub(super) fn process_transfer_refresh(&mut self) {
        if self.refresh_after_transfer && !self.loading {
            self.refresh_after_transfer = false;
            let error = self.error.take();
            self.refresh();
            if error.is_some() {
                self.error = error;
            }
        }
    }

    pub(super) fn capture_files(&mut self, mode: file_ops::Mode, ctx: &egui::Context) {
        if self.is_trash() || self.is_applications() {
            return;
        }
        let paths: Vec<_> = filesystem::visible_entries(
            &self.entries,
            &self.query,
            self.hidden_visible(),
            self.sort,
            self.descending,
        )
        .into_iter()
        .map(|i| self.entries[i].path.clone())
        .filter(|path| self.selected.contains(path))
        .collect();
        if paths.is_empty() {
            return;
        }
        self.clipboard_serial = self.clipboard_serial.wrapping_add(1);
        self.file_clipboard_text = paths
            .iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()
            .join("\n");
        self.file_clipboard = Some(file_ops::Clipboard {
            id: self.clipboard_serial,
            mode,
            paths,
        });
        // egui emits native Paste events only when the system clipboard contains text.
        // Keep exact PathBufs internally; this text also makes the clipboard useful in text fields.
        ctx.copy_text(self.clipboard_text());
        let count = self.file_clipboard.as_ref().map_or(0, |c| c.paths.len());
        let (one, many) = if mode == file_ops::Mode::Cut {
            (
                "{} item recortado. Abra a pasta de destino e use Colar.",
                "{} itens recortados. Abra a pasta de destino e use Colar.",
            )
        } else {
            (
                "{} item copiado. Abra a pasta de destino e use Colar.",
                "{} itens copiados. Abra a pasta de destino e use Colar.",
            )
        };
        self.post_notice(crate::i18n::plural(one, many, count));
    }

    fn clipboard_text(&self) -> String {
        if self.file_clipboard.is_some() {
            self.file_clipboard_text.clone()
        } else {
            String::new()
        }
    }

    pub(super) fn can_paste(&self) -> bool {
        !self.loading
            && self.dnd.trash_prepare.is_none()
            && !self.is_trash()
            && !self.is_applications()
            && self.active_restore.is_none()
            && self.active_empty_trash.is_none()
            && self.empty_trash_prompt.is_none()
            && !self.close_after_transfer
            && !self.current.as_os_str().is_empty()
            && self.active_transfer.is_none()
            && self.active_deletion.is_none()
            && self.rename.is_none()
            && self.delete_confirmation.is_none()
            && self
                .file_clipboard
                .as_ref()
                .is_some_and(|c| !c.paths.is_empty())
    }

    pub(super) fn paste_files(&mut self, ctx: &egui::Context) {
        self.paste_files_into(self.current.clone(), ctx);
    }

    fn paste_files_into(&mut self, destination: PathBuf, ctx: &egui::Context) {
        if !self.can_paste() {
            return;
        }
        let Some(clipboard) = self.file_clipboard.clone() else {
            return;
        };
        self.start_file_transfer(clipboard, destination, ctx);
    }

    pub(super) fn start_file_transfer(
        &mut self,
        clipboard: file_ops::Clipboard,
        destination: PathBuf,
        ctx: &egui::Context,
    ) {
        let clipboard_id = clipboard.id;
        let mode = clipboard.mode;
        let request = file_ops::Request {
            clipboard,
            destination,
        };
        let progress = file_ops::Progress {
            total: request.clipboard.paths.len(),
            ..Default::default()
        };
        let cancel = Arc::new(AtomicBool::new(false));
        let worker_cancel = cancel.clone();
        let sender = self.event_sender.clone();
        let context = ctx.clone();
        let locale = crate::i18n::current();
        let worker = thread::Builder::new()
            .name("file-transfer".into())
            .spawn(move || {
                crate::i18n::set_locale(locale);
                let conflict_sender = sender.clone();
                let conflict_context = context.clone();
                let mut last_update = Instant::now() - Duration::from_secs(1);
                let report = file_ops::run(
                    request,
                    worker_cancel,
                    move |conflict| {
                        let (reply, answer) = mpsc::channel();
                        let prompt = ConflictPrompt {
                            conflict,
                            reply,
                            apply_all: false,
                        };
                        if conflict_sender
                            .send(Event::TransferConflict(Box::new(prompt)))
                            .is_err()
                        {
                            return file_ops::Decision {
                                choice: file_ops::Choice::Cancel,
                                apply_all: false,
                            };
                        }
                        conflict_context.request_repaint();
                        answer.recv().unwrap_or(file_ops::Decision {
                            choice: file_ops::Choice::Cancel,
                            apply_all: false,
                        })
                    },
                    |progress| {
                        if last_update.elapsed() >= Duration::from_millis(50)
                            || progress.done == progress.total
                        {
                            let _ = sender.send(Event::TransferProgress(progress));
                            context.request_repaint();
                            last_update = Instant::now();
                        }
                    },
                );
                let _ = sender.send(Event::TransferFinished(report));
                context.request_repaint();
            });
        match worker {
            Ok(_) => {
                self.active_transfer = Some(ActiveTransfer {
                    progress,
                    cancel,
                    clipboard_id,
                    mode,
                });
                self.error = None;
            }
            Err(error) => self.report_error(crate::i18n::message(
                "Não foi possível iniciar a transferência: {error}",
                &[(error).to_string()],
            )),
        }
    }

    pub(super) fn finish_transfer(&mut self, report: file_ops::Report) {
        self.finish_native_transfer(&report);
        if report.mode == file_ops::Mode::Cut
            && let Some(clipboard) = &mut self.file_clipboard
            && clipboard.id == report.clipboard_id
        {
            clipboard
                .paths
                .retain(|path| !report.completed.contains(path));
            if clipboard.paths.is_empty() {
                self.file_clipboard = None;
            }
        }
        self.post_operation_notice(
            format!(
                "{}; {}{}",
                crate::i18n::plural(
                    "{} item concluído",
                    "{} itens concluídos",
                    report.completed.len()
                ),
                crate::i18n::plural("{} item ignorado.", "{} itens ignorados.", report.skipped),
                if report.cancelled {
                    crate::i18n::tr(" Transferência cancelada.")
                } else {
                    ""
                }
            ),
            &report.errors,
            report.cancelled || report.skipped > 0,
        );
        if !report.errors.is_empty() {
            self.error = Some(crate::i18n::message(
                "{} falha(s) na transferência:\n{}",
                &[
                    (report.errors.len()).to_string(),
                    (report
                        .errors
                        .iter()
                        .take(3)
                        .cloned()
                        .collect::<Vec<_>>()
                        .join("\n"))
                    .to_string(),
                ],
            ));
        }
        self.active_transfer = None;
        self.transfer_conflict = None;
        self.refresh_after_transfer = true;
    }

    pub(super) fn transfer_shortcuts(&mut self, ctx: &egui::Context) {
        let (copy, cut, paste, text) = ctx.input(|i| {
            let command = i.modifiers.command || i.modifiers.ctrl;
            let copy = i.events.iter().any(|e| matches!(e, egui::Event::Copy))
                || command && i.key_pressed(Key::C);
            let cut = i.events.iter().any(|e| matches!(e, egui::Event::Cut))
                || command && i.key_pressed(Key::X);
            let text = i.events.iter().find_map(|e| {
                if let egui::Event::Paste(text) = e {
                    Some(text.clone())
                } else {
                    None
                }
            });
            (
                copy,
                cut,
                text.is_some() || command && i.key_pressed(Key::V),
                text,
            )
        });
        if ctx.wants_keyboard_input() {
            if copy || cut {
                self.file_clipboard = None;
            }
            return;
        }
        if ctx.input(|i| i.key_pressed(Key::Escape)) && self.cancel_pending_cut(ctx) {
            return;
        }
        if copy {
            self.capture_files(file_ops::Mode::Copy, ctx);
        } else if cut {
            self.capture_files(file_ops::Mode::Cut, ctx);
        } else if paste {
            if text.is_some_and(|text| text != self.clipboard_text().replace("\r\n", "\n")) {
                self.file_clipboard = None;
                return;
            }
            self.paste_files(ctx);
        }
        if copy || cut || paste {
            ctx.input_mut(|i| {
                i.events.retain(|e| {
                    !matches!(
                        e,
                        egui::Event::Copy | egui::Event::Cut | egui::Event::Paste(_)
                    )
                })
            });
        }
    }

    pub(super) fn file_transfer_menu(&mut self, ui: &mut egui::Ui, entry: &Entry) {
        if self
            .menu_item(
                ui,
                Icon::Copy,
                crate::i18n::tr("Copiar"),
                "Ctrl+C",
                !self.selected.is_empty(),
                false,
            )
            .clicked()
        {
            self.capture_files(file_ops::Mode::Copy, ui.ctx());
            ui.close();
        }
        if self
            .menu_item(
                ui,
                Icon::Cut,
                crate::i18n::tr("Recortar"),
                "Ctrl+X",
                !self.selected.is_empty(),
                false,
            )
            .clicked()
        {
            self.capture_files(file_ops::Mode::Cut, ui.ctx());
            ui.close();
        }
        let destination = if entry.is_dir {
            entry.path.clone()
        } else {
            self.current.clone()
        };
        self.paste_menu_into(ui, destination);
    }

    pub(super) fn paste_menu(&mut self, ui: &mut egui::Ui) {
        self.paste_menu_into(ui, self.current.clone());
    }

    fn paste_menu_into(&mut self, ui: &mut egui::Ui, destination: PathBuf) {
        if self
            .menu_item(
                ui,
                Icon::Paste,
                crate::i18n::tr("Colar"),
                "Ctrl+V",
                self.can_paste(),
                false,
            )
            .clicked()
        {
            self.paste_files_into(destination, ui.ctx());
            ui.close();
        }
    }

    pub(super) fn cancel_transfer(&mut self) {
        if let Some(transfer) = &self.active_transfer {
            transfer.cancel.store(true, Ordering::Relaxed);
        }
        if let Some(prompt) = self.transfer_conflict.take() {
            let _ = prompt.reply.send(file_ops::Decision {
                choice: file_ops::Choice::Cancel,
                apply_all: false,
            });
        }
    }

    pub(super) fn receive_transfer_conflict(&mut self, prompt: ConflictPrompt) {
        let cancelled = self
            .active_transfer
            .as_ref()
            .map(|transfer| transfer.cancel.load(Ordering::Relaxed))
            .or_else(|| {
                self.active_restore
                    .as_ref()
                    .map(|restore| restore.cancel.load(Ordering::Relaxed))
            })
            .unwrap_or(true);
        if self.close_after_transfer || cancelled {
            let _ = prompt.reply.send(file_ops::Decision {
                choice: file_ops::Choice::Cancel,
                apply_all: false,
            });
        } else {
            self.transfer_conflict = Some(prompt);
        }
    }

    pub(super) fn handle_transfer_close(&mut self, ctx: &egui::Context) {
        if ctx.input(|i| i.viewport().close_requested())
            && (self.active_transfer.is_some()
                || self.active_deletion.is_some()
                || self.active_restore.is_some()
                || self.active_empty_trash.is_some()
                || self.print_cancel.is_some()
                || self.image_pending()
                || self.rename_pending()
                || self.dnd.trash_prepare.is_some())
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.close_after_transfer = true;
            self.cancel_transfer();
            self.cancel_deletion();
            self.cancel_restore();
            self.cancel_empty_trash();
            self.cancel_images();
            self.cancel_print();
        }
        if self.close_after_transfer
            && self.active_transfer.is_none()
            && self.active_deletion.is_none()
            && self.active_restore.is_none()
            && self.active_empty_trash.is_none()
            && self.print_cancel.is_none()
            && !self.image_pending()
            && !self.rename_pending()
            && self.dnd.trash_prepare.is_none()
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    pub(super) fn transfer_status(&mut self, ui: &mut egui::Ui) {
        if self.active_transfer.is_none() {
            return;
        }
        ui.scope(|ui| {
            ui.spacing_mut().interact_size.y = 32.0;
            if let Some(transfer) = &self.active_transfer {
                let text = crate::i18n::message(
                    "Transferindo {}/{} · {} · {}",
                    &[
                        (transfer.progress.done).to_string(),
                        (transfer.progress.total).to_string(),
                        (filesystem::format_size(transfer.progress.bytes)).to_string(),
                        (transfer
                            .progress
                            .current
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy())
                        .to_string(),
                    ],
                );
                ui.horizontal_wrapped(|ui| {
                    ui.spinner();
                    ui.label(text);
                    if ui
                        .button(crate::i18n::tr("Cancelar transferência"))
                        .clicked()
                    {
                        self.cancel_transfer();
                    }
                });
            }
        });
    }

    pub(super) fn transfer_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut prompt) = self.transfer_conflict.take() else {
            return;
        };
        let restoring = self.active_restore.is_some();
        let mut choice = None;
        let modal = egui::Modal::new(egui::Id::new("file_conflict")).show(ctx, |ui| {
            ui.spacing_mut().interact_size.y = 32.0;
            ui.set_width((ctx.content_rect().width() - 80.0).clamp(350.0, 620.0));
            ui.heading(crate::i18n::tr("Já existe um item com este nome"));
            if !restoring {
                ui.label(crate::i18n::message(
                    "Origem: {}",
                    &[(prompt.conflict.source.display()).to_string()],
                ));
            }
            ui.label(crate::i18n::message(
                "Destino: {}",
                &[(prompt.conflict.destination.display()).to_string()],
            ));
            ui.label(if restoring {
                crate::i18n::tr(
                    "O destino original já existe. Manter ambos restaura com outro nome.",
                )
            } else {
                crate::i18n::tr("Manter ambos cria uma cópia com outro nome.")
            });
            ui.checkbox(
                &mut prompt.apply_all,
                crate::i18n::tr("Aplicar aos próximos conflitos"),
            );
            ui.horizontal(|ui| {
                if ui.button(crate::i18n::tr("Manter ambos")).clicked() {
                    choice = Some(file_ops::Choice::KeepBoth);
                }
                if ui.button(crate::i18n::tr("Ignorar")).clicked() {
                    choice = Some(file_ops::Choice::Skip);
                }
                if ui
                    .button(if restoring {
                        crate::i18n::tr("Cancelar restauração")
                    } else {
                        crate::i18n::tr("Cancelar transferência")
                    })
                    .clicked()
                {
                    choice = Some(file_ops::Choice::Cancel);
                }
            });
        });
        if modal.should_close() {
            choice = Some(file_ops::Choice::Cancel);
        }
        if let Some(choice) = choice {
            let _ = prompt.reply.send(file_ops::Decision {
                choice,
                apply_all: prompt.apply_all,
            });
        } else {
            self.transfer_conflict = Some(prompt);
        }
    }
}
