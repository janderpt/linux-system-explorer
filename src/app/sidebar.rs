use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Location {
    Applications,
    Standard(locations::Item),
    Pinned(usize),
    Network(usize),
    Remote(usize),
    LocalShare(usize),
    Trash,
}

pub(super) struct SidebarEntry {
    pub id: Location,
    pub label: String,
    pub path: PathBuf,
    icon: Icon,
}

pub(super) fn closest(current: &Path, entries: &[SidebarEntry]) -> Option<Location> {
    let mut active = None;
    let mut depth = 0;
    for entry in entries {
        let candidate = entry.path.components().count();
        if current.starts_with(&entry.path) && candidate > depth {
            depth = candidate;
            active = Some(entry.id);
        }
    }
    active
}

impl Explorer {
    pub(super) fn sidebar_entries(&self) -> Vec<SidebarEntry> {
        let mut entries: Vec<_> = self
            .places
            .iter()
            .filter(|(item, _)| self.sidebar_options.visible.contains(item))
            .map(|(item, path)| SidebarEntry {
                id: Location::Standard(*item),
                label: item.label().into(),
                path: path.clone(),
                icon: item.icon(),
            })
            .collect();
        if self.sidebar_options.show_applications {
            entries.insert(
                0,
                SidebarEntry {
                    id: Location::Applications,
                    label: crate::i18n::tr("Aplicativos").into(),
                    path: crate::applications::menu::location(),
                    icon: Icon::Applications,
                },
            );
        }
        if self.sidebar_options.show_trash {
            entries.push(SidebarEntry {
                id: Location::Trash,
                label: crate::i18n::tr("Lixeira").into(),
                path: crate::trashbin::location(),
                icon: Icon::TrashBin,
            });
        }
        if self.sidebar_options.show_pinned {
            entries.extend(
                self.sidebar_options
                    .pinned
                    .iter()
                    .enumerate()
                    .map(|(i, p)| SidebarEntry {
                        id: Location::Pinned(i),
                        label: p.title(),
                        path: p.path.clone(),
                        icon: Icon::Pin,
                    }),
            );
        }
        if self.sidebar_options.show_local_shares && !crate::sandbox::active() {
            entries.extend(self.local_share_entries().into_iter().enumerate().map(
                |(index, share)| SidebarEntry {
                    id: Location::LocalShare(index),
                    label: share.name,
                    path: share.path,
                    icon: Icon::ShareFolder,
                },
            ));
        }
        if self.sidebar_options.show_network {
            let mut seen = BTreeSet::new();
            for (index, remote) in self.sidebar_options.remote.iter().enumerate() {
                let path = remote.mount.clone().unwrap_or_default();
                if path.is_absolute() {
                    seen.insert(path.clone());
                }
                entries.push(SidebarEntry {
                    id: Location::Remote(index),
                    label: remote.title(),
                    path,
                    icon: Icon::Network,
                });
            }
            // Explicit names take priority over automatically discovered mount names.
            for (index, shortcut) in self
                .sidebar_options
                .network
                .iter()
                .chain(&self.network_mounts)
                .enumerate()
            {
                if seen.insert(shortcut.path.clone())
                    && !self
                        .sidebar_options
                        .hidden_network
                        .contains(&settings::folder_key(&shortcut.path))
                {
                    entries.push(SidebarEntry {
                        id: Location::Network(index),
                        label: shortcut.title(),
                        path: shortcut.path.clone(),
                        icon: Icon::Network,
                    });
                }
            }
        }
        entries
    }

    pub(super) fn sidebar(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("places")
            .default_width(205.0)
            .min_width(170.0)
            .max_width(280.0)
            .show(ctx, |ui| self.sidebar_body(ui));
    }

    pub(super) fn sidebar_inside(&mut self, ui: &mut egui::Ui) {
        let maximum = (ui.available_width() * 0.48).clamp(120.0, 240.0);
        egui::SidePanel::left(egui::Id::new(("pane_places", self.tab_id)))
            .default_width(170.0)
            .min_width(120.0)
            .max_width(maximum)
            .show_inside(ui, |ui| self.sidebar_body(ui));
    }

