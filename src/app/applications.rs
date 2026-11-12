use super::*;
use crate::applications::menu::{self, Launcher};

#[derive(Default)]
pub(super) struct Browser {
    pub items: Vec<Launcher>,
    selected: Option<String>,
    message: Option<String>,
    reveal: bool,
    columns: usize,
    order: AppColumn,
    descending: bool,
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
        let mut visible: Vec<_> = self
            .items
            .iter()
            .enumerate()
            .filter(|(_, item)| item.application.name.to_lowercase().contains(&query))
            .map(|(index, _)| index)
            .collect();
        visible.sort_by_cached_key(|&i| {
            (
                self.order.value(&self.items[i]).to_lowercase(),
                self.items[i].application.id.clone(),
            )
        });
        if self.descending {
            visible.reverse();
        }
        visible
    }
}
#[derive(Clone, Copy, Default, PartialEq)]
enum AppColumn {
    #[default]
    Name,
    Description,
    Id,
}
impl AppColumn {
    fn value(self, item: &Launcher) -> &str {
        match self {
            Self::Name => &item.application.name,
            Self::Description => &item.application.description,
            Self::Id => &item.application.id,
        }
    }
    fn label(self) -> &'static str {
        crate::i18n::tr(match self {
            Self::Name => "Nome",
            Self::Description => "Descrição",
            Self::Id => "Identificador do aplicativo",
        })
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
            Key::ArrowLeft,
            Key::ArrowRight,
            Key::Home,
            Key::End,
            Key::PageUp,
            Key::PageDown,
        ] {
            if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, key)) {
                let current = selected.unwrap_or(0);
                let page = self.navigation_page.max(1);
                let columns = if self.view == View::Icons {
                    self.applications.columns.max(1)
                } else {
                    1
                };
                let next = match key {
                    Key::Home => 0,
                    Key::End => visible.len() - 1,
                    Key::ArrowUp => current.saturating_sub(columns),
                    Key::ArrowLeft => current.saturating_sub(1),
                    Key::ArrowDown if selected.is_some() => {
                        (current + columns).min(visible.len() - 1)
                    }
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
        if self.view == View::Details {
            egui::ScrollArea::horizontal()
                .id_salt((self.tab_id, "applications_columns"))
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.set_min_width(660.0);
                    self.application_headers(ui);
                    let visible = self.applications.visible(&self.query);
                    self.application_rows(ui, &visible);
                });
        } else {
            self.application_rows(ui, &visible);
        }
    }
    fn application_headers(&mut self, ui: &mut egui::Ui) {
        let width = ui.available_width();
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            for (column, fraction) in [
                (AppColumn::Name, 0.34),
                (AppColumn::Description, 0.40),
                (AppColumn::Id, 0.26),
            ] {
                let (rect, response) = ui.allocate_exact_size(
                    Vec2::new(width * fraction, self.chrome_row_height()),
                    Sense::click(),
                );
                let label = if self.applications.order == column {
                    format!(
                        "{} {}",
                        column.label(),
                        if self.applications.descending {
                            "↓"
                        } else {
                            "↑"
                        }
                    )
                } else {
                    column.label().to_owned()
                };
                ui.painter().with_clip_rect(rect.shrink(5.0)).text(
                    rect.left_center() + Vec2::new(5.0, 0.0),
                    egui::Align2::LEFT_CENTER,
                    label,
                    FontId::proportional(13.0),
                    ui.visuals().text_color(),
                );
                if response.clicked() {
                    if self.applications.order == column {
                        self.applications.descending = !self.applications.descending;
                    } else {
                        self.applications.order = column;
                        self.applications.descending = false;
                    }
                }
            }
        });
        ui.separator();
    }
    fn application_rows(&mut self, ui: &mut egui::Ui, visible: &[usize]) {
        let scale = self.item_scale;
        let grid = self.view == View::Icons;
        let tile_width = 136.0 * scale;
        let height = if grid {
            122.0
        } else if self.view == View::Details {
            36.0
        } else {
            44.0
        } * scale;
        let gap = 8.0;
        let columns = if grid {
            ((ui.available_width() + gap) / (tile_width + gap))
                .floor()
                .max(1.0) as usize
        } else {
            1
        };
        self.applications.columns = columns;
        self.navigation_page = (ui.available_height() / height).floor().max(1.0) as usize * columns;
        let mut scroll = egui::ScrollArea::vertical()
            .id_salt((self.tab_id, "applications", self.view as u8))
            .auto_shrink([false, false]);
        if self.applications.reveal {
            if let Some(index) = visible.iter().position(|&i| {
                Some(&self.applications.items[i].application.id)
                    == self.applications.selected.as_ref()
            }) {
                scroll = scroll.vertical_scroll_offset((index / columns) as f32 * height);
            }
            self.applications.reveal = false;
        }
        scroll.show_rows(ui, height, visible.len().div_ceil(columns), |ui, rows| {
            let needed = visible
                .iter()
                .skip(rows.start * columns)
                .take(rows.len() * columns)
                .filter_map(|&i| {
                    self.applications.items[i]
                        .icon
                        .as_ref()
                        .map(|icon| icon.path.clone())
                })
                .collect();
            self.thumbnails.cancel_unseen(&needed);
            for row in rows {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = gap;
                    for &i in visible.iter().skip(row * columns).take(columns) {
                        let item = self.applications.items[i].clone();
                        let width = if grid {
                            tile_width
                        } else {
                            ui.available_width()
                        };
                        let (rect, response) =
                            ui.allocate_exact_size(Vec2::new(width, height), Sense::click());
                        let selected =
                            self.applications.selected.as_ref() == Some(&item.application.id);
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
                        let icon_size = if grid {
                            56.0
                        } else if self.view == View::Details {
                            24.0
                        } else {
                            32.0
                        } * scale;
                        let center = if grid {
                            rect.center_top() + Vec2::new(0.0, 38.0 * scale)
                        } else {
                            rect.left_center() + Vec2::new(24.0 * scale, 0.0)
                        };
                        self.application_icon(
                            ui,
                            &item,
                            egui::Rect::from_center_size(center, Vec2::splat(icon_size)),
                            color,
                        );
                        if grid {
                            let text = egui::Rect::from_min_max(
                                rect.left_top() + Vec2::new(6.0, 72.0 * scale),
                                rect.right_bottom() - Vec2::splat(6.0),
                            );
                            let mut job = egui::text::LayoutJob::simple(
                                item.application.name.clone(),
                                FontId::proportional(13.0 * scale),
                                color,
                                text.width(),
                            );
                            job.wrap.max_rows = 2;
                            job.wrap.break_anywhere = true;
                            let galley = ui.fonts_mut(|f| f.layout_job(job));
                            ui.painter().with_clip_rect(text).galley(
                                egui::pos2(text.center().x - galley.size().x * 0.5, text.top()),
                                galley,
                                color,
                            );
                        } else {
                            let name_width = if self.view == View::Details {
                                width * 0.34
                            } else {
                                width
                            };
                            let text = egui::Rect::from_min_max(
                                rect.left_top() + Vec2::new(50.0 * scale, 0.0),
                                egui::pos2(rect.left() + name_width - 6.0, rect.bottom()),
                            );
                            ui.painter().with_clip_rect(text).text(
                                text.left_center(),
                                egui::Align2::LEFT_CENTER,
                                &item.application.name,
                                FontId::proportional(14.0 * scale),
                                color,
                            );
                            if self.view == View::Details {
                                for (value, start, fraction) in [
                                    (&item.application.description, 0.34, 0.40),
                                    (&item.application.id, 0.74, 0.26),
                                ] {
                                    let text = egui::Rect::from_min_max(
                                        rect.left_top() + Vec2::new(width * start + 5.0, 0.0),
                                        egui::pos2(
                                            rect.left() + width * (start + fraction) - 6.0,
                                            rect.bottom(),
                                        ),
                                    );
                                    ui.painter().with_clip_rect(text).text(
                                        text.left_center(),
                                        egui::Align2::LEFT_CENTER,
                                        value,
                                        FontId::proportional(13.0 * scale),
                                        color,
                                    );
                                }
                            }
                        }
                        self.application_response(ui, response, &item, selected);
                    }
                });
            }
        });
    }
    fn application_icon(
        &mut self,
        ui: &egui::Ui,
        item: &Launcher,
        icon: egui::Rect,
        color: Color32,
    ) {
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
    }
    fn application_response(
        &mut self,
        ui: &egui::Ui,
        response: egui::Response,
        item: &Launcher,
        selected: bool,
    ) {
        let application = &item.application;
        response.widget_info(|| {
            egui::WidgetInfo::selected(
                egui::WidgetType::SelectableLabel,
                true,
                selected,
                &application.name,
            )
        });
        if response.clicked() || response.secondary_clicked() {
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
            .on_hover_text(format!(
                "{}\n{}\n{}",
                application.name, application.description, application.id
            ))
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
                if self
                    .menu_item(
                        ui,
                        Icon::SoftwareManager,
                        crate::i18n::tr("Abrir no gerenciador de software"),
                        "",
                        true,
                        false,
                    )
                    .clicked()
                {
                    self.manage_application(application.clone(), ui.ctx());
                    ui.close();
                }
            });
    }
    fn manage_application(
        &self,
        application: crate::applications::Application,
        ctx: &egui::Context,
    ) {
        let sender = self.event_sender.clone();
        let context = ctx.clone();
        let locale = crate::i18n::current();
        thread::spawn(move || {
            crate::i18n::set_locale(locale);
            let _ = sender.send(Event::SoftwareOpened(
                crate::applications::menu::software::open(&application),
            ));
            context.request_repaint();
        });
    }
}

#[cfg(test)]
mod tests;
