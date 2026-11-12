use super::*;
use crate::search::{Job, MAX_RESULTS, Progress, Update, Worker};
use std::time::{Duration, Instant};
pub(super) struct Search {
    ctx: egui::Context,
    base: Option<Vec<Entry>>,
    key: Option<(PathBuf, String, bool)>,
    worker: Option<Worker>,
    job: Option<Job>,
    due: Instant,
    seen: BTreeSet<PathBuf>,
    pub progress: Progress,
    done: bool,
}
impl Search {
    pub fn new(ctx: &egui::Context) -> Self {
        Self {
            ctx: ctx.clone(),
            base: None,
            key: None,
            worker: None,
            job: None,
            due: Instant::now(),
            seen: Default::default(),
            progress: Default::default(),
            done: true,
        }
    }
    pub fn active(&self) -> bool {
        self.base.is_some()
    }
    pub fn running(&self) -> bool {
        self.active() && !self.done
    }
    pub fn cancel(&mut self) {
        self.job = None;
        self.key = None;
    }
    pub fn reset(&mut self) {
        self.cancel();
        self.base = None;
        self.seen.clear();
        self.progress = Default::default();
        self.done = true;
    }
}
impl Explorer {
    pub(super) fn open_result_location(&mut self, entry: &Entry, ctx: &egui::Context) {
        if self.loading || !self.recursive_search.active() || self.query.is_empty() {
            return;
        }
        let Some(parent) = entry.path.parent().filter(|path| path.is_absolute()) else {
            return;
        };
        self.editing_address = false;
        self.focus_address = false;
        self.focus_search = false;
        if let Some(id) = ctx.memory(|memory| memory.focused()) {
            ctx.memory_mut(|memory| memory.surrender_focus(id));
        }
        // Always reread, including a match in the current folder. Do not replace
        // the search entries while their context menu is being rendered.
        self.navigate(parent.to_owned(), Navigation::Visit);
        if self.loading {
            self.reveal_external(vec![entry.path.clone()], false);
        }
    }

    pub(super) fn result_location_menu(&mut self, ui: &mut egui::Ui, entry: &Entry) {
        if self.recursive_search.active()
            && !self.query.is_empty()
            && self
                .menu_item(
                    ui,
                    Icon::OpenLocation,
                    crate::i18n::tr("Abrir local do arquivo"),
                    "",
                    !self.loading,
                    false,
                )
                .clicked()
        {
            self.open_result_location(entry, ui.ctx());
            ui.close();
        }
    }

    pub(super) fn directory_entries(&self) -> &[Entry] {
        self.recursive_search
            .base
            .as_deref()
            .unwrap_or(&self.entries)
    }
    pub(super) fn result_name(&self, entry: &Entry) -> String {
        if self.recursive_search.active() {
            entry
                .path
                .strip_prefix(&self.current)
                .unwrap_or(&entry.path)
                .display()
                .to_string()
        } else {
            entry.name.clone()
        }
    }
    pub(super) fn update_search(&mut self) {
        if self.loading {
            return;
        }
        if self.query.is_empty()
            || self.is_trash()
            || self.is_applications()
            || !self.current.is_absolute()
        {
            if let Some(base) = self.recursive_search.base.take() {
                self.entries = base;
                self.thumbnails.cancel_pending();
            }
            self.recursive_search.reset();
            return;
        }
        let key = (
            self.current.clone(),
            self.query.clone(),
            self.hidden_visible(),
        );
        let ctx = self.recursive_search.ctx.clone();
        if self.recursive_search.key.as_ref() != Some(&key) {
            self.recursive_search.cancel();
            self.thumbnails.cancel_pending();
            if self.recursive_search.base.is_none() {
                self.recursive_search.base = Some(std::mem::take(&mut self.entries));
            }
            let query = key.1.to_lowercase();
            self.entries = self
                .recursive_search
                .base
                .as_ref()
                .unwrap()
                .iter()
                .filter(|e| (key.2 || !e.hidden) && e.name.to_lowercase().contains(&query))
                .take(MAX_RESULTS)
                .cloned()
                .collect();
            self.recursive_search.seen = self.entries.iter().map(|e| e.path.clone()).collect();
            self.recursive_search.key = Some(key.clone());
            self.recursive_search.progress = Default::default();
            self.recursive_search.done = false;
            self.recursive_search.due = Instant::now() + Duration::from_millis(250);
        }
        if self.recursive_search.running() && self.recursive_search.job.is_none() {
            if Instant::now() < self.recursive_search.due {
                ctx.request_repaint_after(
                    self.recursive_search
                        .due
                        .saturating_duration_since(Instant::now()),
                );
                return;
            }
            let worker = self
                .recursive_search
                .worker
                .get_or_insert_with(|| Worker::new(&ctx));
            self.recursive_search.job = Some(worker.start(key.0, key.1, key.2));
        }
        for _ in 0..8 {
            let Some(job) = &self.recursive_search.job else {
                break;
            };
            let update = match job.incoming.try_recv() {
                Ok(update) => update,
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    Update::Finished(Err(std::io::Error::other(crate::i18n::tr(
                        "O serviço de pesquisa foi interrompido.",
                    ))))
                }
            };
            match update {
                Update::Batch(entries) => {
                    for entry in entries {
                        if self.entries.len() < MAX_RESULTS
                            && self.recursive_search.seen.insert(entry.path.clone())
                        {
                            self.entries.push(entry);
                        }
                    }
                }
                Update::Progress(progress) => self.recursive_search.progress = progress,
                Update::Finished(result) => {
                    self.recursive_search.job = None;
                    self.recursive_search.done = true;
                    match result {
                        Ok(progress) => {
                            if progress.limited {
                                self.post_typed_notice(toast::NoticeKind::Warning,crate::i18n::tr("Pesquisa parcial: limite de resultados ou de percurso atingido. Refine a pesquisa.").into());
                            }
                            if progress.skipped > 0 {
                                self.post_typed_notice(toast::NoticeKind::Warning,crate::i18n::message("Pesquisa parcial: {} itens ou pastas não puderam ser lidos.",&[progress.skipped.to_string()]));
                            }
                            self.recursive_search.progress = progress;
                        }
                        Err(error) => self.report_error(crate::i18n::message(
                            "Não foi possível concluir a pesquisa: {}",
                            &[error.to_string()],
                        )),
                    }
                }
            }
        }
        if self.recursive_search.running() {
            ctx.request_repaint_after(Duration::from_millis(50));
        }
    }
}

#[cfg(test)]
mod tests;