    fn sidebar_body(&mut self, ui: &mut egui::Ui) {
        if self.sidebar_options.show_local_shares && !crate::sandbox::active() {
            self.refresh_local_share_sidebar(ui.ctx());
        }
        let entries = self.sidebar_entries();
        let active = closest(&self.current, &entries);
        let mut remove_remote = None;
        let height = (ui.available_height() - 30.0).max(0.0);
        egui::ScrollArea::vertical().id_salt("sidebar_scroll").max_height(height).auto_shrink([false,false]).show(ui,|ui| {
                self.drag_hover_scroll(ui);
                ui.add_space(10.0);
                for entry in entries.iter().filter(|e|e.id==Location::Applications) { self.sidebar_place(ui,entry,active==Some(entry.id)); }
                ui.add_space(6.0);
                for computer in [false,true] {
                    let group:Vec<_>=entries.iter().filter(|entry|matches!(entry.id,Location::Standard(item) if item.computer()==computer)).collect();
                    if !group.is_empty() || computer && self.sidebar_options.show_trash {
                        sidebar_heading(ui,if computer {crate::i18n::tr("ESTE COMPUTADOR")} else {crate::i18n::tr("ACESSO RÁPIDO")});
                        for entry in group {self.sidebar_place(ui,entry,active==Some(entry.id));}
                        if computer { for entry in entries.iter().filter(|entry| entry.id == Location::Trash) { self.sidebar_place(ui,entry,active==Some(entry.id)); } }
                        ui.add_space(12.0);
                    }
                }
                if self.sidebar_options.show_pinned {
                    sidebar_heading(ui,crate::i18n::tr("PASTAS FIXADAS"));
                    for entry in entries.iter().filter(|e|matches!(e.id,Location::Pinned(_))) {self.sidebar_place(ui,entry,active==Some(entry.id));}
                    let enabled=!self.loading && !self.is_trash() && !self.is_applications() && !self.current.as_os_str().is_empty() && !self.sidebar_options.pinned.iter().any(|p|p.path==self.current);
                    if self.menu_item(ui,Icon::Pin,crate::i18n::tr("Fixar pasta atual"),"",enabled,false).clicked() {self.pin_folder(self.current.clone());}
                    ui.add_space(12.0);
                }
                if self.sidebar_options.show_local_shares && !crate::sandbox::active() {
                    self.local_share_heading(ui);
                    let local: Vec<_> = entries.iter().filter(|e| matches!(e.id, Location::LocalShare(_))).collect();
                    for entry in &local { self.sidebar_place(ui,entry,active==Some(entry.id)); }
                    let (loaded,error) = { let cache=self.local_share_sidebar.lock().unwrap_or_else(|e|e.into_inner()); (cache.loaded,cache.error.clone()) };
                    if let Some(error) = error {
                        ui.label(RichText::new(crate::i18n::tr("Não foi possível atualizar os compartilhamentos locais.")).small().weak()).on_hover_text(error);
                    } else if local.is_empty() {
                        ui.label(RichText::new(if loaded {crate::i18n::tr("Nenhuma pasta local compartilhada")} else {crate::i18n::tr("Carregando compartilhamentos locais…")} ).small().weak());
                    }
                    ui.add_space(12.0);
                }
                if self.sidebar_options.show_network {
                    sidebar_heading(ui,crate::i18n::tr("REDE"));
                    let network:Vec<_>=entries.iter().filter(|e|matches!(e.id,Location::Network(_)|Location::Remote(_))).collect();
                    if network.is_empty() {ui.label(RichText::new(crate::i18n::tr("Nenhuma conexão de rede configurada")).small().weak());}
                    for entry in network {
                        if let Some(index) = self.sidebar_place(ui,entry,active==Some(entry.id)) {
                            remove_remote = Some(index);
                        }
                    }
                    if self.menu_item(ui,Icon::AddNetwork,crate::i18n::tr("Adicionar compartilhamento de rede"),"",!self.loading,false).on_hover_text(crate::i18n::tr("Adicionar compartilhamento de rede")).clicked(){self.request_network(None,ui.ctx());}
                }
            });
        ui.add_space(5.0);
        ui.label(
            RichText::new(concat!(
                "Linux System Explorer  ",
                env!("CARGO_PKG_VERSION")
            ))
            .small()
            .weak(),
        );
        // Entries are a snapshot of the indices used throughout this frame. Apply
        // removals only after every entry and its context menu has been rendered.
        if let Some(index) = remove_remote {
            self.sidebar_options.remote.remove(index);
        }
    }

