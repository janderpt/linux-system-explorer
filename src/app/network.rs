use super::*;
use std::io;
#[cfg(test)]
mod tests;

pub(super) struct ConnectDialog {
    address: String,
    label: String,
    focus: bool,
    error: Option<String>,
    result: Option<Receiver<io::Result<PathBuf>>>,
    browse_result: Option<Receiver<io::Result<Vec<crate::network::ServerShare>>>>,
    shares: Option<(String, Vec<crate::network::ServerShare>)>,
    selected_share: Option<usize>,
    url: Option<String>,
    ready: Option<PathBuf>,
    cancel: std::sync::Arc<crate::network::Cancellation>,
}
impl ConnectDialog {
    fn new(address: String, label: String) -> Self {
        Self {
            address,
            label,
            focus: true,
            error: None,
            result: None,
            browse_result: None,
            shares: None,
            selected_share: None,
            url: None,
            ready: None,
            cancel: Default::default(),
        }
    }
}
impl Drop for ConnectDialog {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}
impl Explorer {
    pub(super) fn network_location(
        &self,
        path: &Path,
    ) -> Option<crate::network::location::Location> {
        crate::network::location::resolve(path, &self.sidebar_options.remote)
    }

    pub(super) fn display_address(&self, path: &Path) -> String {
        if crate::trashbin::is_location(path) {
            "trash:///".into()
        } else {
            self.network_location(path)
                .map(|l| l.address)
                .unwrap_or_else(|| path.display().to_string())
        }
    }

    pub(super) fn display_title(&self, path: &Path) -> String {
        if crate::trashbin::is_location(path) {
            crate::i18n::tr("Lixeira").into()
        } else {
            self.network_location(path)
                .map(|l| l.title)
                .unwrap_or_else(|| {
                    path.file_name()
                        .unwrap_or(path.as_os_str())
                        .to_string_lossy()
                        .into_owned()
                })
        }
    }

    pub(super) fn navigate_up(&mut self, ctx: &egui::Context) {
        if self.is_trash() {
            return;
        }
        if let Some(mut location) = self.network_location(&self.current) {
            location.crumbs.pop();
            if let Some(parent) = location.crumbs.pop() {
                if let Some(path) = parent.local {
                    self.navigate(path, Navigation::Visit);
                } else {
                    self.request_network(
                        Some(crate::network::Connection {
                            url: parent.address,
                            label: String::new(),
                            mount: None,
                        }),
                        ctx,
                    );
                }
            }
        } else if let Some(path) = self.current.parent() {
            self.navigate(path.to_owned(), Navigation::Visit);
        }
    }

