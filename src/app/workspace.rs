use super::*;

pub struct Workspace {
    tabs: Vec<Explorer>,
    active: usize,
    next_id: u64,
    shared: Preferences,
    settings_path: Option<PathBuf>,
    window: Option<crate::file_manager::window::Window>,
    closing: bool,
}
impl Workspace {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        path: PathBuf,
        selection: Vec<PathBuf>,
        properties: bool,
    ) -> Self {
        let mut first = Explorer::new(cc, path);
        first.reveal_external(selection, properties);
        let shared = first.stored_preferences(&first.preferences());
        let settings_path = first.settings_path.clone();
        let window = match crate::file_manager::window::Window::attach(&cc.egui_ctx) {
            Ok(window) => Some(window),
            Err(error) => {
                eprintln!("Could not register explorer window: {error}");
                None
            }
        };
        Self {
            tabs: vec![first],
            active: 0,
            next_id: 1,
            shared,
            settings_path,
            window,
            closing: false,
        }
    }
    fn capture(&mut self, ctx: &egui::Context) {
        let active = &mut self.tabs[self.active];
        active.persist_preferences(ctx);
        self.shared = active.stored_preferences(&active.preferences());
    }
    fn activate(&mut self, index: usize, ctx: &egui::Context) {
        if index == self.active {
            return;
        }
        self.capture(ctx);
        let native = self.tabs[self.active].dnd.native.take();
        if let Some(native) = &native {
            native.cancel();
        }
        let activation = self.tabs[self.active].activation.take();
        let notices = std::mem::take(&mut self.tabs[self.active].notice_toasts);
        self.tabs[self.active].dnd = drag_drop::DragDrop::default();
        self.active = index;
        self.tabs[index].dnd.native = native;
        self.tabs[index].activation = activation;
        self.tabs[index].receive_notices(notices);
        self.tabs[index].folder_layouts = self.shared.folders.clone();
        self.tabs[index].default_layout = self.shared.layout();
        self.tabs[index].apply_global_preferences(&self.shared, ctx);
        if let Some(id) = ctx.memory(|m| m.focused()) {
            ctx.memory_mut(|m| m.surrender_focus(id));
        }
        egui::Popup::close_all(ctx);
    }
    fn add_tab(&mut self, path: PathBuf, ctx: &egui::Context) {
        self.capture(ctx);
        let mut tab = Explorer::with_context(ctx, path);
        tab.tab_id = self.next_id;
        self.next_id += 1;
        tab.settings_path = self.settings_path.clone();
        tab.apply_preferences(self.shared.clone(), ctx);
        tab.file_clipboard = self.tabs[self.active].file_clipboard.clone();
        tab.file_clipboard_text = self.tabs[self.active].file_clipboard_text.clone();
        tab.clipboard_serial = self.tabs[self.active].clipboard_serial;
        self.tabs.push(tab);
        self.activate(self.tabs.len() - 1, ctx);
    }
    fn close_tab(&mut self, index: usize, ctx: &egui::Context) {
        if self.tabs[index].busy() {
            return;
        }
        if self.tabs.len() == 1 {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }
        if index == self.active {
            self.activate(if index == 0 { 1 } else { index - 1 }, ctx);
        }
        self.tabs.remove(index);
        if index < self.active {
            self.active -= 1;
        }
    }
    fn external(&mut self, request: crate::file_manager::window::Request, ctx: &egui::Context) {
        let index = std::iter::once(self.active)
            .chain(0..self.tabs.len())
            .find(|&i| self.tabs[i].location() == request.folder());
        if let Some(index) = index {
            if index != self.active
                && (self.tabs[self.active].drag_blocks_tabs()
                    || self.tabs[self.active].has_dialog())
            {
                let _ = request.accept(Path::new(""));
                return;
            }
            self.activate(index, ctx);
            self.tabs[index].accept_external(request, ctx);
        } else {
            let _ = request.accept(Path::new(""));
        }
    }
    fn background(&mut self) {
        for index in 0..self.tabs.len() {
            if index == self.active {
                continue;
            }
            let old = self.tabs[index]
                .active_transfer
                .as_ref()
                .and_then(|_| self.tabs[index].file_clipboard.clone());
            self.tabs[index].poll();
            self.tabs[index].process_transfer_refresh();
            let notices = std::mem::take(&mut self.tabs[index].notice_toasts);
            self.tabs[self.active].receive_notices(notices);
            if old.is_some()
                && self.tabs[index].file_clipboard != old
                && self.tabs[self.active].file_clipboard == old
            {
                self.tabs[self.active].file_clipboard = self.tabs[index].file_clipboard.clone();
            }
        }
    }
    fn share(&mut self) {
        self.shared =
            self.tabs[self.active].stored_preferences(&self.tabs[self.active].preferences());
        let clipboard = self.tabs[self.active].file_clipboard.clone();
        let text = self.tabs[self.active].file_clipboard_text.clone();
        let serial = self.tabs[self.active].clipboard_serial;
        for (index, tab) in self.tabs.iter_mut().enumerate() {
            if index != self.active {
                tab.default_layout = self.shared.layout();
                if tab.folder_layouts != self.shared.folders {
                    tab.folder_layouts = self.shared.folders.clone();
                }
                if tab.file_clipboard != clipboard {
                    tab.file_clipboard = clipboard.clone();
                }
                if tab.file_clipboard_text != text {
                    tab.file_clipboard_text = text.clone();
                }
                tab.clipboard_serial = serial;
            }
        }
    }
    fn tab_bar(&mut self, ctx: &egui::Context) {
        let mut select = None;
        let mut close = None;
        let mut add = false;
        let locked = self.closing
            || self.tabs[self.active].drag_blocks_tabs()
            || self.tabs[self.active].has_dialog();
        egui::TopBottomPanel::top("explorer_tabs").show(ctx, |ui| {
            egui::ScrollArea::horizontal()
                .id_salt("tab_strip")
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        for (index, tab) in self.tabs.iter().enumerate() {
                            let path = tab.location();
                            let title = if tab.is_trash() {
                                crate::i18n::tr("Lixeira").to_owned()
                            } else {
                                path.file_name()
                                    .unwrap_or(path.as_os_str())
                                    .to_string_lossy()
                                    .into_owned()
                            };
                            let mut title: String = title.chars().take(25).collect();
                            if tab.busy() {
                                title.push_str(" ·");
                            }
                            ui.push_id(tab.tab_id, |ui| {
                                if ui
                                    .add_enabled(
                                        !locked,
                                        egui::Button::new(title).selected(index == self.active),
                                    )
                                    .on_hover_text(path.display().to_string())
                                    .clicked()
                                {
                                    select = Some(index);
                                }
                                if ui
                                    .add_enabled(
                                        !locked && !tab.busy(),
                                        egui::Button::new("×").small(),
                                    )
                                    .on_hover_text(crate::i18n::tr("Fechar aba · Ctrl+W"))
                                    .clicked()
                                {
                                    close = Some(index);
                                }
                            });
                        }
                        add = ui
                            .add_enabled(!locked, egui::Button::new("+"))
                            .on_hover_text(crate::i18n::tr("Nova aba · Ctrl+T"))
                            .clicked();
                    });
                });
        });
        if !locked {
            let terminal = self.tabs[self.active].terminal_focused(ctx);
            if !terminal && ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, Key::T)) {
                add = true;
            }
            if !terminal && ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, Key::W)) {
                close = Some(self.active);
            }
            let backward = ctx.input_mut(|i| {
                i.consume_key(egui::Modifiers::CTRL | egui::Modifiers::SHIFT, Key::Tab)
            });
            let forward = ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, Key::Tab));
            if backward || forward {
                select = Some(if backward {
                    (self.active + self.tabs.len() - 1) % self.tabs.len()
                } else {
                    (self.active + 1) % self.tabs.len()
                });
            }
        }
        if let Some(index) = select {
            self.activate(index, ctx);
        }
        if let Some(index) = close {
            self.close_tab(index, ctx);
        }
        if add {
            self.add_tab(dirs::home_dir().unwrap_or_else(|| "/".into()), ctx);
        }
    }
}
impl Workspace {
    fn show(&mut self, ctx: &egui::Context) {
        let busy_before: Vec<_> = self
            .tabs
            .iter()
            .map(|tab| (tab.tab_id, tab.busy()))
            .collect();
        self.background();
        let requests = self
            .window
            .as_ref()
            .map(|w| w.pending())
            .unwrap_or_default();
        for request in requests {
            self.external(request, ctx);
        }
        if ctx.input(|i| i.viewport().close_requested()) && self.tabs.iter().any(Explorer::busy) {
            self.closing = true;
            for tab in &mut self.tabs {
                tab.cancel_transfer();
                tab.cancel_deletion();
                tab.cancel_restore();
                tab.cancel_empty_trash();
            }
        }
        self.tab_bar(ctx);
        self.tabs[self.active].show(ctx);
        let completed = busy_before
            .into_iter()
            .any(|(id, busy)| busy && self.tabs.iter().any(|tab| tab.tab_id == id && !tab.busy()));
        if completed {
            for tab in &mut self.tabs {
                tab.refresh_after_transfer = true;
            }
        }
        let requests = std::mem::take(&mut self.tabs[self.active].requested_tabs);
        self.share();
        for path in requests {
            self.add_tab(path, ctx);
            ctx.request_repaint();
        }
        if self.closing {
            ctx.send_viewport_cmd(if self.tabs.iter().any(Explorer::busy) {
                egui::ViewportCommand::CancelClose
            } else {
                egui::ViewportCommand::Close
            });
            ctx.request_repaint_after(std::time::Duration::from_millis(30));
        }
    }
}
impl eframe::App for Workspace {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.show(ctx);
    }
}
impl Explorer {
    fn location(&self) -> &Path {
        self.loading_path.as_deref().unwrap_or(&self.current)
    }
    fn busy(&self) -> bool {
        self.active_transfer.is_some()
            || self.active_deletion.is_some()
            || self.active_restore.is_some()
            || self.active_empty_trash.is_some()
            || self.rename_pending()
            || self.dnd.trash_prepare.is_some()
    }
    pub(super) fn has_dialog(&self) -> bool {
        self.new_folder.is_some()
            || self.properties.is_some()
            || self.settings_editor.is_some()
            || self.transfer_conflict.is_some()
            || self.delete_confirmation.is_some()
            || self.empty_trash_prompt.is_some()
            || self.folder_picker.is_some()
            || self.open_with.is_some()
            || self.rename.is_some()
    }
}
#[cfg(test)]
mod tests;
