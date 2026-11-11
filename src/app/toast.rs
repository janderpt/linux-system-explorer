use super::*;

pub(super) use settings::ToastKind as NoticeKind;

impl NoticeKind {
    fn timing(self, options: &settings::ToastOptions) -> settings::ToastTiming {
        match self {
            Self::Success => options.success,
            Self::Warning => options.warning,
            Self::Error => options.error,
        }
    }

    fn icon(self) -> Icon {
        match self {
            Self::Success => Icon::NoticeSuccess,
            Self::Warning => Icon::NoticeWarning,
            Self::Error => Icon::NoticeError,
        }
    }

    fn color(self, dark: bool) -> Color32 {
        match (self, dark) {
            (Self::Success, false) => Color32::from_rgb(21, 128, 61),
            (Self::Success, true) => Color32::from_rgb(74, 222, 128),
            (Self::Warning, false) => Color32::from_rgb(146, 92, 0),
            (Self::Warning, true) => Color32::from_rgb(251, 191, 36),
            (Self::Error, false) => Color32::from_rgb(185, 28, 28),
            (Self::Error, true) => Color32::from_rgb(248, 113, 113),
        }
    }
}

pub(super) struct NoticeToast {
    id: u64,
    pub(super) text: String,
    pub(super) kind: NoticeKind,
    pub(super) timing_override: Option<settings::ToastTiming>,
    pub(super) icon_override: Option<String>,
    started: Option<f64>,
}

impl NoticeToast {
    pub(super) fn update_options(
        &mut self,
        previous: &settings::ToastOptions,
        next: &settings::ToastOptions,
    ) {
        if self.timing_override.is_none() && self.kind.timing(previous) != self.kind.timing(next) {
            self.started = None;
        }
    }
}

impl Explorer {
    pub(super) fn receive_notices(&mut self, notices: Vec<NoticeToast>) {
        for mut notice in notices {
            self.toast_serial = self.toast_serial.wrapping_add(1);
            notice.id = self.toast_serial;
            self.notice_toasts.push(notice);
        }
    }
    pub(super) fn post_notice(&mut self, text: String) {
        self.post_typed_notice(NoticeKind::Success, text);
    }

    fn post_typed_notice(&mut self, kind: NoticeKind, text: String) {
        self.enqueue_notice(kind, text, None, None);
    }

    fn enqueue_notice(
        &mut self,
        kind: NoticeKind,
        text: String,
        timing_override: Option<settings::ToastTiming>,
        icon_override: Option<String>,
    ) {
        self.toast_serial = self.toast_serial.wrapping_add(1);
        self.notice_toasts.push(NoticeToast {
            id: self.toast_serial,
            text,
            kind,
            timing_override,
            icon_override,
            started: None,
        });
    }

    pub(super) fn post_preview_notice(
        &mut self,
        kind: NoticeKind,
        timing: settings::ToastTiming,
        appearance: &icons::Appearance,
    ) {
        let text = match kind {
            NoticeKind::Success => crate::i18n::tr("Teste: operação concluída com sucesso."),
            NoticeKind::Warning => crate::i18n::tr("Teste: operação concluída com avisos."),
            NoticeKind::Error => crate::i18n::tr("Teste: não foi possível concluir a operação."),
        };
        self.enqueue_notice(
            kind,
            text.into(),
            Some(timing),
            Some(appearance.name(kind.icon()).to_owned()),
        );
    }

    pub(super) fn post_operation_notice(
        &mut self,
        mut text: String,
        errors: &[String],
        interrupted: bool,
    ) {
        let kind = if let Some(error) = errors.first() {
            text.push_str(&format!(
                "\n{} {error}",
                crate::i18n::plural("{} falha.", "{} falhas.", errors.len())
            ));
            NoticeKind::Error
        } else if interrupted {
            NoticeKind::Warning
        } else {
            NoticeKind::Success
        };
        self.post_typed_notice(kind, text);
    }

