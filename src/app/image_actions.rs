use super::*;
use crate::image_ops::{self, Format, Info, Operation, Rotation};
use std::{
    io,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
#[derive(Clone, Copy)]
pub(super) enum Action {
    Rotate,
    Resize,
    Convert,
}
impl Action {
    fn title(self) -> &'static str {
        crate::i18n::tr(match self {
            Self::Rotate => "Girar imagem…",
            Self::Resize => "Redimensionar imagem…",
            Self::Convert => "Converter imagem…",
        })
    }
}
pub(super) struct Editor {
    entries: Vec<Entry>,
    action: Action,
    info: Option<Info>,
    probe: Receiver<io::Result<Info>>,
    rotation: Rotation,
    width: u32,
    height: u32,
    proportional: bool,
    format: Option<Format>,
    quality: u8,
    running: Option<Arc<AtomicBool>>,
    pub done: usize,
    pub total: usize,
}
impl Editor {
    fn operation(&self) -> Operation {
        match self.action {
            Action::Rotate => Operation::Rotate(self.rotation),
            Action::Resize => Operation::Resize {
                width: self.width,
                height: self.height,
                proportional: self.proportional,
            },
            Action::Convert => Operation::Convert,
        }
    }
    fn output(&self) -> Format {
        self.format
            .unwrap_or_else(|| self.info.map_or(Format::Png, |i| i.format))
    }
}
impl Explorer {
    pub(super) fn image_pending(&self) -> bool {
        self.image_editor
            .as_ref()
            .is_some_and(|e| e.running.is_some())
    }
    pub(super) fn cancel_images(&self) {
        if let Some(cancel) = self.image_editor.as_ref().and_then(|e| e.running.as_ref()) {
            cancel.store(true, Ordering::Release);
        }
    }
    pub(super) fn image_actions_menu(&mut self, ui: &mut egui::Ui, entry: &Entry) {
        let entries = self.selected_images(entry);
        if entries.is_empty() {
            return;
        }
        ui.separator();
        ui.label(RichText::new(crate::i18n::tr("Imagem")).small().weak());
        let enabled = !self.loading
            && !self.has_dialog()
            && self.active_transfer.is_none()
            && self.active_deletion.is_none()
            && self.active_restore.is_none();
        for (action, icon, label) in [
            (Action::Rotate, Icon::RotateImage, "Girar…"),
            (Action::Resize, Icon::ResizeImage, "Redimensionar…"),
            (Action::Convert, Icon::ConvertImage, "Converter…"),
        ] {
            if self
                .menu_item(ui, icon, crate::i18n::tr(label), "", enabled, false)
                .clicked()
            {
                self.request_image_action(entries.clone(), action, ui.ctx());
                ui.close();
            }
        }
        ui.separator();
    }
    pub(super) fn request_image_action(
        &mut self,
        entries: Vec<Entry>,
        action: Action,
        ctx: &egui::Context,
    ) {
        if entries.is_empty() || self.has_dialog() || self.is_trash() || self.is_applications() {
            return;
        }
        let first = entries[0].clone();
        let (sender, probe) = mpsc::channel();
        let context = ctx.clone();
        let locale = crate::i18n::current();
        thread::spawn(move || {
            crate::i18n::set_locale(locale);
            let _ = sender.send(image_ops::inspect(&first));
            context.request_repaint();
        });
        self.image_editor = Some(Editor {
            total: entries.len(),
            entries,
            action,
            info: None,
            probe,
            rotation: Rotation::Right,
            width: 1,
            height: 1,
            proportional: true,
            format: matches!(action, Action::Convert).then_some(Format::Png),
            quality: 90,
            running: None,
            done: 0,
        });
    }
    pub(super) fn image_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut editor) = self.image_editor.take() else {
            return;
        };
        if editor.info.is_none() {
            match editor.probe.try_recv() {
                Ok(Ok(info)) => {
                    editor.width = info.width;
                    editor.height = info.height;
                    editor.info = Some(info);
                }
                Ok(Err(error)) => {
                    self.report_error(error.to_string());
                    return;
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.report_error(crate::i18n::tr("O serviço de imagem foi encerrado.").into());
                    return;
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        let busy = editor.running.is_some();
        let mut create = false;
        let mut cancel = false;
        let modal=egui::Modal::new(egui::Id::new((self.tab_id,"image_operation"))).show(ctx,|ui|{
            let width=(ctx.content_rect().width()-70.0).clamp(300.0,540.0);ui.set_width(width);ui.heading(editor.action.title());
            ui.allocate_ui_with_layout(Vec2::new(width,265.0),Layout::top_down(Align::Min),|ui|{
                ui.label(crate::i18n::plural("{} imagem selecionada","{} imagens selecionadas",editor.entries.len()));
                ui.label(crate::i18n::tr("Uma nova cópia será criada na mesma pasta. Os originais serão preservados."));
                if let Some(info)=editor.info {
                    ui.label(crate::i18n::message("Imagem de referência: {} × {} px",&[info.width.to_string(),info.height.to_string()]));
                    ui.add_enabled_ui(!busy,|ui|{
                        match editor.action {
                            Action::Rotate=>{egui::ComboBox::from_id_salt("image_rotation").selected_text(rotation_label(editor.rotation)).show_ui(ui,|ui|{for rotation in [Rotation::Right,Rotation::Left,Rotation::Half]{ui.selectable_value(&mut editor.rotation,rotation,rotation_label(rotation));}});},
                            Action::Resize=>{
                                ui.horizontal(|ui|{
                                    ui.label(crate::i18n::tr("Largura (px)"));let w=ui.add(egui::DragValue::new(&mut editor.width).range(1..=image_ops::MAX_EDGE)).changed();
                                    if w&&editor.proportional{editor.height=((f64::from(editor.width)*f64::from(info.height)/f64::from(info.width)).round() as u32).clamp(1,image_ops::MAX_EDGE);}
                                    ui.label(crate::i18n::tr("Altura (px)"));let h=ui.add(egui::DragValue::new(&mut editor.height).range(1..=image_ops::MAX_EDGE)).changed();
                                    if h&&editor.proportional{editor.width=((f64::from(editor.height)*f64::from(info.width)/f64::from(info.height)).round() as u32).clamp(1,image_ops::MAX_EDGE);}
                                });
                                ui.checkbox(&mut editor.proportional,crate::i18n::tr("Manter proporção (encaixar nos limites)"));
                            },Action::Convert=>{},
                        }
                        ui.horizontal(|ui|{
                            ui.label(crate::i18n::tr("Formato da cópia"));
                            egui::ComboBox::from_id_salt("image_output_format").selected_text(editor.format.map_or_else(||crate::i18n::tr("Manter formato").to_owned(),|f|f.label().to_owned())).show_ui(ui,|ui|{
                                if !matches!(editor.action,Action::Convert){ui.selectable_value(&mut editor.format,None,crate::i18n::tr("Manter formato"));}
                                for format in Format::ALL {ui.selectable_value(&mut editor.format,Some(format),format.label());}
                            });
                        });
                        if editor.output()==Format::Jpeg {
                            ui.horizontal(|ui|{ui.label(crate::i18n::tr("Qualidade JPEG"));ui.add(egui::Slider::new(&mut editor.quality,1..=100));});
                            ui.label(crate::i18n::tr("No JPEG, áreas transparentes serão preenchidas com branco."));
                        }
                        ui.label(crate::i18n::message("Nova cópia: {}",&[image_ops::suggested_name(&editor.entries[0],editor.operation(),editor.output(),1).to_string_lossy().into_owned()]));
                        ui.label(RichText::new(crate::i18n::tr("Se o nome já existir, a cópia receberá uma numeração.")).small().weak());
                    });
                }else{ui.horizontal(|ui|{ui.spinner();ui.label(crate::i18n::tr("Lendo imagem…"));});}
                if busy {ui.add(egui::ProgressBar::new(editor.done as f32/editor.total.max(1) as f32).text(crate::i18n::message("Processando imagens: {}/{}",&[editor.done.to_string(),editor.total.to_string()])));}
            });
            ui.separator();ui.with_layout(Layout::right_to_left(Align::Center),|ui|{
                create=ui.add_enabled(!busy&&editor.info.is_some(),egui::Button::new(crate::i18n::tr("Criar cópia"))).clicked();
                cancel=ui.button(crate::i18n::tr("Cancelar")).clicked();
            });
        });
        if cancel || modal.should_close() {
            if let Some(token) = &editor.running {
                token.store(true, Ordering::Release);
            } else {
                return;
            }
        }
        if create {
            let token = Arc::new(AtomicBool::new(false));
            let worker_cancel = token.clone();
            let entries = editor.entries.clone();
            let operation = editor.operation();
            let format = editor.format;
            let quality = editor.quality;
            let sender = self.event_sender.clone();
            let context = ctx.clone();
            let locale = crate::i18n::current();
            thread::spawn(move || {
                crate::i18n::set_locale(locale);
                let report = image_ops::run(
                    &entries,
                    operation,
                    format,
                    quality,
                    &worker_cancel,
                    |done, total| {
                        let _ = sender.send(Event::ImagesProgress(done, total));
                        context.request_repaint();
                    },
                );
                let _ = sender.send(Event::ImagesFinished(report));
                context.request_repaint();
            });
            editor.running = Some(token);
        }
        self.image_editor = Some(editor);
    }
    pub(super) fn finish_images(&mut self, report: image_ops::Report) {
        self.image_editor = None;
        if !report.created.is_empty() {
            self.post_notice(crate::i18n::plural(
                "{} cópia de imagem criada. O original foi preservado.",
                "{} cópias de imagens criadas. Os originais foram preservados.",
                report.created.len(),
            ));
            if report
                .created
                .iter()
                .all(|p| p.parent() == Some(self.current.as_path()))
            {
                self.reveal_external(report.created, false);
            }
            self.refresh_after_transfer = true;
        }
        if report.cancelled {
            self.post_typed_notice(
                toast::NoticeKind::Warning,
                crate::i18n::tr("Operação cancelada. As cópias já concluídas foram mantidas.")
                    .into(),
            );
        }
        if !report.errors.is_empty() {
            let summary = crate::i18n::plural(
                "{} imagem não pôde ser processada.",
                "{} imagens não puderam ser processadas.",
                report.errors.len(),
            );
            self.report_error(format!(
                "{summary}\n{}",
                report
                    .errors
                    .iter()
                    .take(3)
                    .cloned()
                    .collect::<Vec<_>>()
                    .join("\n")
            ));
        }
    }
}
fn rotation_label(rotation: Rotation) -> &'static str {
    crate::i18n::tr(match rotation {
        Rotation::Right => "90° para a direita",
        Rotation::Left => "90° para a esquerda",
        Rotation::Half => "180°",
    })
}
#[cfg(test)]
mod tests;
