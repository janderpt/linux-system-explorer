use super::*;

pub(super) struct Choice {
    pub(super) paths: Vec<PathBuf>,
    pub(super) destination: PathBuf,
    pub(super) preferred: Action,
    pub(super) allowed: Actions,
    completion: Option<Completion>,
}
impl Explorer {
    pub(in crate::app) fn has_drop_choice(&self) -> bool {
        self.dnd.choice.is_some()
    }
    pub(super) fn reject_drop(&self, token: Option<Completion>) {
        if let (Some(native), Some(token)) = (&self.dnd.native, token) {
            native.complete(token, None);
        }
    }
    pub(in crate::app) fn cancel_drop_choice(&mut self) {
        if let Some(choice) = self.dnd.choice.take() {
            self.reject_drop(choice.completion);
        }
    }
    pub(super) fn offer_drop(
        &mut self,
        mut pending: PendingDrop,
        destination: PathBuf,
        preferred: Action,
    ) {
        if !self.drop_destination_allowed(&destination, &pending.paths) || self.has_drop_choice() {
            self.reject_drop(pending.completion);
            return;
        }
        let mut seen = BTreeSet::new();
        pending.paths.retain(|path| {
            path.is_absolute()
                && path.parent() != Some(destination.as_path())
                && seen.insert(path.clone())
        });
        if pending.paths.is_empty() {
            self.reject_drop(pending.completion);
            return;
        }
        if pending.internal {
            // The native transaction is ours too: end it without a filesystem
            // action and carry out the user's explicit choice internally.
            self.reject_drop(pending.completion.take());
            pending.allowed = Actions::BOTH;
        }
        let preferred = if pending.allowed.allows(preferred) {
            preferred
        } else if pending.allowed.copy {
            Action::Copy
        } else {
            Action::Move
        };
        self.dnd.choice = Some(Choice {
            paths: pending.paths,
            destination,
            preferred,
            allowed: pending.allowed,
            completion: pending.completion,
        });
        self.type_ahead = Default::default();
    }
    pub(super) fn perform_drop(
        &mut self,
        pending: PendingDrop,
        destination: PathBuf,
        action: Action,
        ctx: &egui::Context,
    ) {
        if !pending.allowed.allows(action) {
            self.reject_drop(pending.completion);
            return;
        }
        let expected = pending.paths.len();
        let mode = if action == Action::Copy {
            file_ops::Mode::Copy
        } else {
            file_ops::Mode::Cut
        };
        let started = self.drop_files(pending.paths, destination, mode, ctx);
        if let Some(token) = pending.completion {
            if started {
                let job = if self.dnd.trash_prepare.is_some() {
                    DropJob::Trash
                } else {
                    DropJob::Transfer(self.clipboard_serial)
                };
                self.dnd.completion = Some(PendingCompletion {
                    token,
                    action,
                    job,
                    expected,
                });
            } else {
                self.reject_drop(Some(token));
            }
        }
    }
    pub(in crate::app) fn drop_choice_dialog(&mut self, ctx: &egui::Context) {
        let Some(choice) = self.dnd.choice.as_ref() else {
            return;
        };
        let mut action = None;
        let mut cancel = false;
        let available = !self.loading;
        let modal = egui::Modal::new(egui::Id::new("drop_choice")).show(ctx, |ui| {
            ui.set_width((ctx.content_rect().width() - 60.0).clamp(320.0, 500.0));
            ui.heading(crate::i18n::tr("Copiar ou mover?"));
            ui.label(crate::i18n::plural(
                "{} item selecionado.",
                "{} itens selecionados.",
                choice.paths.len(),
            ));
            egui::ScrollArea::vertical()
                .id_salt("drop_choice_sources")
                .max_height(120.0)
                .show_rows(ui, 24.0, choice.paths.len(), |ui, rows| {
                    for row in rows {
                        let path = &choice.paths[row];
                        ui.add(
                            egui::Label::new(
                                path.file_name().unwrap_or_default().to_string_lossy(),
                            )
                            .truncate(),
                        )
                        .on_hover_text(self.display_address(path));
                    }
                });
            let parents: BTreeSet<_> = choice
                .paths
                .iter()
                .filter_map(|path| path.parent())
                .collect();
            if parents.len() > 1 {
                ui.label(crate::i18n::tr("Origem: várias pastas"));
            } else if let Some(parent) = parents.first() {
                let origin = crate::i18n::message("Origem: {}", &[self.display_address(parent)]);
                ui.add(egui::Label::new(&origin).truncate())
                    .on_hover_text(origin);
            }
            let destination =
                crate::i18n::message("Destino: {}", &[self.display_address(&choice.destination)]);
            ui.add(egui::Label::new(&destination).truncate())
                .on_hover_text(destination);
            ui.separator();
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.button(crate::i18n::tr("Cancelar")).clicked() {
                    cancel = true;
                }
                for (operation, icon, label) in [
                    (Action::Move, Icon::Cut, "Mover"),
                    (Action::Copy, Icon::Copy, "Copiar"),
                ] {
                    let enabled = available && choice.allowed.allows(operation);
                    if self
                        .menu_item(
                            ui,
                            icon,
                            crate::i18n::tr(label),
                            "",
                            enabled,
                            operation == choice.preferred,
                        )
                        .on_disabled_hover_text(crate::i18n::tr(
                            "O aplicativo de origem não permite esta operação neste arraste.",
                        ))
                        .clicked()
                    {
                        action = Some(operation);
                    }
                }
            });
        });
        if cancel || modal.should_close() {
            self.cancel_drop_choice();
        } else if let Some(action) = action {
            let choice = self.dnd.choice.take().unwrap();
            let pending = PendingDrop {
                position: egui::Pos2::ZERO,
                paths: choice.paths,
                internal: false,
                action: Some(action),
                allowed: choice.allowed,
                completion: choice.completion,
            };
            self.perform_drop(pending, choice.destination, action, ctx);
        }
    }
}
