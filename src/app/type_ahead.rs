//! Select loaded items by name; never turn typing into a recursive search or open.
use super::*;

const TIMEOUT: f64 = 1.0;
const MAX_CHARACTERS: usize = 255;

#[derive(Default)]
pub(super) struct State {
    prefix: String,
    last_input: Option<f64>,
}
impl State {
    fn active(&self, now: f64) -> bool {
        !self.prefix.is_empty()
            && self
                .last_input
                .is_some_and(|last| (0.0..=TIMEOUT).contains(&(now - last)))
    }
    fn input(&mut self, text: &str, now: f64) -> Option<bool> {
        if !self.active(now) {
            *self = Self::default();
        }
        let text: String = text
            .chars()
            .filter(|c| !c.is_control())
            .take(MAX_CHARACTERS)
            .flat_map(char::to_lowercase)
            .collect();
        if text.is_empty() || (self.prefix.is_empty() && text.trim().is_empty()) {
            return None;
        }
        let cycle = self.prefix.chars().count() == 1 && text == self.prefix;
        if !cycle {
            let room = MAX_CHARACTERS.saturating_sub(self.prefix.chars().count());
            self.prefix.extend(text.chars().take(room));
        }
        self.last_input = Some(now);
        Some(cycle)
    }
}
impl Explorer {
    pub(super) fn type_ahead_shortcuts(&mut self, ctx: &egui::Context) {
        let blocked = ctx.input(|input| {
            let m = input.modifiers;
            m.ctrl
                || m.command
                || m.alt
                || m.mac_cmd
                || input.pointer.any_down()
                || input.viewport().focused == Some(false)
        });
        if self.loading || ctx.wants_keyboard_input() || self.drag_blocks_tabs() || blocked {
            self.type_ahead = State::default();
            return;
        }
        let reset = ctx.input(|input| {
            input.pointer.any_pressed()
                || input.events.iter().any(|event| {
                    matches!(
                        event,
                        egui::Event::Key {
                            key: Key::Escape
                                | Key::ArrowUp
                                | Key::ArrowDown
                                | Key::ArrowLeft
                                | Key::ArrowRight
                                | Key::PageUp
                                | Key::PageDown
                                | Key::Home
                                | Key::End,
                            pressed: true,
                            ..
                        }
                    )
                })
        });
        if reset {
            self.type_ahead = State::default();
        }
        let now = ctx.input(|input| input.time);
        let mut state = std::mem::take(&mut self.type_ahead);
        let texts = ctx.input(|input| {
            let preview_space = input.events.iter().any(|event| {
                matches!(
                    event,
                    egui::Event::Key {
                        key: Key::Space,
                        pressed: true,
                        ..
                    }
                )
            });
            input
                .events
                .iter()
                .filter_map(|event| {
                    if let egui::Event::Text(text) = event {
                        (!preview_space || !text.trim().is_empty()).then(|| text.clone())
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
        });
        let backspace = state.active(now)
            && ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, Key::Backspace));
        if texts.is_empty() && !backspace {
            self.type_ahead = state;
            return;
        }
        let visible = filesystem::visible_entries(
            &self.entries,
            &self.query,
            self.hidden_visible(),
            self.sort,
            self.descending,
        );
        if backspace {
            state.prefix.pop();
            state.last_input = Some(now);
            if !state.prefix.is_empty() {
                self.select_prefix(&state.prefix, false, &visible);
            }
        }
        let mut consumed = false;
        for text in texts {
            if let Some(cycle) = state.input(&text, now) {
                consumed = true;
                self.select_prefix(&state.prefix, cycle, &visible);
            }
        }
        if consumed {
            // Leave Space available for Quick Look, including immediately after
            // selecting a file by typing. Focused text fields keep all input.
            ctx.input_mut(|input| {
                input
                    .events
                    .retain(|event| !matches!(event, egui::Event::Text(_)));
            });
        }
        self.type_ahead = state;
    }
    fn select_prefix(&mut self, prefix: &str, cycle: bool, visible: &[usize]) {
        let current = self
            .selection_cursor
            .as_ref()
            .and_then(|path| visible.iter().position(|&i| &self.entries[i].path == path));
        let start = if cycle {
            current.map_or(0, |index| index + 1)
        } else {
            0
        };
        let found = (start..visible.len())
            .chain(0..start.min(visible.len()))
            .find_map(|position| {
                let entry = &self.entries[visible[position]];
                entry
                    .name
                    .to_lowercase()
                    .starts_with(prefix)
                    .then(|| entry.path.clone())
            });
        if let Some(path) = found {
            self.select_entry(&path, visible, egui::Modifiers::NONE);
            self.reveal_selection = true;
        }
    }
}

#[cfg(test)]
mod tests;
