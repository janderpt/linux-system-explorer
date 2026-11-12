use crate::{
    columns::Column,
    filesystem::{self, Entry, Listing, SortBy},
    icons::{self, Icon},
    navigation::{History, Navigation},
    settings::{self, FolderLayout, Preferences, View},
    sidebar as locations, transfers as file_ops,
};
use eframe::egui::{self, Align, Color32, FontId, Key, Layout, RichText, Sense, Vec2};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
    sync::mpsc::{self, Receiver, Sender},
    thread,
};

mod breadcrumbs;
mod deletions;
mod drag_drop;
mod empty_trash;
mod folder_actions;
mod folder_picker;
mod local_shares;
mod network;
mod open_with;
mod preview;
mod properties;
mod rename;
mod selection;
mod sidebar;
mod table;
mod terminal_panel;
#[cfg(test)]
mod tests;
mod thumbnails;
mod toast;
mod toolbar;
mod transfers;
mod trash;
mod workspace;
pub use workspace::Workspace;

const BLUE: Color32 = Color32::from_rgb(0, 103, 192);

struct ReadRequest {
    path: PathBuf,
    navigation: Navigation,
    locale: crate::i18n::Locale,
}

enum Event {
    Listed(std::io::Result<Listing>, Navigation),
    OpenFailed(String),
    TransferProgress(file_ops::Progress),
    TransferConflict(Box<transfers::ConflictPrompt>),
    TransferFinished(file_ops::Report),
    DeletionProgress(crate::deletions::Progress),
    DeletionFinished(crate::deletions::Report),
    RestoreProgress(file_ops::Progress),
    RestoreFinished(crate::trashbin::Report),
    EmptyTrashListed(u64, std::io::Result<Listing>),
    EmptyTrashProgress(file_ops::Progress),
    EmptyTrashFinished(crate::trashbin::Report),
}

pub struct Explorer {
    terminal: Option<terminal_panel::Panel>,
    requested_tabs: Vec<PathBuf>,
    tab_id: u64,
    activation: Option<crate::activation::Activation>,
    loading_path: Option<PathBuf>,
    initial_path: PathBuf,
    current: PathBuf,
    entries: Vec<Entry>,
    history: History,
    address: String,
    editing_address: bool,
    folder_picker: Option<folder_picker::FolderPicker>,
    open_with: Option<open_with::Chooser>,
    preview: Option<preview::Preview>,
    thumbnails: thumbnails::Cache,
    rename: Option<rename::RenameDialog>,
    network_dialog: Option<network::ConnectDialog>,
    local_shares: Option<local_shares::Manager>,
    local_share_sidebar: local_shares::sidebar_cache::SharedSidebar,
    dnd: drag_drop::DragDrop,
    focus_address: bool,
    query: String,
    focus_search: bool,
    show_hidden: bool,
    confirm_trash: bool,
    restore_tabs: bool,
    toast_options: settings::ToastOptions,
    language: crate::i18n::Language,
    item_scale: f32,
    sort: SortBy,
    descending: bool,
    visible_columns: Vec<Column>,
    column_widths: BTreeMap<Column, f32>,
    default_layout: FolderLayout,
    folder_layouts: BTreeMap<String, FolderLayout>,
    appearance: icons::Appearance,
    settings_editor: Option<settings::Editor>,
    settings_path: Option<PathBuf>,
    last_attempted_preferences: Preferences,
    view: View,
    selected: BTreeSet<PathBuf>,
    selection_anchor: Option<PathBuf>,
    selection_cursor: Option<PathBuf>,
    reveal_selection: bool,
    pending_external: Option<(Vec<PathBuf>, bool)>,
    reveal_hidden: bool,
    navigation_page: usize,
    loading: bool,
    error: Option<String>,
    skipped: usize,
    requests: Sender<ReadRequest>,
    events: Receiver<Event>,
    event_sender: Sender<Event>,
    places: Vec<(locations::Item, PathBuf)>,
    sidebar_options: locations::Preferences,
    network_mounts: Vec<locations::Shortcut>,
    new_folder: Option<String>,
    properties: Option<Entry>,
    properties_usage: Option<crate::disk_usage::Job>,
    dark: bool,
    split_view: bool,
    requested_split: bool,
    cut_transfer_running: bool,
    folder_background_area: Option<egui::Rect>,
    folder_item_regions: Vec<egui::Rect>,
    file_clipboard: Option<file_ops::Clipboard>,
    file_clipboard_text: String,
    clipboard_serial: u64,
    active_transfer: Option<transfers::ActiveTransfer>,
    transfer_conflict: Option<transfers::ConflictPrompt>,
    notice_toasts: Vec<toast::NoticeToast>,
    toast_serial: u64,
    refresh_after_transfer: bool,
    close_after_transfer: bool,
    active_deletion: Option<deletions::ActiveDeletion>,
    delete_confirmation: Option<deletions::Confirmation>,
    active_restore: Option<trash::ActiveTrashOperation>,
    empty_trash_prompt: Option<empty_trash::EmptyTrashPrompt>,
    empty_trash_serial: u64,
    active_empty_trash: Option<trash::ActiveTrashOperation>,
}

impl Explorer {
    pub fn new(cc: &eframe::CreationContext<'_>, initial: PathBuf) -> Self {
        let mut app = Self::with_context(&cc.egui_ctx, initial);
        match crate::native_dnd::Native::attach(cc) {
            Ok(native) => app.dnd.native = Some(native),
            Err(error) => app.report_error(error),
        }
        app.settings_path = Preferences::path();
        if let Some(path) = &app.settings_path {
            match Preferences::load(path) {
                Ok(preferences) => app.apply_preferences(preferences, &cc.egui_ctx),
                Err(error) => app.report_error(crate::i18n::message(
                    "Não foi possível carregar as preferências: {error}",
                    &[(error).to_string()],
                )),
            }
        }
        app.activation = crate::activation::Activation::attach(cc);
        app.local_share_sidebar
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .enabled = true;
        app
    }

