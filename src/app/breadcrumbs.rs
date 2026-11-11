use eframe::egui::{self, RichText};
use std::path::{Path, PathBuf};

/// Collapse leading ancestors into a menu, reserving space for the current folder.
pub(super) fn show(ui: &mut egui::Ui, current: &Path) -> Option<PathBuf> {
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        ui.spacing_mut().button_padding.x = 4.0;
        show_path(ui, current)
    })
    .inner
}

fn show_path(ui: &mut egui::Ui, current: &Path) -> Option<PathBuf> {
    let font = egui::FontId::proportional(12.0);
    let text_width = |text: &str| {
        ui.painter()
            .layout_no_wrap(text.to_owned(), font.clone(), ui.visuals().text_color())
            .size()
            .x
    };
    let padding = ui.spacing().button_padding.x * 2.0;
    let gap = text_width("›") + ui.spacing().item_spacing.x * 2.0;
    let mut path = PathBuf::new();
    let crumbs: Vec<_> = current
        .components()
        .map(|component| {
            path.push(component.as_os_str());
            let label = if component.as_os_str() == "/" {
                "/".to_owned()
            } else {
                component.as_os_str().to_string_lossy().into_owned()
            };
            let width = text_width(&label) + padding;
            (label, path.clone(), width)
        })
        .collect();
    if crumbs.is_empty() {
        return None;
    }

    let available = ui.available_width();
    let full_width = crumbs.iter().map(|(_, _, width)| width).sum::<f32>()
        + gap * crumbs.len().saturating_sub(1) as f32;
    let mut start = 0;
    let mut current_width = crumbs.last().unwrap().2;
    let menu_width = text_width("…") + padding;
    if full_width > available && crumbs.len() > 1 {
        start = crumbs.len() - 1;
        current_width = current_width.min((available - menu_width - gap).max(1.0));
        let mut used = menu_width + gap + current_width;
        while start > 1 && used + gap + crumbs[start - 1].2 <= available {
            start -= 1;
            used += gap + crumbs[start].2;
        }
    } else {
        current_width = current_width.min(available.max(1.0));
    }

    let mut destination = None;
    ui.horizontal(|ui| {
        if start > 0 {
            egui::containers::menu::MenuButton::from_button(
                egui::Button::new(RichText::new("…").font(font.clone()))
                    .frame(false)
                    .small()
                    .min_size(egui::vec2(menu_width, 22.0)),
            )
            .ui(ui, |ui| {
                ui.set_max_width(360.0);
                for (label, path, _) in &crumbs[..start] {
                    if ui
                        .add(egui::Button::new(label).truncate())
                        .on_hover_text(path.display().to_string())
                        .clicked()
                    {
                        destination = Some(path.clone());
                        ui.close();
                    }
                }
            })
            .0
            .on_hover_text(crate::i18n::tr("Pastas anteriores do caminho"));
            ui.label(RichText::new("›").size(12.0).weak());
        }
        for (index, (label, path, width)) in crumbs.iter().enumerate().skip(start) {
            let last = index == crumbs.len() - 1;
            let width = if last { current_width } else { *width };
            let text = RichText::new(label).font(font.clone());
            let button = egui::Button::new(if last { text.strong() } else { text.weak() })
                .frame(false)
                .small()
                .truncate();
            let response = ui
                .add_sized([width, 22.0], button)
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .on_hover_text(path.display().to_string());
            if response.hovered() {
                ui.painter().line_segment(
                    [
                        response.rect.left_bottom() + egui::vec2(4.0, -3.0),
                        response.rect.right_bottom() + egui::vec2(-4.0, -3.0),
                    ],
                    (1.0, ui.visuals().selection.stroke.color),
                );
            }
            if response.clicked() {
                destination = Some(path.clone());
            }
            if !last {
                ui.label(RichText::new("›").size(12.0).weak());
            }
        }
    });
    destination
}
