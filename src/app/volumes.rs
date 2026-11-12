use super::*;
use crate::volumes::{Action, Outcome, Volume};
use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

pub(super) type Shared = Arc<Mutex<Sidebar>>;
type Listing = Receiver<Result<Vec<Volume>, String>>;
pub(super) struct Sidebar {
    pub enabled: bool,
    pub volumes: Vec<Volume>,
    pub error: Option<String>,
    pub loaded: bool,
    pub busy: bool,
    needs_sync: bool,
    revision: u64,
    pub change_serial: u64,
    departed: std::collections::VecDeque<(u64, Vec<PathBuf>)>,
    pending: Option<(u64, Listing)>,
    next: Instant,
}
impl Default for Sidebar {
    fn default() -> Self {
        Self {
            enabled: false,
            volumes: vec![],
            error: None,
            loaded: false,
            busy: false,
            needs_sync: false,
            revision: 0,
            change_serial: 0,
            departed: Default::default(),
            pending: None,
            next: Instant::now(),
        }
    }
}
impl Sidebar {
    pub fn invalidate(&mut self) {
        self.revision = self.revision.wrapping_add(1);
        self.next = Instant::now();
    }
    fn removed(&mut self, paths: Vec<PathBuf>) {
        if paths.is_empty() {
            return;
        }
        self.change_serial = self.change_serial.wrapping_add(1);
        self.departed.push_back((self.change_serial, paths));
        while self.departed.len() > 64 {
            self.departed.pop_front();
        }
    }
    fn refresh(&mut self, ctx: &egui::Context) -> Option<String> {
        let mut notice = None;
        if let Some((revision, receiver)) = &self.pending {
            let result = match receiver.try_recv() {
                Ok(r) => Some(r),
                Err(mpsc::TryRecvError::Disconnected) => Some(Err(crate::i18n::tr(
                    "A atualização das unidades foi interrompida.",
                )
                .into())),
                Err(mpsc::TryRecvError::Empty) => None,
            };
            if let Some(result) = result {
                let fresh = *revision == self.revision;
                self.pending = None;
                if fresh {
                    self.loaded = true;
                    self.needs_sync = false;
                    match result {
                        Ok(volumes) => {
                            let removed = self
                                .volumes
                                .iter()
                                .flat_map(|v| v.mounts.iter())
                                .filter(|p| !volumes.iter().any(|v| v.mounts.contains(p)))
                                .cloned()
                                .collect();
                            self.removed(removed);
                            self.volumes = volumes;
                            ctx.request_repaint();
                            self.error = None;
                            self.next = Instant::now() + Duration::from_secs(5);
                        }
                        Err(error) => {
                            if self.error.as_ref() != Some(&error) {
                                notice = Some(error.clone());
                            }
                            self.error = Some(error);
                            self.next = Instant::now() + Duration::from_secs(30);
                        }
                    }
                }
            }
        }
        if !self.enabled {
            return notice;
        }
        if !self.busy && self.pending.is_none() && Instant::now() >= self.next {
            let (sender, receiver) = mpsc::channel();
            let ctx = ctx.clone();
            let locale = crate::i18n::current();
            thread::spawn(move || {
                crate::i18n::set_locale(locale);
                let _ = sender.send(crate::volumes::list());
                ctx.request_repaint();
            });
            self.pending = Some((self.revision, receiver));
        }
        ctx.request_repaint_after(if self.pending.is_some() || self.busy {
            Duration::from_millis(250)
        } else {
            self.next.saturating_duration_since(Instant::now())
        });
        notice
    }
}
pub(super) struct Pending {
    pub volume: Volume,
    pub result: Receiver<Result<Outcome, String>>,
}
impl Explorer {
    pub(super) fn volume_entries(&self) -> Vec<Volume> {
        self.volume_sidebar
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .volumes
            .clone()
    }
    pub(super) fn refresh_volumes(&mut self, ctx: &egui::Context) {
        if crate::sandbox::active() {
            return;
        }
        let error = self
            .volume_sidebar
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .refresh(ctx);
        if let Some(error) = error {
            self.report_error(crate::i18n::message(
                "Não foi possível listar as unidades: {}",
                &[error],
            ));
        }
    }
    pub(super) fn request_volume(&mut self, volume: &Volume, action: Action, ctx: &egui::Context) {
        self.start_volume(volume, action, ctx, crate::volumes::execute);
    }
    fn start_volume(
        &mut self,
        volume: &Volume,
        action: Action,
        ctx: &egui::Context,
        task: impl FnOnce(&Volume, Action) -> Result<Outcome, String> + Send + 'static,
    ) {
        if self.loading || self.volume_operation.is_some() {
            return;
        }
        {
            let mut sidebar = self
                .volume_sidebar
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            if sidebar.busy {
                return;
            }
            sidebar.busy = true;
            sidebar.needs_sync = true;
            sidebar.invalidate();
        }
        let (sender, result) = mpsc::channel();
        let selected = volume.clone();
        let cache = self.volume_sidebar.clone();
        let ctx = ctx.clone();
        let locale = crate::i18n::current();
        thread::spawn(move || {
            crate::i18n::set_locale(locale);
            let outcome = task(&selected, action);
            {
                let mut cache = cache.lock().unwrap_or_else(|e| e.into_inner());
                cache.busy = false;
                if let Ok(outcome) = &outcome {
                    cache.removed(outcome.removed().to_vec());
                }
                cache.invalidate();
            }
            let _ = sender.send(outcome);
            ctx.request_repaint();
        });
        self.volume_operation = Some(Pending {
            volume: volume.clone(),
            result,
        });
    }
    pub(super) fn poll_volumes(&mut self) {
        if self.loading {
            return;
        }
        let removed = {
            let cache = self
                .volume_sidebar
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            let paths = cache
                .departed
                .iter()
                .filter(|(serial, _)| *serial > self.volume_seen)
                .flat_map(|(_, p)| p.iter().cloned())
                .collect::<Vec<_>>();
            self.volume_seen = cache.change_serial;
            paths
        };
        if removed.iter().any(|path| self.current.starts_with(path))
            && let Some(home) = dirs::home_dir()
        {
            self.navigate(home, Navigation::Visit);
            return;
        }
        let Some(pending) = &self.volume_operation else {
            return;
        };
        let result = match pending.result.try_recv() {
            Ok(r) => r,
            Err(mpsc::TryRecvError::Empty) => return,
            Err(mpsc::TryRecvError::Disconnected) => {
                Err(crate::i18n::tr("A operação da unidade foi interrompida.").into())
            }
        };
        let pending = self.volume_operation.take().unwrap();
        match result {
            Ok(Outcome::Opened(path)) => self.navigate(path, Navigation::Visit),
            Ok(Outcome::Cancelled) => {}
            Ok(outcome) => {
                if pending
                    .volume
                    .mounts
                    .iter()
                    .any(|p| self.current.starts_with(p))
                    && let Some(home) = dirs::home_dir()
                {
                    self.navigate(home, Navigation::Visit);
                }
                self.post_typed_notice(
                    toast::NoticeKind::Success,
                    crate::i18n::message(
                        if matches!(outcome, Outcome::Ejected(_)) {
                            "Unidade ejetada: {}"
                        } else {
                            "Volume desmontado: {}"
                        },
                        &[pending.volume.name],
                    ),
                );
            }
            Err(error) => self.report_error(crate::i18n::message(
                "Não foi possível concluir a operação da unidade: {}",
                &[error],
            )),
        }
    }
    pub(super) fn volumes_section(
        &mut self,
        ui: &mut egui::Ui,
        entries: &[sidebar::SidebarEntry],
        active: Option<sidebar::Location>,
    ) {
        use sidebar::Location;
        let busy = {
            let cache = self
                .volume_sidebar
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            cache.busy || cache.needs_sync
        };
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(crate::i18n::tr("UNIDADES"))
                    .small()
                    .weak()
                    .strong(),
            );
            if busy {
                ui.add(egui::Spinner::new().size(14.0))
                    .on_hover_text(crate::i18n::tr("Aguardando operação da unidade…"));
            }
        });
        ui.add_space(6.0);
        if crate::sandbox::active() {
            ui.weak(crate::i18n::tr(
                "Unidades indisponíveis no Flatpak experimental.",
            ));
            return;
        }
        let volumes = self.volume_entries();
        let (loaded, busy, error) = {
            let cache = self
                .volume_sidebar
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            (
                cache.loaded,
                cache.busy || cache.needs_sync,
                cache.error.clone(),
            )
        };
        if volumes.is_empty() {
            ui.label(
                RichText::new(if error.is_some() {
                    crate::i18n::tr("Unidades indisponíveis")
                } else if loaded {
                    crate::i18n::tr("Nenhum volume disponível")
                } else {
                    crate::i18n::tr("Carregando unidades…")
                })
                .small()
                .weak(),
            );
        }
        for entry in entries
            .iter()
            .filter(|e| matches!(e.id, Location::Volume(_)))
        {
            let Location::Volume(index) = entry.id else {
                continue;
            };
            let Some(volume) = volumes.get(index) else {
                continue;
            };
            ui.push_id((&volume.id, "volume"), |ui| {
                let selected = active == Some(entry.id);
                let (rect, response) =
                    ui.allocate_exact_size(Vec2::new(ui.available_width(), 54.0), Sense::click());
                if selected {
                    self.sidebar_scroll.record(rect);
                }
                if selected || response.hovered() {
                    ui.painter().rect_filled(
                        rect,
                        4.0,
                        if selected {
                            ui.visuals().selection.bg_fill
                        } else {
                            ui.visuals().widgets.hovered.bg_fill
                        },
                    );
                }
                let color = if selected {
                    ui.visuals().selection.stroke.color
                } else {
                    ui.visuals().text_color()
                };
                let icon = if volume.optical {
                    Icon::FileDisk
                } else if volume.removable {
                    Icon::Removable
                } else {
                    Icon::Drive
                };
                icons::paint(
                    ui.painter(),
                    egui::Rect::from_center_size(
                        rect.left_center() + Vec2::new(16.0, 0.0),
                        Vec2::splat(20.0),
                    ),
                    icon,
                    color,
                    &self.appearance,
                );
                let painter = ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
                painter.text(
                    rect.left_top() + Vec2::new(33.0, 13.0),
                    egui::Align2::LEFT_CENTER,
                    &volume.name,
                    FontId::proportional(13.0),
                    color,
                );
                painter.text(
                    rect.left_top() + Vec2::new(33.0, 31.0),
                    egui::Align2::LEFT_CENTER,
                    volume.capacity(),
                    FontId::proportional(10.5),
                    ui.visuals().weak_text_color(),
                );
                if let Some(space) = &volume.space
                    && space.total > 0
                {
                    let bar = egui::Rect::from_min_size(
                        rect.left_bottom() + Vec2::new(33.0, -8.0),
                        Vec2::new((rect.width() - 42.0).max(1.0), 3.0),
                    );
                    painter.rect_filled(bar, 2.0, ui.visuals().widgets.inactive.bg_fill);
                    let used = 1.0 - space.available as f32 / space.total as f32;
                    painter.rect_filled(
                        egui::Rect::from_min_size(bar.min, Vec2::new(bar.width() * used, 3.0)),
                        2.0,
                        ui.visuals().selection.stroke.color,
                    );
                }
                let response = response.on_hover_text(format!(
                    "{}\n{}\n{}\n{}",
                    volume.name,
                    volume.device.display(),
                    volume.filesystem,
                    volume
                        .mounts
                        .iter()
                        .map(|p| p.display().to_string())
                        .collect::<Vec<_>>()
                        .join("\n")
                ));
                if response.clicked() && !busy {
                    self.request_volume(volume, Action::Open, ui.ctx());
                }
                if !busy
                    && error.is_none()
                    && let Some(path) = volume.mount()
                {
                    self.register_drop_target(ui, response.rect, path.to_owned());
                }
                response.context_menu(|ui| {
                    if self
                        .menu_item(
                            ui,
                            if volume.mount().is_some() {
                                Icon::Open
                            } else {
                                Icon::MountVolume
                            },
                            if volume.mount().is_some() {
                                crate::i18n::tr("Abrir")
                            } else {
                                crate::i18n::tr("Montar e abrir")
                            },
                            "",
                            !busy && !self.loading,
                            false,
                        )
                        .clicked()
                    {
                        self.request_volume(volume, Action::Open, ui.ctx());
                        ui.close();
                    }
                    if volume.mount().is_some()
                        && self
                            .menu_item(
                                ui,
                                Icon::UnmountVolume,
                                crate::i18n::tr("Desmontar"),
                                "",
                                !busy && !self.loading && volume.can_unmount(),
                                false,
                            )
                            .clicked()
                    {
                        self.request_volume(volume, Action::Unmount, ui.ctx());
                        ui.close();
                    }
                    if (volume.ejectable || volume.power_off)
                        && self
                            .menu_item(
                                ui,
                                Icon::EjectVolume,
                                crate::i18n::tr("Ejetar com segurança"),
                                "",
                                !busy && !self.loading && volume.can_eject(),
                                false,
                            )
                            .clicked()
                    {
                        self.request_volume(volume, Action::Eject, ui.ctx());
                        ui.close();
                    }
                });
            });
        }
        ui.add_space(12.0);
    }
}

#[cfg(test)]
mod tests;