    pub(super) fn show_notice_toast(&mut self, ctx: &egui::Context) {
        if self.notice_toasts.is_empty() {
            return;
        }
        let now = ctx.input(|input| input.time);
        let width = (ctx.content_rect().width() - 64.0).clamp(180.0, 360.0);
        let max_height = ((ctx.content_rect().height() - 100.0) * 0.6).clamp(80.0, 420.0);
        let mut dismissed = Vec::new();
        let mut close_all = false;
        let mut next_expiry: Option<f64> = None;
        let stack_id = egui::Id::new("operation_notice_stack");
        // Previews remain interactive above the settings modal.
        ctx.move_to_top(egui::LayerId::new(egui::Order::Foreground, stack_id));
        egui::Area::new(stack_id)
            .anchor(egui::Align2::RIGHT_BOTTOM, Vec2::new(-16.0, -38.0))
            .order(egui::Order::Foreground)
            .interactable(true)
            .show(ctx, |ui| {
                ui.set_width(width + 24.0);
                ui.spacing_mut().item_spacing.y = 8.0;
                let actions_height = if self.notice_toasts.len() > 1 {
                    // Keep the stack action accessible even when older notices are scrolled away.
                    let actions = ui.allocate_ui_with_layout(
                        Vec2::new(width + 24.0, ui.spacing().interact_size.y),
                        Layout::right_to_left(Align::Center),
                        |ui| {
                            close_all = ui.small_button(crate::i18n::tr("Fechar todos")).clicked();
                        },
                    );
                    actions.response.rect.height() + ui.spacing().item_spacing.y
                } else {
                    0.0
                };
                egui::ScrollArea::vertical()
                    .id_salt("notice_stack_scroll")
                    .max_height((max_height - actions_height).max(1.0))
                    .auto_shrink([false, true])
                    .stick_to_bottom(true)
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = 8.0;
                        for toast in &mut self.notice_toasts {
                            let timing = toast
                                .timing_override
                                .unwrap_or_else(|| toast.kind.timing(&self.toast_options));
                            let duration = f64::from(timing.duration_seconds);
                            let automatic = timing.auto_dismiss;
                            let started = toast.started.get_or_insert(now);
                            let mut close = false;
                            let color = toast.kind.color(ui.visuals().dark_mode);
                            // IDs belong to notifications, not to their stack positions.
                            let card = ui
                                .push_id(toast.id, |ui| {
                                    egui::Frame::popup(ui.style())
                                        .fill(ui.visuals().window_fill().lerp_to_gamma(color, 0.08))
                                        .stroke(egui::Stroke::new(
                                            1.0_f32,
                                            color.gamma_multiply(0.6),
                                        ))
                                        .inner_margin(12.0)
                                        .show(ui, |ui| {
                                            ui.set_width(width);
                                            ui.horizontal_top(|ui| {
                                                let (rect, response) = ui.allocate_exact_size(
                                                    Vec2::splat(20.0),
                                                    Sense::hover(),
                                                );
                                                let icon = toast.kind.icon();
                                                let glyph = icons::glyph(
                                                    toast.icon_override.as_deref().unwrap_or_else(
                                                        || self.appearance.name(icon),
                                                    ),
                                                )
                                                .or_else(|| icons::glyph(icon.default_name()))
                                                .expect("notification icon missing");
                                                icons::paint_glyph(
                                                    ui.painter(),
                                                    rect,
                                                    glyph,
                                                    color,
                                                );
                                                response.on_hover_text(icon.label());
                                                let text_width = (width - 68.0).max(1.0);
                                                ui.allocate_ui_with_layout(
                                                    Vec2::new(text_width, 0.0),
                                                    Layout::top_down(Align::Min),
                                                    |ui| {
                                                        ui.set_min_width(text_width);
                                                        ui.add(
                                                            egui::Label::new(&toast.text).wrap(),
                                                        );
                                                    },
                                                );
                                                close = ui
                                                    .small_button("×")
                                                    .on_hover_text(crate::i18n::tr("Fechar aviso"))
                                                    .clicked();
                                            });
                                        })
                                        .response
                                })
                                .inner;
                            // Check hover before expiry, including timer-driven redraws.
                            // Hovering one card never renews another card's timeout.
                            if automatic && card.contains_pointer() {
                                *started = now;
                            }
                            if close || automatic && now - *started >= duration {
                                dismissed.push(toast.id);
                            } else if automatic {
                                let remaining = (duration - (now - *started)).max(0.01);
                                next_expiry =
                                    Some(next_expiry.map_or(remaining, |next| next.min(remaining)));
                            }
                        }
                    });
            });
        if close_all {
            self.notice_toasts.clear();
            ctx.request_repaint();
            return;
        }
        if !dismissed.is_empty() {
            self.notice_toasts
                .retain(|toast| !dismissed.contains(&toast.id));
            ctx.request_repaint();
        }
        if let Some(remaining) = next_expiry {
            ctx.request_repaint_after(std::time::Duration::from_secs_f64(remaining));
        }
    }
}

#[cfg(test)]
mod tests;