    pub(super) fn submit_address(&mut self, ctx: &egui::Context) {
        let input = self.address.trim();
        let remote = input.starts_with("\\\\")
            || input.starts_with("//")
            || ["smb://", "sftp://", "nfs://", "ftp://", "dav://", "davs://"]
                .iter()
                .any(|prefix| input.to_ascii_lowercase().starts_with(prefix));
        if remote {
            if input == self.display_address(self.location()) {
                let path = self.location().to_owned();
                self.cancel_address_edit(ctx);
                self.navigate(path, Navigation::Visit);
                return;
            }
            let connection = crate::network::Connection {
                url: input.into(),
                label: String::new(),
                mount: None,
            };
            self.cancel_address_edit(ctx);
            self.request_network(Some(connection), ctx);
        } else {
            self.navigate(self.expand_address(), Navigation::Visit);
        }
    }
    pub(super) fn request_network(
        &mut self,
        connection: Option<crate::network::Connection>,
        ctx: &egui::Context,
    ) {
        let mut dialog = match connection {
            Some(remote) => ConnectDialog::new(remote.url, remote.label),
            None => ConnectDialog::new(String::new(), String::new()),
        };
        if !dialog.address.is_empty() {
            Self::start_network_connection(&mut dialog, ctx);
        }
        self.network_dialog = Some(dialog);
    }
    fn start_network_connection(dialog: &mut ConnectDialog, ctx: &egui::Context) {
        dialog.error = None;
        let result = if crate::network::is_local_input(&dialog.address) {
            Ok(None)
        } else {
            crate::network::remote_url(&dialog.address).map(Some)
        };
        let mut url = match result {
            Ok(result) => result,
            Err(error) => {
                dialog.error = Some(error.to_string());
                return;
            }
        };
        if let Some(server) = url
            .as_ref()
            .filter(|uri| crate::network::is_smb_server(uri))
            .cloned()
        {
            let selected = dialog
                .shares
                .as_ref()
                .filter(|(address, _)| address == &server)
                .and_then(|(_, shares)| dialog.selected_share.and_then(|i| shares.get(i)))
                .cloned();
            if let Some(share) = selected {
                url = Some(share.url);
            } else {
                let (sender, receiver) = mpsc::channel();
                let context = ctx.clone();
                let locale = crate::i18n::current();
                dialog.cancel = Default::default();
                let cancel = dialog.cancel.clone();
                dialog.shares = None;
                dialog.selected_share = None;
                dialog.url = Some(server.clone());
                thread::spawn(move || {
                    crate::i18n::set_locale(locale);
                    let _ = sender.send(crate::network::browse_server(&server, &cancel));
                    context.request_repaint();
                });
                dialog.browse_result = Some(receiver);
                return;
            }
        }
        dialog.url = url.clone();
        let input = dialog.address.clone();
        let (sender, receiver) = mpsc::channel();
        let context = ctx.clone();
        let locale = crate::i18n::current();
        dialog.cancel = Default::default();
        let cancel = dialog.cancel.clone();
        thread::spawn(move || {
            crate::i18n::set_locale(locale);
            let result = if let Some(url) = url {
                crate::network::connect_cancellable(&url, &cancel)
            } else {
                crate::network::mounted_input(&input).and_then(|path| {
                    path.ok_or_else(|| {
                        io::Error::other(crate::i18n::tr(
                            "Escolha um ponto de montagem que seja uma pasta existente.",
                        ))
                    })
                })
            };
            let _ = sender.send(result);
            context.request_repaint();
        });
        dialog.result = Some(receiver);
    }
    fn finish_network_connection(
        &mut self,
        url: Option<String>,
        label: String,
        path: PathBuf,
    ) -> io::Result<()> {
        let mut updated = self.sidebar_options.clone();
        if let Some(url) = url {
            if let Some(remote) = updated.remote.iter_mut().find(|r| r.url == url) {
                remote.mount = Some(path.clone());
                if !label.trim().is_empty() {
                    remote.label = label.trim().into();
                }
            } else {
                updated.remote.push(crate::network::Connection {
                    url,
                    label: label.trim().into(),
                    mount: Some(path.clone()),
                });
            }
        } else if let Some(shortcut) = updated.network.iter_mut().find(|s| s.path == path) {
            if !label.trim().is_empty() {
                shortcut.label = label.trim().into();
            }
        } else {
            updated.network.push(locations::Shortcut {
                path: path.clone(),
                label: label.trim().into(),
            });
        }
        updated.hidden_network.remove(&settings::folder_key(&path));
        updated.show_network = true;
        let mut prefs = self.stored_preferences(&self.preferences());
        prefs.sidebar = updated.clone();
        if let Some(file) = &self.settings_path {
            prefs.save(file)?;
        }
        self.sidebar_options = updated;
        self.last_attempted_preferences = prefs;
        self.navigate(path, Navigation::Visit);
        self.post_notice(crate::i18n::tr("Compartilhamento adicionado à Rede.").into());
        Ok(())
    }
    pub(super) fn network_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut dialog) = self.network_dialog.take() else {
            return;
        };
        if let Some(receiver) = &dialog.browse_result {
            let result = match receiver.try_recv() {
                Ok(result) => Some(result),
                Err(mpsc::TryRecvError::Disconnected) => Some(Err(io::Error::other(
                    crate::i18n::tr("A listagem de compartilhamentos foi interrompida."),
                ))),
                Err(mpsc::TryRecvError::Empty) => None,
            };
            if let Some(result) = result {
                dialog.browse_result = None;
                match result {
                    Ok(shares) => {
                        dialog.shares = Some((dialog.url.clone().unwrap_or_default(), shares));
                        dialog.selected_share = None;
                    }
                    Err(error) => dialog.error = Some(error.to_string()),
                }
            }
        }
        if let Some(receiver) = &dialog.result {
            match receiver.try_recv() {
                Ok(Ok(path)) => {
                    dialog.ready = Some(path);
                    dialog.result = None;
                }
                Ok(Err(error)) => {
                    dialog.error = Some(error.to_string());
                    dialog.result = None;
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    dialog.error =
                        Some(crate::i18n::tr("A conexão de rede foi interrompida.").into());
                    dialog.result = None;
                }
                Err(mpsc::TryRecvError::Empty) => (),
            }
        }
        if !self.loading
            && let Some(path) = dialog.ready.take()
        {
            match self.finish_network_connection(dialog.url.clone(), dialog.label.clone(), path) {
                Ok(()) => return,
                Err(error) => dialog.error = Some(error.to_string()),
            }
        }
        let mut connect = false;
        let mut cancel = false;
        let mut refresh = false;
        let server = crate::network::remote_url(&dialog.address)
            .ok()
            .is_some_and(|uri| crate::network::is_smb_server(&uri));
        let busy =
            dialog.result.is_some() || dialog.ready.is_some() || dialog.browse_result.is_some();
        let modal = egui::Modal::new(egui::Id::new("network_connection_dialog")).show(ctx, |ui| {
            ui.set_width((ctx.content_rect().width() - 100.0).clamp(340.0, 540.0));
            ui.spacing_mut().interact_size.y = 32.0;
            ui.heading(crate::i18n::tr("Adicionar compartilhamento de rede"));
            ui.label(crate::i18n::tr(
                "Endereço do compartilhamento ou pasta já montada:",
            ));
            let response = ui.add_enabled(
                !busy,
                egui::TextEdit::singleline(&mut dialog.address)
                    .desired_width(f32::INFINITY)
                    .vertical_align(Align::Center)
                    .hint_text(crate::i18n::tr(
                        "smb://servidor/ ou smb://servidor/compartilhamento",
                    )),
            );
            if response.changed() {
                dialog.shares = None;
                dialog.selected_share = None;
            }
            if dialog.focus {
                response.request_focus();
                dialog.focus = false;
            }
            ui.add_enabled(
                !busy,
                egui::TextEdit::singleline(&mut dialog.label)
                    .desired_width(f32::INFINITY)
                    .vertical_align(Align::Center)
                    .hint_text(crate::i18n::tr("Nome do compartilhamento (opcional)")),
            );
            ui.weak(crate::i18n::tr(
                "Use SMB, SFTP, NFS, FTP, WebDAV ou uma pasta já montada.",
            ));
            ui.weak(crate::i18n::tr(
                "Para listar os compartilhamentos, informe apenas smb://servidor/.",
            ));
            ui.weak(crate::i18n::tr(
                "O sistema solicitará a autenticação. Senhas não são salvas pelo explorador.",
            ));
            if let Some((_, shares)) = &dialog.shares {
                ui.label(crate::i18n::tr("Compartilhamentos disponíveis"));
                if shares.is_empty() {
                    ui.weak(crate::i18n::tr(
                        "O servidor não retornou compartilhamentos disponíveis para esta conta.",
                    ));
                }
                egui::ScrollArea::vertical()
                    .id_salt("server_share_choices")
                    .auto_shrink([false, true])
                    .max_height((ctx.content_rect().height() - 440.0).clamp(80.0, 180.0))
                    .show(ui, |ui| {
                        ui.add_enabled_ui(!busy, |ui| {
                            for (i, share) in shares.iter().enumerate() {
                                if ui
                                    .horizontal(|ui| {
                                        ui.add(
                                            egui::Button::selectable(
                                                dialog.selected_share == Some(i),
                                                (share.name.as_str(), egui::Atom::grow()),
                                            )
                                            .min_size(Vec2::new(ui.available_width(), 32.0))
                                            .truncate(),
                                        )
                                    })
                                    .inner
                                    .on_hover_text(&share.url)
                                    .clicked()
                                {
                                    dialog.selected_share = Some(i);
                                }
                            }
                        });
                    });
                refresh = ui
                    .add_enabled(
                        !busy,
                        egui::Button::new(crate::i18n::tr("Atualizar lista de compartilhamentos")),
                    )
                    .clicked();
            }
            if busy {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(if dialog.browse_result.is_some() {
                        crate::i18n::tr("Listando compartilhamentos…")
                    } else {
                        crate::i18n::tr("Conectando ao compartilhamento…")
                    });
                });
            }
            if let Some(error) = &dialog.error {
                ui.colored_label(ui.visuals().error_fg_color, error);
            }
            ui.separator();
            ui.horizontal(|ui| {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let action = if server && dialog.shares.is_none() {
                        crate::i18n::tr("Listar compartilhamentos")
                    } else {
                        crate::i18n::tr("Conectar e adicionar")
                    };
                    let selected =
                        !server || dialog.shares.is_none() || dialog.selected_share.is_some();
                    connect = ui
                        .add_enabled(
                            !busy && !dialog.address.trim().is_empty() && selected,
                            egui::Button::new(action),
                        )
                        .clicked();
                    cancel = ui.button(crate::i18n::tr("Cancelar")).clicked();
                });
            });
        });
        if modal.should_close() || cancel {
            return;
        }
        if refresh {
            dialog.shares = None;
            dialog.selected_share = None;
            connect = true;
        }
        if connect {
            Self::start_network_connection(&mut dialog, ctx);
        }
        self.network_dialog = Some(dialog);
    }

    pub(super) fn configure_network_sharing(&mut self, path: PathBuf, ctx: &egui::Context) {
        self.request_local_shares(Some(path), ctx);
    }

    pub(super) fn configure_network_sharing_system(&mut self, path: PathBuf, ctx: &egui::Context) {
        self.post_typed_notice(toast::NoticeKind::Warning,crate::i18n::tr("Na janela do sistema, use a aba Compartilhar para configurar o Samba. O sistema pode solicitar instalação ou permissões de administrador.").into());
        let context = ctx.clone();
        let sender = self.event_sender.clone();
        let locale = crate::i18n::current();
        thread::spawn(move || {
            crate::i18n::set_locale(locale);
            if let Err(error) = crate::network::configure_sharing(&path) {
                let _ = sender.send(Event::OpenFailed(error.to_string()));
                context.request_repaint();
            }
        });
    }
}