    fn local_share_heading(&mut self, ui: &mut egui::Ui) {
        let label = crate::i18n::tr("Gerenciar compartilhamentos locais");
        let response = ui
            .horizontal(|ui| {
                let heading = ui.label(
                    RichText::new(crate::i18n::tr("Compartilhamentos locais"))
                        .small()
                        .weak()
                        .strong(),
                );
                let response = ui.add_enabled(
                    !self.loading,
                    egui::Button::new("")
                        .frame(false)
                        .min_size(Vec2::splat(22.0)),
                );
                response.widget_info(|| {
                    egui::WidgetInfo::labeled(
                        egui::WidgetType::Button,
                        !self.loading && ui.is_enabled(),
                        label,
                    )
                });
                let color = ui.visuals().weak_text_color();
                icons::paint(
                    ui.painter(),
                    egui::Rect::from_center_size(
                        egui::pos2(response.rect.center().x, heading.rect.center().y),
                        Vec2::splat(14.0),
                    ),
                    Icon::Settings,
                    if self.loading {
                        color.gamma_multiply(0.4)
                    } else {
                        color
                    },
                    &self.appearance,
                );
                response.on_hover_text(label)
            })
            .inner;
        ui.add_space(6.0);
        if response.clicked() {
            self.request_local_shares(None, ui.ctx());
        }
    }

