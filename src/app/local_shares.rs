use super::*;
use crate::local_shares::{Access, Rule, Share};
use std::io;

#[cfg(test)]
mod tests;

type Loaded = (Vec<Share>, Vec<String>);

pub(super) struct Manager {
    shares: Vec<Share>,
    hosts: Vec<String>,
    host: usize,
    original: Option<Share>,
    draft: Option<Share>,
    requested_folder: Option<PathBuf>,
    pending: Option<Receiver<io::Result<Loaded>>>,
    success: Option<&'static str>,
    error: Option<String>,
    confirm_stop: bool,
}
impl Manager {
    fn new(folder: Option<PathBuf>) -> Self {
        Self {
            shares: vec![],
            hosts: vec!["localhost".into()],
            host: 0,
            original: None,
            draft: None,
            requested_folder: folder,
            pending: None,
            success: None,
            error: None,
            confirm_stop: false,
        }
    }
    fn select(&mut self, share: Share) {
        self.original = Some(share.clone());
        self.draft = Some(share);
        self.confirm_stop = false;
        self.error = None;
    }
    fn start(
        &mut self,
        ctx: &egui::Context,
        task: impl FnOnce() -> io::Result<Vec<Share>> + Send + 'static,
    ) {
        let (sender, receiver) = mpsc::channel();
        let ctx = ctx.clone();
        let locale = crate::i18n::current();
        thread::spawn(move || {
            crate::i18n::set_locale(locale);
            let result = task().map(|shares| (shares, crate::local_shares::hosts()));
            let _ = sender.send(result);
            ctx.request_repaint();
        });
        self.pending = Some(receiver);
        self.error = None;
    }
    fn refresh(&mut self, ctx: &egui::Context) {
        self.start(ctx, || crate::local_shares::Service::default().list());
    }
    fn accept(&mut self, shares: Vec<Share>, hosts: Vec<String>) {
        let selected = self.draft.as_ref().map(|s| s.name.clone());
        let selected_host = self.hosts.get(self.host).cloned();
        self.shares = shares;
        self.hosts = hosts;
        self.host = selected_host
            .and_then(|host| self.hosts.iter().position(|h| h == &host))
            .unwrap_or(0);
        if let Some(folder) = self.requested_folder.take() {
            if let Some(share) = self.shares.iter().find(|s| s.path == folder).cloned() {
                self.select(share);
            } else {
                self.original = None;
                self.draft = Some(Share::new(folder));
            }
        } else if let Some(share) = selected
            .and_then(|name| self.shares.iter().find(|s| s.name == name))
            .cloned()
            .or_else(|| self.shares.first().cloned())
        {
            self.select(share);
        } else {
            self.original = None;
            self.draft = None;
        }
    }
}
impl Explorer {
    pub(super) fn request_local_shares(&mut self, folder: Option<PathBuf>, ctx: &egui::Context) {
        let mut manager = Manager::new(folder);
        manager.refresh(ctx);
        self.local_shares = Some(manager);
    }
    pub(super) fn poll_local_shares(&mut self) {
        let Some(mut manager) = self.local_shares.take() else {
            return;
        };
        if let Some(receiver) = &manager.pending {
            let result = match receiver.try_recv() {
                Ok(result) => Some(result),
                Err(mpsc::TryRecvError::Disconnected) => Some(Err(io::Error::other(
                    crate::i18n::tr("A operação de compartilhamento foi interrompida."),
                ))),
                Err(mpsc::TryRecvError::Empty) => None,
            };
            if let Some(result) = result {
                manager.pending = None;
                match result {
                    Ok((shares, hosts)) => {
                        manager.accept(shares, hosts);
                        if let Some(notice) = manager.success.take() {
                            self.post_notice(crate::i18n::tr(notice).into());
                        }
                    }
                    Err(error) => {
                        manager.success = None;
                        manager.error = Some(error.to_string());
                    }
                }
            }
        }
        self.local_shares = Some(manager);
    }
    pub(super) fn local_shares_pending(&self) -> bool {
        self.local_shares
            .as_ref()
            .is_some_and(|manager| manager.pending.is_some())
    }
    pub(super) fn local_shares_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut manager) = self.local_shares.take() else {
            return;
        };
        let busy = manager.pending.is_some();
        let mut close = false;
        let mut refresh = false;
        let mut save = false;
        let mut stop = false;
        let mut system = false;
        let body_height = (ctx.content_rect().height() - 270.0).clamp(160.0, 400.0);
        let modal = egui::Modal::new(egui::Id::new("local_shares_manager")).show(ctx, |ui| {
            ui.set_width((ctx.content_rect().width() - 60.0).clamp(620.0, 880.0));
            ui.spacing_mut().interact_size.y = 30.0;
            ui.heading(crate::i18n::tr("Compartilhamentos locais"));
            ui.weak(crate::i18n::tr("Compartilhamentos Samba de usuários. Apenas os seus podem ser alterados aqui."));
            ui.separator();
            ui.add_enabled_ui(!busy, |ui| {
                ui.horizontal_top(|ui| {
                    ui.allocate_ui_with_layout(Vec2::new(225.0, body_height), Layout::top_down(Align::Min), |ui| {
                        egui::ScrollArea::vertical().id_salt("local_share_list").max_height(body_height).min_scrolled_height(body_height).auto_shrink([false,false]).show(ui, |ui| {
                            if manager.shares.is_empty() { ui.weak(crate::i18n::tr("Nenhuma pasta compartilhada pelo usuário.")); }
                            let mut selected = None;
                            for share in &manager.shares {
                                let active = manager.original.as_ref().is_some_and(|s| s.name == share.name);
                                let response = ui.add(egui::Button::selectable(active, &share.name).truncate()).on_hover_text(share.path.display().to_string());
                                if response.clicked() { selected = Some(share.clone()); }
                                if !share.editable { ui.weak(crate::i18n::tr("Outro proprietário")); }
                            }
                            if let Some(share) = selected { manager.select(share); }
                        });
                    });
                    ui.add_space(12.0);
                    let editable = manager.draft.as_ref().is_some_and(|s| s.editable);
                    ui.allocate_ui_with_layout(Vec2::new(ui.available_width(), body_height), Layout::top_down(Align::Min), |ui| {
                    egui::ScrollArea::vertical().id_salt("local_share_editor").max_height(body_height).min_scrolled_height(body_height).auto_shrink([false,false]).show(ui, |ui| {
                        ui.set_min_width(ui.available_width());
                        if let Some(draft) = &mut manager.draft {
                            ui.label(crate::i18n::tr("Pasta"));
                            ui.label(draft.path.display().to_string());
                            ui.add_enabled_ui(editable, |ui| {
                                ui.label(crate::i18n::tr("Nome do compartilhamento"));
                                ui.add(egui::TextEdit::singleline(&mut draft.name).desired_width(f32::INFINITY).vertical_align(Align::Center));
                                ui.label(crate::i18n::tr("Descrição"));
                                ui.add(egui::TextEdit::singleline(&mut draft.comment).desired_width(f32::INFINITY).vertical_align(Align::Center));
                                ui.add_space(5.0);
                                ui.label(crate::i18n::tr("Usuários e permissões"));
                                let mut remove = None;
                                for (i, rule) in draft.rules.iter_mut().enumerate() {
                                    ui.push_id(i, |ui| { ui.horizontal(|ui| {
                                        ui.add(egui::TextEdit::singleline(&mut rule.principal).desired_width((ui.available_width()-205.0).max(100.0)).hint_text(crate::i18n::tr("Usuário ou grupo")).vertical_align(Align::Center));
                                        egui::ComboBox::from_id_salt("permission").width(135.0).selected_text(rule.access.label()).show_ui(ui, |ui| {
                                            for access in Access::ALL { ui.selectable_value(&mut rule.access, access, access.label()); }
                                        });
                                        if ui.button("×").on_hover_text(crate::i18n::tr("Remover permissão")).clicked() { remove = Some(i); }
                                    }); });
                                }
                                if let Some(i) = remove { draft.rules.remove(i); }
                                if ui.button(crate::i18n::tr("Adicionar usuário ou grupo")).clicked() { draft.rules.push(Rule { principal: String::new(), access: Access::Read }); }
                            });
                            ui.weak(crate::i18n::tr("Use nomes como servidor\\usuário ou Everyone. As permissões da pasta no Linux também se aplicam."));
                            ui.weak(if draft.guests { crate::i18n::tr("Convidados: permitido pela configuração existente.") } else { crate::i18n::tr("Convidados: desativado. O acesso exige uma conta Samba.") });
                            if let Some(original) = &manager.original {
                                ui.separator();
                                ui.horizontal(|ui| {
                                    ui.label(crate::i18n::tr("Endereço de acesso"));
                                    egui::ComboBox::from_id_salt("local_share_host").selected_text(&manager.hosts[manager.host]).show_ui(ui, |ui| {
                                        for (i, host) in manager.hosts.iter().enumerate() { ui.selectable_value(&mut manager.host, i, host); }
                                    });
                                });
                                let host = &manager.hosts[manager.host];
                                let address = original.address(host);
                                ui.label(&address);
                                ui.horizontal(|ui| {
                                    if ui.button(crate::i18n::tr("Copiar endereço SMB")).clicked() { ctx.copy_text(address); }
                                    if ui.button(crate::i18n::tr("Copiar endereço Windows")).clicked() { ctx.copy_text(original.windows_address(host)); }
                                });
                            }
                        } else { ui.label(crate::i18n::tr("Selecione um compartilhamento ou use Compartilhar pela rede no menu de uma pasta.")); }
                    });
                    });
                });
            });
            if busy { ui.horizontal(|ui| { ui.spinner(); ui.label(crate::i18n::tr("Atualizando compartilhamentos…")); }); }
            if let Some(error) = &manager.error { egui::ScrollArea::vertical().id_salt("local_share_error").max_height(60.0).show(ui, |ui| { ui.colored_label(ui.visuals().error_fg_color, error); }); }
            if manager.confirm_stop {
                ui.separator();
                ui.label(crate::i18n::tr("Parar de compartilhar esta pasta?"));
                ui.weak(crate::i18n::tr("A pasta e os arquivos serão mantidos. Conexões já abertas podem continuar até serem encerradas."));
                ui.horizontal(|ui| { ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    stop = ui.add_enabled(!busy, egui::Button::new(crate::i18n::tr("Confirmar remoção do compartilhamento"))).clicked();
                    if ui.add_enabled(!busy, egui::Button::new(crate::i18n::tr("Cancelar"))).clicked() { manager.confirm_stop = false; }
                }); });
            }
            ui.separator();
            ui.horizontal(|ui| {
                refresh = ui.add_enabled(!busy, egui::Button::new(crate::i18n::tr("Atualizar"))).clicked();
                system = ui.add_enabled(!busy, egui::Button::new(crate::i18n::tr("Configurar pelo sistema"))).clicked();
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    close = ui.add_enabled(!busy, egui::Button::new(crate::i18n::tr("Fechar"))).clicked();
                    let editable = manager.draft.as_ref().is_some_and(|s| s.editable);
                    save = ui.add_enabled(!busy && editable && !manager.confirm_stop, egui::Button::new(crate::i18n::tr("Salvar compartilhamento"))).clicked();
                    if ui.add_enabled(!busy && editable && manager.original.is_some() && !manager.confirm_stop, egui::Button::new(crate::i18n::tr("Parar de compartilhar"))).clicked() { manager.confirm_stop = true; manager.error = None; }
                });
            });
        });
        if !busy && (close || modal.should_close()) {
            return;
        }
        if refresh {
            manager.requested_folder = manager.draft.as_ref().map(|s| s.path.clone());
            manager.refresh(ctx);
        }
        if save && let Some(draft) = manager.draft.clone() {
            if let Err(error) = draft.validate() {
                manager.error = Some(error.to_string());
            } else {
                let original = manager.original.clone();
                manager.success = Some("Compartilhamento salvo.");
                manager.start(ctx, move || {
                    crate::local_shares::Service::default().save(original.as_ref(), &draft)
                });
            }
        }
        if stop && let Some(original) = manager.original.clone() {
            manager.confirm_stop = false;
            manager.success = Some("A pasta deixou de ser compartilhada.");
            manager.start(ctx, move || {
                crate::local_shares::Service::default().stop(&original)
            });
        }
        if system {
            let path = manager
                .draft
                .as_ref()
                .map(|s| s.path.clone())
                .unwrap_or_else(|| self.current.clone());
            self.configure_network_sharing_system(path, ctx);
        }
        self.local_shares = Some(manager);
    }
}
