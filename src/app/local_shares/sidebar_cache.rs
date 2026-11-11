use super::*;
use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

pub(in crate::app) type SharedSidebar = Arc<Mutex<SidebarShares>>;
type Listing = Receiver<io::Result<Vec<Share>>>;

pub(in crate::app) struct SidebarShares {
    pub shares: Vec<Share>,
    pub error: Option<String>,
    pub loaded: bool,
    pub enabled: bool,
    revision: u64,
    pending: Option<(u64, Listing)>,
    next_refresh: Instant,
}
impl Default for SidebarShares {
    fn default() -> Self {
        Self {
            shares: vec![],
            error: None,
            loaded: false,
            enabled: false,
            revision: 0,
            pending: None,
            next_refresh: Instant::now(),
        }
    }
}
impl SidebarShares {
    pub fn publish(&mut self, shares: Vec<Share>) {
        self.shares = shares;
        self.error = None;
        self.loaded = true;
        self.revision = self.revision.wrapping_add(1);
        self.next_refresh = Instant::now() + Duration::from_secs(5);
    }
    pub fn invalidate(&mut self) {
        self.next_refresh = Instant::now();
    }
    fn poll(&mut self) {
        let Some((revision, receiver)) = &self.pending else {
            return;
        };
        let revision = *revision;
        let result = match receiver.try_recv() {
            Ok(result) => Some(result),
            Err(mpsc::TryRecvError::Disconnected) => Some(Err(io::Error::other(crate::i18n::tr(
                "A atualização dos compartilhamentos locais foi interrompida.",
            )))),
            Err(mpsc::TryRecvError::Empty) => None,
        };
        if let Some(result) = result {
            self.pending = None;
            // A query started before a manager change must not restore old names.
            if revision != self.revision {
                return;
            }
            match result {
                Ok(shares) => self.publish(shares),
                Err(error) => {
                    self.error = Some(error.to_string());
                    self.loaded = true;
                    self.next_refresh = Instant::now() + Duration::from_secs(30);
                }
            }
        }
    }
    fn refresh(&mut self, ctx: &egui::Context) {
        self.poll();
        if !self.enabled {
            return;
        }
        if self.pending.is_none() && Instant::now() >= self.next_refresh {
            let (sender, receiver) = mpsc::channel();
            let ctx = ctx.clone();
            let locale = crate::i18n::current();
            thread::spawn(move || {
                crate::i18n::set_locale(locale);
                let _ = sender.send(crate::local_shares::Service::default().list());
                ctx.request_repaint();
            });
            self.pending = Some((self.revision, receiver));
        }
        let wait = if self.pending.is_some() {
            Duration::from_secs(1)
        } else {
            self.next_refresh.saturating_duration_since(Instant::now())
        };
        ctx.request_repaint_after(wait);
    }
}
impl Explorer {
    pub(in crate::app) fn refresh_local_share_sidebar(&mut self, ctx: &egui::Context) {
        self.local_share_sidebar
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .refresh(ctx);
    }
    pub(in crate::app) fn publish_local_share_sidebar(&mut self, shares: Vec<Share>) {
        self.local_share_sidebar
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .publish(shares);
    }
    pub(in crate::app) fn local_share_entries(&self) -> Vec<Share> {
        self.local_share_sidebar
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .shares
            .clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stale_queries_do_not_undo_a_create_rename_or_removal() {
        let folder = tempfile::tempdir().unwrap();
        for old_count in [0, 1] {
            for new_name in [Some("new-name"), None] {
                let mut cache = SidebarShares::default();
                let mut old = Share::new(folder.path().into());
                old.name = "old-name".into();
                let old_list = if old_count == 0 {
                    vec![]
                } else {
                    vec![old.clone()]
                };
                cache.publish(old_list.clone());
                let (sender, receiver) = mpsc::channel();
                cache.pending = Some((cache.revision, receiver));
                let new_list = if let Some(name) = new_name {
                    old.name = name.into();
                    vec![old]
                } else {
                    vec![]
                };
                cache.publish(new_list.clone());
                sender.send(Ok(old_list)).unwrap();
                cache.poll();
                assert_eq!(cache.shares, new_list);
            }
        }
    }
    #[test]
    fn background_failure_keeps_last_known_shares_and_reports_the_error() {
        let folder = tempfile::tempdir().unwrap();
        let share = Share::new(folder.path().into());
        let mut cache = SidebarShares::default();
        cache.publish(vec![share.clone()]);
        let (sender, receiver) = mpsc::channel();
        cache.pending = Some((cache.revision, receiver));
        sender
            .send(Err(io::Error::other("Samba unavailable")))
            .unwrap();
        cache.poll();
        assert_eq!(cache.shares, vec![share]);
        assert!(cache.error.is_some());
        assert!(cache.next_refresh > Instant::now());
    }
}