    fn with_context(ctx: &egui::Context, initial: PathBuf) -> Self {
        icons::install(ctx);
        ctx.set_visuals(egui::Visuals::light());
        ctx.style_mut(|style| {
            style.spacing.item_spacing = Vec2::new(8.0, 8.0);
            style.spacing.button_padding = Vec2::new(10.0, 7.0);
            style.visuals.selection.bg_fill = Color32::from_rgb(214, 233, 250);
            style.visuals.selection.stroke.color = BLUE;
        });
        let (requests, incoming) = mpsc::channel::<ReadRequest>();
        let (event_sender, events) = mpsc::channel();
        let sender = event_sender.clone();
        let context = ctx.clone();
        thread::spawn(move || {
            while let Ok(request) = incoming.recv() {
                crate::i18n::set_locale(request.locale);
                let result = if crate::trashbin::is_location(&request.path) {
                    crate::trashbin::read()
                } else {
                    filesystem::read_directory(&request.path)
                };
                if sender
                    .send(Event::Listed(result, request.navigation))
                    .is_err()
                {
                    break;
                }
                context.request_repaint();
            }
        });
        let places = locations::available_places();
        let mut app = Self {
            terminal: None,
            requested_tabs: vec![],
            tab_id: 0,
            activation: None,
            loading_path: None,
            initial_path: if crate::trashbin::is_location(&initial) {
                initial.clone()
            } else {
                filesystem::absolute_path(&initial).unwrap_or_else(|_| initial.clone())
            },
            current: PathBuf::new(),
            entries: Vec::new(),
            history: History::default(),
            address: initial.display().to_string(),
            editing_address: false,
            folder_picker: None,
            open_with: None,
            preview: None,
            thumbnails: Default::default(),
            rename: None,
            network_dialog: None,
            local_shares: None,
            local_share_sidebar: Default::default(),
            dnd: drag_drop::DragDrop::default(),
            focus_address: false,
            query: String::new(),
            focus_search: false,
            show_hidden: false,
            confirm_trash: true,
            restore_tabs: true,
            toast_options: settings::ToastOptions::default(),
            language: crate::i18n::Language::System,
            item_scale: 1.0,
            sort: SortBy::Name,
            descending: false,
            visible_columns: Column::DEFAULT.to_vec(),
            column_widths: BTreeMap::new(),
            default_layout: FolderLayout::default(),
            folder_layouts: BTreeMap::new(),
            appearance: icons::Appearance::default(),
            settings_editor: None,
            settings_path: None,
            last_attempted_preferences: Preferences::default(),
            view: View::Details,
            selected: BTreeSet::new(),
            selection_anchor: None,
            selection_cursor: None,
            reveal_selection: false,
            pending_external: None,
            reveal_hidden: false,
            navigation_page: 1,
            loading: false,
            error: None,
            skipped: 0,
            requests,
            events,
            event_sender,
            places,
            sidebar_options: locations::Preferences::default(),
            network_mounts: locations::network_mounts(),
            new_folder: None,
            properties: None,
            properties_usage: None,
            dark: false,
            split_view: false,
            requested_split: false,
            cut_transfer_running: false,
            folder_background_area: None,
            folder_item_regions: Vec::new(),
            file_clipboard: None,
            file_clipboard_text: String::new(),
            clipboard_serial: 0,
            active_transfer: None,
            transfer_conflict: None,
            notice_toasts: Vec::new(),
            toast_serial: 0,
            refresh_after_transfer: false,
            close_after_transfer: false,
            active_deletion: None,
            delete_confirmation: None,
            active_restore: None,
            empty_trash_prompt: None,
            empty_trash_serial: 0,
            active_empty_trash: None,
        };
        app.navigate(initial, Navigation::Visit);
        app
    }

    fn navigate(&mut self, path: PathBuf, navigation: Navigation) {
        self.thumbnails.cancel_pending();
        if self.loading {
            return;
        }
        let loading_path = if crate::trashbin::is_location(&path) {
            path.clone()
        } else {
            filesystem::absolute_path(&path).unwrap_or_else(|_| path.clone())
        };
        if self
            .requests
            .send(ReadRequest {
                path,
                navigation,
                locale: crate::i18n::current(),
            })
            .is_ok()
        {
            self.loading = true;
            self.loading_path = Some(loading_path);
            self.error = None;
        } else {
            self.report_error(
                crate::i18n::tr("O serviço de leitura foi encerrado. Reinicie o explorador.")
                    .into(),
            );
        }
    }

    fn refresh(&mut self) {
        self.local_share_sidebar
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .invalidate();
        self.places = locations::available_places();
        self.network_mounts = locations::network_mounts();
        let (path, navigation) = if self.current.as_os_str().is_empty() {
            (self.initial_path.clone(), Navigation::Visit)
        } else {
            (self.current.clone(), Navigation::Refresh)
        };
        self.navigate(path, navigation);
    }

    fn poll(&mut self) {
        self.poll_local_shares();
        if let Some(panel) = &mut self.terminal {
            panel.poll();
        }
        while let Ok(event) = self.events.try_recv() {
            match event {
                Event::Listed(result, navigation) => {
                    self.loading = false;
                    self.loading_path = None;
                    match result {
                        Ok(listing) => {
                            let changed = self.current != listing.path;
                            if changed {
                                self.remember_current_folder();
                            }
                            self.history.commit(listing.path.clone(), navigation);
                            self.current = listing.path;
                            if changed {
                                self.restore_current_folder();
                            }
                            self.entries = listing.entries;
                            self.skipped = listing.skipped;
                            self.address = self.display_address(&self.current);
                            self.editing_address = false;
                            if changed {
                                self.query.clear();
                                self.clear_selection();
                                self.reveal_hidden = false;
                            } else {
                                let paths: BTreeSet<_> = self
                                    .entries
                                    .iter()
                                    .map(|entry| entry.path.clone())
                                    .collect();
                                self.retain_selection(&paths);
                            }
                            self.complete_external_reveal();
                        }
                        Err(error) => {
                            self.pending_external = None;
                            self.report_error(crate::i18n::message(
                                "Não foi possível abrir a pasta: {error}",
                                &[(error).to_string()],
                            ))
                        }
                    }
                }
                Event::OpenFailed(message) => self.report_error(message),
                Event::TransferProgress(progress) => {
                    if let Some(transfer) = &mut self.active_transfer {
                        transfer.progress = progress;
                    }
                }
                Event::TransferConflict(prompt) => self.receive_transfer_conflict(*prompt),
                Event::TransferFinished(report) => self.finish_transfer(report),
                Event::DeletionProgress(progress) => {
                    if let Some(deletion) = &mut self.active_deletion {
                        deletion.progress = progress;
                    }
                }
                Event::DeletionFinished(report) => self.finish_deletion(report),
                Event::RestoreProgress(progress) => {
                    if let Some(restore) = &mut self.active_restore {
                        restore.progress = progress;
                    }
                }
                Event::RestoreFinished(report) => self.finish_restore(report),
                Event::EmptyTrashListed(id, result) => self.receive_empty_trash_listing(id, result),
                Event::EmptyTrashProgress(progress) => {
                    if let Some(operation) = &mut self.active_empty_trash {
                        operation.progress = progress;
                    }
                }
                Event::EmptyTrashFinished(report) => self.finish_empty_trash(report),
            }
        }
    }

