use super::*;
use crate::deletions as removal;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

pub(super) struct ActiveDeletion {
    pub progress: removal::Progress,
    cancel: Arc<AtomicBool>,
}

enum Targets {
    Files(Vec<removal::Target>),
    Trash(Vec<Entry>),
}

impl Targets {
    fn len(&self) -> usize {
        match self {
            Self::Files(items) => items.len(),
            Self::Trash(items) => items.len(),
        }
    }
    fn display_path(&self, index: usize) -> &Path {
        match self {
            Self::Files(items) => &items[index].path,
            Self::Trash(items) => items[index]
                .trash
                .as_ref()
                .map(|record| record.original_path.as_path())
                .unwrap_or(&items[index].path),
        }
    }
}

pub(super) struct Confirmation {
    targets: Targets,
    mode: removal::Mode,
    focus_cancel: bool,
}

impl Explorer {
    pub(super) fn delete_dropped_entries(&mut self, entries: Vec<Entry>, ctx: &egui::Context) {
        if !self.file_drop_available() || entries.is_empty() {
            return;
        }
        let targets = Targets::Files(
            entries
                .into_iter()
                .map(|entry| removal::Target {
                    path: entry.path,
                    device: entry.device,
                    inode: entry.inode,
                })
                .collect(),
        );
        self.prepare_deletion(targets, removal::Mode::Trash, ctx);
    }

    fn can_delete(&self) -> bool {
        !self.loading
            && !self.is_applications()
            && self.dnd.trash_prepare.is_none()
            && self.active_restore.is_none()
            && self.active_empty_trash.is_none()
            && self.empty_trash_prompt.is_none()
            && !self.close_after_transfer
            && !self.selected.is_empty()
            && self.active_transfer.is_none()
            && self.active_deletion.is_none()
            && self.rename.is_none()
            && self.delete_confirmation.is_none()
    }

    pub(super) fn request_delete(&mut self, mode: removal::Mode, ctx: &egui::Context) {
        if !self.can_delete() {
            return;
        }
        let entries = self.selected_deletion_entries();
        if entries.is_empty() {
            return;
        }
        let (mode, targets) = if self.is_trash() {
            (
                removal::Mode::Permanent,
                Targets::Trash(entries.into_iter().cloned().collect()),
            )
        } else {
            (
                mode,
                Targets::Files(
                    entries
                        .into_iter()
                        .map(|entry| removal::Target {
                            path: entry.path.clone(),
                            device: entry.device,
                            inode: entry.inode,
                        })
                        .collect(),
                ),
            )
        };
        self.prepare_deletion(targets, mode, ctx);
    }

    fn prepare_deletion(&mut self, targets: Targets, mut mode: removal::Mode, ctx: &egui::Context) {
        if mode == removal::Mode::Trash
            && let Targets::Files(items) = &targets
        {
            let ftp = items
                .iter()
                .filter(|item| self.is_ftp_item(&item.path))
                .count();
            if ftp > 0 {
                // Never silently promote local items in a mixed selection to
                // permanent deletion just because another item is on FTP.
                if ftp != items.len() {
                    self.post_typed_notice(super::toast::NoticeKind::Warning, crate::i18n::tr("Selecione os itens FTP separadamente para excluir. Eles não podem ser enviados à lixeira.").into());
                    return;
                }
                mode = removal::Mode::Permanent;
            }
        }
        if mode == removal::Mode::Trash && !self.confirm_trash {
            self.start_deletion(targets, mode, ctx);
        } else {
            self.delete_confirmation = Some(Confirmation {
                targets,
                mode,
                focus_cancel: true,
            });
        }
    }

    fn is_ftp_item(&self, path: &Path) -> bool {
        crate::network::location::requires_permanent_deletion(path, &self.sidebar_options.remote)
    }

    fn selected_ftp(&self) -> (bool, bool) {
        let entries = self.selected_deletion_entries();
        let any = entries.iter().any(|entry| self.is_ftp_item(&entry.path));
        (
            any,
            any && entries.iter().all(|entry| self.is_ftp_item(&entry.path)),
        )
    }

    fn selected_deletion_entries(&self) -> Vec<&Entry> {
        filesystem::visible_entries(
            &self.entries,
            &self.query,
            self.hidden_visible(),
            self.sort,
            self.descending,
        )
        .into_iter()
        .map(|i| &self.entries[i])
        .filter(|entry| self.selected.contains(&entry.path))
        .collect()
    }

