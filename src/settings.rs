use crate::{
    columns::Column,
    icons::{self, Appearance, Icon, IconMode},
    sidebar::{self, Item as SidebarItem, Shortcut},
};
use eframe::egui::{self, Align, Vec2};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs,
    io::{self, Write},
    os::unix::ffi::OsStrExt,
    path::{Path, PathBuf},
};
mod sidebar_ui;

#[derive(Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum View {
    #[default]
    Details,
    Icons,
    List,
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FolderLayout {
    pub view: View,
    pub item_scale: f32,
    pub columns: Vec<Column>,
    pub column_widths: BTreeMap<Column, f32>,
    pub sort: Column,
    pub descending: bool,
}

impl Default for FolderLayout {
    fn default() -> Self {
        Self {
            view: View::Details,
            item_scale: 1.0,
            columns: Column::DEFAULT.to_vec(),
            column_widths: BTreeMap::new(),
            sort: Column::Name,
            descending: false,
        }
    }
}

impl FolderLayout {
    fn normalize(&mut self) {
        self.item_scale = if self.item_scale.is_finite() {
            (self.item_scale.clamp(0.75, 2.0) * 4.0).round() / 4.0
        } else {
            1.0
        };
        self.column_widths.retain(|_, width| width.is_finite());
        for width in self.column_widths.values_mut() {
            *width = width.clamp(80.0, 1200.0);
        }
        let mut columns = Vec::new();
        for &column in &self.columns {
            if !columns.contains(&column) {
                columns.push(column);
            }
        }
        self.columns = if columns.is_empty() {
            Column::DEFAULT.to_vec()
        } else {
            columns
        };
        if !self.columns.contains(&self.sort) {
            self.sort = self.columns[0];
            self.descending = false;
        }
    }
}

pub fn folder_key(path: &Path) -> String {
    if let Some(path) = path.to_str() {
        return path.to_owned();
    }
    // Absolute UTF-8 paths start with '/', so this prefix cannot collide with them.
    let mut key = String::from("unix-bytes:");
    for byte in path.as_os_str().as_bytes() {
        use std::fmt::Write;
        write!(&mut key, "{byte:02x}").expect("escrita em String não falha");
    }
    key
}

