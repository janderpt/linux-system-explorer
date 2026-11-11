use super::*;

use crate::session::closed::BrowsingContext;
pub(super) use crate::session::closed::ClosedTab;

impl Workspace {
    pub(super) fn restore_closed_history(
        &mut self,
        saved: &std::collections::VecDeque<ClosedTab>,
        restore_open: bool,
    ) {
        let mut ids: BTreeMap<u64, u64> = if restore_open {
            self.tabs
                .iter()
                .enumerate()
                .map(|(i, tab)| (i as u64, tab.tab_id))
                .collect()
        } else {
            BTreeMap::new()
        };
        for tab in saved.iter().rev().take(25).rev() {
            ids.insert(tab.id, self.next_id);
            self.next_id += 1;
        }
        self.closed_tabs = saved
            .iter()
            .rev()
            .take(25)
            .rev()
            .cloned()
            .map(|mut tab| {
                tab.id = ids[&tab.id];
                tab.left = tab.left.and_then(|id| ids.get(&id).copied());
                tab.right = tab.right.and_then(|id| ids.get(&id).copied());
                tab
            })
            .collect();
    }
    pub(super) fn remember_closed(&mut self, index: usize) {
        let tab = &self.tabs[index];
        let context =
            (tab.location() == tab.current && !tab.current.as_os_str().is_empty()).then(|| {
                BrowsingContext {
                    history: tab.history.clone(),
                    layout: tab.current_layout(),
                    query: tab.query.clone(),
                    selected: tab.selected.clone(),
                    anchor: tab.selection_anchor.clone(),
                    cursor: tab.selection_cursor.clone(),
                    reveal_hidden: tab.reveal_hidden,
                }
            });
        self.closed_tabs.push_back(ClosedTab {
            id: tab.tab_id,
            position: index,
            left: index.checked_sub(1).map(|i| self.tabs[i].tab_id),
            right: self.tabs.get(index + 1).map(|tab| tab.tab_id),
            path: tab.location().to_owned(),
            context,
        });
        if self.closed_tabs.len() > 25 {
            self.closed_tabs.pop_front();
        }
    }

    pub(super) fn reopen_closed_tab(&mut self, ctx: &egui::Context) {
        if let Some(id) = self.closed_tabs.back().map(|tab| tab.id) {
            self.reopen_closed_tab_id(id, ctx);
        }
    }

    pub(super) fn reopen_closed_tab_id(&mut self, id: u64, ctx: &egui::Context) {
        let Some(index) = self.closed_tabs.iter().position(|tab| tab.id == id) else {
            return;
        };
        let Some(closed) = self.closed_tabs.remove(index) else {
            return;
        };
        self.capture(ctx);
        self.append_tab(closed.path.clone(), ctx);
        let copy = self.tabs.last_mut().unwrap();
        copy.tab_id = closed.id;
        if let Some(context) = closed.context {
            // The initial read refreshes this path without clearing the restored context.
            copy.current = closed.path;
            copy.history = context.history;
            copy.apply_layout(context.layout);
            copy.query = context.query;
            copy.selected = context.selected;
            copy.selection_anchor = context.anchor;
            copy.selection_cursor = context.cursor;
            copy.reveal_selection = !copy.selected.is_empty();
            copy.reveal_hidden = context.reveal_hidden;
        }
        let id = copy.tab_id;
        let position = closed
            .right
            .and_then(|id| self.tabs.iter().position(|tab| tab.tab_id == id))
            .or_else(|| {
                closed.left.and_then(|id| {
                    self.tabs
                        .iter()
                        .position(|tab| tab.tab_id == id)
                        .map(|i| i + 1)
                })
            })
            .unwrap_or(closed.position)
            .min(self.tabs.len() - 1);
        self.reorder_tab(id, position);
        self.activate(position, ctx);
        ctx.request_repaint();
    }

    pub(super) fn recently_closed_menu(&self, ui: &mut egui::Ui) -> Option<u64> {
        use egui::AtomExt;
        let mut chosen = None;
        let appearance = &self.tabs[self.active].appearance;
        ui.add_enabled_ui(!self.closed_tabs.is_empty(), |ui| {
            let button = egui::Button::new((
                icons::menu_symbol(ui, Icon::ReopenTab, appearance),
                crate::i18n::tr("Abas fechadas recentemente").atom_shrink(true),
            ))
            .right_text(egui::containers::menu::SubMenuButton::RIGHT_ARROW)
            .min_size(Vec2::new(0.0, (appearance.size + 10.0).max(30.0)));
            egui::containers::menu::SubMenuButton::from_button(button).ui(ui, |ui| {
                ui.set_width((ui.ctx().content_rect().width() - 80.0).clamp(220.0, 440.0));
                egui::ScrollArea::vertical()
                    .id_salt("recently_closed_tabs")
                    .max_height((ui.ctx().content_rect().height() - 160.0).clamp(100.0, 320.0))
                    .show(ui, |ui| {
                        for tab in self.closed_tabs.iter().rev() {
                            ui.push_id(tab.id, |ui| {
                                let trash = crate::trashbin::is_location(&tab.path);
                                let title = if trash {
                                    crate::i18n::tr("Lixeira").to_owned()
                                } else {
                                    tab.path
                                        .file_name()
                                        .unwrap_or(tab.path.as_os_str())
                                        .to_string_lossy()
                                        .into_owned()
                                };
                                let path = tab.path.display().to_string();
                                let width =
                                    (ui.available_width() - appearance.size - 32.0).max(40.0);
                                let mut title_job = egui::text::LayoutJob::simple_singleline(
                                    title,
                                    FontId::proportional(13.0),
                                    ui.visuals().text_color(),
                                );
                                title_job.wrap.max_width = width;
                                title_job.wrap.max_rows = 1;
                                title_job.wrap.break_anywhere = true;
                                let title_galley = ui.painter().layout_job(title_job);
                                let visible_title: String = title_galley
                                    .rows
                                    .iter()
                                    .flat_map(|row| &row.glyphs)
                                    .map(|glyph| glyph.chr)
                                    .collect();
                                let mut job = egui::text::LayoutJob::default();
                                job.append(
                                    &visible_title,
                                    0.0,
                                    egui::TextFormat {
                                        font_id: FontId::proportional(13.0),
                                        color: ui.visuals().text_color(),
                                        ..Default::default()
                                    },
                                );
                                job.append(
                                    &format!("\n{path}"),
                                    0.0,
                                    egui::TextFormat {
                                        font_id: FontId::proportional(11.0),
                                        color: ui.visuals().weak_text_color(),
                                        ..Default::default()
                                    },
                                );
                                job.wrap.max_width = width;
                                job.wrap.max_rows = 2;
                                job.wrap.break_anywhere = true;
                                let button = egui::Button::new((
                                    icons::menu_symbol(
                                        ui,
                                        if trash { Icon::TrashBin } else { Icon::Folder },
                                        appearance,
                                    ),
                                    egui::WidgetText::Galley(ui.painter().layout_job(job))
                                        .atom_shrink(true),
                                ))
                                .truncate()
                                .min_size(Vec2::new(ui.available_width(), 42.0));
                                if ui.add(button).on_hover_text(&path).clicked() {
                                    chosen = Some(tab.id);
                                    ui.close();
                                }
                            });
                        }
                    });
            });
        });
        chosen
    }
}