    pub(super) fn deletion_shortcuts(&mut self, ctx: &egui::Context) {
        if ctx.wants_keyboard_input() {
            return;
        }
        // Ignore auto-repeat, Ctrl+Delete (text editing), and other modifier combinations.
        let modifiers = ctx.input(|i| {
            i.events.iter().find_map(|event| match event {
                egui::Event::Key {
                    key: Key::Delete,
                    pressed: true,
                    repeat: false,
                    modifiers,
                    ..
                } if !modifiers.ctrl
                    && !modifiers.command
                    && !modifiers.alt
                    && !modifiers.mac_cmd =>
                {
                    Some(*modifiers)
                }
                _ => None,
            })
        });
        if let Some(modifiers) = modifiers {
            ctx.input_mut(|i| {
                i.consume_key(modifiers, Key::Delete);
            });
            self.request_delete(
                if modifiers.shift {
                    removal::Mode::Permanent
                } else {
                    removal::Mode::Trash
                },
                ctx,
            );
        }
    }

    fn start_deletion(&mut self, targets: Targets, mode: removal::Mode, ctx: &egui::Context) {
        let progress = removal::Progress {
            total: targets.len(),
            ..Default::default()
        };
        let cancel = Arc::new(AtomicBool::new(false));
        let worker_cancel = cancel.clone();
        let sender = self.event_sender.clone();
        let context = ctx.clone();
        let locale = crate::i18n::current();
        let worker = thread::Builder::new()
            .name("file-deletion".into())
            .spawn(move || {
                crate::i18n::set_locale(locale);
                let mut last_update = std::time::Instant::now() - std::time::Duration::from_secs(1);
                let mut notify = |progress: removal::Progress| {
                    if last_update.elapsed() >= std::time::Duration::from_millis(50)
                        || progress.done == progress.total
                    {
                        let _ = sender.send(Event::DeletionProgress(progress));
                        context.request_repaint();
                        last_update = std::time::Instant::now();
                    }
                };
                let report = match targets {
                    Targets::Files(items) => removal::run(items, mode, &worker_cancel, &mut notify),
                    Targets::Trash(items) => {
                        let report = crate::trashbin::purge(items, worker_cancel, |progress| {
                            notify(removal::Progress {
                                done: progress.done,
                                total: progress.total,
                                current: progress.current,
                            });
                        });
                        removal::Report {
                            mode: removal::Mode::Permanent,
                            completed: report.completed,
                            errors: report.errors,
                            cancelled: report.cancelled,
                        }
                    }
                };
                let _ = sender.send(Event::DeletionFinished(report));
                context.request_repaint();
            });
        match worker {
            Ok(_) => {
                self.active_deletion = Some(ActiveDeletion { progress, cancel });
                self.error = None;
            }
            Err(error) => self.report_error(crate::i18n::message(
                "Não foi possível iniciar a exclusão: {error}",
                &[(error).to_string()],
            )),
        }
    }

