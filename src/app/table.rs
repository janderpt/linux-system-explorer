use super::*;

impl Explorer {
    pub(super) fn column_headers(
        &mut self,
        ui: &mut egui::Ui,
        columns: &[(Column, f32)],
    ) -> egui::Rect {
        let mut move_column = None;
        let header = ui
            .horizontal(|ui| {
                for (index, &(column, width)) in columns.iter().enumerate() {
                    let (rect, _) = ui.allocate_exact_size(
                        Vec2::new(width, self.chrome_row_height()),
                        Sense::hover(),
                    );
                    let mut body = rect;
                    body.max.x -= 5.0;
                    let response = ui.interact(
                        body,
                        ui.id().with(("column_header", column)),
                        Sense::click_and_drag(),
                    );
                    if response.hovered() || response.dragged() {
                        ui.painter()
                            .rect_filled(rect, 3.0, ui.visuals().widgets.hovered.bg_fill);
                    }
                    let mut text_clip = body;
                    text_clip.max.x -= if self.sort == column { 25.0 } else { 4.0 };
                    ui.painter().with_clip_rect(text_clip).text(
                        rect.left_center() + Vec2::new(5.0, 0.0),
                        egui::Align2::LEFT_CENTER,
                        column.label(),
                        FontId::proportional(13.0),
                        ui.visuals().text_color(),
                    );
                    if self.sort == column {
                        paint_arrow(
                            ui.painter(),
                            rect.right_center() - Vec2::new(16.0, 0.0),
                            if self.descending {
                                Arrow::Down
                            } else {
                                Arrow::Up
                            },
                            ui.visuals().text_color(),
                        );
                    }
                    if response.clicked() {
                        if self.sort == column {
                            self.descending = !self.descending;
                        } else {
                            self.sort = column;
                            self.descending = false;
                        }
                    }
                    response.dnd_set_drag_payload(column);
                    if response.dragged() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
                        if let Some(pointer) = ui.input(|i| i.pointer.interact_pos()) {
                            let painter = ui.ctx().layer_painter(egui::LayerId::new(
                                egui::Order::Tooltip,
                                egui::Id::new("dragged_column"),
                            ));
                            painter.text(
                                pointer + Vec2::new(14.0, 16.0),
                                egui::Align2::LEFT_TOP,
                                column.label(),
                                FontId::proportional(13.0),
                                ui.visuals().text_color(),
                            );
                        }
                    }
                    if let Some(source) = response.dnd_hover_payload::<Column>() {
                        let right = ui.input(|i| {
                            i.pointer
                                .interact_pos()
                                .is_some_and(|p| p.x > rect.center().x)
                        });
                        if *source != column {
                            let x = if right { rect.right() } else { rect.left() };
                            ui.painter().line_segment(
                                [egui::pos2(x, rect.top()), egui::pos2(x, rect.bottom())],
                                (3.0, ui.visuals().selection.stroke.color),
                            );
                        }
                        if let Some(source) = response.dnd_release_payload::<Column>() {
                            move_column = Some((*source, index + usize::from(right)));
                        }
                    }
                    response.on_hover_text(crate::i18n::message(
                        "{}\nClique para ordenar. Arraste para mover a coluna.",
                        &[(column.description()).to_string()],
                    ));

                    let edge = egui::Rect::from_center_size(
                        rect.right_center(),
                        Vec2::new(8.0, rect.height()),
                    );
                    let resize = ui
                        .interact(edge, ui.id().with(("column_resize", column)), Sense::drag())
                        .on_hover_cursor(egui::CursorIcon::ResizeHorizontal);
                    let color = if resize.hovered() || resize.dragged() {
                        ui.visuals().selection.stroke.color
                    } else {
                        ui.visuals().widgets.noninteractive.bg_stroke.color
                    };
                    ui.painter().line_segment(
                        [
                            rect.right_top() + Vec2::new(0.0, 5.0),
                            rect.right_bottom() - Vec2::new(0.0, 5.0),
                        ],
                        (1.0, color),
                    );
                    if resize.drag_started() {
                        // Freeze the current automatic layout so adjacent columns do not move during a resize.
                        for &(column, width) in columns {
                            self.column_widths.entry(column).or_insert(width);
                        }
                    }
                    if (resize.dragged() || resize.drag_stopped())
                        && let Some(pointer) = ui.input(|i| i.pointer.interact_pos())
                    {
                        self.column_widths
                            .insert(column, (pointer.x - rect.left()).clamp(80.0, 1200.0));
                    }
                    resize.on_hover_text(crate::i18n::tr("Arraste para redimensionar a coluna"));
                }
            })
            .response
            .rect;
        if let Some((source, mut insertion)) = move_column
            && let Some(from) = self
                .visible_columns
                .iter()
                .position(|&column| column == source)
        {
            self.visible_columns.remove(from);
            if from < insertion {
                insertion -= 1;
            }
            self.visible_columns
                .insert(insertion.min(self.visible_columns.len()), source);
        }
        header
    }

    pub(super) fn list(&mut self, ui: &mut egui::Ui, visible: &[usize]) {
        let scale = self.item_scale;
        let row_height = 26.0 * scale;
        ui.scope(|ui| {
            ui.spacing_mut().item_spacing.y = 2.0 * scale;
            let rows = egui::ScrollArea::vertical()
                .id_salt((self.tab_id, &self.current, "list"))
                .auto_shrink([false, false])
                .show_rows(ui, row_height, visible.len(), |ui, range| {
                    self.reveal_selected_row(ui, visible, range.start, row_height, 1);
                    for row in range {
                        let entry = self.entries[visible[row]].clone();
                        let (rect, response) = ui.allocate_exact_size(
                            Vec2::new(ui.available_width(), row_height),
                            Sense::click_and_drag(),
                        );
                        let selected = self.selected.contains(&entry.path);
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
                        let icon = egui::Rect::from_center_size(
                            rect.left_center() + Vec2::new(14.0 * scale, 0.0),
                            Vec2::splat(18.0 * scale),
                        );
                        self.paint_file_icon(ui.painter(), icon, &entry);
                        let mut text_rect = rect;
                        text_rect.min.x += 32.0 * scale;
                        ui.painter().with_clip_rect(text_rect).text(
                            text_rect.left_center(),
                            egui::Align2::LEFT_CENTER,
                            &entry.name,
                            FontId::proportional(13.0 * scale),
                            if selected {
                                ui.visuals().selection.stroke.color
                            } else {
                                ui.visuals().text_color()
                            },
                        );
                        self.entry_response(ui, response, &entry, visible);
                    }
                });
            self.folder_background_area = Some(rows.inner_rect.intersect(ui.clip_rect()));
        });
    }
}
