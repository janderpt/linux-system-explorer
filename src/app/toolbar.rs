use super::*;

#[derive(Clone, Copy)]
enum FolderAction {
    NewFolder,
    Copy,
    Cut,
    Paste,
    Hidden,
    Details,
    List,
    Icons,
    ZoomOut,
    ZoomIn,
}

impl FolderAction {
    const ALL: [Self; 10] = [
        Self::NewFolder,
        Self::Copy,
        Self::Cut,
        Self::Paste,
        Self::Hidden,
        Self::Details,
        Self::List,
        Self::Icons,
        Self::ZoomOut,
        Self::ZoomIn,
    ];

    fn icon(self) -> Icon {
        match self {
            Self::NewFolder => Icon::NewFolder,
            Self::Copy => Icon::Copy,
            Self::Cut => Icon::Cut,
            Self::Paste => Icon::Paste,
            Self::Hidden => Icon::Hidden,
            Self::Details => Icon::Details,
            Self::List => Icon::List,
            Self::Icons => Icon::Grid,
            Self::ZoomOut => Icon::ZoomOut,
            Self::ZoomIn => Icon::ZoomIn,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::NewFolder => crate::i18n::tr("Nova pasta"),
            Self::Copy => crate::i18n::tr("Copiar"),
            Self::Cut => crate::i18n::tr("Recortar"),
            Self::Paste => crate::i18n::tr("Colar"),
            Self::Hidden => crate::i18n::tr("Itens ocultos"),
            Self::Details => crate::i18n::tr("Exibir detalhes"),
            Self::List => crate::i18n::tr("Exibir lista"),
            Self::Icons => crate::i18n::tr("Exibir ícones"),
            Self::ZoomOut => crate::i18n::tr("Diminuir tamanho dos itens"),
            Self::ZoomIn => crate::i18n::tr("Aumentar tamanho dos itens"),
        }
    }

    fn shortcut(self) -> &'static str {
        match self {
            Self::NewFolder => "Ctrl+Shift+N",
            Self::Copy => "Ctrl+C",
            Self::Cut => "Ctrl+X",
            Self::Paste => "Ctrl+V",
            Self::Hidden => "Ctrl+H",
            _ => "",
        }
    }
}

impl Explorer {
    pub(super) fn zoom_items(&mut self, delta: f32) {
        self.item_scale = ((self.item_scale + delta).clamp(0.75, 2.0) * 4.0).round() / 4.0;
        self.reveal_selection = self.selection_cursor.is_some();
    }

    pub(super) fn folder_actions(&mut self, ui: &mut egui::Ui) {
        let width = (self.appearance.size + 14.0).max(32.0);
        let step = width + ui.spacing().item_spacing.x;
        // Navigation reserves room for global controls; keep an overflow button when needed.
        let budget = ui.available_width();
        let all_fit = budget >= step * FolderAction::ALL.len() as f32;
        let visible = if all_fit {
            FolderAction::ALL.len()
        } else {
            ((budget - step).max(0.0) / step) as usize
        };
        for action in FolderAction::ALL.into_iter().take(visible) {
            self.folder_action(ui, action, false);
        }
        if visible < FolderAction::ALL.len() {
            let (response, _) = egui::containers::menu::MenuButton::from_button(
                egui::Button::new("")
                    .min_size(Vec2::new(width, (self.appearance.size + 10.0).max(28.0))),
            )
            .ui(ui, |ui| {
                for action in FolderAction::ALL.into_iter().skip(visible) {
                    self.folder_action(ui, action, true);
                }
                ui.separator();
                if self
                    .menu_item(
                        ui,
                        Icon::CopyPath,
                        crate::i18n::tr("Copiar caminhos selecionados"),
                        "",
                        !self.selected.is_empty(),
                        false,
                    )
                    .clicked()
                {
                    self.file_clipboard = None;
                    ui.ctx().copy_text(self.selected_paths_text());
                    ui.close();
                }
            });
            icons::paint(
                ui.painter(),
                egui::Rect::from_center_size(
                    response.rect.center(),
                    Vec2::splat(self.appearance.size),
                ),
                Icon::More,
                ui.visuals().text_color(),
                &self.appearance,
            );
            response.widget_info(|| {
                egui::WidgetInfo::labeled(
                    egui::WidgetType::Button,
                    true,
                    crate::i18n::tr("Mais ações da pasta"),
                )
            });
            response.on_hover_text(crate::i18n::tr("Mais ações da pasta"));
        }
    }

    fn folder_action(&mut self, ui: &mut egui::Ui, action: FolderAction, menu: bool) {
        let enabled = !self.loading
            && !self.current.as_os_str().is_empty()
            && (!self.is_applications()
                || matches!(action, FolderAction::ZoomOut | FolderAction::ZoomIn))
            && match action {
                FolderAction::NewFolder | FolderAction::Hidden if self.is_trash() => false,
                FolderAction::Copy | FolderAction::Cut => {
                    !self.is_trash() && !self.selected.is_empty()
                }
                FolderAction::Paste => self.can_paste(),
                FolderAction::ZoomOut => self.item_scale > 0.75,
                FolderAction::ZoomIn => self.item_scale < 2.0,
                _ => true,
            };
        let selected = match action {
            FolderAction::Hidden => self.hidden_visible(),
            FolderAction::Details => self.view == View::Details,
            FolderAction::List => self.view == View::List,
            FolderAction::Icons => self.view == View::Icons,
            _ => false,
        };
        let response = ui
            .add_enabled_ui(enabled, |ui| {
                if menu {
                    self.menu_item(
                        ui,
                        action.icon(),
                        action.label(),
                        action.shortcut(),
                        true,
                        selected,
                    )
                } else {
                    let label = if action.shortcut().is_empty() {
                        action.label().to_owned()
                    } else {
                        format!("{} · {}", action.label(), action.shortcut())
                    };
                    self.icon_toggle(ui, action.icon(), &label, selected)
                }
            })
            .inner;
        if matches!(action, FolderAction::ZoomIn | FolderAction::ZoomOut) {
            response.clone().on_hover_text(crate::i18n::message(
                "Tamanho dos itens: {}%",
                &[((self.item_scale * 100.0) as u32).to_string()],
            ));
        }
        if response.clicked() {
            match action {
                FolderAction::NewFolder => {
                    self.error = None;
                    self.new_folder = Some(String::new());
                }
                FolderAction::Copy => self.capture_files(file_ops::Mode::Copy, ui.ctx()),
                FolderAction::Cut => self.capture_files(file_ops::Mode::Cut, ui.ctx()),
                FolderAction::Paste => self.paste_files(ui.ctx()),
                FolderAction::Hidden => self.toggle_hidden(),
                FolderAction::Details => self.view = View::Details,
                FolderAction::List => self.view = View::List,
                FolderAction::Icons => self.view = View::Icons,
                FolderAction::ZoomOut => self.zoom_items(-0.25),
                FolderAction::ZoomIn => self.zoom_items(0.25),
            }
            if menu {
                ui.close();
            }
        }
    }
}