pub fn folder_path(key: &str) -> Option<PathBuf> {
    use std::{ffi::OsString, os::unix::ffi::OsStringExt};
    let Some(encoded) = key.strip_prefix("unix-bytes:") else {
        return Some(key.into());
    };
    if encoded.len() % 2 != 0 || !encoded.is_ascii() {
        return None;
    }
    let bytes = (0..encoded.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&encoded[i..i + 2], 16))
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    Some(PathBuf::from(OsString::from_vec(bytes)))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ToastTiming {
    pub duration_seconds: u32,
    pub auto_dismiss: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToastKind {
    Success,
    Warning,
    Error,
}

impl Default for ToastTiming {
    fn default() -> Self {
        Self {
            duration_seconds: 6,
            auto_dismiss: true,
        }
    }
}

impl ToastTiming {
    fn normalize(&mut self) {
        self.duration_seconds = self.duration_seconds.clamp(1, 300);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "StoredToastOptions")]
pub struct ToastOptions {
    pub success: ToastTiming,
    pub warning: ToastTiming,
    pub error: ToastTiming,
}

impl Default for ToastOptions {
    fn default() -> Self {
        Self {
            success: ToastTiming::default(),
            warning: ToastTiming::default(),
            error: ToastTiming {
                auto_dismiss: false,
                ..ToastTiming::default()
            },
        }
    }
}

// Accept the former global options without letting them make errors disappear automatically.
#[derive(Default, Deserialize)]
#[serde(default)]
struct StoredToastOptions {
    duration_seconds: Option<u32>,
    auto_dismiss: Option<bool>,
    success: Option<StoredToastTiming>,
    warning: Option<StoredToastTiming>,
    error: Option<StoredToastTiming>,
}

#[derive(Default, Deserialize)]
#[serde(default)]
struct StoredToastTiming {
    duration_seconds: Option<u32>,
    auto_dismiss: Option<bool>,
}

impl StoredToastTiming {
    fn resolve(self, default: ToastTiming) -> ToastTiming {
        ToastTiming {
            duration_seconds: self.duration_seconds.unwrap_or(default.duration_seconds),
            auto_dismiss: self.auto_dismiss.unwrap_or(default.auto_dismiss),
        }
    }
}

impl From<StoredToastOptions> for ToastOptions {
    fn from(stored: StoredToastOptions) -> Self {
        let legacy = StoredToastTiming {
            duration_seconds: stored.duration_seconds,
            auto_dismiss: stored.auto_dismiss,
        }
        .resolve(ToastTiming::default());
        Self {
            success: stored.success.unwrap_or_default().resolve(legacy),
            warning: stored.warning.unwrap_or_default().resolve(legacy),
            error: stored
                .error
                .unwrap_or_default()
                .resolve(Self::default().error),
        }
    }
}

impl ToastOptions {
    fn normalize(&mut self) {
        self.success.normalize();
        self.warning.normalize();
        self.error.normalize();
    }
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    pub version: u32,
    pub columns: Vec<Column>,
    pub column_widths: BTreeMap<Column, f32>,
    pub folders: BTreeMap<String, FolderLayout>,
    pub appearance: Appearance,
    pub sidebar: sidebar::Preferences,
    pub dark: bool,
    pub show_hidden: bool,
    pub confirm_trash: bool,
    pub toast: ToastOptions,
    pub language: crate::i18n::Language,
    pub item_scale: f32,
    pub view: View,
    pub sort: Column,
    pub descending: bool,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            version: 1,
            columns: Column::DEFAULT.to_vec(),
            column_widths: BTreeMap::new(),
            folders: BTreeMap::new(),
            appearance: Appearance::default(),
            sidebar: sidebar::Preferences::default(),
            dark: false,
            show_hidden: false,
            confirm_trash: true,
            toast: ToastOptions::default(),
            language: crate::i18n::Language::System,
            item_scale: 1.0,
            view: View::Details,
            sort: Column::Name,
            descending: false,
        }
    }
}

impl Preferences {
    pub fn path() -> Option<PathBuf> {
        dirs::config_dir().map(|p| p.join("linux-system-explorer/preferences.json"))
    }

    pub fn normalize(&mut self) {
        let mut layout = self.layout();
        layout.normalize();
        self.set_layout(&layout);
        for layout in self.folders.values_mut() {
            layout.normalize();
        }
        self.appearance.normalize();
        self.sidebar.normalize();
        self.toast.normalize();
    }

    pub fn layout(&self) -> FolderLayout {
        FolderLayout {
            view: self.view,
            item_scale: self.item_scale,
            columns: self.columns.clone(),
            column_widths: self.column_widths.clone(),
            sort: self.sort,
            descending: self.descending,
        }
    }

    pub fn set_layout(&mut self, layout: &FolderLayout) {
        self.view = layout.view;
        self.item_scale = layout.item_scale;
        self.columns = layout.columns.clone();
        self.column_widths = layout.column_widths.clone();
        self.sort = layout.sort;
        self.descending = layout.descending;
    }

    pub fn layout_for(&self, path: &Path) -> FolderLayout {
        self.folders
            .get(&folder_key(path))
            .cloned()
            .unwrap_or_else(|| self.layout())
    }

