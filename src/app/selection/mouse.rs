use super::*;

pub(in crate::app) struct Gesture {
    origin: egui::Pos2,
    last: egui::Pos2,
    points: Vec<egui::Pos2>,
    active: bool,
    additive: bool,
    start: PathBuf,
    folder: PathBuf,
    query: String,
    view: View,
    previous: BTreeSet<PathBuf>,
    anchor: Option<PathBuf>,
    cursor: Option<PathBuf>,
}

impl Explorer {
    pub(in crate::app) fn prepare_selection_drag(&mut self, ctx: &egui::Context) {
        let Some(mut gesture) = self.pointer_selection.take() else {
            return;
        };
        if self.loading
            || self.has_dialog()
            || egui::Popup::is_any_open(ctx)
            || gesture.folder != self.current
            || gesture.query != self.query
            || gesture.view != self.view
            || ctx.input(|i| !i.focused || i.viewport().focused == Some(false))
        {
            return;
        }
        if ctx.input(|i| i.key_pressed(Key::Escape)) {
            self.selected = gesture.previous;
            self.selection_anchor = gesture.anchor;
            self.selection_cursor = gesture.cursor;
            ctx.request_repaint();
            return;
        }
        if !ctx.input(|i| {
            i.pointer.primary_down() || i.pointer.button_released(egui::PointerButton::Primary)
        }) {
            return;
        }
        // Keep every movement segment, including fast motion across several items.
        gesture.points.clear();
        gesture.points.push(gesture.last);
        ctx.input(|i| {
            for event in &i.events {
                if let egui::Event::PointerMoved(point) = event {
                    gesture.points.push(*point);
                }
            }
            if let Some(point) = i.pointer.interact_pos() {
                gesture.points.push(point);
            }
        });
        if !gesture.active
            && gesture
                .points
                .iter()
                .any(|point| point.distance(gesture.origin) >= 6.0)
        {
            gesture.active = true;
            if let Some(id) = ctx.memory(|memory| memory.focused()) {
                ctx.memory_mut(|memory| memory.surrender_focus(id));
            }
            self.type_ahead = Default::default();
            if !gesture.additive {
                self.selected.clear();
            }
            self.selected.insert(gesture.start.clone());
            self.selection_anchor = Some(gesture.start.clone());
            self.selection_cursor = Some(gesture.start.clone());
            self.reveal_selection = false;
        }
        self.pointer_selection = Some(gesture);
    }

    pub(in crate::app) fn selection_drag_response(
        &mut self,
        ui: &egui::Ui,
        response: &egui::Response,
        entry: &Entry,
    ) {
        if self.pointer_selection.is_none()
            && !self.selected.contains(&entry.path)
            && !self.loading
            && !self.has_dialog()
            && !self.drag_blocks_tabs()
            && !egui::Popup::is_any_open(ui.ctx())
            && response.is_pointer_button_down_on()
            && let Some((origin, modifiers)) = ui.input(|i| {
                i.events.iter().find_map(|event| match event {
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed: true,
                        modifiers,
                    } => Some((*pos, *modifiers)),
                    _ => None,
                })
            })
            && response.rect.intersect(ui.clip_rect()).contains(origin)
        {
            self.pointer_selection = Some(Gesture {
                origin,
                last: origin,
                points: vec![origin],
                active: false,
                additive: modifiers.ctrl || modifiers.command,
                start: entry.path.clone(),
                folder: self.current.clone(),
                query: self.query.clone(),
                view: self.view,
                previous: self.selected.clone(),
                anchor: self.selection_anchor.clone(),
                cursor: self.selection_cursor.clone(),
            });
        }
        let Some(gesture) = &self.pointer_selection else {
            return;
        };
        let rect = response.rect.intersect(ui.clip_rect());
        if gesture.active
            && rect.is_positive()
            && gesture
                .points
                .windows(2)
                .any(|points| intersects_segment(rect, points[0], points[1]))
        {
            if self.selected.insert(entry.path.clone()) {
                ui.ctx().request_repaint();
            }
            if gesture
                .points
                .last()
                .is_some_and(|point| rect.contains(*point))
            {
                self.selection_cursor = Some(entry.path.clone());
            }
        }
    }

    pub(in crate::app) fn finish_selection_drag(&mut self, ctx: &egui::Context) {
        if let Some(gesture) = &mut self.pointer_selection {
            if gesture.active
                && let Some(point) = ctx.pointer_interact_pos()
            {
                gesture.last = point;
            }
            if gesture.active {
                ctx.set_cursor_icon(egui::CursorIcon::Default);
            }
        }
        if !ctx.input(|i| i.pointer.primary_down()) {
            self.pointer_selection = None;
        }
    }

    pub(in crate::app) fn selecting_with_mouse(&self) -> bool {
        self.pointer_selection
            .as_ref()
            .is_some_and(|gesture| gesture.active)
    }
}

fn intersects_segment(rect: egui::Rect, from: egui::Pos2, to: egui::Pos2) -> bool {
    let delta = to - from;
    let (mut near, mut far) = (0.0_f32, 1.0_f32);
    for (start, direction, min, max) in [
        (from.x, delta.x, rect.left(), rect.right()),
        (from.y, delta.y, rect.top(), rect.bottom()),
    ] {
        if direction.abs() < f32::EPSILON {
            if start < min || start > max {
                return false;
            }
        } else {
            let (a, b) = ((min - start) / direction, (max - start) / direction);
            near = near.max(a.min(b));
            far = far.min(a.max(b));
            if near > far {
                return false;
            }
        }
    }
    true
}