    fn open(&mut self, entry: &Entry, ctx: &egui::Context) {
        if self.is_trash() {
            return;
        }
        if entry.is_dir {
            self.navigate(entry.path.clone(), Navigation::Visit);
        } else {
            let path = entry.path.clone();
            let sender = self.event_sender.clone();
            let ctx = ctx.clone();
            // Use the MIME launcher without blocking the UI or invoking a shell.
            let locale = crate::i18n::current();
            thread::spawn(move || {
                crate::i18n::set_locale(locale);
                if let Err(error) = crate::applications::open_default(&path) {
                    let _ = sender.send(Event::OpenFailed(error));
                    ctx.request_repaint();
                }
            });
        }
    }

    fn shortcuts(&mut self, ctx: &egui::Context) {
        if self.new_folder.is_some()
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
            || self.preview.is_some()
        {
            return;
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::F4)) {
            if let Some(panel) = &mut self.terminal {
                panel.toggle(ctx, self.tab_id);
            } else if !self.is_trash() && !self.current.as_os_str().is_empty() {
                self.open_terminal(self.current.clone(), ctx);
            }
            return;
        }
        if self.terminal_focused(ctx) {
            return;
        }
        if !ctx.wants_keyboard_input() && !self.drag_blocks_tabs() && preview::consume_space(ctx) {
            self.preview_selected(ctx);
            return;
        }
        self.transfer_shortcuts(ctx);
        self.deletion_shortcuts(ctx);
        self.selection_shortcuts(ctx);
        self.rename_shortcut(ctx);
        let (address, search, back, forward, up, refresh, hidden, create) = ctx.input(|i| {
            (
                i.modifiers.command && i.key_pressed(Key::L),
                i.modifiers.command && i.key_pressed(Key::F),
                i.modifiers.alt && i.key_pressed(Key::ArrowLeft),
                i.modifiers.alt && i.key_pressed(Key::ArrowRight),
                i.modifiers.alt && i.key_pressed(Key::ArrowUp),
                i.key_pressed(Key::F5),
                i.modifiers.command && i.key_pressed(Key::H),
                i.modifiers.command && i.modifiers.shift && i.key_pressed(Key::N),
            )
        });
        if address {
            self.editing_address = true;
            self.focus_address = true;
            self.address = self.display_address(self.location());
        }
        if search {
            self.focus_search = true;
        }
        if back && let Some(path) = self.history.back() {
            self.navigate(path.to_owned(), Navigation::Back);
        }
        if forward && let Some(path) = self.history.forward() {
            self.navigate(path.to_owned(), Navigation::Forward);
        }
        if up {
            self.navigate_up(ctx);
        }
        if refresh {
            self.refresh();
        }
        if hidden && !self.is_trash() {
            self.toggle_hidden();
        }
        if create && !self.loading && !self.is_trash() && !self.current.as_os_str().is_empty() {
            self.error = None;
            self.new_folder = Some(String::new());
        }
    }

    pub(super) fn top_toolbar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("main_toolbar")
            .frame(
                egui::Frame::new()
                    .fill(ctx.style().visuals.panel_fill)
                    .inner_margin(egui::Margin {
                        left: 8,
                        right: 8,
                        top: 0,
                        bottom: 0,
                    }),
            )
            .show_separator_line(true)
            .show(ctx, |ui| {
                self.folder_navigation(ui);
            });
    }

    pub(super) fn chrome_row_height(&self) -> f32 {
        (self.appearance.size + 10.0).max(32.0) + 8.0
    }

    fn folder_navigation(&mut self, ui: &mut egui::Ui) -> egui::Response {
        let height = self.chrome_row_height();
        ui.allocate_ui_with_layout(
            Vec2::new(ui.available_width(), height),
            Layout::left_to_right(Align::Center),
            |ui| {
                ui.set_min_height(height);
                if self
                    .navigation_button(
                        ui,
                        !self.loading && self.history.back().is_some(),
                        Icon::Back,
                        crate::i18n::tr("Voltar · Alt+←"),
                    )
                    .on_hover_text(crate::i18n::tr("Voltar · Alt+←"))
                    .clicked()
                    && let Some(path) = self.history.back()
                {
                    self.navigate(path.to_owned(), Navigation::Back);
                }
                if self
                    .navigation_button(
                        ui,
                        !self.loading && self.history.forward().is_some(),
                        Icon::Forward,
                        crate::i18n::tr("Avançar · Alt+→"),
                    )
                    .on_hover_text(crate::i18n::tr("Avançar · Alt+→"))
                    .clicked()
                    && let Some(path) = self.history.forward()
                {
                    self.navigate(path.to_owned(), Navigation::Forward);
                }
                if self
                    .navigation_button(
                        ui,
                        !self.loading && !self.is_trash() && self.current.parent().is_some(),
                        Icon::Up,
                        crate::i18n::tr("Pasta acima · Alt+↑"),
                    )
                    .on_hover_text(crate::i18n::tr("Pasta acima · Alt+↑"))
                    .clicked()
                {
                    self.navigate_up(ui.ctx());
                }
                if self
                    .navigation_button(
                        ui,
                        !self.loading,
                        Icon::Refresh,
                        crate::i18n::tr("Atualizar · F5"),
                    )
                    .clicked()
                {
                    self.refresh();
                }
                if self
                    .icon_button(ui, Icon::Edit, crate::i18n::tr("Editar caminho · Ctrl+L"))
                    .clicked()
                {
                    self.editing_address = true;
                    self.focus_address = true;
                    self.address = self.display_address(self.location());
                }
                ui.add_space(10.0);
                let controls_width =
                    ((self.appearance.size + 14.0).max(32.0) + ui.spacing().item_spacing.x) * 3.0;
                ui.allocate_ui_with_layout(
                    Vec2::new((ui.available_width() - controls_width).max(1.0), height),
                    Layout::left_to_right(Align::Center),
                    |ui| self.folder_actions(ui),
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| self.toolbar(ui));
            },
        )
        .response
    }

    fn toolbar(&mut self, ui: &mut egui::Ui) {
        if self
            .icon_button(
                ui,
                if self.dark { Icon::Sun } else { Icon::Moon },
                if self.dark {
                    crate::i18n::tr("Tema claro")
                } else {
                    crate::i18n::tr("Tema escuro")
                },
            )
            .clicked()
        {
            self.dark = !self.dark;
            ui.ctx().set_visuals(if self.dark {
                egui::Visuals::dark()
            } else {
                egui::Visuals::light()
            });
        }
        self.column_settings(ui);
        if ui
            .add_enabled_ui(!self.has_dialog() && !self.drag_blocks_tabs(), |ui| {
                self.icon_toggle(
                    ui,
                    Icon::SplitView,
                    if self.split_view {
                        crate::i18n::tr("Exibir um painel")
                    } else {
                        crate::i18n::tr("Dividir em dois painéis")
                    },
                    self.split_view,
                )
            })
            .inner
            .clicked()
        {
            self.requested_split = true;
        }
    }

    fn filter_field(&mut self, ui: &mut egui::Ui) -> egui::Response {
        let search_width = ui.available_width().clamp(200.0, 320.0);
        ui.allocate_ui_with_layout(
            Vec2::new(ui.available_width(), 32.0),
            Layout::right_to_left(Align::Center),
            |ui| {
                let response = ui.add_sized(
                    [search_width, 32.0],
                    egui::TextEdit::singleline(&mut self.query)
                        .vertical_align(Align::Center)
                        .hint_text(crate::i18n::tr("Pesquisar nesta pasta · Ctrl+F")),
                );
                if self.focus_search {
                    response.request_focus();
                    self.focus_search = false;
                }
                if response.has_focus() && ui.input(|i| i.key_pressed(Key::Escape)) {
                    self.query.clear();
                }
                response
            },
        )
        .inner
    }

    fn folder_path(&mut self, ui: &mut egui::Ui) -> egui::Response {
        let editing = self.editing_address || self.current.as_os_str().is_empty();
        let height = self.chrome_row_height();
        ui.allocate_ui_with_layout(
            Vec2::new(ui.available_width(), height),
            Layout::left_to_right(Align::Center),
            |ui| {
                ui.set_min_height(height);
                let filter_width = (ui.available_width() * 0.35).clamp(200.0, 320.0);
                let path_width =
                    (ui.available_width() - filter_width - ui.spacing().item_spacing.x).max(1.0);
                ui.allocate_ui_with_layout(
                    Vec2::new(path_width, height),
                    Layout::left_to_right(Align::Center),
                    |ui| {
                        if self.is_trash() && !self.editing_address {
                            ui.label(RichText::new(crate::i18n::tr("Lixeira")).strong());
                        } else if editing {
                            if self
                                .icon_button(
                                    ui,
                                    Icon::CancelEdit,
                                    crate::i18n::tr("Sair da edição do caminho · Esc"),
                                )
                                .clicked()
                            {
                                self.cancel_address_edit(ui.ctx());
                            }
                            let button_width = (self.appearance.size + 14.0).max(32.0);
                            let field_width =
                                (ui.available_width() - button_width - ui.spacing().item_spacing.x)
                                    .max(1.0);
                            let response = ui.add_sized(
                                [field_width, (self.appearance.size + 10.0).max(32.0)],
                                egui::TextEdit::singleline(&mut self.address)
                                    .vertical_align(Align::Center)
                                    .hint_text(crate::i18n::tr("Caminho da pasta")),
                            );
                            if self.focus_address {
                                response.request_focus();
                                self.focus_address = false;
                            }
                            if response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                                self.submit_address(ui.ctx());
                            }
                            if (response.has_focus() || response.lost_focus())
                                && ui.input(|i| i.key_pressed(Key::Escape))
                            {
                                self.cancel_address_edit(ui.ctx());
                            }
                            if self
                                .navigation_button(
                                    ui,
                                    !self.loading,
                                    Icon::BrowseFolders,
                                    crate::i18n::tr("Navegar e selecionar pasta"),
                                )
                                .clicked()
                            {
                                self.open_folder_picker(ui.ctx());
                            }
                        } else if let Some(location) = self.network_location(&self.current) {
                            match breadcrumbs::show_network(ui, location) {
                                Some(breadcrumbs::Destination::Folder(path)) => {
                                    self.navigate(path, Navigation::Visit)
                                }
                                Some(breadcrumbs::Destination::Network(url)) => self
                                    .request_network(
                                        Some(crate::network::Connection {
                                            url,
                                            label: String::new(),
                                            mount: None,
                                        }),
                                        ui.ctx(),
                                    ),
                                None => (),
                            }
                        } else if let Some(path) = breadcrumbs::show(ui, &self.current) {
                            self.navigate(path, Navigation::Visit);
                        }
                    },
                );
                self.filter_field(ui);
            },
        )
        .response
    }

    fn column_settings(&mut self, ui: &mut egui::Ui) -> egui::Response {
        let response = ui
            .add_enabled_ui(!self.loading, |ui| {
                self.icon_button(ui, Icon::Settings, crate::i18n::tr("Configurações"))
            })
            .inner;
        if response.clicked() {
            self.settings_editor = Some(settings::Editor::new(self.preferences()));
        }
        response
    }

    fn preferences(&self) -> Preferences {
        Preferences {
            version: 1,
            columns: self.visible_columns.clone(),
            column_widths: self.column_widths.clone(),
            folders: self.folder_layouts.clone(),
            appearance: self.appearance.clone(),
            sidebar: self.sidebar_options.clone(),
            dark: self.dark,
            show_hidden: self.show_hidden,
            confirm_trash: self.confirm_trash,
            restore_tabs: self.restore_tabs,
            toast: self.toast_options,
            language: self.language,
            item_scale: self.item_scale,
            view: self.view,
            sort: self.sort,
            descending: self.descending,
        }
    }

    fn apply_preferences(&mut self, mut preferences: Preferences, ctx: &egui::Context) {
        preferences.normalize();
        self.default_layout = preferences.layout();
        let layout = if self.is_trash()
            && !preferences
                .folders
                .contains_key(&settings::folder_key(&self.current))
        {
            self.default_current_layout()
        } else {
            preferences.layout_for(&self.current)
        };
        self.folder_layouts = preferences.folders.clone();
        self.apply_layout(layout);
        self.apply_global_preferences(&preferences, ctx);
        self.last_attempted_preferences = preferences;
    }

    fn apply_global_preferences(&mut self, preferences: &Preferences, ctx: &egui::Context) {
        self.places = locations::available_places();
        self.appearance = preferences.appearance.clone();
        self.sidebar_options = preferences.sidebar.clone();
        self.dark = preferences.dark;
        self.show_hidden = preferences.show_hidden;
        self.confirm_trash = preferences.confirm_trash;
        self.restore_tabs = preferences.restore_tabs;
        if self.toast_options != preferences.toast {
            for toast in &mut self.notice_toasts {
                toast.update_options(&self.toast_options, &preferences.toast);
            }
            self.toast_options = preferences.toast;
        }
        self.language = preferences.language;
        crate::i18n::set_locale(self.language.resolve());
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(
            crate::i18n::tr("Explorador de Arquivos").into(),
        ));
        ctx.set_visuals(if self.dark {
            egui::Visuals::dark()
        } else {
            egui::Visuals::light()
        });
    }

    fn current_layout(&self) -> FolderLayout {
        FolderLayout {
            view: self.view,
            item_scale: self.item_scale,
            columns: self.visible_columns.clone(),
            column_widths: self.column_widths.clone(),
            sort: self.sort,
            descending: self.descending,
        }
    }

    fn apply_layout(&mut self, layout: FolderLayout) {
        self.visible_columns = layout.columns;
        self.column_widths = layout.column_widths;
        self.view = layout.view;
        self.item_scale = layout.item_scale;
        self.sort = layout.sort;
        self.descending = layout.descending;
    }

    fn remember_current_folder(&mut self) {
        if self.current.as_os_str().is_empty() {
            return;
        }
        let key = settings::folder_key(&self.current);
        let layout = self.current_layout();
        if layout != self.default_current_layout() || self.folder_layouts.contains_key(&key) {
            self.folder_layouts.insert(key, layout);
        }
    }

    fn restore_current_folder(&mut self) {
        let layout = self
            .folder_layouts
            .get(&settings::folder_key(&self.current))
            .cloned()
            .unwrap_or_else(|| self.default_current_layout());
        self.apply_layout(layout);
    }

    fn stored_preferences(&self, current: &Preferences) -> Preferences {
        let mut stored = current.clone();
        stored.folders = self.folder_layouts.clone();
        if self.current.as_os_str().is_empty() {
            return stored;
        }
        let key = settings::folder_key(&self.current);
        let layout = current.layout();
        if layout != self.default_current_layout() || stored.folders.contains_key(&key) {
            stored.folders.insert(key, layout);
        }
        stored.set_layout(&self.default_layout);
        stored
    }

    fn settings_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut editor) = self.settings_editor.take() else {
            return;
        };
        match editor.show(ctx) {
            settings::Action::Close => return,
            settings::Action::None => (),
            settings::Action::TestToast(kind, timing) => {
                self.post_preview_notice(kind, timing, &editor.draft.appearance)
            }
            settings::Action::ResetFolder => match self.reset_current_folder() {
                Ok(()) => {
                    editor.draft.set_layout(&self.default_current_layout());
                    editor.draft.folders = self.folder_layouts.clone();
                    editor.error = None;
                }
                Err(error) => {
                    editor.error = Some(crate::i18n::message(
                        "Não foi possível restaurar a pasta: {error}",
                        &[(error).to_string()],
                    ))
                }
            },
            settings::Action::Apply(mut preferences, close) => {
                preferences.normalize();
                let stored = self.stored_preferences(&preferences);
                if let Some(path) = &self.settings_path
                    && let Err(error) = stored.save(path)
                {
                    editor.error = Some(crate::i18n::message(
                        "Não foi possível salvar: {error}",
                        &[(error).to_string()],
                    ));
                    self.settings_editor = Some(editor);
                    return;
                }
                self.apply_preferences(stored, ctx);
                editor.draft = self.preferences();
                editor.error = None;
                if close {
                    return;
                }
            }
        }
        self.settings_editor = Some(editor);
    }

    fn reset_current_folder(&mut self) -> std::io::Result<()> {
        let mut stored = self.stored_preferences(&self.preferences());
        stored.folders.remove(&settings::folder_key(&self.current));
        if let Some(path) = &self.settings_path {
            stored.save(path)?;
        }
        self.folder_layouts = stored.folders.clone();
        self.apply_layout(self.default_current_layout());
        self.last_attempted_preferences = stored;
        Ok(())
    }

    fn persist_preferences(&mut self, ctx: &egui::Context) {
        // Commit a resize once the pointer is released, rather than writing on every pixel.
        if ctx.input(|i| i.pointer.any_down()) {
            return;
        }
        self.remember_current_folder();
        let preferences = self.stored_preferences(&self.preferences());
        if preferences == self.last_attempted_preferences {
            return;
        }
        self.last_attempted_preferences = preferences.clone();
        if let Some(path) = &self.settings_path
            && let Err(error) = preferences.save(path)
        {
            self.report_error(crate::i18n::message(
                "Não foi possível salvar as preferências: {error}",
                &[(error).to_string()],
            ));
        }
    }

    fn expand_address(&self) -> PathBuf {
        let text = self.address.trim();
        if text == self.location().display().to_string() {
            return self.location().to_owned();
        }
        if matches!(text, "trash:" | "trash://" | "trash:///") {
            return crate::trashbin::location();
        }
        if text == "~" {
            return dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));
        }
        if let Some(rest) = text.strip_prefix("~/") {
            return dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("/"))
                .join(rest);
        }
        let path = PathBuf::from(text);
        if path.is_absolute() {
            path
        } else if self.is_trash() {
            dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("/"))
                .join(path)
        } else {
            self.current.join(path)
        }
    }

    fn contents(&mut self, ctx: &egui::Context, visible: &[usize]) {
        egui::CentralPanel::default()
            .frame(
                egui::Frame::central_panel(&ctx.style()).inner_margin(egui::Margin {
                    left: 8,
                    right: 8,
                    top: 0,
                    bottom: 0,
                }),
            )
            .show(ctx, |ui| {
                self.contents_ui(ui, visible);
            });
    }

    fn contents_ui(&mut self, ui: &mut egui::Ui, visible: &[usize]) {
        self.folder_item_regions.clear();
        ui.spacing_mut().item_spacing.y = 0.0;
        self.folder_path(ui);
        ui.add(egui::Separator::default().spacing(0.0));
        self.transfer_status(ui);
        self.deletion_status(ui);
        self.restore_status(ui);
        self.empty_trash_status(ui);
        if self.loading {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(crate::i18n::tr("Carregando pasta…"));
            });
        }
        let background = ui.available_rect_before_wrap();
        self.folder_background_area = Some(background);
        self.register_drop_target(ui, background, self.current.clone());
        let background_response = ui.interact(
            background,
            ui.id().with("folder_background"),
            Sense::click(),
        );
        ui.add_enabled_ui(!self.loading, |ui| {
            if visible.is_empty() && !self.loading {
                ui.add_space(80.0);
                ui.vertical_centered(|ui| {
                    ui.label(
                        RichText::new(if self.query.is_empty() {
                            if self.is_trash() {
                                crate::i18n::tr("A lixeira está vazia")
                            } else {
                                crate::i18n::tr("Esta pasta está vazia")
                            }
                        } else {
                            crate::i18n::tr("Nenhum item encontrado")
                        })
                        .size(20.0),
                    );
                    if !self.query.is_empty() {
                        ui.label(crate::i18n::tr(
                            "A pesquisa considera os nomes dos itens nesta pasta.",
                        ));
                        if ui.button(crate::i18n::tr("Limpar pesquisa")).clicked() {
                            self.query.clear();
                        }
                    }
                });
                return;
            }
            match self.view {
                View::Details => self.details(ui, visible),
                View::Icons => self.icons(ui, visible),
                View::List => self.list(ui, visible),
            }
        });
        // Scroll containers own the hit-test in their empty viewport. Resolve
        // secondary clicks after rows are rendered, excluding items and scrollbars.
        let blank_click = !self.has_dialog()
            && ui.input(|i| i.pointer.secondary_clicked())
            && ui.ctx().pointer_interact_pos().is_some_and(|point| {
                self.folder_background_area
                    .is_some_and(|area| area.contains(point))
                    && ui.rect_contains_pointer(background)
                    && !self
                        .folder_item_regions
                        .iter()
                        .any(|rect| rect.contains(point))
            });
        let mut popup = egui::Popup::context_menu(&background_response);
        if blank_click {
            popup = popup.open_memory(egui::SetOpenCommand::Bool(true));
        }
        popup.show(|ui| self.folder_background_menu(ui));
    }

    fn folder_background_menu(&mut self, ui: &mut egui::Ui) {
        if self.is_trash() {
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
            if self
                .menu_item(
                    ui,
                    Icon::Refresh,
                    crate::i18n::tr("Atualizar"),
                    "F5",
                    !self.loading,
                    false,
                )
                .clicked()
            {
                self.refresh();
                ui.close();
            }
            return;
        }
        self.paste_menu(ui);
        if !self.current.as_os_str().is_empty() && !self.loading && !self.is_trash() {
            ui.separator();
            self.folder_actions_menu(ui, &self.current.clone());
            self.pin_menu(ui, &self.current.clone());
        }
    }

    fn details(&mut self, ui: &mut egui::Ui, visible: &[usize]) {
        ui.spacing_mut().item_spacing.y = 0.0;
        let scale = self.item_scale;
        let row_height = 34.0 * scale;
        let viewport_width = (ui.available_width() - 14.0).max(1.0);
        let columns = column_layout(
            viewport_width,
            ui.spacing().item_spacing.x,
            &self.visible_columns,
            &self.column_widths,
        );
        let width = columns.iter().map(|(_, width)| width).sum::<f32>()
            + ui.spacing().item_spacing.x * columns.len().saturating_sub(1) as f32;
        egui::ScrollArea::horizontal()
            .id_salt((self.tab_id, "detail_columns_scroll"))
            .scroll_bar_visibility(if width > viewport_width + 0.5 {
                egui::scroll_area::ScrollBarVisibility::AlwaysVisible
            } else {
                egui::scroll_area::ScrollBarVisibility::VisibleWhenNeeded
            })
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.set_min_width(width);
                self.column_headers(ui, &columns);
                ui.add(egui::Separator::default().spacing(0.0));
                ui.scope(|ui| {
                    ui.spacing_mut().item_spacing.y = 8.0;
                    let rows = egui::ScrollArea::vertical()
                        .id_salt((self.tab_id, &self.current, "details"))
                        .auto_shrink([false, false])
                        .show_rows(ui, row_height, visible.len(), |ui, range| {
                            self.reveal_selected_row(ui, visible, range.start, row_height, 1);
                            for row in range {
                                let entry = self.entries[visible[row]].clone();
                                let (rect, response) = ui.allocate_exact_size(
                                    Vec2::new(width, row_height),
                                    Sense::click_and_drag(),
                                );
                                let selected = self.selected.contains(&entry.path);
                                if selected || response.hovered() {
                                    ui.painter().rect_filled(
                                        rect,
                                        4.0,
                                        if selected {
                                            ui.visuals().selection.bg_fill
                                        } else {
                                            ui.visuals().widgets.hovered.bg_fill
                                        },
                                    );
                                }
                                let mut child = ui.new_child(
                                    egui::UiBuilder::new()
                                        .max_rect(rect)
                                        .layout(Layout::left_to_right(Align::Center)),
                                );
                                child.multiply_opacity(self.cut_opacity(&entry.path));
                                for &(column, column_width) in &columns {
                                    if column == Column::Name {
                                        child.allocate_ui_with_layout(
                                            Vec2::new(column_width, 32.0 * scale),
                                            Layout::left_to_right(Align::Center),
                                            |ui| {
                                                ui.set_min_width(column_width);
                                                ui.set_max_width(column_width);
                                                ui.add_space(5.0);
                                                self.file_icon(ui, &entry, 22.0 * scale);
                                                ui.add(
                                                    egui::Label::new(
                                                        RichText::new(&entry.name)
                                                            .size(13.0 * scale)
                                                            .color(if selected {
                                                                ui.visuals().selection.stroke.color
                                                            } else {
                                                                ui.visuals().text_color()
                                                            }),
                                                    )
                                                    .truncate()
                                                    .selectable(false),
                                                );
                                            },
                                        );
                                    } else {
                                        let text = column.value(&entry);
                                        cell(&mut child, &text, column_width, scale);
                                    }
                                }
                                self.entry_response(ui, response, &entry, visible);
                            }
                        });
                    self.folder_background_area = Some(rows.inner_rect.intersect(ui.clip_rect()));
                });
            });
    }

    fn icons(&mut self, ui: &mut egui::Ui, visible: &[usize]) {
        ui.spacing_mut().item_spacing.y = 8.0;
        let scale = self.item_scale;
        let tile_width = 114.0 * scale;
        let row_height = 104.0 * scale;
        let gap = ui.spacing().item_spacing.x;
        let columns = (((ui.available_width() + gap) / (tile_width + gap)) as usize).max(1);
        let row_count = visible.len().div_ceil(columns);
        let rows = egui::ScrollArea::vertical()
            .id_salt((self.tab_id, &self.current, "icons"))
            .auto_shrink([false, false])
            .show_rows(ui, row_height, row_count, |ui, range| {
                let requested = visible
                    .iter()
                    .skip(range.start * columns)
                    .take((range.end - range.start) * columns)
                    .map(|&i| self.entries[i].path.clone())
                    .collect();
                self.thumbnails.cancel_unseen(&requested);
                self.reveal_selected_row(ui, visible, range.start, row_height, columns);
                for row in range {
                    ui.horizontal(|ui| {
                        for &index in visible.iter().skip(row * columns).take(columns) {
                            let entry = self.entries[index].clone();
                            let (rect, response) = ui.allocate_exact_size(
                                Vec2::new(tile_width, row_height),
                                Sense::click_and_drag(),
                            );
                            let selected = self.selected.contains(&entry.path);
                            if selected || response.hovered() {
                                ui.painter().rect_filled(
                                    rect,
                                    5.0,
                                    if selected {
                                        ui.visuals().selection.bg_fill
                                    } else {
                                        ui.visuals().widgets.hovered.bg_fill
                                    },
                                );
                            }
                            let mut child = ui.new_child(
                                egui::UiBuilder::new()
                                    .max_rect(rect.shrink(8.0))
                                    .layout(Layout::top_down(Align::Center)),
                            );
                            child.multiply_opacity(self.cut_opacity(&entry.path));
                            self.grid_picture(&mut child, &entry, scale);
                            child.add(
                                egui::Label::new(RichText::new(&entry.name).size(13.0 * scale))
                                    .truncate()
                                    .selectable(false),
                            );
                            self.entry_response(ui, response, &entry, visible);
                        }
                    });
                }
            });
        self.folder_background_area = Some(rows.inner_rect.intersect(ui.clip_rect()));
    }

    fn entry_response(
        &mut self,
        ui: &egui::Ui,
        response: egui::Response,
        entry: &Entry,
        visible: &[usize],
    ) {
        self.folder_item_regions
            .push(response.rect.intersect(ui.clip_rect()));
        self.file_drag_response(ui, &response, entry, visible);
        let modifiers = response.ctx.input(|i| i.modifiers);
        if response.clicked() {
            self.select_entry(&entry.path, visible, modifiers);
        }
        if response.secondary_clicked() {
            self.selection_cursor = Some(entry.path.clone());
        }
        if response.secondary_clicked() && !self.selected.contains(&entry.path) {
            self.selected.clear();
            self.selected.insert(entry.path.clone());
            self.selection_anchor = Some(entry.path.clone());
        }
        if response.double_clicked() && !modifiers.ctrl && !modifiers.command && !modifiers.shift {
            self.open(entry, &response.ctx);
        }
        response.clone().on_hover_text(format!(
            "{}\n{}",
            entry.name,
            entry
                .trash
                .as_ref()
                .map(|record| record.original_path.as_path())
                .unwrap_or(&entry.path)
                .display()
        ));
        response.context_menu(|ui| {
            if self.is_trash() {
                self.trash_context_menu(ui, entry);
                return;
            }
            let multiple = self.selected.len() > 1;
            if self
                .menu_item(
                    ui,
                    Icon::Open,
                    if multiple {
                        crate::i18n::tr("Abrir este item")
                    } else {
                        crate::i18n::tr("Abrir")
                    },
                    "",
                    true,
                    false,
                )
                .clicked()
            {
                self.open(entry, ui.ctx());
                ui.close();
            }
            if !entry.is_dir
                && self
                    .menu_item(
                        ui,
                        Icon::Preview,
                        crate::i18n::tr("Pré-visualizar"),
                        crate::i18n::tr("Espaço"),
                        true,
                        false,
                    )
                    .clicked()
            {
                self.request_preview(entry.path.clone(), ui.ctx());
                ui.close();
            }
            if !entry.is_dir
                && self
                    .menu_item(
                        ui,
                        Icon::OpenWith,
                        if multiple {
                            crate::i18n::tr("Abrir este item com…")
                        } else {
                            crate::i18n::tr("Abrir com…")
                        },
                        "",
                        true,
                        false,
                    )
                    .clicked()
            {
                self.choose_application(entry, ui.ctx());
                ui.close();
            }
            if entry.is_dir {
                self.folder_actions_menu(ui, &entry.path);
            }
            self.file_transfer_menu(ui, entry);
            if self
                .menu_item(
                    ui,
                    Icon::Rename,
                    crate::i18n::tr("Renomear"),
                    "F2",
                    self.can_rename(),
                    false,
                )
                .clicked()
            {
                self.request_rename(entry);
                ui.close();
            }
            self.deletion_menu(ui);
            if entry.is_dir {
                self.pin_menu(ui, &entry.path);
            }
            ui.separator();
            if self
                .menu_item(
                    ui,
                    Icon::CopyPath,
                    if multiple {
                        crate::i18n::tr("Copiar caminhos selecionados")
                    } else {
                        crate::i18n::tr("Copiar caminho")
                    },
                    "",
                    true,
                    false,
                )
                .clicked()
            {
                self.file_clipboard = None;
                ui.ctx().copy_text(self.selected_paths_text());
                ui.close();
            }
            if self
                .menu_item(
                    ui,
                    Icon::Properties,
                    if multiple {
                        crate::i18n::tr("Propriedades deste item")
                    } else {
                        crate::i18n::tr("Propriedades")
                    },
                    "",
                    true,
                    false,
                )
                .clicked()
            {
                self.properties = Some(entry.clone());
                ui.close();
            }
        });
    }

    fn dialogs(&mut self, ctx: &egui::Context) {
        if let Some(mut name) = self.new_folder.clone() {
            let mut create = false;
            let mut cancel = false;
            let modal = egui::Modal::new(egui::Id::new("new_folder_dialog")).show(ctx, |ui| {
                ui.set_min_width(320.0);
                ui.heading(crate::i18n::tr("Nova pasta"));
                ui.label(crate::i18n::tr("Nome da pasta"));
                let response = ui.text_edit_singleline(&mut name);
                if response.gained_focus() || name.is_empty() {
                    response.request_focus();
                }
                ui.horizontal(|ui| {
                    create = ui
                        .add_enabled(
                            !name.trim().is_empty(),
                            egui::Button::new(crate::i18n::tr("Criar")),
                        )
                        .clicked()
                        || (response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)));
                    cancel = ui.button(crate::i18n::tr("Cancelar")).clicked()
                        || ui.input(|i| i.key_pressed(Key::Escape));
                });
                if let Some(error) = &self.error {
                    ui.colored_label(
                        if self.dark {
                            Color32::LIGHT_RED
                        } else {
                            Color32::DARK_RED
                        },
                        error,
                    );
                }
            });
            self.new_folder = if !modal.should_close() && !cancel {
                Some(name.clone())
            } else {
                None
            };
            if create {
                let result =
                    filesystem::child_path(&self.current, &name).and_then(std::fs::create_dir);
                match result {
                    Ok(()) => {
                        self.new_folder = None;
                        self.refresh();
                    }
                    Err(error) => self.report_error(crate::i18n::message(
                        "Não foi possível criar a pasta: {error}",
                        &[(error).to_string()],
                    )),
                }
            }
        }
        self.properties_dialog(ctx);
    }
}

