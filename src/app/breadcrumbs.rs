use eframe::egui::{self, RichText};
use std::path::{Path, PathBuf};

#[derive(Clone)]
pub(super) enum Destination {
    Folder(PathBuf),
    Network(String),
}

/// Collapse leading ancestors into a menu, reserving space for the current folder.
pub(super) fn show(ui: &mut egui::Ui, current: &Path) -> Option<PathBuf> {
    let mut path = PathBuf::new();
    let crumbs = current
        .components()
        .map(|component| {
            path.push(component.as_os_str());
            (
                component.as_os_str().to_string_lossy().into_owned(),
                Destination::Folder(path.clone()),
                path.display().to_string(),
            )
        })
        .collect();
    match show_crumbs(ui, crumbs)? {
        Destination::Folder(path) => Some(path),
        Destination::Network(_) => None,
    }
}

pub(super) fn show_network(
    ui: &mut egui::Ui,
    location: crate::network::location::Location,
) -> Option<Destination> {
    show_crumbs(
        ui,
        location
            .crumbs
            .into_iter()
            .map(|crumb| {
                let destination = crumb
                    .local
                    .map(Destination::Folder)
                    .unwrap_or_else(|| Destination::Network(crumb.address.clone()));
                (crumb.label, destination, crumb.address)
            })
            .collect(),
    )
}

fn show_crumbs(
    ui: &mut egui::Ui,
    crumbs: Vec<(String, Destination, String)>,
) -> Option<Destination> {
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        ui.spacing_mut().button_padding.x = 4.0;
        show_path(ui, crumbs)
    })
    .inner
}

fn show_path(ui: &mut egui::Ui, crumbs: Vec<(String, Destination, String)>) -> Option<Destination> {
    let font = egui::FontId::proportional(12.0);
    let text_width = |text: &str| {
        ui.painter()
            .layout_no_wrap(text.to_owned(), font.clone(), ui.visuals().text_color())
            .size()
            .x
    };
    let padding = ui.spacing().button_padding.x * 2.0;
    let gap = text_width("›") + ui.spacing().item_spacing.x * 2.0;
    let crumbs: Vec<_> = crumbs
        .into_iter()
        .map(|(label, destination, tooltip)| {
            let width = text_width(&label) + padding;
            (label, destination, tooltip, width)
        })
        .collect();
    if crumbs.is_empty() {
        return None;
    }

    let available = ui.available_width();
    let full_width = crumbs.iter().map(|(_, _, _, width)| width).sum::<f32>()
        + gap * crumbs.len().saturating_sub(1) as f32;
    let mut start = 0;
    let mut current_width = crumbs.last().unwrap().3;
    let menu_width = text_width("…") + padding;
    if full_width > available && crumbs.len() > 1 {
        start = crumbs.len() - 1;
        current_width = current_width.min((available - menu_width - gap).max(1.0));
        let mut used = menu_width + gap + current_width;
        while start > 1 && used + gap + crumbs[start - 1].3 <= available {
            start -= 1;
            used += gap + crumbs[start].3;
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
                for (label, target, tooltip, _) in &crumbs[..start] {
                    if ui
                        .add(egui::Button::new(label).truncate())
                        .on_hover_text(tooltip)
                        .clicked()
                    {
                        destination = Some(target.clone());
                        ui.close();
                    }
                }
            })
            .0
            .on_hover_text(crate::i18n::tr("Pastas anteriores do caminho"));
            ui.label(RichText::new("›").size(12.0).weak());
        }
        for (index, (label, target, tooltip, width)) in crumbs.iter().enumerate().skip(start) {
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
                .on_hover_text(tooltip);
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
                destination = Some(target.clone());
            }
            if !last {
                ui.label(RichText::new("›").size(12.0).weak());
            }
        }
    });
    destination
}
