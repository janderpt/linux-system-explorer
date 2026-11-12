use super::*;

/// A separate payload type keeps tab rearrangement independent of file/column drags.
#[derive(Clone, Copy)]
struct TabDrag(u64);

#[derive(Clone, Copy)]
pub(super) enum TabAction {
    Duplicate,
    CloseOthers,
    CloseRight,
    Reopen,
    ReopenRecent(u64),
}

const HEIGHT: f32 = 30.0;
const GAP: f32 = 0.0;

impl Workspace {
    pub(super) fn tab_action(&mut self, id: u64, action: TabAction, ctx: &egui::Context) {
        let Some(index) = self.tabs.iter().position(|tab| tab.tab_id == id) else {
            return;
        };
        match action {
            TabAction::Duplicate => {
                self.capture(ctx);
                self.append_tab(self.tabs[index].location().to_owned(), ctx);
                let last = self.tabs.len() - 1;
                let (originals, duplicate) = self.tabs.split_at_mut(last);
                let source = &originals[index];
                let copy = &mut duplicate[0];
                // Preserve the browsing context while the new tab refreshes its own listing.
                // Transfers and terminal processes continue to belong only to the original.
                if source.location() == source.current && !source.current.as_os_str().is_empty() {
                    copy.current = source.current.clone();
                    copy.entries = source.entries.clone();
                    copy.skipped = source.skipped;
                    copy.history = source.history.clone();
                    copy.query = source.query.clone();
                    copy.selected = source.selected.clone();
                    copy.selection_anchor = source.selection_anchor.clone();
                    copy.selection_cursor = source.selection_cursor.clone();
                    copy.reveal_selection = !copy.selected.is_empty();
                    copy.reveal_hidden = source.reveal_hidden;
                    copy.apply_layout(source.current_layout());
                }
                let duplicate_id = copy.tab_id;
                self.reorder_tab(duplicate_id, index + 1);
                self.activate(index + 1, ctx);
            }
            TabAction::CloseOthers | TabAction::CloseRight => {
                let closing: BTreeSet<_> = self
                    .tabs
                    .iter()
                    .enumerate()
                    .filter(|(i, tab)| {
                        *i != index
                            && !tab.busy()
                            && (matches!(action, TabAction::CloseOthers) || *i > index)
                    })
                    .map(|(_, tab)| tab.tab_id)
                    .collect();
                for i in 0..self.tabs.len() {
                    if closing.contains(&self.tabs[i].tab_id) {
                        self.remember_closed(i);
                    }
                }
                if closing.contains(&self.tabs[self.active].tab_id) {
                    self.activate(index, ctx);
                }
                let active_id = self.tabs[self.active].tab_id;
                self.tabs.retain(|tab| !closing.contains(&tab.tab_id));
                self.active = self
                    .tabs
                    .iter()
                    .position(|tab| tab.tab_id == active_id)
                    .unwrap();
            }
            TabAction::Reopen => self.reopen_closed_tab(ctx),
            TabAction::ReopenRecent(id) => self.reopen_closed_tab_id(id, ctx),
        }
        ctx.request_repaint();
    }

    pub(super) fn reorder_tab(&mut self, id: u64, insertion: usize) {
        let Some(from) = self.tabs.iter().position(|tab| tab.tab_id == id) else {
            return;
        };
        let active_id = self.tabs[self.active].tab_id;
        let tab = self.tabs.remove(from);
        let insertion = insertion.saturating_sub(usize::from(from < insertion));
        self.tabs.insert(insertion.min(self.tabs.len()), tab);
        self.active = self
            .tabs
            .iter()
            .position(|tab| tab.tab_id == active_id)
            .unwrap();
    }

