use super::*;

pub(super) struct SplitView {
    left: u64,
    right: u64,
    ratio: f32,
    rects: Option<[egui::Rect; 2]>,
}

impl Workspace {
    pub(super) fn split_state(&self) -> Option<crate::session::Split> {
        let [left, right] = self.split_indices()?;
        Some(crate::session::Split {
            left,
            right,
            ratio: self.split.as_ref()?.ratio,
        })
    }

    pub(super) fn restore_split(&mut self, state: &crate::session::State) {
        self.split = state.normalized_split().and_then(|split| {
            Some(SplitView {
                left: self.tabs.get(split.left)?.tab_id,
                right: self.tabs.get(split.right)?.tab_id,
                ratio: split.ratio,
                rects: None,
            })
        });
        for tab in &mut self.tabs {
            tab.split_view = self.split.is_some();
        }
    }

    pub(super) fn toggle_split(&mut self, ctx: &egui::Context) {
        if self.tabs[self.active].has_dialog() || self.tabs[self.active].drag_blocks_tabs() {
            return;
        }
        if self.split.take().is_none() {
            self.capture(ctx);
            let source = self.active;
            let path = self.tabs[source].location().to_owned();
            self.append_tab(path, ctx);
            let right = self.tabs.len() - 1;
            let (left, copy) = two_tabs(&mut self.tabs, source, right);
            copy.query = left.query.clone();
            copy.apply_layout(left.current_layout());
            if left.location() == left.current {
                copy.current = left.current.clone();
                copy.entries = left.directory_entries().to_vec();
                copy.history = left.history.clone();
            }
            self.split = Some(SplitView {
                left: left.tab_id,
                right: copy.tab_id,
                ratio: 0.5,
                rects: None,
            });
        }
        for tab in &mut self.tabs {
            tab.split_view = self.split.is_some();
        }
        ctx.request_repaint();
    }

    fn split_indices(&self) -> Option<[usize; 2]> {
        let split = self.split.as_ref()?;
        Some([
            self.tabs.iter().position(|t| t.tab_id == split.left)?,
            self.tabs.iter().position(|t| t.tab_id == split.right)?,
        ])
    }

    pub(super) fn normalize_split(&mut self) {
        if self.split.is_some() && self.split_indices().is_none() {
            self.split = None;
        }
    }

    pub(super) fn assign_split_tab(&mut self, index: usize) {
        let current = self.tabs[self.active].tab_id;
        let next = self.tabs[index].tab_id;
        if let Some(split) = &mut self.split
            && next != split.left
            && next != split.right
        {
            if current == split.left {
                split.left = next;
            } else {
                split.right = next;
            }
        }
    }

    pub(super) fn focus_split(&mut self, ctx: &egui::Context) {
        if self.tabs[self.active].has_dialog() || self.tabs[self.active].drag_blocks_tabs() {
            return;
        }
        let Some(indices) = self.split_indices() else {
            return;
        };
        let position = ctx.input(|i| {
            i.pointer
                .any_pressed()
                .then(|| i.pointer.interact_pos())
                .flatten()
        });
        if let Some(position) = position
            && let Some(rects) = self.split.as_ref().and_then(|s| s.rects)
            && let Some(side) = rects.iter().position(|rect| rect.contains(position))
        {
            self.activate(indices[side], ctx);
        }
    }

