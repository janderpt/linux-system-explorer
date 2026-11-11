use super::*;

impl Editor {
    pub(super) fn sidebar(&mut self, ui: &mut egui::Ui) {
        ui.label(crate::i18n::tr(
            "Escolha os atalhos e as seções da barra lateral.",
        ));
        egui::Grid::new("sidebar_choices")
            .num_columns(2)
            .spacing([24.0, 6.0])
            .show(ui, |ui| {
                for (index, item) in SidebarItem::ALL.into_iter().enumerate() {
                    let mut visible = self.draft.sidebar.visible.contains(&item);
                    let available = self.available_sidebar.contains(&item);
                    if ui
                        .add_enabled(available, egui::Checkbox::new(&mut visible, item.label()))
                        .on_hover_text(item.path().map(|p| p.display().to_string()).unwrap_or_else(
                            || crate::i18n::tr("Pasta não configurada no sistema").into(),
                        ))
                        .changed()
                    {
                        if visible {
                            self.draft.sidebar.visible.insert(item);
                        } else {
                            self.draft.sidebar.visible.remove(&item);
                        }
                    }
                    if index % 2 == 1 {
                        ui.end_row();
                    }
                }
            });
        ui.checkbox(
            &mut self.draft.sidebar.show_trash,
            crate::i18n::tr("Exibir Lixeira"),
        );
        ui.separator();
        ui.checkbox(
            &mut self.draft.sidebar.show_pinned,
            crate::i18n::tr("Exibir seção de pastas fixadas"),
        );
        shortcut_list(ui, &mut self.draft.sidebar.pinned, "pinned_choices");
        ui.horizontal(|ui| {
            let width = (ui.available_width() - 110.0).max(160.0);
            ui.add_sized(
                [width, 32.0],
                egui::TextEdit::singleline(&mut self.new_pin)
                    .vertical_align(Align::Center)
                    .hint_text(crate::i18n::tr(
                        "Caminho da pasta a fixar: /caminho ou ~/pasta",
                    )),
            );
            if ui.button(crate::i18n::tr("Fixar pasta")).clicked() {
                match absolute_input(&self.new_pin).and_then(|path| self.draft.sidebar.pin(&path)) {
                    Ok(_) => {
                        self.new_pin.clear();
                        self.error = None;
                    }
                    Err(error) => self.error = Some(error.to_string()),
                }
            }
        });
        ui.separator();
        ui.checkbox(
            &mut self.draft.sidebar.show_network,
            crate::i18n::tr("Exibir seção Rede"),
        );
        ui.weak(crate::i18n::tr("Compartilhamentos montados no Linux são detectados automaticamente. F5 atualiza essa lista."));
        for shortcut in &self.network_mounts {
            let key = folder_key(&shortcut.path);
            let mut visible = !self.draft.sidebar.hidden_network.contains(&key);
            if ui
                .checkbox(
                    &mut visible,
                    crate::i18n::message("{} (montado)", &[(shortcut.title()).to_string()]),
                )
                .on_hover_text(shortcut.path.display().to_string())
                .changed()
            {
                if visible {
                    self.draft.sidebar.hidden_network.remove(&key);
                } else {
                    self.draft.sidebar.hidden_network.insert(key);
                }
            }
        }
        shortcut_list(ui, &mut self.draft.sidebar.network, "network_choices");
        ui.weak(crate::i18n::tr("Adicione o caminho local onde o compartilhamento está ou será montado, por exemplo /mnt/servidor."));
        ui.add_sized(
            [ui.available_width(), 32.0],
            egui::TextEdit::singleline(&mut self.new_network_label)
                .vertical_align(Align::Center)
                .hint_text(crate::i18n::tr("Nome do compartilhamento (opcional)")),
        );
        ui.horizontal(|ui| {
            let width = (ui.available_width() - 150.0).max(160.0);
            ui.add_sized(
                [width, 32.0],
                egui::TextEdit::singleline(&mut self.new_network)
                    .vertical_align(Align::Center)
                    .hint_text(crate::i18n::tr("Ponto de montagem: /mnt/servidor")),
            );
            if ui.button(crate::i18n::tr("Adicionar à Rede")).clicked() {
                match absolute_input(&self.new_network) {
                    Ok(path) => {
                        if !self.draft.sidebar.network.iter().any(|p| p.path == path) {
                            self.draft.sidebar.network.push(Shortcut {
                                path,
                                label: self.new_network_label.trim().to_owned(),
                            });
                        }
                        self.draft.sidebar.show_network = true;
                        self.new_network.clear();
                        self.new_network_label.clear();
                        self.error = None;
                    }
                    Err(error) => self.error = Some(error.to_string()),
                }
            }
        });
    }
}

fn shortcut_list(ui: &mut egui::Ui, shortcuts: &mut Vec<Shortcut>, id: &str) {
    let mut remove = None;
    ui.push_id(id, |ui| {
        for (index, shortcut) in shortcuts.iter().enumerate() {
            ui.push_id(index, |ui| {
                ui.horizontal(|ui| {
                    ui.add_sized(
                        [(ui.available_width() - 90.0).max(120.0), 32.0],
                        egui::Label::new(format!(
                            "{} · {}",
                            shortcut.title(),
                            shortcut.path.display()
                        ))
                        .truncate()
                        .selectable(false),
                    )
                    .on_hover_text(shortcut.path.display().to_string());
                    if ui.button(crate::i18n::tr("Remover")).clicked() {
                        remove = Some(index);
                    }
                });
            });
        }
    });
    if let Some(index) = remove {
        shortcuts.remove(index);
    }
}

fn absolute_input(value: &str) -> io::Result<PathBuf> {
    let value = value.trim();
    let path = if value == "~" {
        dirs::home_dir().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                crate::i18n::tr("Pasta pessoal não encontrada"),
            )
        })?
    } else if let Some(rest) = value.strip_prefix("~/") {
        dirs::home_dir()
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::NotFound,
                    crate::i18n::tr("Pasta pessoal não encontrada"),
                )
            })?
            .join(rest)
    } else {
        PathBuf::from(value)
    };
    if !path.is_absolute() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            crate::i18n::tr("Informe um caminho absoluto, como /mnt/servidor, ou use ~/pasta"),
        ));
    }
    Ok(path.components().collect())
}