    fn sidebar_place(
        &mut self,
        ui: &mut egui::Ui,
        entry: &SidebarEntry,
        selected: bool,
    ) -> Option<usize> {
        let mut remove_remote = None;
        let response = ui
            .add_enabled_ui(!self.loading, |ui| {
                let (rect, response) =
                    ui.allocate_exact_size(Vec2::new(ui.available_width(), 34.0), Sense::click());
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
                self.paint_folder_icon(
                    ui.painter(),
                    &entry.path,
                    egui::Rect::from_center_size(
                        rect.left_center() + Vec2::new(16.0, 0.0),
                        Vec2::splat(20.0),
                    ),
                    entry.icon,
                    color,
                );
                let clip = egui::Rect::from_min_max(
                    egui::pos2(rect.left() + 33.0, rect.top()),
                    rect.right_bottom(),
                );
                ui.painter().with_clip_rect(clip).text(
                    clip.left_center(),
                    egui::Align2::LEFT_CENTER,
                    &entry.label,
                    FontId::proportional(13.0),
                    color,
                );
                response.widget_info(|| {
                    egui::WidgetInfo::selected(
                        egui::WidgetType::SelectableLabel,
                        ui.is_enabled(),
                        selected,
                        &entry.label,
                    )
                });
                response
            })
            .inner
            .on_hover_text(if let Location::Remote(i) = entry.id {
                self.sidebar_options.remote[i].url.clone()
            } else {
                entry.path.display().to_string()
            });
        if response.clicked() {
            if let Location::Remote(i) = entry.id {
                self.request_network(Some(self.sidebar_options.remote[i].clone()), ui.ctx());
            } else {
                self.navigate(entry.path.clone(), Navigation::Visit);
            }
        }
        if entry.path.is_absolute() {
            self.register_drop_target(ui, response.rect, entry.path.clone());
        }
        response.context_menu(|ui| {
            if entry.id == Location::Applications {
                if self
                    .menu_item(
                        ui,
                        Icon::NewTab,
                        crate::i18n::tr("Abrir em nova aba"),
                        "",
                        true,
                        false,
                    )
                    .clicked()
                {
                    self.requested_tabs.push(entry.path.clone());
                    ui.close();
                }
                return;
            }
            if let Location::Remote(i) = entry.id {
                if self
                    .menu_item(
                        ui,
                        Icon::Network,
                        crate::i18n::tr("Conectar ao compartilhamento"),
                        "",
                        !self.loading,
                        false,
                    )
                    .clicked()
                {
                    self.request_network(Some(self.sidebar_options.remote[i].clone()), ui.ctx());
                    ui.close();
                }
                if entry.path.is_absolute() {
                    self.folder_actions_menu(ui, &entry.path);
                }
                if self
                    .menu_item(
                        ui,
                        Icon::Unpin,
                        crate::i18n::tr("Remover da Rede"),
                        "",
                        true,
                        false,
                    )
                    .clicked()
                {
                    remove_remote = Some(i);
                    ui.close();
                }
                return;
            }
            if self
                .menu_item(ui, Icon::Open, crate::i18n::tr("Abrir"), "", true, false)
                .clicked()
            {
                self.navigate(entry.path.clone(), Navigation::Visit);
                ui.close();
            }
            if entry.id == Location::Trash {
                ui.separator();
                if self
                    .menu_item(
                        ui,
                        Icon::EmptyTrash,
                        crate::i18n::tr("Esvaziar lixeira"),
                        "",
                        self.can_empty_trash(),
                        false,
                    )
                    .clicked()
                {
                    self.request_empty_trash(ui.ctx());
                    ui.close();
                }
                return;
            }
            self.folder_actions_menu(ui, &entry.path);
            self.pin_menu(ui, &entry.path);
            if matches!(entry.id, Location::Network(_)) {
                let custom = self
                    .sidebar_options
                    .network
                    .iter()
                    .any(|p| p.path == entry.path);
                if self
                    .menu_item(
                        ui,
                        Icon::Unpin,
                        if custom {
                            crate::i18n::tr("Remover da Rede")
                        } else {
                            crate::i18n::tr("Ocultar compartilhamento")
                        },
                        "",
                        true,
                        false,
                    )
                    .clicked()
                {
                    self.sidebar_options
                        .network
                        .retain(|p| p.path != entry.path);
                    self.sidebar_options
                        .hidden_network
                        .insert(settings::folder_key(&entry.path));
                    ui.close();
                }
            }
        });
        remove_remote
    }

    pub(super) fn pin_folder(&mut self, path: PathBuf) {
        if let Err(error) = self.sidebar_options.pin(&path) {
            self.report_error(crate::i18n::message(
                "Não foi possível fixar a pasta: {error}",
                &[(error).to_string()],
            ));
        }
    }

    pub(super) fn pin_menu(&mut self, ui: &mut egui::Ui, path: &Path) {
        let pinned = self.sidebar_options.pinned.iter().any(|p| p.path == path);
        if self
            .menu_item(
                ui,
                if pinned { Icon::Unpin } else { Icon::Pin },
                if pinned {
                    crate::i18n::tr("Desafixar da barra lateral")
                } else {
                    crate::i18n::tr("Fixar na barra lateral")
                },
                "",
                true,
                false,
            )
            .clicked()
        {
            if pinned {
                self.sidebar_options.pinned.retain(|p| p.path != path);
            } else {
                self.pin_folder(path.to_owned());
            }
            ui.close();
        }
    }

    pub(super) fn menu_item(
        &self,
        ui: &mut egui::Ui,
        icon: Icon,
        label: &str,
        shortcut: &str,
        enabled: bool,
        selected: bool,
    ) -> egui::Response {
        icons::menu_item(
            ui,
            icon,
            label,
            shortcut,
            enabled,
            selected,
            &self.appearance,
        )
    }
}

fn sidebar_heading(ui: &mut egui::Ui, label: &str) {
    ui.label(RichText::new(label).small().weak().strong());
    ui.add_space(6.0);
}