impl Explorer {
    fn prepare_view(&mut self, ctx: &egui::Context) {
        crate::i18n::set_locale(self.language.resolve());
        self.poll();
        self.begin_drag_drop(ctx);
        self.handle_transfer_close(ctx);
        self.process_transfer_refresh();
    }

    fn visible_for_display(&mut self) -> Vec<usize> {
        let visible = filesystem::visible_entries(
            &self.entries,
            &self.query,
            self.hidden_visible(),
            self.sort,
            self.descending,
        );
        let visible_paths = visible
            .iter()
            .map(|&index| self.entries[index].path.clone())
            .collect();
        // A reopened tab restores its selection before its first listing arrives.
        if !self.loading || !self.entries.is_empty() {
            self.retain_selection(&visible_paths);
        }
        visible
    }

    fn status_bar(&mut self, ctx: &egui::Context, visible: &[usize]) {
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(crate::i18n::plural("{} item", "{} itens", visible.len()));
                if visible.len() != self.entries.len() {
                    ui.weak(crate::i18n::message(
                        "de {}",
                        &[(self.entries.len()).to_string()],
                    ));
                }
                if !self.selected.is_empty() {
                    ui.separator();
                    let count = self.selected.len();
                    ui.label(if count == 1 {
                        crate::i18n::tr("1 item selecionado").into()
                    } else {
                        crate::i18n::message("{count} itens selecionados", &[(count).to_string()])
                    });
                    let sizes: Vec<_> = self
                        .entries
                        .iter()
                        .filter(|entry| self.selected.contains(&entry.path))
                        .filter_map(|entry| entry.size)
                        .collect();
                    if !sizes.is_empty() {
                        ui.weak(filesystem::format_size(
                            sizes.into_iter().fold(0_u64, u64::saturating_add),
                        ));
                    }
                }
                if self.skipped > 0 {
                    ui.weak(crate::i18n::message(
                        "{} itens não puderam ser lidos",
                        &[(self.skipped).to_string()],
                    ));
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.weak(if self.is_trash() {
                        crate::i18n::tr("Clique direito para restaurar")
                    } else {
                        crate::i18n::tr("Duplo clique para abrir")
                    });
                });
            });
        });
    }

    fn view_overlays(&mut self, ctx: &egui::Context) {
        self.dialogs(ctx);
        self.settings_dialog(ctx);
        self.transfer_dialog(ctx);
        self.deletion_dialog(ctx);
        self.empty_trash_dialog(ctx);
        self.folder_picker_dialog(ctx);
        self.open_with_dialog(ctx);
        self.rename_dialog(ctx);
        self.network_dialog(ctx);
        self.local_shares_dialog(ctx);
        self.preview_dialog(ctx);
        self.show_notice_toast(ctx);
    }

    fn show(&mut self, ctx: &egui::Context) {
        self.prepare_view(ctx);
        self.shortcuts(ctx);
        self.top_toolbar(ctx);
        let visible = self.visible_for_display();
        self.status_bar(ctx, &visible);
        self.terminal_panel(ctx);
        self.sidebar(ctx);
        self.contents(ctx, &visible);
        self.finish_drag_drop(ctx);
        self.view_overlays(ctx);
        self.persist_preferences(ctx);
    }
}

