use super::*;
use std::io;

pub(super) struct RenameDialog {
    entry: Entry,
    name: String,
    focus: bool,
    error: Option<String>,
    result: Option<Receiver<io::Result<PathBuf>>>,
}

impl Explorer {
    pub(super) fn rename_pending(&self) -> bool {
        self.rename
            .as_ref()
            .is_some_and(|dialog| dialog.result.is_some())
    }

    pub(super) fn can_rename(&self) -> bool {
        !self.is_trash()
            && self.dnd.trash_prepare.is_none()
            && !self.loading
            && !self.close_after_transfer
            && self.selected.len() == 1
            && self.rename.is_none()
            && self.active_transfer.is_none()
            && self.active_deletion.is_none()
            && self.active_restore.is_none()
            && self.active_empty_trash.is_none()
            && self.empty_trash_prompt.is_none()
            && self.delete_confirmation.is_none()
    }

    pub(super) fn request_rename(&mut self, entry: &Entry) {
        if self.can_rename() && self.selected.contains(&entry.path) {
            self.rename = Some(RenameDialog {
                entry: entry.clone(),
                name: entry.name.clone(),
                focus: true,
                error: None,
                result: None,
            });
        }
    }

    pub(super) fn rename_shortcut(&mut self, ctx: &egui::Context) {
        if ctx.wants_keyboard_input() || !self.can_rename() {
            return;
        }
        let pressed = ctx.input(|input| input.events.iter().any(|event| matches!(event,
            egui::Event::Key { key: Key::F2, pressed: true, repeat: false, modifiers, .. } if modifiers.is_none()
        )));
        if !pressed {
            return;
        }
        ctx.input_mut(|input| {
            input.consume_key(egui::Modifiers::NONE, Key::F2);
        });
        let target = filesystem::visible_entries(
            &self.entries,
            &self.query,
            self.show_hidden,
            self.sort,
            self.descending,
        )
        .into_iter()
        .map(|i| &self.entries[i])
        .find(|entry| self.selected.contains(&entry.path))
        .cloned();
        if let Some(entry) = target {
            self.request_rename(&entry);
        }
    }