    pub(super) fn tab_bar(&mut self, ctx: &egui::Context) {
        let mut select = None;
        let mut close = None;
        let mut add = false;
        let mut reopen = false;
        let mut reorder = None;
        let mut menu_action = None;
        let mut menu_open = false;
        let locked = self.closing
            || self.tabs[self.active].drag_blocks_tabs()
            || self.tabs[self.active].has_dialog();
        let dragging = egui::DragAndDrop::has_payload_of_type::<TabDrag>(ctx);
        let visible_id = egui::Id::new("visible_explorer_tab");
        let active_id = self.tabs[self.active].tab_id;
        let reveal_active = ctx.data_mut(|data| {
            let previous = data.get_temp::<u64>(visible_id);
            data.insert_temp(visible_id, active_id);
            previous != Some(active_id)
        });
        egui::TopBottomPanel::top("explorer_tabs")
            .frame(
                egui::Frame::new()
                    .fill(ctx.style().visuals.panel_fill)
                    .inner_margin(egui::Margin {
                        left: 8,
                        right: 8,
                        top: 3,
                        bottom: 0,
                    }),
            )
            .show(ctx, |ui| {
                let width = ((ui.available_width() - 40.0) / self.tabs.len() as f32 - GAP)
                    .clamp(150.0, 220.0);
                let mut scroll = egui::ScrollArea::horizontal()
                    .id_salt("tab_strip")
                    .auto_shrink([false, true])
                    .scroll_source(egui::scroll_area::ScrollSource {
                        drag: false,
                        ..Default::default()
                    })
                    .show(ui, |ui| {
                        let mut rects = Vec::with_capacity(self.tabs.len());
                        ui.spacing_mut().item_spacing.x = GAP;
                        ui.horizontal(|ui| {
                            for (index, tab) in self.tabs.iter().enumerate() {
                                let (rect, response, x) =
                                    tab_widget(ui, tab, index == self.active, width, !locked);
                                rects.push(rect);
                                if reveal_active && index == self.active && !dragging {
                                    ui.scroll_to_rect(rect, Some(Align::Center));
                                }
                                if response.clicked() {
                                    select = Some(index);
                                }
                                if x.clicked() {
                                    close = Some(index);
                                }
                                if !locked && !dragging {
                                    response.context_menu(|ui| {
                                        for (action, icon, enabled) in [
                                            (TabAction::Duplicate, Icon::DuplicateTab, true),
                                            (
                                                TabAction::CloseOthers,
                                                Icon::CloseOtherTabs,
                                                self.tabs
                                                    .iter()
                                                    .enumerate()
                                                    .any(|(i, t)| i != index && !t.busy()),
                                            ),
                                            (
                                                TabAction::CloseRight,
                                                Icon::CloseRightTabs,
                                                self.tabs[index + 1..].iter().any(|t| !t.busy()),
                                            ),
                                        ] {
                                            if icons::menu_item(
                                                ui,
                                                icon,
                                                icon.label(),
                                                "",
                                                enabled,
                                                false,
                                                &self.tabs[self.active].appearance,
                                            )
                                            .clicked()
                                            {
                                                menu_action = Some((tab.tab_id, action));
                                                ui.close();
                                            }
                                        }
                                        ui.separator();
                                        if icons::menu_item(
                                            ui,
                                            Icon::ReopenTab,
                                            crate::i18n::tr("Reabrir aba fechada"),
                                            "Ctrl+Shift+T",
                                            !self.closed_tabs.is_empty(),
                                            false,
                                            &self.tabs[self.active].appearance,
                                        )
                                        .clicked()
                                        {
                                            menu_action = Some((tab.tab_id, TabAction::Reopen));
                                            ui.close();
                                        }
                                        if let Some(id) = self.recently_closed_menu(ui) {
                                            menu_action =
                                                Some((tab.tab_id, TabAction::ReopenRecent(id)));
                                        }
                                    });
                                    menu_open |= response.context_menu_opened();
                                }
                                if !locked && response.drag_started_by(egui::PointerButton::Primary)
                                {
                                    egui::DragAndDrop::set_payload(ctx, TabDrag(tab.tab_id));
                                }
                            }
                            let plus = new_tab_button(
                                ui,
                                !locked && !dragging,
                                &self.tabs[self.active].appearance,
                            );
                            add = plus.clicked();
                            if !locked && !dragging {
                                plus.context_menu(|ui| {
                                    if icons::menu_item(
                                        ui,
                                        Icon::ReopenTab,
                                        crate::i18n::tr("Reabrir aba fechada"),
                                        "Ctrl+Shift+T",
                                        !self.closed_tabs.is_empty(),
                                        false,
                                        &self.tabs[self.active].appearance,
                                    )
                                    .clicked()
                                    {
                                        reopen = true;
                                        ui.close();
                                    }
                                    if let Some(id) = self.recently_closed_menu(ui) {
                                        menu_action = Some((
                                            self.tabs[self.active].tab_id,
                                            TabAction::ReopenRecent(id),
                                        ));
                                    }
                                });
                                menu_open |= plus.context_menu_opened();
                            }
                        });
                        if !locked
                            && let Some(payload) = egui::DragAndDrop::payload::<TabDrag>(ctx)
                            && let Some(pointer) = ui.input(|i| i.pointer.interact_pos())
                            && ui.clip_rect().contains(pointer)
                        {
                            let insertion = rects
                                .iter()
                                .position(|r| pointer.x < r.center().x)
                                .unwrap_or(rects.len());
                            let source = self.tabs.iter().position(|tab| tab.tab_id == payload.0);
                            if source.is_some_and(|from| insertion != from && insertion != from + 1)
                            {
                                let x = if insertion == 0 {
                                    rects[0].left()
                                } else {
                                    rects[insertion - 1].right() + GAP / 2.0
                                };
                                let color = ui.visuals().selection.stroke.color;
                                ui.painter().line_segment(
                                    [
                                        egui::pos2(x, rects[0].top() + 3.0),
                                        egui::pos2(x, rects[0].bottom() - 3.0),
                                    ],
                                    (3.0, color),
                                );
                            }
                            if ui.input(|i| i.pointer.primary_released()) {
                                reorder = Some((payload.0, insertion));
                                egui::DragAndDrop::clear_payload(ctx);
                            }
                        }
                    });
                // Keep dragging useful when the strip contains more tabs than fit on screen.
                if egui::DragAndDrop::has_payload_of_type::<TabDrag>(ctx)
                    && let Some(pointer) = ui.input(|i| i.pointer.interact_pos())
                    && scroll.inner_rect.contains(pointer)
                {
                    let direction = if pointer.x < scroll.inner_rect.left() + 28.0 {
                        -1.0
                    } else if pointer.x > scroll.inner_rect.right() - 28.0 {
                        1.0
                    } else {
                        0.0
                    };
                    let limit = (scroll.content_size.x - scroll.inner_rect.width()).max(0.0);
                    let step = ui.input(|i| i.stable_dt).min(0.1) * 500.0;
                    scroll.state.offset.x =
                        (scroll.state.offset.x + direction * step).clamp(0.0, limit);
                    scroll.state.store(ctx, scroll.id);
                    if direction != 0.0 {
                        ctx.request_repaint();
                    }
                }
            });
        if let Some(payload) = egui::DragAndDrop::payload::<TabDrag>(ctx)
            && let Some(tab) = self.tabs.iter().find(|tab| tab.tab_id == payload.0)
            && let Some(pointer) = ctx.input(|i| i.pointer.interact_pos())
        {
            let painter = ctx.layer_painter(egui::LayerId::new(
                egui::Order::Tooltip,
                egui::Id::new("dragged_explorer_tab"),
            ));
            let visuals = ctx.style().visuals.clone();
            let mut job = egui::text::LayoutJob::simple_singleline(
                tab_title(tab),
                FontId::proportional(13.0),
                visuals.text_color(),
            );
            job.wrap.max_width = 190.0;
            job.wrap.max_rows = 1;
            job.wrap.break_anywhere = true;
            let text = painter.layout_job(job);
            let rect = egui::Rect::from_min_size(
                pointer + Vec2::new(12.0, 18.0),
                text.size() + Vec2::new(24.0, 16.0),
            );
            painter.rect(
                rect,
                6.0,
                visuals.panel_fill,
                (1.0, visuals.selection.stroke.color),
                egui::StrokeKind::Inside,
            );
            painter.galley(rect.min + Vec2::new(12.0, 8.0), text, visuals.text_color());
        }
        if !locked
            && !menu_open
            && !egui::DragAndDrop::has_payload_of_type::<TabDrag>(ctx)
            && !dragging
        {
            let terminal = self.tabs[self.active].terminal_focused(ctx);
            if ctx.input_mut(|i| {
                i.consume_key(egui::Modifiers::CTRL | egui::Modifiers::SHIFT, Key::T)
            }) {
                reopen = true;
            } else if !terminal && ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, Key::T)) {
                add = true;
            }
            if !terminal && ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, Key::W)) {
                close = Some(self.active);
            }
            let backward = ctx.input_mut(|i| {
                i.consume_key(egui::Modifiers::CTRL | egui::Modifiers::SHIFT, Key::Tab)
            });
            let forward = ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, Key::Tab));
            if backward || forward {
                select = Some(if backward {
                    (self.active + self.tabs.len() - 1) % self.tabs.len()
                } else {
                    (self.active + 1) % self.tabs.len()
                });
            }
        }
        if let Some((id, action)) = menu_action {
            self.tab_action(id, action, ctx);
            return;
        }
        if reopen {
            self.reopen_closed_tab(ctx);
            return;
        }
        if let Some(index) = select {
            self.activate(index, ctx);
        }
        if let Some(index) = close {
            self.close_tab(index, ctx);
        }
        if add {
            self.add_tab(dirs::home_dir().unwrap_or_else(|| "/".into()), ctx);
        }
        if let Some((id, insertion)) = reorder {
            self.reorder_tab(id, insertion);
            ctx.request_repaint();
        }
    }
}