    pub(super) fn show_split(&mut self, ctx: &egui::Context) {
        let [left, right] = self.split_indices().expect("validated split");
        let active = self.active;
        let other = if active == left { right } else { left };
        self.tabs[active].prepare_view(ctx);
        self.tabs[active].shortcuts(ctx);
        self.tabs[active].top_toolbar(ctx);
        let preferences = self.tabs[active].preferences();
        let tab = &mut self.tabs[other];
        if tab.sidebar_options != preferences.sidebar
            || tab.appearance != preferences.appearance
            || tab.dark != preferences.dark
            || tab.show_hidden != preferences.show_hidden
            || tab.language != preferences.language
            || tab.toast_options != preferences.toast
        {
            tab.apply_global_preferences(&preferences, ctx);
        }
        let left_visible = self.tabs[left].visible_for_display();
        let right_visible = self.tabs[right].visible_for_display();
        self.tabs[active].status_bar(
            ctx,
            if active == left {
                &left_visible
            } else {
                &right_visible
            },
        );
        self.tabs[active].terminal_panel(ctx);
        {
            let (source, other) = two_tabs(&mut self.tabs, active, other);
            other.mirror_drag_feedback(source);
        }
        let mut ratio = self.split.as_ref().unwrap().ratio;
        let mut pane_rects = None;
        egui::CentralPanel::default()
            .frame(egui::Frame::central_panel(&ctx.style()).inner_margin(0))
            .show(ctx, |ui| {
                let rect = ui.available_rect_before_wrap();
                let width = (rect.width() - 8.0).max(1.0);
                let minimum = (320.0 / width).min(0.45);
                ratio = ratio.clamp(minimum, 1.0 - minimum);
                let divider = egui::Rect::from_min_max(
                    egui::pos2(rect.left() + width * ratio, rect.top()),
                    egui::pos2(rect.left() + width * ratio + 8.0, rect.bottom()),
                );
                let response = ui
                    .interact(divider, ui.id().with("pane_divider"), Sense::drag())
                    .on_hover_cursor(egui::CursorIcon::ResizeHorizontal);
                if response.dragged()
                    && let Some(pointer) = response.interact_pointer_pos()
                {
                    ratio = ((pointer.x - rect.left() - 4.0) / width).clamp(minimum, 1.0 - minimum);
                    ctx.request_repaint();
                }
                let divider_x = rect.left() + width * ratio;
                let panes = [
                    egui::Rect::from_min_max(rect.min, egui::pos2(divider_x, rect.bottom())),
                    egui::Rect::from_min_max(egui::pos2(divider_x + 8.0, rect.top()), rect.max),
                ];
                pane_rects = Some(panes);
                ui.painter().line_segment(
                    [
                        egui::pos2(divider_x + 4.0, rect.top()),
                        egui::pos2(divider_x + 4.0, rect.bottom()),
                    ],
                    (
                        1.0,
                        if response.hovered() || response.dragged() {
                            ui.visuals().selection.stroke.color
                        } else {
                            ui.visuals().widgets.noninteractive.bg_stroke.color
                        },
                    ),
                );
                for (index, pane, visible) in [
                    (left, panes[0], &left_visible),
                    (right, panes[1], &right_visible),
                ] {
                    ui.scope_builder(
                        egui::UiBuilder::new()
                            .max_rect(pane)
                            .id_salt(("folder_pane", self.tabs[index].tab_id))
                            .layout(Layout::top_down(Align::Min)),
                        |ui| {
                            ui.set_clip_rect(pane.intersect(ui.clip_rect()));
                            self.tabs[index].sidebar_inside(ui);
                            egui::CentralPanel::default()
                                .frame(egui::Frame::NONE.inner_margin(egui::Margin {
                                    left: 6,
                                    right: 6,
                                    top: 0,
                                    bottom: 0,
                                }))
                                .show_inside(ui, |ui| self.tabs[index].contents_ui(ui, visible));
                        },
                    );
                    if index == active {
                        ui.painter().line_segment(
                            [pane.left_top(), pane.right_top()],
                            (
                                2.0,
                                ui.visuals().selection.stroke.color.gamma_multiply(0.65),
                            ),
                        );
                    }
                }
            });
        if let Some(split) = &mut self.split {
            split.ratio = ratio;
            split.rects = pane_rects;
        }
        {
            let (source, other) = two_tabs(&mut self.tabs, active, other);
            source.merge_drop_targets(other);
        }
        self.tabs[active].finish_drag_drop(ctx);
        self.tabs[active].view_overlays(ctx);
        self.tabs[active].persist_preferences(ctx);
    }
}

fn two_tabs(tabs: &mut [Explorer], first: usize, second: usize) -> (&mut Explorer, &mut Explorer) {
    assert_ne!(first, second);
    if first < second {
        let (before, after) = tabs.split_at_mut(second);
        (&mut before[first], &mut after[0])
    } else {
        let (before, after) = tabs.split_at_mut(first);
        (&mut after[0], &mut before[second])
    }
}

#[cfg(test)]
mod tests;