    pub fn load(path: &Path) -> io::Result<Self> {
        match fs::read(path) {
            Ok(bytes) => {
                let mut data: serde_json::Value = serde_json::from_slice(&bytes)
                    .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
                // Older profiles inherit the previously global zoom, preserving their appearance.
                let zoom = data
                    .get("item_scale")
                    .cloned()
                    .unwrap_or(serde_json::json!(1.0));
                if let Some(folders) = data
                    .get_mut("folders")
                    .and_then(serde_json::Value::as_object_mut)
                {
                    for layout in folders
                        .values_mut()
                        .filter_map(serde_json::Value::as_object_mut)
                    {
                        layout.entry("item_scale").or_insert_with(|| zoom.clone());
                    }
                }
                let mut preferences: Self = serde_json::from_value(data)
                    .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
                if preferences.version != 1 {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        crate::i18n::tr("Versão de preferências não suportada"),
                    ));
                }
                preferences.normalize();
                Ok(preferences)
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Self::default()),
            Err(error) => Err(error),
        }
    }

    pub fn save(&self, path: &Path) -> io::Result<()> {
        let parent = path.parent().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                crate::i18n::tr("Caminho de preferências inválido"),
            )
        })?;
        fs::create_dir_all(parent)?;
        let mut file = tempfile::NamedTempFile::new_in(parent)?;
        let data = serde_json::to_vec_pretty(self).map_err(io::Error::other)?;
        file.write_all(&data)?;
        file.write_all(b"\n")?;
        file.as_file().sync_all()?;
        file.persist(path).map_err(|error| error.error)?;
        Ok(())
    }
}

#[derive(PartialEq, Hash)]
enum Tab {
    Columns,
    Appearance,
    General,
    Sidebar,
}

pub struct Editor {
    pub draft: Preferences,
    pub error: Option<String>,
    tab: Tab,
    target: Icon,
    search: String,
    new_pin: String,
    new_network: String,
    new_network_label: String,
    available_sidebar: Vec<SidebarItem>,
    network_mounts: Vec<Shortcut>,
}

pub enum Action {
    None,
    Close,
    ResetFolder,
    Apply(Box<Preferences>, bool),
    TestToast(ToastKind, ToastTiming),
}