fn tab_widget(
    ui: &mut egui::Ui,
    tab: &Explorer,
    active: bool,
    width: f32,
    enabled: bool,
) -> (egui::Rect, egui::Response, egui::Response) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, HEIGHT), Sense::hover());
    let x_rect = egui::Rect::from_center_size(
        rect.right_center() - Vec2::new(17.0, 0.0),
        Vec2::splat(24.0),
    );
    let mut body = rect;
    body.max.x = x_rect.left() - 2.0;
    let response = ui.interact(
        body,
        ui.id().with(("tab", tab.tab_id)),
        if enabled {
            Sense::click_and_drag()
        } else {
            Sense::hover()
        },
    );
    let x = ui.interact(
        x_rect,
        ui.id().with(("close_tab", tab.tab_id)),
        if enabled && !tab.busy() {
            Sense::click()
        } else {
            Sense::hover()
        },
    );
    let visuals = ui.visuals();
    let fill = if active {
        visuals.panel_fill.lerp_to_gamma(
            crate::system_style::accent_color(visuals.selection.stroke.color),
            if visuals.dark_mode { 0.18 } else { 0.16 },
        )
    } else if response.hovered() {
        visuals.widgets.hovered.bg_fill
    } else {
        visuals.widgets.inactive.bg_fill
    };
    let border = visuals.widgets.noninteractive.bg_stroke.color;
    ui.painter().rect(
        rect,
        egui::CornerRadius {
            nw: 7,
            ne: 7,
            sw: 0,
            se: 0,
        },
        fill,
        (1.0, border),
        egui::StrokeKind::Inside,
    );
    let color = if enabled {
        visuals.text_color()
    } else {
        visuals.weak_text_color()
    };
    icons::paint(
        ui.painter(),
        egui::Rect::from_center_size(rect.left_center() + Vec2::new(19.0, 0.0), Vec2::splat(16.0)),
        if crate::applications::menu::is_location(tab.location()) {
            Icon::Applications
        } else if crate::trashbin::is_location(tab.location()) {
            Icon::TrashBin
        } else {
            Icon::Folder
        },
        color,
        &tab.appearance,
    );
    let title = tab_title(tab);
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, enabled, active, &title)
    });
    x.widget_info(|| {
        egui::WidgetInfo::labeled(
            egui::WidgetType::Button,
            enabled && !tab.busy(),
            crate::i18n::tr("Fechar aba · Ctrl+W"),
        )
    });
    let mut job =
        egui::text::LayoutJob::simple_singleline(title, FontId::proportional(13.0), color);
    job.wrap.max_width = (body.width() - 40.0).max(1.0);
    job.wrap.max_rows = 1;
    job.wrap.break_anywhere = true;
    let galley = ui.painter().layout_job(job);
    ui.painter().galley(
        rect.left_center() + Vec2::new(34.0, -galley.size().y / 2.0),
        galley,
        color,
    );
    if x.hovered() && enabled && !tab.busy() {
        ui.painter()
            .rect_filled(x_rect, 4.0, visuals.widgets.hovered.bg_fill);
    }
    let cross = x_rect.shrink(8.0);
    let x_color = if enabled && !tab.busy() {
        color
    } else {
        visuals.weak_text_color()
    };
    ui.painter()
        .line_segment([cross.left_top(), cross.right_bottom()], (1.5, x_color));
    ui.painter()
        .line_segment([cross.right_top(), cross.left_bottom()], (1.5, x_color));
    (
        rect,
        response.on_hover_text(crate::i18n::message(
            "{}\nArraste para reorganizar a aba.",
            &[tab.display_address(tab.location())],
        )),
        x.on_hover_text(crate::i18n::tr("Fechar aba · Ctrl+W")),
    )
}

