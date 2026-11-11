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
        let url = match result {
            Ok(result) => result,
            Err(error) => {
                dialog.error = Some(error.to_string());
                return;
            }
        };
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
        let busy = dialog.result.is_some() || dialog.ready.is_some();
        let modal=egui::Modal::new(egui::Id::new("network_connection_dialog")).show(ctx,|ui| {
            ui.set_width((ctx.content_rect().width()-100.0).clamp(340.0,540.0));
            ui.spacing_mut().interact_size.y=32.0;
            ui.heading(crate::i18n::tr("Adicionar compartilhamento de rede"));
            ui.label(crate::i18n::tr("Endereço do compartilhamento ou pasta já montada:"));
            let response=ui.add_enabled(!busy,egui::TextEdit::singleline(&mut dialog.address).desired_width(f32::INFINITY).vertical_align(Align::Center).hint_text("smb://servidor/compartilhamento"));
            if dialog.focus {response.request_focus();dialog.focus=false;}
            ui.add_enabled(!busy,egui::TextEdit::singleline(&mut dialog.label).desired_width(f32::INFINITY).vertical_align(Align::Center).hint_text(crate::i18n::tr("Nome do compartilhamento (opcional)")));
            ui.weak(crate::i18n::tr("Use SMB, SFTP, NFS, FTP, WebDAV ou uma pasta já montada. O explorador usa GVfs no GNOME e KIO Fuse no KDE."));
            ui.weak(crate::i18n::tr("O sistema solicitará a autenticação. Senhas não são salvas pelo explorador."));
            if busy {ui.horizontal(|ui|{ui.spinner();ui.label(crate::i18n::tr("Conectando ao compartilhamento…"));});}
            if let Some(error)=&dialog.error {ui.colored_label(ui.visuals().error_fg_color,error);}
            ui.separator();
            ui.with_layout(Layout::right_to_left(Align::Center),|ui| {
                connect=ui.add_enabled(!busy&&!dialog.address.trim().is_empty(),egui::Button::new(crate::i18n::tr("Conectar e adicionar"))).clicked();
                cancel=ui.button(crate::i18n::tr("Cancelar")).clicked();
            });
        });
        if modal.should_close() || cancel {
            return;
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