    pub(super) fn finish_deletion(&mut self, report: removal::Report) {
        self.finish_native_trash(
            report.completed.len(),
            !report.cancelled && report.errors.is_empty(),
        );
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
        let (one, many) = if report.mode == removal::Mode::Trash {
            ("{} item enviado à lixeira.", "{} itens enviados à lixeira.")
        } else {
            (
                "{} item excluído permanentemente.",
                "{} itens excluídos permanentemente.",
            )
        };
        self.post_operation_notice(
            format!(
                "{}{}",
                crate::i18n::plural(one, many, report.completed.len()),
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
                "{} falha(s) na exclusão:\n{}",
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
        self.active_deletion = None;
        self.refresh_after_transfer = true;
    }

    pub(super) fn cancel_deletion(&self) {
        if let Some(deletion) = &self.active_deletion {
            deletion.cancel.store(true, Ordering::Relaxed);
        }
    }

    pub(super) fn deletion_menu(&mut self, ui: &mut egui::Ui) {
        ui.separator();
        let (any_ftp, all_ftp) = self.selected_ftp();
        for (mode, icon, label, shortcut) in [
            (
                removal::Mode::Trash,
                Icon::Trash,
                crate::i18n::tr("Mover para a lixeira"),
                "Del",
            ),
            (
                removal::Mode::Permanent,
                Icon::DeletePermanently,
                crate::i18n::tr("Excluir permanentemente…"),
                "Shift+Del",
            ),
        ] {
            if (self.is_trash() || any_ftp) && mode == removal::Mode::Trash {
                continue;
            }
            let shortcut = if self.is_trash() {
                "Del"
            } else if all_ftp {
                "Del / Shift+Del"
            } else {
                shortcut
            };
            if self
                .menu_item(ui, icon, label, shortcut, self.can_delete(), false)
                .clicked()
            {
                self.request_delete(mode, ui.ctx());
                ui.close();
            }
        }
    }

    pub(super) fn deletion_status(&mut self, ui: &mut egui::Ui) {
        let Some(deletion) = &self.active_deletion else {
            return;
        };
        let text = crate::i18n::message(
            "Excluindo {}/{} · {}",
            &[
                (deletion.progress.done).to_string(),
                (deletion.progress.total).to_string(),
                (deletion.progress.current.display()).to_string(),
            ],
        );
        ui.horizontal_wrapped(|ui| {
            ui.spinner();
            ui.label(text);
            if ui
                .button(crate::i18n::tr("Interromper após este item"))
                .clicked()
            {
                self.cancel_deletion();
            }
        });
    }

    pub(super) fn deletion_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut confirmation) = self.delete_confirmation.take() else {
            return;
        };
        let mut confirm = false;
        let mut cancel = false;
        let permanent = confirmation.mode == removal::Mode::Permanent;
        let modal =
            egui::Modal::new(egui::Id::new(("deletion_confirmation", permanent))).show(ctx, |ui| {
                ui.spacing_mut().interact_size.y = 32.0;
                ui.set_width((ctx.content_rect().width() - 80.0).clamp(350.0, 620.0));
                ui.heading(if permanent {
                    crate::i18n::tr("Excluir permanentemente?")
                } else {
                    crate::i18n::tr("Enviar para a lixeira?")
                });
                ui.label(format!(
                    "{} {}",
                    crate::i18n::plural(
                        "{} item selecionado.",
                        "{} itens selecionados.",
                        confirmation.targets.len()
                    ),
                    if permanent {
                        crate::i18n::tr("Esta ação não pode ser desfeita pela lixeira.")
                    } else {
                        crate::i18n::tr("Os itens poderão ser restaurados pela lixeira do sistema.")
                    }
                ));
                if let Targets::Files(items) = &confirmation.targets && items.iter().any(|item| self.is_ftp_item(&item.path)) {
                    ui.label(crate::i18n::tr("Os itens FTP serão excluídos diretamente do servidor, sem passar pela lixeira."));
                }
                egui::Frame::new()
                    .fill(ui.visuals().extreme_bg_color)
                    .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
                    .corner_radius(4.0)
                    .inner_margin(8.0)
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = 4.0;
                        egui::ScrollArea::vertical()
                            .id_salt("deletion_items")
                            .min_scrolled_height(230.0)
                            .max_height(230.0)
                            .auto_shrink([false, false])
                            .show_rows(ui, 28.0, confirmation.targets.len(), |ui, rows| {
                                for row in rows {
                                    let path = confirmation.targets.display_path(row);
                                    ui.allocate_ui_with_layout(
                                        Vec2::new(ui.available_width(), 28.0),
                                        Layout::left_to_right(Align::Center),
                                        |ui| {
                                            ui.add(
                                                egui::Label::new(
                                                    path.file_name()
                                                        .unwrap_or_default()
                                                        .to_string_lossy(),
                                                )
                                                .truncate(),
                                            )
                                            .on_hover_text(path.display().to_string());
                                        },
                                    );
                                }
                            });
                    });
                ui.separator();
                egui::Frame::new()
                    .fill(ui.visuals().faint_bg_color)
                    .corner_radius(4.0)
                    .inner_margin(egui::Margin::symmetric(12, 10))
                    .show(ui, |ui| {
                        ui.set_min_width(ui.available_width());
                        ui.spacing_mut().item_spacing.x = 10.0;
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            let action = if permanent {
                                egui::Button::new(
                                    RichText::new(crate::i18n::tr("Excluir permanentemente"))
                                        .color(Color32::WHITE),
                                )
                                .fill(Color32::from_rgb(182, 45, 45))
                            } else {
                                egui::Button::new(
                                    RichText::new(crate::i18n::tr("Mover para a lixeira"))
                                        .color(ui.visuals().selection.stroke.color),
                                )
                                .fill(ui.visuals().selection.bg_fill)
                            };
                            confirm = ui.add(action.min_size(Vec2::new(190.0, 36.0))).clicked();
                            let button = ui.add(
                                egui::Button::new(crate::i18n::tr("Cancelar"))
                                    .min_size(Vec2::new(96.0, 36.0)),
                            );
                            if confirmation.focus_cancel {
                                button.request_focus();
                                confirmation.focus_cancel = false;
                            }
                            cancel = button.clicked();
                        });
                    });
            });
        if cancel || modal.should_close() {
            return;
        }
        if confirm {
            self.start_deletion(confirmation.targets, confirmation.mode, ctx);
        } else {
            self.delete_confirmation = Some(confirmation);
        }
    }
}

#[cfg(test)]
mod tests;