    pub(super) fn rename_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut dialog) = self.rename.take() else {
            return;
        };
        if let Some(result) =
            dialog
                .result
                .as_ref()
                .and_then(|receiver| match receiver.try_recv() {
                    Ok(result) => Some(result),
                    Err(mpsc::TryRecvError::Disconnected) => Some(Err(io::Error::other(
                        crate::i18n::tr("O serviço de renomeação foi encerrado."),
                    ))),
                    Err(mpsc::TryRecvError::Empty) => None,
                })
        {
            dialog.result = None;
            match result {
                Ok(destination) => {
                    self.finish_rename(&dialog.entry, &destination, ctx);
                    return;
                }
                Err(error) => {
                    dialog.error = Some(crate::i18n::message(
                        "Não foi possível renomear: {}",
                        &[error.to_string()],
                    ))
                }
            }
        }
        let busy = dialog.result.is_some();
        let mut accept = false;
        let mut cancel = false;
        let modal = egui::Modal::new(egui::Id::new("rename_dialog")).show(ctx, |ui| {
            ui.set_width((ctx.content_rect().width() - 70.0).clamp(300.0, 520.0));
            ui.heading(crate::i18n::tr("Renomear"));
            ui.label(crate::i18n::tr("Novo nome"));
            ui.add_enabled_ui(!busy, |ui| {
                let mut field = egui::TextEdit::singleline(&mut dialog.name)
                    .id_salt("rename_name")
                    .desired_width(f32::INFINITY)
                    .show(ui);
                if dialog.focus {
                    field.response.request_focus();
                    let selected = if dialog.entry.is_dir {
                        dialog.name.chars().count()
                    } else {
                        Path::new(&dialog.name)
                            .file_stem()
                            .and_then(|name| name.to_str())
                            .unwrap_or(&dialog.name)
                            .chars()
                            .count()
                    };
                    field
                        .state
                        .cursor
                        .set_char_range(Some(egui::text::CCursorRange::two(
                            egui::text::CCursor::new(0),
                            egui::text::CCursor::new(selected),
                        )));
                    field.state.store(ctx, field.response.id);
                    dialog.focus = false;
                }
                let valid = dialog
                    .entry
                    .path
                    .parent()
                    .is_some_and(|parent| filesystem::child_path(parent, &dialog.name).is_ok());
                if !valid {
                    ui.colored_label(
                        ui.visuals().error_fg_color,
                        crate::i18n::tr("Informe um nome sem barras, diferente de . e .."),
                    );
                }
                accept = valid
                    && field.response.lost_focus()
                    && ui.input(|input| input.key_pressed(Key::Enter));
                if let Some(error) = &dialog.error {
                    ui.colored_label(ui.visuals().error_fg_color, error);
                }
                ui.separator();
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    accept |= ui
                        .add_enabled(valid, egui::Button::new(crate::i18n::tr("Renomear")))
                        .clicked();
                    cancel = ui.button(crate::i18n::tr("Cancelar")).clicked();
                });
            });
            if busy {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(crate::i18n::tr("Renomeando…"));
                });
            }
        });
        if !busy && (cancel || modal.should_close()) {
            return;
        }
        if accept && !busy {
            let (sender, receiver) = mpsc::channel();
            let entry = dialog.entry.clone();
            let name = dialog.name.clone();
            let context = ctx.clone();
            let locale = crate::i18n::current();
            match thread::Builder::new()
                .name("file-rename".into())
                .spawn(move || {
                    crate::i18n::set_locale(locale);
                    let _ = sender.send(crate::renames::rename(&entry, &name));
                    context.request_repaint();
                }) {
                Ok(_) => {
                    dialog.result = Some(receiver);
                    dialog.error = None;
                }
                Err(error) => {
                    dialog.error = Some(crate::i18n::message(
                        "Não foi possível renomear: {}",
                        &[error.to_string()],
                    ))
                }
            }
        }
        self.rename = Some(dialog);
    }

    fn finish_rename(&mut self, entry: &Entry, destination: &Path, ctx: &egui::Context) {
        let old = &entry.path;
        let remap = |path: &Path| -> PathBuf {
            if path == old {
                destination.to_owned()
            } else if entry.is_dir
                && let Ok(relative) = path.strip_prefix(old)
            {
                destination.join(relative)
            } else {
                path.to_owned()
            }
        };
        self.selected = self.selected.iter().map(|path| remap(path)).collect();
        self.selection_anchor = self.selection_anchor.as_deref().map(remap);
        self.selection_cursor = self.selection_cursor.as_deref().map(remap);
        self.reveal_selection = true;
        for item in &mut self.entries {
            if item.path == *old {
                item.path = destination.to_owned();
                item.name = destination
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned();
                item.hidden = item.name.starts_with('.');
            }
        }
        if let Some(clipboard) = &mut self.file_clipboard {
            let paths: Vec<_> = clipboard.paths.iter().map(|path| remap(path)).collect();
            if paths != clipboard.paths {
                clipboard.paths = paths;
                self.file_clipboard_text = clipboard
                    .paths
                    .iter()
                    .map(|path| path.display().to_string())
                    .collect::<Vec<_>>()
                    .join("\n");
                ctx.copy_text(self.file_clipboard_text.clone());
            }
        }
        if entry.is_dir {
            for shortcut in self
                .sidebar_options
                .pinned
                .iter_mut()
                .chain(&mut self.sidebar_options.network)
            {
                shortcut.path = remap(&shortcut.path);
            }
            let moved: Vec<_> = self
                .folder_layouts
                .keys()
                .filter_map(|key| {
                    let path = settings::folder_path(key)?;
                    let new = remap(&path);
                    (path != new).then(|| (key.clone(), settings::folder_key(&new)))
                })
                .collect();
            let layouts: Vec<_> = moved
                .into_iter()
                .filter_map(|(old, new)| {
                    self.folder_layouts.remove(&old).map(|layout| (new, layout))
                })
                .collect();
            self.folder_layouts.extend(layouts);
            self.history.rename_folder(old, destination);
        }
        self.error = None;
        self.refresh_after_transfer = true;
        ctx.request_repaint();
    }
}

#[cfg(test)]
mod tests;
