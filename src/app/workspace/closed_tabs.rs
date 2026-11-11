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
        let Some(closed) = self.closed_tabs.pop_back() else {
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
}
