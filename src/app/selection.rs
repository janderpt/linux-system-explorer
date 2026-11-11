use super::*;

impl Explorer {
    /// Applied only after the asynchronous initial directory read has completed.
    pub fn reveal_external(&mut self, paths: Vec<PathBuf>, properties: bool) {
        if !paths.is_empty() {
            self.pending_external = Some((paths, properties));
        }
    }

    pub(super) fn hidden_visible(&self) -> bool {
        self.show_hidden || self.reveal_hidden
    }

    pub(super) fn toggle_hidden(&mut self) {
        self.show_hidden = !self.hidden_visible();
        self.reveal_hidden = false;
    }

    pub(super) fn complete_external_reveal(&mut self) {
        let Some((paths, properties)) = self.pending_external.take() else {
            return;
        };
        self.query.clear();
        let requested: BTreeSet<_> = paths.iter().cloned().collect();
        self.selected = self
            .entries
            .iter()
            .filter(|entry| requested.contains(&entry.path))
            .map(|entry| entry.path.clone())
            .collect();
        let cursor = paths
            .iter()
            .find(|path| self.selected.contains(*path))
            .cloned();
        self.selection_anchor = cursor.clone();
        self.selection_cursor = cursor;
        self.reveal_selection = self.selection_cursor.is_some();
        self.reveal_hidden = self
            .entries
            .iter()
            .any(|entry| entry.hidden && self.selected.contains(&entry.path));
        if properties {
            self.properties = paths
                .first()
                .and_then(|path| filesystem::read_entry(path).ok());
        }
        let missing = paths
            .iter()
            .filter(|path| !self.selected.contains(*path) && *path != &self.current)
            .count();
        if missing > 0 {
            self.post_operation_notice(
                crate::i18n::plural(
                    "{} item solicitado não está disponível nesta pasta.",
                    "{} itens solicitados não estão disponíveis nesta pasta.",
                    missing,
                ),
                &[],
                true,
            );
        }
    }

    pub(super) fn select_entry(
        &mut self,
        path: &Path,
        visible: &[usize],
        modifiers: egui::Modifiers,
    ) {
        self.selection_cursor = Some(path.to_path_buf());
        self.reveal_selection = false;
        let additive = modifiers.ctrl || modifiers.command;
        if modifiers.shift {
            let anchor = self
                .selection_anchor
                .as_deref()
                .and_then(|path| visible.iter().position(|&i| self.entries[i].path == path));
            let target = visible.iter().position(|&i| self.entries[i].path == path);
            if let (Some(anchor), Some(target)) = (anchor, target) {
                if !additive {
                    self.selected.clear();
                }
                for &index in &visible[anchor.min(target)..=anchor.max(target)] {
                    self.selected.insert(self.entries[index].path.clone());
                }
                return;
            }
        }
        if additive {
            if !self.selected.remove(path) {
                self.selected.insert(path.to_path_buf());
            }
        } else {
            self.selected.clear();
            self.selected.insert(path.to_path_buf());
        }
        self.selection_anchor = Some(path.to_path_buf());
    }

    pub(super) fn clear_selection(&mut self) {
        self.selected.clear();
        self.selection_anchor = None;
        self.selection_cursor = None;
        self.reveal_selection = false;
    }

    pub(super) fn retain_selection(&mut self, available: &BTreeSet<PathBuf>) {
        self.selected.retain(|path| available.contains(path));
        if self
            .selection_anchor
            .as_ref()
            .is_some_and(|path| !available.contains(path))
        {
            self.selection_anchor = None;
        }
        if self
            .selection_cursor
            .as_ref()
            .is_some_and(|path| !available.contains(path))
        {
            self.selection_cursor = None;
            self.reveal_selection = false;
        }
    }

    pub(super) fn selection_shortcuts(&mut self, ctx: &egui::Context) {
        if self.loading || ctx.wants_keyboard_input() {
            return;
        }
        let keys: Vec<_> = ctx.input(|input| {
            input
                .events
                .iter()
                .filter_map(|event| match event {
                    egui::Event::Key {
                        key:
                            key @ (Key::ArrowUp
                            | Key::ArrowDown
                            | Key::PageUp
                            | Key::PageDown
                            | Key::Home
                            | Key::End),
                        pressed: true,
                        modifiers,
                        ..
                    } if (modifiers.shift || !matches!(key, Key::ArrowUp | Key::ArrowDown))
                        && !modifiers.ctrl
                        && !modifiers.command
                        && !modifiers.alt
                        && !modifiers.mac_cmd =>
                    {
                        Some((*key, *modifiers))
                    }
                    _ => None,
                })
                .collect()
        });
        if keys.is_empty() {
            return;
        }
        let visible = filesystem::visible_entries(
            &self.entries,
            &self.query,
            self.hidden_visible(),
            self.sort,
            self.descending,
        );
        let available = visible
            .iter()
            .map(|&i| self.entries[i].path.clone())
            .collect();
        self.retain_selection(&available);
        for (key, modifiers) in keys {
            ctx.input_mut(|input| {
                input.consume_key(modifiers, key);
            });
            if visible.is_empty() {
                continue;
            }
            let cursor = self
                .selection_cursor
                .as_deref()
                .or(self.selection_anchor.as_deref())
                .and_then(|path| visible.iter().position(|&i| self.entries[i].path == path));
            let next = match (cursor, key) {
                (_, Key::Home) => 0,
                (_, Key::End) => visible.len() - 1,
                (Some(index), Key::PageUp) => index.saturating_sub(self.navigation_page),
                (Some(index), Key::PageDown) => index
                    .saturating_add(self.navigation_page)
                    .min(visible.len() - 1),
                (None, Key::PageUp) => 0,
                (None, Key::PageDown) => self
                    .navigation_page
                    .saturating_sub(1)
                    .min(visible.len() - 1),
                (Some(index), Key::ArrowUp) => index.saturating_sub(1),
                (Some(index), _) => (index + 1).min(visible.len() - 1),
                (None, Key::ArrowUp) => visible.len() - 1,
                (None, _) => 0,
            };
            let path = self.entries[visible[next]].path.clone();
            self.select_entry(&path, &visible, modifiers);
            self.reveal_selection = true;
        }
    }

    pub(super) fn reveal_selected_row(
        &mut self,
        ui: &mut egui::Ui,
        visible: &[usize],
        first_row: usize,
        height: f32,
        columns: usize,
    ) {
        self.drag_hover_scroll(ui);
        let stride = height + ui.spacing().item_spacing.y;
        self.navigation_page =
            ((ui.clip_rect().height() / stride).floor() as usize).max(1) * columns;
        if !self.reveal_selection {
            return;
        }
        self.reveal_selection = false;
        let Some(index) = self
            .selection_cursor
            .as_deref()
            .and_then(|path| visible.iter().position(|&i| self.entries[i].path == path))
        else {
            return;
        };
        let top =
            ui.max_rect().top() - first_row as f32 * stride + (index / columns) as f32 * stride;
        let rect = egui::Rect::from_min_size(
            egui::pos2(ui.clip_rect().center().x, top),
            Vec2::new(1.0, height),
        );
        ui.scroll_to_rect(rect, None);
    }

    pub(super) fn selected_paths_text(&self) -> String {
        filesystem::visible_entries(
            &self.entries,
            &self.query,
            self.hidden_visible(),
            self.sort,
            self.descending,
        )
        .into_iter()
        .map(|index| &self.entries[index].path)
        .filter(|path| self.selected.contains(*path))
        .map(|path| path.display().to_string())
        .collect::<Vec<_>>()
        .join("\n")
    }
}
