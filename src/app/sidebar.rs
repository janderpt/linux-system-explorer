use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Location {
    Standard(locations::Item),
    Pinned(usize),
    Network(usize),
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
        if self.sidebar_options.show_network {
            let mut seen = BTreeSet::new();
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
        let entries = self.sidebar_entries();
        let active = closest(&self.current, &entries);
        egui::SidePanel::left("places").default_width(205.0).min_width(170.0).max_width(280.0).show(ctx,|ui| {
            let height=(ui.available_height()-30.0).max(0.0);
            egui::ScrollArea::vertical().id_salt("sidebar_scroll").max_height(height).auto_shrink([false,false]).show(ui,|ui| {
                self.drag_hover_scroll(ui);
                ui.add_space(10.0);
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
                    let enabled=!self.loading && !self.is_trash() && !self.current.as_os_str().is_empty() && !self.sidebar_options.pinned.iter().any(|p|p.path==self.current);
                    if self.menu_item(ui,Icon::Pin,crate::i18n::tr("Fixar pasta atual"),"",enabled,false).clicked() {self.pin_folder(self.current.clone());}
                    ui.add_space(12.0);
                }
                if self.sidebar_options.show_network {
                    sidebar_heading(ui,crate::i18n::tr("REDE"));
                    let network:Vec<_>=entries.iter().filter(|e|matches!(e.id,Location::Network(_))).collect();
                    if network.is_empty() {ui.label(RichText::new(crate::i18n::tr("Nenhum compartilhamento configurado")).small().weak());}
                    for entry in network {self.sidebar_place(ui,entry,active==Some(entry.id));}
                }
            });
            ui.add_space(5.0);
            ui.label(RichText::new("Linux System Explorer  0.1").small().weak());
        });
    }

    fn sidebar_place(&mut self, ui: &mut egui::Ui, entry: &SidebarEntry, selected: bool) {
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
                icons::paint(
                    ui.painter(),
                    egui::Rect::from_center_size(
                        rect.left_center() + Vec2::new(16.0, 0.0),
                        Vec2::splat(20.0),
                    ),
                    entry.icon,
                    color,
                    &self.appearance,
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
            .on_hover_text(entry.path.display().to_string());
        if response.clicked() {
            self.navigate(entry.path.clone(), Navigation::Visit);
        }
        self.register_drop_target(ui, response.rect, entry.path.clone());
        response.context_menu(|ui| {
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
    }

    pub(super) fn pin_folder(&mut self, path: PathBuf) {
        if let Err(error) = self.sidebar_options.pin(&path) {
            self.error = Some(crate::i18n::message(
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
