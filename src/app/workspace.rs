use super::*;
mod closed_tabs;
mod split;
mod tab_bar;

pub struct Workspace {
    tabs: Vec<Explorer>,
    active: usize,
    next_id: u64,
    shared: Preferences,
    settings_path: Option<PathBuf>,
    window: Option<crate::file_manager::window::Window>,
    closing: bool,
    session: crate::session::Store,
    closed_tabs: std::collections::VecDeque<closed_tabs::ClosedTab>,
    split: Option<split::SplitView>,
}
impl Workspace {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        path: PathBuf,
        selection: Vec<PathBuf>,
        properties: bool,
        startup: crate::session::Startup,
    ) -> Self {
        let first_path = startup
            .saved
            .as_ref()
            .and_then(|state| state.paths().ok())
            .and_then(|paths| paths.into_iter().next())
            .unwrap_or(path);
        let mut first = Explorer::new(cc, first_path);
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
        let mut app = Self {
            tabs: vec![first],
            active: 0,
            next_id: 1,
            shared,
            settings_path,
            window,
            closing: false,
            session: startup.store,
            closed_tabs: Default::default(),
            split: None,
        };
        if let Some(saved) = startup.saved {
            app.restore_tabs(&saved, &cc.egui_ctx);
        } else {
            app.restore_closed_history(&startup.closed_tabs, false);
        }
        if let Some(error) = startup.error {
            app.tabs[app.active].post_typed_notice(
                toast::NoticeKind::Error,
                crate::i18n::message(
                    "Não foi possível carregar a sessão de abas: {error}",
                    &[error],
                ),
            );
        }
        app
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
        self.assign_split_tab(index);
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
        self.append_tab(path, ctx);
        self.activate(self.tabs.len() - 1, ctx);
    }
    fn append_tab(&mut self, path: PathBuf, ctx: &egui::Context) {
        let mut tab = Explorer::with_context(ctx, path);
        tab.local_share_sidebar = self.tabs[self.active].local_share_sidebar.clone();
        tab.tab_id = self.next_id;
        self.next_id += 1;
        tab.settings_path = self.settings_path.clone();
        tab.apply_preferences(self.shared.clone(), ctx);
        tab.file_clipboard = self.tabs[self.active].file_clipboard.clone();
        tab.file_clipboard_text = self.tabs[self.active].file_clipboard_text.clone();
        tab.clipboard_serial = self.tabs[self.active].clipboard_serial;
        self.tabs.push(tab);
    }
    fn restore_tabs(&mut self, state: &crate::session::State, ctx: &egui::Context) {
        let Ok(paths) = state.paths() else { return };
        for path in paths.into_iter().skip(1) {
            self.append_tab(path, ctx);
        }
        self.activate(state.active.min(self.tabs.len() - 1), ctx);
        self.share();
        self.restore_closed_history(&state.closed_tabs, true);
        self.restore_split(state);
    }
    fn session_state(&self) -> crate::session::State {
        let mut state = crate::session::State::from_paths(
            self.tabs.iter().map(|tab| {
                if tab.current.as_os_str().is_empty() {
                    tab.initial_path.clone()
                } else {
                    tab.current.clone()
                }
            }),
            self.active,
        );
        state.split = self.split_state();
        // Serialized open IDs are their indices. Canonicalizing keeps snapshots stable
        // after a restart and avoids restoring arbitrary counters from disk.
        let ids: BTreeMap<_, _> = self
            .tabs
            .iter()
            .map(|tab| tab.tab_id)
            .chain(self.closed_tabs.iter().map(|tab| tab.id))
            .enumerate()
            .map(|(i, id)| (id, i as u64))
            .collect();
        state.closed_tabs = self
            .closed_tabs
            .iter()
            .cloned()
            .map(|mut tab| {
                tab.id = ids[&tab.id];
                tab.left = tab.left.and_then(|id| ids.get(&id).copied());
                tab.right = tab.right.and_then(|id| ids.get(&id).copied());
                tab
            })
            .collect();
        state
    }
    fn persist_session(&mut self, force: bool) {
        if let Err(error) = self.session.save(self.session_state(), force) {
            self.tabs[self.active].post_typed_notice(
                toast::NoticeKind::Error,
                crate::i18n::message(
                    "Não foi possível salvar a sessão de abas: {error}",
                    &[error.to_string()],
                ),
            );
        }
    }
    fn close_tab(&mut self, index: usize, ctx: &egui::Context) {
        if self.tabs[index].busy() {
            return;
        }
        if self.tabs.len() == 1 {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }
        self.remember_closed(index);
        if index == self.active {
            self.activate(if index == 0 { 1 } else { index - 1 }, ctx);
        }
        self.tabs.remove(index);
        if index < self.active {
            self.active -= 1;
        }
        self.normalize_split();
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
}
impl Workspace {
    fn show(&mut self, ctx: &egui::Context) {
        let busy_before: Vec<_> = self
            .tabs
            .iter()
            .map(|tab| (tab.tab_id, tab.busy()))
            .collect();
        self.background();
        let moving: Vec<_> = self
            .tabs
            .iter()
            .filter_map(|tab| {
                tab.active_transfer
                    .as_ref()
                    .filter(|job| job.mode == file_ops::Mode::Cut)
                    .map(|job| job.clipboard_id)
            })
            .collect();
        for tab in &mut self.tabs {
            tab.cut_transfer_running = tab.file_clipboard.as_ref().is_some_and(|clipboard| {
                clipboard.mode == file_ops::Mode::Cut && moving.contains(&clipboard.id)
            });
        }
        self.normalize_split();
        self.focus_split(ctx);
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
                tab.cancel_images();
                tab.cancel_print();
            }
        }
        self.tab_bar(ctx);
        self.normalize_split();
        for tab in &mut self.tabs {
            tab.split_view = self.split.is_some();
        }
        if self.split.is_some() {
            self.show_split(ctx);
        } else {
            self.tabs[self.active].show(ctx);
        }
        if std::mem::take(&mut self.tabs[self.active].requested_split) {
            self.toggle_split(ctx);
        }
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
        self.persist_session(false);
    }
}
impl eframe::App for Workspace {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.show(ctx);
    }
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.persist_session(true);
    }
}
impl Explorer {
    pub(super) fn location(&self) -> &Path {
        self.loading_path.as_deref().unwrap_or_else(|| {
            if self.current.as_os_str().is_empty() {
                &self.initial_path
            } else {
                &self.current
            }
        })
    }
    fn busy(&self) -> bool {
        self.active_transfer.is_some()
            || self.active_deletion.is_some()
            || self.active_restore.is_some()
            || self.active_empty_trash.is_some()
            || self.print_cancel.is_some()
            || self.image_pending()
            || self.rename_pending()
            || self.local_shares_pending()
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
            || self.network_dialog.is_some()
            || self.local_shares.is_some()
            || self.image_editor.is_some()
            || self.preview.is_some()
    }
}
#[cfg(test)]
mod tests;
