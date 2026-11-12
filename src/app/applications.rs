use super::*;
use crate::applications::menu::{self, Launcher};

#[derive(Default)]
pub(super) struct Browser {
    pub items: Vec<Launcher>,
    selected: Option<String>,
    message: Option<String>,
    reveal: bool,
}
impl Browser {
    pub fn replace(&mut self, items: Vec<Launcher>, message: Option<String>) {
        if self
            .selected
            .as_ref()
            .is_some_and(|id| !items.iter().any(|item| &item.application.id == id))
        {
            self.selected = None;
        }
        self.items = items;
        self.message = message;
    }
    pub fn visible(&self, query: &str) -> Vec<usize> {
        let query = query.trim().to_lowercase();
        self.items
            .iter()
            .enumerate()
            .filter(|(_, item)| item.application.name.to_lowercase().contains(&query))
            .map(|(index, _)| index)
            .collect()
    }
}
impl Explorer {
    pub(super) fn is_applications(&self) -> bool {
        menu::is_location(&self.current)
    }
    fn launch_application(
        &self,
        application: crate::applications::Application,
        ctx: &egui::Context,
    ) {
        let sender = self.event_sender.clone();
        let context = ctx.clone();
        let locale = crate::i18n::current();
        thread::spawn(move || {
            crate::i18n::set_locale(locale);
            if let Err(error) = menu::launch(&application) {
                let _ = sender.send(Event::OpenFailed(error));
                context.request_repaint();
            }
        });
    }
    pub(super) fn application_shortcuts(&mut self, ctx: &egui::Context) {
        if ctx.wants_keyboard_input() || self.loading {
            return;
        }
        let visible = self.applications.visible(&self.query);
        let selected = self.applications.selected.as_ref().and_then(|id| {
            visible
                .iter()
                .position(|&i| &self.applications.items[i].application.id == id)
        });
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Enter))
            && let Some(index) = selected
        {
            self.launch_application(
                self.applications.items[visible[index]].application.clone(),
                ctx,
            );
            return;
        }
        if visible.is_empty() {
            return;
        }
        for key in [
            Key::ArrowUp,
            Key::ArrowDown,
            Key::Home,
            Key::End,
            Key::PageUp,
            Key::PageDown,
        ] {
            if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, key)) {
                let current = selected.unwrap_or(0);
                let page = self.navigation_page.max(1);
                let next = match key {
                    Key::Home => 0,
                    Key::End => visible.len() - 1,
                    Key::ArrowUp => current.saturating_sub(1),
                    Key::PageUp => current.saturating_sub(page),
                    Key::PageDown => (current + page).min(visible.len() - 1),
                    _ => {
                        if selected.is_none() {
                            0
                        } else {
                            (current + 1).min(visible.len() - 1)
                        }
                    }
                };
                self.applications.selected = Some(
                    self.applications.items[visible[next]]
                        .application
                        .id
                        .clone(),
                );
                self.applications.reveal = true;
                break;
            }
        }
    }
    pub(super) fn application_list(&mut self, ui: &mut egui::Ui) {
        if self.loading {
            return;
        }
        if let Some(message) = &self.applications.message {
            ui.add_space(18.0);
            ui.label(message);
            return;
        }
        let visible = self.applications.visible(&self.query);
        if visible.is_empty() {
            ui.add_space(18.0);
            ui.label(crate::i18n::tr("Nenhum aplicativo encontrado"));
            return;
        }
        let scale = self.item_scale;
        let height = 44.0 * scale;
        self.navigation_page = (ui.available_height() / height).floor().max(1.0) as usize;
        let mut scroll = egui::ScrollArea::vertical()
            .id_salt((self.tab_id, "applications"))
            .auto_shrink([false, false]);
        if self.applications.reveal {
            if let Some(index) = visible.iter().position(|&i| {
                Some(&self.applications.items[i].application.id)
                    == self.applications.selected.as_ref()
            }) {
                scroll = scroll.vertical_scroll_offset(index as f32 * height);
            }
            self.applications.reveal = false;
        }
        scroll.show_rows(ui, height, visible.len(), |ui, rows| {
            let needed = rows
                .clone()
                .filter_map(|row| {
                    self.applications.items[visible[row]]
                        .icon
                        .as_ref()
                        .map(|icon| icon.path.clone())
                })
                .collect();
            self.thumbnails.cancel_unseen(&needed);
            for row in rows {
                let item = self.applications.items[visible[row]].clone();
                let application = &item.application;
                let (rect, response) =
                    ui.allocate_exact_size(Vec2::new(ui.available_width(), height), Sense::click());
                let selected = self.applications.selected.as_ref() == Some(&application.id);
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
                let color = if selected {
                    ui.visuals().selection.stroke.color
                } else {
                    ui.visuals().text_color()
                };
                let icon = egui::Rect::from_center_size(
                    rect.left_center() + Vec2::new(24.0 * scale, 0.0),
                    Vec2::splat(32.0 * scale),
                );
                if let Some(texture) = item
                    .icon
                    .as_ref()
                    .and_then(|entry| self.thumbnails.texture(entry, ui.ctx()))
                {
                    let size = texture.size_vec2();
                    let shown = size * (icon.width() / size.x).min(icon.height() / size.y);
                    egui::Image::new((texture.id(), shown))
                        .paint_at(ui, egui::Rect::from_center_size(icon.center(), shown));
                } else {
                    icons::paint(
                        ui.painter(),
                        icon,
                        Icon::Applications,
                        color,
                        &self.appearance,
                    );
                }
                let text = egui::Rect::from_min_max(
                    egui::pos2(rect.left() + 50.0 * scale, rect.top()),
                    rect.right_bottom(),
                );
                ui.painter().with_clip_rect(text).text(
                    text.left_center(),
                    egui::Align2::LEFT_CENTER,
                    &application.name,
                    FontId::proportional(14.0 * scale),
                    color,
                );
                response.widget_info(|| {
                    egui::WidgetInfo::selected(
                        egui::WidgetType::SelectableLabel,
                        true,
                        selected,
                        &application.name,
                    )
                });
                if response.clicked() {
                    self.applications.selected = Some(application.id.clone());
                    ui.ctx().memory_mut(|memory| {
                        if let Some(id) = memory.focused() {
                            memory.surrender_focus(id)
                        }
                    });
                }
                if response.double_clicked() {
                    self.launch_application(application.clone(), ui.ctx());
                }
                response
                    .on_hover_text(format!("{}\n{}", application.description, application.id))
                    .context_menu(|ui| {
                        if self
                            .menu_item(
                                ui,
                                Icon::Open,
                                crate::i18n::tr("Abrir"),
                                "Enter",
                                true,
                                false,
                            )
                            .clicked()
                        {
                            self.launch_application(application.clone(), ui.ctx());
                            ui.close();
                        }
                    });
            }
        });
    }
}

#[cfg(test)]
mod tests;