fn new_tab_button(
    ui: &mut egui::Ui,
    enabled: bool,
    appearance: &icons::Appearance,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(
        Vec2::new(36.0, HEIGHT),
        if enabled {
            Sense::click()
        } else {
            Sense::hover()
        },
    );
    let visuals = ui.visuals();
    ui.painter().rect(
        rect,
        egui::CornerRadius {
            nw: 7,
            ne: 7,
            sw: 0,
            se: 0,
        },
        if response.hovered() && enabled {
            visuals.widgets.hovered.bg_fill
        } else {
            visuals.widgets.inactive.bg_fill
        },
        (1.0, visuals.widgets.noninteractive.bg_stroke.color),
        egui::StrokeKind::Inside,
    );
    icons::paint(
        ui.painter(),
        egui::Rect::from_center_size(rect.center(), Vec2::splat(16.0)),
        Icon::NewTab,
        if enabled {
            visuals.text_color()
        } else {
            visuals.weak_text_color()
        },
        appearance,
    );
    response.widget_info(|| {
        egui::WidgetInfo::labeled(
            egui::WidgetType::Button,
            enabled,
            crate::i18n::tr("Nova aba · Ctrl+T"),
        )
    });
    response.on_hover_text(crate::i18n::tr("Nova aba · Ctrl+T"))
}

fn tab_title(tab: &Explorer) -> String {
    let mut title = tab.display_title(tab.location());
    if tab.busy() {
        title.push_str(" ·");
    }
    title
}
