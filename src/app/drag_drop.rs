use super::*;
use crate::native_dnd::{Action, Completion, Event as NativeEvent, Native, modifier_action};

struct PendingDrop {
    position: egui::Pos2,
    paths: Vec<PathBuf>,
    internal: bool,
    action: Option<Action>,
    completion: Option<Completion>,
}
enum DropJob {
    Transfer(u64),
    Trash,
}
struct PendingCompletion {
    token: Completion,
    action: Action,
    job: DropJob,
    expected: usize,
}

#[derive(Default)]
pub(super) struct DragDrop {
    pub native: Option<Native>,
    pub trash_prepare: Option<Receiver<std::io::Result<Vec<Entry>>>>,
    paths: Vec<PathBuf>,
    native_active: bool,
    refresh_native: bool,
    hover: Option<egui::Pos2>,
    pending: Option<PendingDrop>,
    completion: Option<PendingCompletion>,
    feedback: Option<Option<Action>>,
    targets: Vec<(egui::Rect, PathBuf)>,
}

impl Explorer {
    pub(super) fn drag_blocks_tabs(&self) -> bool {
        self.dnd.native_active
            || self.dnd.pending.is_some()
            || self.dnd.completion.is_some()
            || self.dnd.trash_prepare.is_some()
    }
    fn finish_native_drop(&mut self, success: bool) {
        if let Some(pending) = self.dnd.completion.take()
            && let Some(native) = &self.dnd.native
        {
            native.complete(pending.token, success.then_some(pending.action));
        }
    }
    pub(super) fn finish_native_transfer(&mut self, report: &file_ops::Report) {
        if let Some(pending) = &self.dnd.completion
            && matches!(pending.job,DropJob::Transfer(id) if id==report.clipboard_id)
        {
            let success = !report.cancelled
                && report.errors.is_empty()
                && report.skipped == 0
                && report.completed.len() == pending.expected;
            self.finish_native_drop(success);
        }
    }
    pub(super) fn finish_native_trash(&mut self, completed: usize, success: bool) {
        if let Some(pending) = &self.dnd.completion
            && matches!(pending.job, DropJob::Trash)
        {
            self.finish_native_drop(success && completed == pending.expected);
        }
    }
    pub(super) fn file_drop_available(&self) -> bool {
        !self.loading
            && !self.close_after_transfer
            && self.dnd.trash_prepare.is_none()
            && self.active_transfer.is_none()
            && self.active_deletion.is_none()
            && self.active_restore.is_none()
            && self.active_empty_trash.is_none()
            && self.rename.is_none()
            && self.open_with.is_none()
            && self.folder_picker.is_none()
            && self.new_folder.is_none()
            && self.properties.is_none()
            && self.settings_editor.is_none()
            && self.delete_confirmation.is_none()
            && self.empty_trash_prompt.is_none()
    }
    pub(super) fn begin_drag_drop(&mut self, ctx: &egui::Context) {
        self.dnd.targets.clear();
        let prepared =
            self.dnd
                .trash_prepare
                .as_ref()
                .and_then(|receiver| match receiver.try_recv() {
                    Ok(result) => Some(result),
                    Err(mpsc::TryRecvError::Disconnected) => Some(Err(std::io::Error::other(
                        crate::i18n::tr("O serviço de leitura foi encerrado."),
                    ))),
                    Err(mpsc::TryRecvError::Empty) => None,
                });
        if let Some(result) = prepared {
            self.dnd.trash_prepare = None;
            match result {
                Ok(entries) => self.delete_dropped_entries(entries, ctx),
                Err(error) => {
                    self.error = Some(crate::i18n::message(
                        "Não foi possível preparar os arquivos arrastados: {}",
                        &[error.to_string()],
                    ))
                }
            }
        }
        if self
            .dnd
            .completion
            .as_ref()
            .is_some_and(|pending| matches!(pending.job, DropJob::Trash))
            && self.dnd.trash_prepare.is_none()
            && self.delete_confirmation.is_none()
            && self.active_deletion.is_none()
        {
            self.finish_native_drop(false);
        }
        if let Some(native) = &mut self.dnd.native {
            native.scale(ctx.pixels_per_point());
            for event in native.poll(ctx.input(|input| input.modifiers)) {
                match event {
                    NativeEvent::Hover(position) => self.dnd.hover = Some(position),
                    NativeEvent::Left => self.dnd.hover = None,
                    NativeEvent::Dropped {
                        position,
                        paths,
                        internal,
                        action,
                        completion,
                    } => {
                        if self.dnd.pending.is_some() {
                            if let Some(token) = completion {
                                native.complete(token, None);
                            }
                        } else {
                            self.dnd.pending = Some(PendingDrop {
                                position,
                                paths,
                                internal,
                                action: Some(action),
                                completion,
                            });
                        }
                    }
                    NativeEvent::Action(action) => self.dnd.feedback = Some(action),
                    NativeEvent::Started => {
                        self.dnd.native_active = true;
                        self.dnd.feedback = None;
                    }
                    NativeEvent::Ended => {
                        self.dnd.native_active = false;
                        self.dnd.paths.clear();
                        self.dnd.hover = None;
                        self.dnd.refresh_native = true;
                        self.dnd.feedback = None;
                    }
                    NativeEvent::Error(error) => {
                        self.error = Some(crate::i18n::message(
                            "Falha no arrastar e soltar: {}",
                            &[error],
                        ));
                        self.dnd.native_active = false;
                        self.dnd.paths.clear();
                        self.dnd.hover = None;
                    }
                }
            }
        }
        // Legacy X11 sources may ignore XdndProxy. Winit still supplies their files.
        let files = ctx.input(|input| {
            input
                .raw
                .dropped_files
                .iter()
                .filter_map(|file| file.path.clone())
                .collect::<Vec<_>>()
        });
        if !files.is_empty()
            && self.dnd.pending.is_none()
            && let Some(position) = self.dnd.hover.or_else(|| ctx.pointer_hover_pos())
        {
            self.dnd.pending = Some(PendingDrop {
                position,
                paths: files,
                internal: false,
                action: None,
                completion: None,
            });
        }
    }
    pub(super) fn drag_hover_scroll(&self, ui: &egui::Ui) {
        if self.dnd.paths.is_empty() && self.dnd.hover.is_none() {
            return;
        }
        if let Some(position) = self.dnd.hover.or_else(|| ui.ctx().pointer_hover_pos()) {
            let rect = ui.clip_rect();
            if rect.contains(position) {
                let delta = if position.y < rect.top() + 28.0 {
                    14.0
                } else if position.y > rect.bottom() - 28.0 {
                    -14.0
                } else {
                    0.0
                };
                if delta != 0.0 {
                    ui.scroll_with_delta(Vec2::new(0.0, delta));
                    ui.ctx()
                        .request_repaint_after(std::time::Duration::from_millis(16));
                }
            }
        }
    }
    pub(super) fn register_drop_target(&mut self, ui: &egui::Ui, rect: egui::Rect, path: PathBuf) {
        let rect = rect.intersect(ui.clip_rect());
        if !rect.is_positive() {
            return;
        }
        self.dnd.targets.push((rect, path.clone()));
        let position = self.dnd.hover.or_else(|| {
            (!self.dnd.paths.is_empty())
                .then(|| ui.ctx().pointer_hover_pos())
                .flatten()
        });
        if let Some(position) = position
            && rect.contains(position)
            && self.drop_destination_allowed(&path, &self.dnd.paths)
        {
            ui.painter().rect_stroke(
                rect.shrink(1.0),
                4.0,
                (2.0, ui.visuals().selection.stroke.color),
                egui::StrokeKind::Inside,
            );
        }
    }
    fn drop_destination_allowed(&self, destination: &Path, paths: &[PathBuf]) -> bool {
        self.file_drop_available()
            && !destination.as_os_str().is_empty()
            && !paths
                .iter()
                .any(|path| destination == path || destination.starts_with(path))
    }
    pub(super) fn file_drag_response(
        &mut self,
        ui: &egui::Ui,
        response: &egui::Response,
        entry: &Entry,
        visible: &[usize],
    ) {
        if entry.is_dir && !self.is_trash() {
            self.register_drop_target(ui, response.rect, entry.path.clone());
        }
        if response.drag_started_by(egui::PointerButton::Primary)
            && !self.is_trash()
            && self.file_drop_available()
        {
            if !self.selected.contains(&entry.path) {
                self.select_entry(&entry.path, visible, egui::Modifiers::NONE);
            }
            self.dnd.paths = visible
                .iter()
                .map(|&i| self.entries[i].path.clone())
                .filter(|path| self.selected.contains(path))
                .collect();
            if let Some(native) = &mut self.dnd.native {
                native.arm(self.dnd.paths.clone(), response.ctx.content_rect().size());
            }
        }
    }
    pub(super) fn finish_drag_drop(&mut self, ctx: &egui::Context) {
        let escape =
            ctx.input(|input| input.key_pressed(Key::Escape) || input.viewport().close_requested());
        if escape {
            if let Some(native) = &self.dnd.native {
                native.cancel();
            }
            self.dnd.paths.clear();
            self.dnd.hover = None;
            if let Some(pending) = self.dnd.pending.take()
                && let Some(token) = pending.completion
                && let Some(native) = &self.dnd.native
            {
                native.complete(token, None);
            }
        }
        if !self.dnd.native_active
            && !self.dnd.paths.is_empty()
            && ctx.input(|input| input.pointer.any_released())
        {
            if let Some(position) = ctx.pointer_interact_pos() {
                self.dnd.pending = Some(PendingDrop {
                    position,
                    paths: std::mem::take(&mut self.dnd.paths),
                    internal: true,
                    action: None,
                    completion: None,
                });
            }
            if let Some(native) = &self.dnd.native {
                native.cancel();
            }
        }
        let position = self.dnd.hover.or_else(|| ctx.pointer_hover_pos());
        let hovered = position
            .and_then(|position| {
                self.dnd
                    .targets
                    .iter()
                    .rev()
                    .find(|(rect, _)| rect.contains(position))
            })
            .map(|(_, path)| path.clone());
        if let Some(native) = &self.dnd.native {
            native.incoming_policy(
                hovered
                    .as_ref()
                    .is_some_and(|path| self.drop_destination_allowed(path, &self.dnd.paths)),
                self.file_drop_available(),
                ctx.input(|input| input.modifiers),
            );
        }
        if let Some(pending) = self.dnd.pending.take() {
            let mut started = false;
            let expected = pending.paths.len();
            let action = if pending.internal {
                modifier_action(ctx.input(|input| input.modifiers)).unwrap_or(Action::Move)
            } else {
                pending.action.unwrap_or_else(|| {
                    modifier_action(ctx.input(|input| input.modifiers)).unwrap_or(Action::Copy)
                })
            };
            if let Some((_, destination)) = self
                .dnd
                .targets
                .iter()
                .rev()
                .find(|(rect, _)| rect.contains(pending.position))
                .cloned()
            {
                let mode = if action == Action::Move {
                    file_ops::Mode::Cut
                } else {
                    file_ops::Mode::Copy
                };
                started = self.drop_files(pending.paths, destination, mode, ctx);
            }
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
                } else if let Some(native) = &self.dnd.native {
                    native.complete(token, None);
                }
            }
            self.dnd.paths.clear();
            self.dnd.hover = None;
        }
        if std::mem::take(&mut self.dnd.refresh_native) {
            self.refresh_after_transfer = true;
            ctx.request_repaint();
        }
        if !self.dnd.paths.is_empty() && (!self.dnd.native_active || self.dnd.hover.is_some()) {
            ctx.request_repaint_after(std::time::Duration::from_millis(16));
            if let Some(position) = position {
                ctx.set_cursor_icon(
                    if hovered
                        .as_ref()
                        .is_some_and(|path| self.drop_destination_allowed(path, &self.dnd.paths))
                    {
                        match modifier_action(ctx.input(|input| input.modifiers))
                            .unwrap_or(Action::Move)
                        {
                            Action::Copy => egui::CursorIcon::Copy,
                            Action::Move => egui::CursorIcon::Default,
                        }
                    } else {
                        egui::CursorIcon::NoDrop
                    },
                );
                egui::Area::new(egui::Id::new("file_drag_preview"))
                    .fixed_pos(position + Vec2::new(16.0, 18.0))
                    .interactable(false)
                    .order(egui::Order::Tooltip)
                    .show(ctx, |ui| {
                        egui::Frame::popup(ui.style()).show(ui, |ui| {
                            ui.label(crate::i18n::plural(
                                "{} item",
                                "{} itens",
                                self.dnd.paths.len(),
                            ));
                        });
                    });
            }
        }
        if self.dnd.native_active {
            let action = if self.dnd.hover.is_some() {
                hovered
                    .as_ref()
                    .filter(|path| self.drop_destination_allowed(path, &self.dnd.paths))
                    .map(|_| {
                        modifier_action(ctx.input(|input| input.modifiers)).unwrap_or(Action::Move)
                    })
            } else {
                self.dnd.feedback.unwrap_or_else(|| {
                    Some(
                        modifier_action(ctx.input(|input| input.modifiers)).unwrap_or(Action::Copy),
                    )
                })
            };
            ctx.set_cursor_icon(match action {
                Some(Action::Copy) => egui::CursorIcon::Copy,
                Some(Action::Move) => egui::CursorIcon::Default,
                None => egui::CursorIcon::NoDrop,
            });
        }
    }
    pub(super) fn drop_files(
        &mut self,
        paths: Vec<PathBuf>,
        destination: PathBuf,
        mode: file_ops::Mode,
        ctx: &egui::Context,
    ) -> bool {
        if paths.is_empty() || !self.drop_destination_allowed(&destination, &paths) {
            return false;
        }
        // Ignore drops back into the source parent, rather than manufacturing conflicts.
        let mut seen = BTreeSet::new();
        let paths: Vec<_> = paths
            .into_iter()
            .filter(|path| {
                path.is_absolute()
                    && path.parent() != Some(destination.as_path())
                    && seen.insert(path.clone())
            })
            .collect();
        if paths.is_empty() {
            return false;
        }
        if crate::trashbin::is_location(&destination) {
            let (sender, receiver) = mpsc::channel();
            let context = ctx.clone();
            thread::spawn(move || {
                let result = paths
                    .iter()
                    .map(|path| filesystem::read_entry(path))
                    .collect();
                let _ = sender.send(result);
                context.request_repaint();
            });
            self.dnd.trash_prepare = Some(receiver);
        } else {
            self.clipboard_serial = self.clipboard_serial.wrapping_add(1);
            self.start_file_transfer(
                file_ops::Clipboard {
                    id: self.clipboard_serial,
                    mode,
                    paths,
                },
                destination,
                ctx,
            );
        }
        self.dnd.trash_prepare.is_some() || self.active_transfer.is_some()
    }
}

#[cfg(test)]
mod tests;
