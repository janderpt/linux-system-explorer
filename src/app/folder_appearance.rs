use super::*;
use crate::folder_style::{self, Style};
pub(super) struct Editor {
    pub path: PathBuf,
    pub appearance: bool,
    pub draft: Option<Style>,
    pub original: Option<Style>,
    search: String,
}
impl Editor {
    pub fn new(path: PathBuf, style: Option<Style>, appearance: bool) -> Self {
        Self {
            path,
            appearance,
            draft: style.clone(),
            original: style,
            search: String::new(),
        }
    }
    fn style(&self, appearance: &icons::Appearance) -> Style {
        self.draft.clone().unwrap_or_else(|| Style {
            icon: appearance.name(Icon::Folder).to_owned(),
            color: [210, 153, 20],
        })
    }
    pub fn show(&mut self, ui: &mut egui::Ui, appearance: &icons::Appearance) {
        ui.label(crate::i18n::tr("Escolha a cor e o ícone desta pasta."));
        ui.label(
            RichText::new(crate::i18n::tr(
                "A personalização tem prioridade sobre o tema e o modo de cores dos ícones.",
            ))
            .small()
            .weak(),
        );
        let mut style = self.style(appearance);
        ui.horizontal(|ui| {
            let (rect, _) = ui.allocate_exact_size(Vec2::splat(48.0), Sense::hover());
            if let Some(style) = &self.draft {
                style.paint(ui.painter(), rect.shrink(4.0));
            } else {
                icons::paint(
                    ui.painter(),
                    rect.shrink(4.0),
                    Icon::Folder,
                    ui.visuals().text_color(),
                    appearance,
                );
            }
            ui.label(crate::i18n::tr("Cor da pasta"));
            if ui.color_edit_button_srgb(&mut style.color).changed() {
                self.draft = Some(style.clone());
            }
            ui.label(format!(
                "#{:02X}{:02X}{:02X}",
                style.color[0], style.color[1], style.color[2]
            ));
        });
        ui.horizontal(|ui| {
            ui.label(crate::i18n::tr("Cores sugeridas"));
            for color in [
                [210, 153, 20],
                [210, 65, 65],
                [36, 132, 204],
                [30, 160, 105],
                [150, 80, 190],
                [230, 125, 35],
                [110, 120, 135],
            ] {
                let (rect, response) = ui.allocate_exact_size(Vec2::splat(24.0), Sense::click());
                ui.painter().rect_filled(
                    rect.shrink(2.0),
                    4.0,
                    Color32::from_rgb(color[0], color[1], color[2]),
                );
                if response.clicked() {
                    style.color = color;
                    self.draft = Some(style.clone());
                }
            }
        });
        ui.add_sized(
            [ui.available_width(), 30.0],
            egui::TextEdit::singleline(&mut self.search)
                .vertical_align(Align::Center)
                .hint_text(crate::i18n::tr(
                    "Buscar ícone Font Awesome: folder, house, gear…",
                )),
        );
        let query = self.search.trim().to_lowercase();
        let matches: Vec<_> = icons::catalog()
            .iter()
            .filter(|g| g.name.contains(&query) || g.label.to_lowercase().contains(&query))
            .collect();
        if matches.is_empty() {
            ui.weak(crate::i18n::tr("Nenhum ícone encontrado."));
        }
        egui::ScrollArea::vertical()
            .id_salt("folder_icon_catalog")
            .max_height(210.0)
            .auto_shrink([false, false])
            .show_rows(ui, 30.0, matches.len(), |ui, rows| {
                for row in rows {
                    let glyph = matches[row];
                    let (rect, response) = ui
                        .allocate_exact_size(Vec2::new(ui.available_width(), 30.0), Sense::click());
                    if style.icon == glyph.name || response.hovered() {
                        ui.painter().rect_filled(
                            rect,
                            3.0,
                            if style.icon == glyph.name {
                                ui.visuals().selection.bg_fill
                            } else {
                                ui.visuals().widgets.hovered.bg_fill
                            },
                        );
                    }
                    icons::paint_glyph(
                        ui.painter(),
                        egui::Rect::from_center_size(
                            rect.left_center() + Vec2::new(18.0, 0.0),
                            Vec2::splat(20.0),
                        ),
                        glyph,
                        Color32::from_rgb(style.color[0], style.color[1], style.color[2]),
                    );
                    ui.painter().text(
                        rect.left_center() + Vec2::new(38.0, 0.0),
                        egui::Align2::LEFT_CENTER,
                        &glyph.name,
                        FontId::proportional(13.0),
                        ui.visuals().text_color(),
                    );
                    if response.on_hover_text(&glyph.label).clicked() {
                        style.icon = glyph.name.clone();
                        self.draft = Some(style.clone());
                    }
                }
            });
    }
}
impl Explorer {
    pub(super) fn paint_folder_icon(
        &self,
        painter: &egui::Painter,
        path: &Path,
        rect: egui::Rect,
        fallback: Icon,
        color: Color32,
    ) {
        if !folder_style::paint(&self.folder_styles, path, painter, rect) {
            icons::paint(painter, rect, fallback, color, &self.appearance);
        }
    }
    pub(super) fn customize_folder_menu(&mut self, ui: &mut egui::Ui, path: &Path) {
        if path.is_absolute()
            && self
                .menu_item(
                    ui,
                    Icon::CustomizeFolder,
                    crate::i18n::tr("Personalizar pasta…"),
                    "",
                    !self.loading,
                    false,
                )
                .clicked()
        {
            match filesystem::read_entry(path) {
                Ok(entry) if entry.is_dir && entry.trash.is_none() => {
                    self.folder_editor = Some(Editor::new(
                        path.to_owned(),
                        self.folder_styles.get(&settings::folder_key(path)).cloned(),
                        true,
                    ));
                    self.properties = Some(entry);
                }
                Ok(_) => {}
                Err(error) => self.report_error(error.to_string()),
            }
            ui.close();
        }
    }
    pub(super) fn save_folder_style(&mut self, path: &Path, style: Option<Style>) -> bool {
        let mut styles = self.folder_styles.clone();
        let key = settings::folder_key(path);
        if let Some(style) = style {
            styles.insert(key, style);
        } else {
            styles.remove(&key);
        }
        let mut preferences = self.stored_preferences(&self.preferences());
        preferences.folder_styles = styles.clone();
        preferences.normalize();
        if let Some(path) = &self.settings_path
            && let Err(error) = preferences.save(path)
        {
            self.report_error(crate::i18n::message(
                "Não foi possível salvar a aparência da pasta: {}",
                &[error.to_string()],
            ));
            return false;
        }
        self.folder_styles = preferences.folder_styles.clone();
        self.last_attempted_preferences = preferences;
        true
    }
}
#[cfg(test)]
mod tests;