fn cell(ui: &mut egui::Ui, text: &str, width: f32, scale: f32) {
    ui.allocate_ui_with_layout(
        Vec2::new(width, 30.0 * scale),
        Layout::left_to_right(Align::Center),
        |ui| {
            ui.set_min_width(width);
            ui.set_max_width(width);
            ui.add_space(5.0);
            ui.add(
                egui::Label::new(RichText::new(text).size(13.0 * scale).weak())
                    .truncate()
                    .selectable(false),
            )
            .on_hover_text(text);
        },
    );
}

fn modified_label(entry: &Entry) -> String {
    Column::Modified.value(entry)
}

fn column_layout(
    width: f32,
    spacing: f32,
    visibility: &[Column],
    widths: &BTreeMap<Column, f32>,
) -> Vec<(Column, f32)> {
    let mut columns: Vec<_> = visibility
        .iter()
        .map(|&column| {
            (
                column,
                widths
                    .get(&column)
                    .copied()
                    .unwrap_or_else(|| column.width()),
            )
        })
        .collect();
    let base = columns.iter().map(|(_, width)| width).sum::<f32>()
        + spacing * columns.len().saturating_sub(1) as f32;
    if widths.is_empty() {
        let expand = columns
            .iter()
            .position(|(c, _)| *c == Column::Name)
            .unwrap_or(0);
        if let Some((_, first)) = columns.get_mut(expand) {
            *first += (width - base).max(0.0);
        }
    }
    columns
}