impl Editor {
    pub fn new(draft: Preferences) -> Self {
        Self {
            draft,
            error: None,
            tab: Tab::General,
            target: Icon::Home,
            search: String::new(),
            new_pin: String::new(),
            new_network: String::new(),
            new_network_label: String::new(),
            available_sidebar: SidebarItem::ALL
                .into_iter()
                .filter(|item| item.path().is_some_and(|p| p.is_dir()))
                .collect(),
            network_mounts: sidebar::network_mounts(),
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Action {
        let mut action = Action::None;
        let modal = egui::Modal::new(egui::Id::new("preferences_dialog")).show(ctx, |ui| {
            // A known row height centers labels before taller controls are laid out.
            ui.spacing_mut().interact_size.y = 32.0;
            ui.set_width((ctx.content_rect().width() - 64.0).clamp(400.0, 740.0));
            ui.heading(crate::i18n::tr("Configurações"));
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.tab, Tab::General, crate::i18n::tr("Geral"));
                ui.selectable_value(&mut self.tab, Tab::Sidebar, crate::i18n::tr("Barra lateral"));
                ui.selectable_value(&mut self.tab, Tab::Columns, crate::i18n::tr("Colunas"));
                ui.selectable_value(&mut self.tab, Tab::Appearance, crate::i18n::tr("Aparência e ícones"));
            });
            ui.separator();
            let content_height = (ctx.content_rect().height() - 230.0).clamp(160.0, 460.0);
            egui::ScrollArea::vertical()
                .id_salt(("preferences_content", &self.tab))
                .min_scrolled_height(content_height)
                .max_height(content_height)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    match self.tab {
                        Tab::Columns => {
                            if ui.button(crate::i18n::tr("Restaurar padrão desta pasta"))
                                .on_hover_text(crate::i18n::tr("Restaura a visualização, o zoom, as colunas e as larguras apenas da pasta aberta.")).clicked() {
                                action = Action::ResetFolder;
                            }
                            self.columns(ui);
                        },
                        Tab::Appearance => self.appearance(ui),
                        Tab::General => action = self.general(ui),
                        Tab::Sidebar => self.sidebar(ui),
                    }
                });
            ui.separator();
            if let Some(error) = &self.error {
                ui.colored_label(ui.visuals().error_fg_color, error);
            }
            ui.horizontal(|ui| {
                if ui.button(crate::i18n::tr("Restaurar padrão")).clicked() {
                    self.draft = Preferences::default();
                    self.error = None;
                }
                ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                    if ui.button(crate::i18n::tr("Salvar e fechar")).clicked() {
                        action = Action::Apply(Box::new(self.draft.clone()), true);
                    }
                    if ui.button(crate::i18n::tr("Aplicar")).clicked() {
                        action = Action::Apply(Box::new(self.draft.clone()), false);
                    }
                    if ui.button(crate::i18n::tr("Cancelar")).clicked() {
                        action = Action::Close;
                    }
                });
            });
        });
        if modal.should_close() {
            Action::Close
        } else {
            action
        }
    }

    fn columns(&mut self, ui: &mut egui::Ui) {
        ui.label(crate::i18n::tr("Escolha as colunas exibidas na lista de detalhes. Todas permitem ordenação pelo cabeçalho."));
        ui.weak(crate::i18n::tr(
            "Modo de visualização, zoom, colunas e larguras são salvos por pasta.",
        ));
        ui.weak(crate::i18n::tr(
            "Datas não fornecidas pelo sistema de arquivos aparecem como —.",
        ));
        ui.add_space(8.0);
        egui::Grid::new("column_choices")
            .num_columns(2)
            .spacing([26.0, 8.0])
            .show(ui, |ui| {
                for (index, column) in Column::ALL.into_iter().enumerate() {
                    let mut enabled = self.draft.columns.contains(&column);
                    let can_toggle = !enabled || self.draft.columns.len() > 1;
                    let response = ui
                        .add_enabled_ui(can_toggle, |ui| ui.checkbox(&mut enabled, column.label()))
                        .inner;
                    if response.changed() {
                        if enabled {
                            self.draft.columns.push(column);
                        } else {
                            self.draft.columns.retain(|&c| c != column);
                        }
                        self.draft.normalize();
                    }
                    response.on_hover_text(column.description());
                    if index % 2 == 1 {
                        ui.end_row();
                    }
                }
            });
    }

    fn appearance(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(crate::i18n::tr("Cores dos ícones:"));
            ui.selectable_value(
                &mut self.draft.appearance.mode,
                IconMode::Monochrome,
                crate::i18n::tr("Preto e branco"),
            );
            ui.selectable_value(
                &mut self.draft.appearance.mode,
                IconMode::Colored,
                crate::i18n::tr("Coloridos"),
            );
        });
        ui.add(
            egui::Slider::new(&mut self.draft.appearance.size, 16.0..=28.0)
                .text(crate::i18n::tr("Tamanho na barra"))
                .suffix(" px")
                .step_by(1.0),
        );
        ui.separator();
        ui.horizontal(|ui| {
            ui.label(crate::i18n::tr("Personalizar:"));
            egui::ComboBox::from_id_salt("icon_target")
                .selected_text(self.target.label())
                .width(210.0)
                .show_ui(ui, |ui| {
                    for target in Icon::ALL {
                        ui.selectable_value(&mut self.target, target, target.label());
                    }
                });
            if ui.button(crate::i18n::tr("Ícone padrão")).clicked() {
                self.draft.appearance.overrides.remove(&self.target);
            }
        });
        ui.allocate_ui_with_layout(
            Vec2::new(ui.available_width(), 42.0),
            egui::Layout::left_to_right(Align::Center),
            |ui| {
                let (rect, _) = ui.allocate_exact_size(Vec2::splat(42.0), egui::Sense::hover());
                icons::paint(
                    ui.painter(),
                    rect.shrink(5.0),
                    self.target,
                    ui.visuals().text_color(),
                    &self.draft.appearance,
                );
                ui.label(format!(
                    "fa-solid fa-{}",
                    self.draft.appearance.name(self.target)
                ));
            },
        );
        ui.add_sized(
            [ui.available_width(), 32.0],
            egui::TextEdit::singleline(&mut self.search)
                .vertical_align(Align::Center)
                .desired_width(ui.available_width())
                .hint_text(crate::i18n::tr(
                    "Buscar ícone Font Awesome: folder, house, gear…",
                )),
        );
        let query = self.search.to_lowercase();
        let matches: Vec<_> = icons::catalog()
            .iter()
            .filter(|g| g.name.contains(&query) || g.label.to_lowercase().contains(&query))
            .collect();
        ui.weak(crate::i18n::message(
            "{} ícones gratuitos · Font Awesome Free 7.1.0 · Solid",
            &[(matches.len()).to_string()],
        ));
        if matches.is_empty() {
            ui.label(crate::i18n::tr("Nenhum ícone encontrado."));
        }
        egui::ScrollArea::vertical()
            .id_salt("fontawesome_picker")
            .max_height(230.0)
            .show_rows(ui, 32.0, matches.len(), |ui, range| {
                for index in range {
                    let glyph = matches[index];
                    let selected = self.draft.appearance.name(self.target) == glyph.name;
                    let (rect, response) = ui.allocate_exact_size(
                        Vec2::new(ui.available_width(), 32.0),
                        egui::Sense::click(),
                    );
                    if selected || response.hovered() {
                        ui.painter().rect_filled(
                            rect,
                            3.0,
                            if selected {
                                ui.visuals().selection.bg_fill
                            } else {
                                ui.visuals().widgets.hovered.bg_fill
                            },
                        );
                    }
                    icons::paint_glyph(
                        ui.painter(),
                        egui::Rect::from_center_size(
                            egui::pos2(rect.left() + 18.0, rect.center().y),
                            Vec2::splat(20.0),
                        ),
                        glyph,
                        ui.visuals().text_color(),
                    );
                    ui.painter().text(
                        egui::pos2(rect.left() + 40.0, rect.center().y),
                        egui::Align2::LEFT_CENTER,
                        &glyph.name,
                        egui::FontId::proportional(13.0),
                        ui.visuals().text_color(),
                    );
                    if response.on_hover_text(&glyph.label).clicked() {
                        self.draft
                            .appearance
                            .overrides
                            .insert(self.target, glyph.name.clone());
                    }
                }
            });
    }

    fn general(&mut self, ui: &mut egui::Ui) -> Action {
        let mut action = Action::None;
        ui.horizontal(|ui| {
            ui.label(crate::i18n::tr("Idioma:"));
            egui::ComboBox::from_id_salt("interface_language")
                .selected_text(match self.draft.language {
                    crate::i18n::Language::System => crate::i18n::tr("Seguir idioma do sistema"),
                    crate::i18n::Language::Portuguese => "Português",
                    crate::i18n::Language::English => "English",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut self.draft.language,
                        crate::i18n::Language::System,
                        crate::i18n::tr("Seguir idioma do sistema"),
                    );
                    ui.selectable_value(
                        &mut self.draft.language,
                        crate::i18n::Language::Portuguese,
                        "Português",
                    );
                    ui.selectable_value(
                        &mut self.draft.language,
                        crate::i18n::Language::English,
                        "English",
                    );
                });
        });
        ui.weak(crate::i18n::tr("Idiomas não disponíveis usam inglês."));
        ui.separator();
        ui.label(crate::i18n::tr("Notificações"));
        egui::Grid::new("toast_timing_settings")
            .num_columns(4)
            .spacing(Vec2::new(16.0, 8.0))
            .min_row_height(28.0)
            .show(ui, |ui| {
                ui.strong(crate::i18n::tr("Tipo"));
                ui.strong(crate::i18n::tr("Tempo de exibição"));
                ui.label("");
                ui.label("");
                ui.end_row();
                for (kind, label, test, timing) in [
                    (
                        ToastKind::Success,
                        crate::i18n::tr("Sucesso"),
                        crate::i18n::tr("Testar sucesso"),
                        &mut self.draft.toast.success,
                    ),
                    (
                        ToastKind::Warning,
                        crate::i18n::tr("Aviso"),
                        crate::i18n::tr("Testar aviso"),
                        &mut self.draft.toast.warning,
                    ),
                    (
                        ToastKind::Error,
                        crate::i18n::tr("Erro"),
                        crate::i18n::tr("Testar erro"),
                        &mut self.draft.toast.error,
                    ),
                ] {
                    ui.label(label);
                    ui.add_enabled(
                        timing.auto_dismiss,
                        egui::DragValue::new(&mut timing.duration_seconds)
                            .range(1..=300)
                            .suffix(crate::i18n::tr(" s")),
                    );
                    let mut manual = !timing.auto_dismiss;
                    if ui
                        .checkbox(&mut manual, crate::i18n::tr("Fechamento manual"))
                        .changed()
                    {
                        timing.auto_dismiss = !manual;
                    }
                    if ui.button(test).clicked() {
                        action = Action::TestToast(kind, *timing);
                    }
                    ui.end_row();
                }
            });
        ui.separator();
        let mut percentage = self.draft.item_scale * 100.0;
        if ui
            .add(
                egui::Slider::new(&mut percentage, 75.0..=200.0)
                    .step_by(25.0)
                    .text(crate::i18n::tr("Tamanho dos itens desta pasta"))
                    .suffix(" %"),
            )
            .changed()
        {
            self.draft.item_scale = percentage / 100.0;
        }
        ui.checkbox(&mut self.draft.dark, crate::i18n::tr("Tema escuro"));
        ui.checkbox(
            &mut self.draft.show_hidden,
            crate::i18n::tr("Exibir arquivos ocultos"),
        );
        ui.checkbox(
            &mut self.draft.confirm_trash,
            crate::i18n::tr("Pedir confirmação antes de enviar itens à lixeira"),
        );
        ui.weak(crate::i18n::tr(
            "A exclusão permanente (Shift+Del) sempre pede confirmação.",
        ));
        ui.horizontal(|ui| {
            ui.label(crate::i18n::tr("Visualização desta pasta:"));
            ui.selectable_value(
                &mut self.draft.view,
                View::Details,
                crate::i18n::tr("Detalhes"),
            );
            ui.selectable_value(&mut self.draft.view, View::List, crate::i18n::tr("Lista"));
            ui.selectable_value(&mut self.draft.view, View::Icons, crate::i18n::tr("Ícones"));
        });
        ui.separator();
        ui.add_space(10.0);
        ui.weak(crate::i18n::tr(
            "As preferências são salvas neste computador e restauradas na próxima execução.",
        ));
        action
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toast_preferences_round_trip_and_clamp_invalid_durations() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("preferences.json");
        let mut preferences = Preferences {
            toast: ToastOptions {
                success: ToastTiming {
                    duration_seconds: 27,
                    auto_dismiss: false,
                },
                warning: ToastTiming {
                    duration_seconds: 15,
                    auto_dismiss: true,
                },
                error: ToastTiming {
                    duration_seconds: 42,
                    auto_dismiss: true,
                },
            },
            ..Default::default()
        };
        preferences.save(&path).unwrap();
        assert_eq!(Preferences::load(&path).unwrap().toast, preferences.toast);
        for (input, expected) in [(0, 1), (u32::MAX, 300)] {
            preferences.toast.success.duration_seconds = input;
            preferences.toast.warning.duration_seconds = input;
            preferences.toast.error.duration_seconds = input;
            preferences.save(&path).unwrap();
            let loaded = Preferences::load(&path).unwrap().toast;
            for timing in [loaded.success, loaded.warning, loaded.error] {
                assert_eq!(timing.duration_seconds, expected);
            }
        }
    }

    #[test]
    fn legacy_toast_preferences_preserve_success_and_warning_and_keep_errors_manual() {
        for automatic in [false, true] {
            let preferences: Preferences = serde_json::from_value(serde_json::json!({
                "toast": {"duration_seconds": 27, "auto_dismiss": automatic}
            }))
            .unwrap();
            let expected = ToastTiming {
                duration_seconds: 27,
                auto_dismiss: automatic,
            };
            assert_eq!(preferences.toast.success, expected);
            assert_eq!(preferences.toast.warning, expected);
            assert!(!preferences.toast.error.auto_dismiss);
            let saved = serde_json::to_value(&preferences).unwrap();
            assert!(saved["toast"].get("duration_seconds").is_none());
            assert_eq!(saved["toast"]["success"]["duration_seconds"], 27);
            assert_eq!(saved["toast"]["error"]["auto_dismiss"], false);
        }
        let partial: Preferences = serde_json::from_value(serde_json::json!({
            "toast": {"success": {"duration_seconds": 10}, "error": {"duration_seconds": 60}}
        }))
        .unwrap();
        assert_eq!(partial.toast.success.duration_seconds, 10);
        assert!(partial.toast.success.auto_dismiss);
        assert_eq!(partial.toast.warning, ToastTiming::default());
        assert_eq!(partial.toast.error.duration_seconds, 60);
        assert!(!partial.toast.error.auto_dismiss);
    }

    #[test]
    fn preferences_round_trip_replaces_existing_file_and_preserves_customization() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("config/preferences.json");
        let mut preferences = Preferences {
            columns: vec![Column::Name, Column::Owner, Column::Inode],
            dark: true,
            confirm_trash: false,
            ..Default::default()
        };
        preferences.appearance.mode = IconMode::Colored;
        preferences
            .appearance
            .overrides
            .insert(Icon::Home, "house-user".into());
        preferences.save(&path).unwrap();
        let restored = Preferences::load(&path).unwrap();
        assert!(preferences == restored);
        preferences.show_hidden = true;
        preferences.save(&path).unwrap();
        assert!(Preferences::load(&path).unwrap().show_hidden);
        assert_eq!(fs::read_dir(path.parent().unwrap()).unwrap().count(), 1);
    }

    #[test]
    fn invalid_preferences_are_reported_without_overwriting_the_file() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("preferences.json");
        assert!(Preferences::load(&path).unwrap() == Preferences::default());
        fs::write(&path, b"{invalid").unwrap();
        assert!(Preferences::load(&path).is_err());
        assert_eq!(fs::read(&path).unwrap(), b"{invalid");
        fs::write(&path, b"{\"version\":99}").unwrap();
        assert!(Preferences::load(&path).is_err());
        assert_eq!(fs::read(&path).unwrap(), b"{\"version\":99}");
    }

    #[test]
    fn normalizes_missing_columns_and_invalid_glyph_names() {
        let mut preferences: Preferences = serde_json::from_str(
            r#"{"columns":[],"appearance":{"size":999,"overrides":{"home":"pro-does-not-exist"}}}"#,
        )
        .unwrap();
        preferences.normalize();
        assert_eq!(preferences.columns, Column::DEFAULT);
        assert_eq!(preferences.appearance.name(Icon::Home), "house");
        assert_eq!(preferences.appearance.size, 28.0);
        for icon in Icon::ALL {
            assert!(icons::glyph(icon.default_name()).is_some());
        }
        assert!(
            icons::catalog()
                .windows(2)
                .all(|pair| pair[0].name < pair[1].name)
        );
    }

    #[test]
    fn supports_existing_preferences_and_preserves_custom_column_layout_and_list_view() {
        let old: Preferences =
            serde_json::from_str(r#"{"columns":["size","name"],"view":"details"}"#).unwrap();
        assert!(old.column_widths.is_empty());
        assert_eq!(old.language, crate::i18n::Language::System);
        assert_eq!(old.item_scale, 1.0);
        assert_eq!(old.toast, ToastOptions::default());
        assert!(
            old.confirm_trash,
            "preferências anteriores devem exigir confirmação"
        );
        let mut preferences = Preferences {
            columns: vec![Column::Size, Column::Name],
            view: View::List,
            ..Default::default()
        };
        preferences.column_widths.insert(Column::Name, 317.0);
        preferences.column_widths.insert(Column::Size, 140.0);
        preferences.column_widths.insert(Column::Inode, f32::NAN);
        preferences.normalize();
        assert!(!preferences.column_widths.contains_key(&Column::Inode));
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("preferences.json");
        preferences.save(&path).unwrap();
        let restored = Preferences::load(&path).unwrap();
        assert_eq!(restored.columns, vec![Column::Size, Column::Name]);
        assert_eq!(restored.column_widths[&Column::Name], 317.0);
        assert!(restored.view == View::List);
    }

    #[test]
    fn folder_layouts_preserve_distinct_non_utf8_paths_and_normalize_invalid_widths() {
        use std::{ffi::OsString, os::unix::ffi::OsStringExt};
        let first = PathBuf::from(OsString::from_vec(b"/tmp/folder-\xff".to_vec()));
        let second = PathBuf::from(OsString::from_vec(b"/tmp/folder-\xfe".to_vec()));
        assert_ne!(folder_key(&first), folder_key(&second));
        let mut preferences = Preferences::default();
        let layout = FolderLayout {
            view: View::List,
            columns: vec![Column::Size, Column::Name, Column::Name],
            column_widths: BTreeMap::from([(Column::Name, 350.0), (Column::Size, 1.0)]),
            ..Default::default()
        };
        preferences.folders.insert(folder_key(&first), layout);
        preferences
            .folders
            .insert(folder_key(&second), FolderLayout::default());
        preferences.normalize();
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("preferences.json");
        preferences.save(&path).unwrap();
        let loaded = Preferences::load(&path).unwrap();
        assert!(loaded.layout_for(&first).view == View::List);
        assert!(loaded.layout_for(&second).view == View::Details);
        assert_eq!(
            loaded.layout_for(&first).columns,
            vec![Column::Size, Column::Name]
        );
        assert_eq!(loaded.layout_for(&first).column_widths[&Column::Size], 80.0);
        assert_eq!(
            loaded.layout_for(&first).column_widths[&Column::Name],
            350.0
        );
    }

    #[test]
    fn item_zoom_is_normalized_for_defaults_and_folder_profiles() {
        for (input, expected) in [
            (f32::NAN, 1.0),
            (f32::INFINITY, 1.0),
            (-1.0, 0.75),
            (10.0, 2.0),
            (1.3, 1.25),
        ] {
            let mut preferences = Preferences {
                item_scale: input,
                ..Default::default()
            };
            preferences.folders.insert(
                "/folder".into(),
                FolderLayout {
                    view: View::List,
                    item_scale: input,
                    ..Default::default()
                },
            );
            preferences.normalize();
            assert_eq!(preferences.item_scale, expected);
            assert_eq!(preferences.layout().item_scale, expected);
            assert_eq!(
                preferences.layout_for(Path::new("/folder")).item_scale,
                expected
            );
            assert!(preferences.layout_for(Path::new("/folder")).view == View::List);
            assert_eq!(preferences.columns, Column::DEFAULT);
        }
    }

    #[test]
    fn old_folder_profiles_inherit_global_zoom_without_overwriting_explicit_zoom() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("preferences.json");
        fs::write(
            &path,
            r#"{
            "item_scale": 1.5,
            "folders": {
                "/old": {"view": "list", "columns": ["size", "name"]},
                "/custom": {"view": "icons", "item_scale": 0.75}
            }
        }"#,
        )
        .unwrap();
        let preferences = Preferences::load(&path).unwrap();
        assert_eq!(preferences.item_scale, 1.5);
        let old = preferences.layout_for(Path::new("/old"));
        assert_eq!(old.item_scale, 1.5);
        assert!(old.view == View::List);
        assert_eq!(old.columns, vec![Column::Size, Column::Name]);
        assert_eq!(
            preferences.layout_for(Path::new("/custom")).item_scale,
            0.75
        );
        assert_eq!(preferences.layout_for(Path::new("/new")).item_scale, 1.5);
        preferences.save(&path).unwrap();
        assert!(Preferences::load(&path).unwrap() == preferences);
        fs::write(&path, r#"{"folders":{"/old":{"view":"details"}}}"#).unwrap();
        assert_eq!(
            Preferences::load(&path)
                .unwrap()
                .layout_for(Path::new("/old"))
                .item_scale,
            1.0
        );
    }
}