#[derive(Clone, Copy)]
enum Arrow {
    Up,
    Down,
}

fn paint_arrow(painter: &egui::Painter, center: egui::Pos2, arrow: Arrow, color: Color32) {
    let transform = |x: f32, y: f32| {
        center
            + match arrow {
                Arrow::Down => Vec2::new(y, x),
                Arrow::Up => Vec2::new(y, -x),
            }
    };
    painter.line_segment([transform(-5.0, 0.0), transform(5.0, 0.0)], (1.5, color));
    painter.line_segment([transform(0.0, -5.0), transform(5.0, 0.0)], (1.5, color));
    painter.line_segment([transform(0.0, 5.0), transform(5.0, 0.0)], (1.5, color));
}

impl Explorer {
    fn icon_button(&self, ui: &mut egui::Ui, icon: Icon, label: &str) -> egui::Response {
        icons::button(ui, icon, label, &self.appearance)
    }

    fn icon_toggle(
        &self,
        ui: &mut egui::Ui,
        icon: Icon,
        label: &str,
        selected: bool,
    ) -> egui::Response {
        icons::selectable_button(ui, icon, label, selected, &self.appearance)
    }

    fn navigation_button(
        &self,
        ui: &mut egui::Ui,
        enabled: bool,
        icon: Icon,
        label: &str,
    ) -> egui::Response {
        ui.add_enabled_ui(enabled, |ui| self.icon_button(ui, icon, label))
            .inner
    }

    fn file_icon(&self, ui: &mut egui::Ui, entry: &Entry, size: f32) {
        let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
        self.paint_file_icon(ui.painter(), rect, entry);
    }

    fn paint_file_icon(&self, painter: &egui::Painter, rect: egui::Rect, entry: &Entry) {
        let visuals = &painter.ctx().style().visuals;
        let size = rect.width();
        icons::paint(
            painter,
            rect.shrink(1.0),
            crate::filetypes::icon(entry),
            visuals.text_color(),
            &self.appearance,
        );
        if entry.is_symlink {
            let badge = egui::Rect::from_min_size(
                rect.left_bottom() - Vec2::new(0.0, size * 0.45),
                Vec2::splat(size * 0.45),
            );
            painter.rect_filled(badge, 2.0, visuals.panel_fill);
            icons::paint(
                painter,
                badge,
                Icon::Link,
                visuals.text_color(),
                &self.appearance,
            );
        }
    }
}
