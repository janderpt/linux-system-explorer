use super::*;
use crate::app::tests::text_center;
use egui::{FullOutput, Pos2, RawInput, Rect};

fn draw(
    app: &mut Explorer,
    ctx: &egui::Context,
    time: f64,
    toast: bool,
) -> (FullOutput, [egui::Rect; 3]) {
    draw_events(app, ctx, time, toast, vec![])
}
fn draw_events(
    app: &mut Explorer,
    ctx: &egui::Context,
    time: f64,
    toast: bool,
    events: Vec<egui::Event>,
) -> (FullOutput, [egui::Rect; 3]) {
    let mut rows = [egui::Rect::NOTHING; 3];
    let output = ctx.run(
        RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1100.0, 760.0))),
            time: Some(time),
            events,
            ..Default::default()
        },
        |ctx| {
            app.top_toolbar(ctx);
            rows[0] =
                Rect::from_min_max(Pos2::ZERO, egui::pos2(1100.0, ctx.available_rect().top()));
            egui::CentralPanel::default()
                .frame(egui::Frame::new().inner_margin(egui::Margin {
                    left: 8,
                    right: 8,
                    top: 0,
                    bottom: 0,
                }))
                .show(ctx, |ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    rows[1] = app.folder_path(ui).rect;
                    ui.add(egui::Separator::default().spacing(0.0));
                    app.transfer_status(ui);
                    rows[2] =
                        app.column_headers(ui, &[(Column::Name, 300.0), (Column::Size, 150.0)]);
                });
            if toast {
                app.show_notice_toast(ctx);
            }
        },
    );
    (output, rows)
}

#[test]
fn toast_can_be_closed_with_its_button() {
    let ctx = egui::Context::default();
    let mut app = Explorer::with_context(&ctx, PathBuf::from("/tmp"));
    app.post_notice("Finished".into());
    draw(&mut app, &ctx, 0.0, true);
    let (output, _) = draw(&mut app, &ctx, 0.2, true);
    let position = text_center(&output, "×").unwrap();
    for (time, pressed) in [(0.4, true), (0.6, false)] {
        draw_events(
            &mut app,
            &ctx,
            time,
            true,
            vec![
                egui::Event::PointerMoved(position),
                egui::Event::PointerButton {
                    pos: position,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
    }
    assert!(app.notice_toasts.is_empty());
}

#[test]
fn chrome_rows_share_height_and_results_float_without_displacing_the_headers() {
    for size in [18.0, 30.0] {
        let ctx = egui::Context::default();
        let mut app = Explorer::with_context(&ctx, PathBuf::from("/tmp"));
        crate::app::tests::complete_navigation(&mut app);
        app.appearance.size = size;
        app.current = "/tmp/layout".into();
        draw(&mut app, &ctx, 0.0, false);
        let (_, before) = draw(&mut app, &ctx, 0.1, false);
        for row in before {
            assert!(
                (row.height() - app.chrome_row_height()).abs() < 1.5,
                "rows={before:?}"
            );
        }
        assert!((before[0].bottom() - before[1].top()).abs() < 1.5);
        assert!((before[1].bottom() - before[2].top()).abs() < 1.5);
        app.post_notice("1 item enviado à lixeira.".into());
        let (inline, after) = draw(&mut app, &ctx, 0.2, false);
        assert!(text_center(&inline, "1 item enviado à lixeira.").is_none());
        assert_eq!(before[2], after[2]);
        draw(&mut app, &ctx, 0.3, true);
        let (output, after) = draw(&mut app, &ctx, 0.4, true);
        let center = text_center(&output, "1 item enviado à lixeira.").unwrap();
        assert!(
            text_center(&output, "×").unwrap().x > 1040.0,
            "close button belongs at the right edge"
        );
        assert!(center.x > 550.0 && center.y > 570.0, "{center:?}");
        assert_eq!(before[2], after[2]);
        assert!(
            (text_center(&output, crate::i18n::tr("Nome")).unwrap().y - before[2].center().y).abs()
                < 1.5
        );
        draw(&mut app, &ctx, 7.0, true);
        assert!(app.notice_toasts.is_empty());
    }
}

#[test]
fn repeating_the_same_notice_preserves_each_notification_and_its_timeout() {
    let ctx = egui::Context::default();
    let mut app = Explorer::with_context(&ctx, PathBuf::from("/tmp"));
    app.post_notice("done".into());
    draw(&mut app, &ctx, 0.0, true);
    draw(&mut app, &ctx, 5.0, true);
    app.post_notice("done".into());
    draw(&mut app, &ctx, 5.1, true);
    assert_eq!(app.notice_toasts.len(), 2);
    assert_ne!(app.notice_toasts[0].id, app.notice_toasts[1].id);
    draw(&mut app, &ctx, 7.0, true);
    assert_eq!(app.notice_toasts.len(), 1);
    draw(&mut app, &ctx, 12.0, true);
    assert!(app.notice_toasts.is_empty());
}

#[test]
fn configured_timeout_and_manual_mode_apply_to_current_and_future_toasts() {
    let ctx = egui::Context::default();
    let mut app = Explorer::with_context(&ctx, PathBuf::from("/tmp"));
    let mut preferences = app.preferences();
    preferences.toast.success.duration_seconds = 2;
    app.apply_preferences(preferences.clone(), &ctx);
    app.post_notice("Timed".into());
    draw(&mut app, &ctx, 0.0, true);
    draw(&mut app, &ctx, 1.9, true);
    assert!(!app.notice_toasts.is_empty());
    draw(&mut app, &ctx, 2.1, true);
    assert!(app.notice_toasts.is_empty());
    preferences.toast.success.auto_dismiss = false;
    app.apply_preferences(preferences.clone(), &ctx);
    app.post_notice("Persistent".into());
    draw(&mut app, &ctx, 3.0, true);
    let (output, _) = draw(&mut app, &ctx, 3603.0, true);
    assert!(text_center(&output, "Persistent").is_some());
    app.post_notice("Another operation".into());
    assert_eq!(app.notice_toasts.len(), 2);
    // Switching back to timed mode restarts the duration of an already visible toast.
    preferences.toast.success.auto_dismiss = true;
    app.apply_preferences(preferences, &ctx);
    draw(&mut app, &ctx, 3604.0, true);
    draw(&mut app, &ctx, 3605.9, true);
    assert!(!app.notice_toasts.is_empty());
    draw(&mut app, &ctx, 3606.1, true);
    assert!(app.notice_toasts.is_empty());
}

fn close_position(output: &FullOutput, label: &str) -> Pos2 {
    let message = text_center(output, label).unwrap();
    output
        .shapes
        .iter()
        .filter_map(|shape| {
            if let egui::Shape::Text(text) = &shape.shape
                && text.galley.text() == "×"
            {
                let point = text.pos + text.galley.size() * 0.5;
                ((point.y - message.y).abs() < 5.0).then_some(point)
            } else {
                None
            }
        })
        .next()
        .expect("each notification must have its own close button")
}

#[test]
fn manual_toasts_stack_and_closing_the_middle_one_preserves_the_others() {
    let ctx = egui::Context::default();
    let mut app = Explorer::with_context(&ctx, PathBuf::from("/tmp"));
    app.toast_options.success.auto_dismiss = false;
    for text in ["First notice", "Second notice", "Third notice"] {
        app.post_notice(text.into());
    }
    let ids: Vec<_> = app.notice_toasts.iter().map(|toast| toast.id).collect();
    draw(&mut app, &ctx, 0.0, true);
    let (output, _) = draw(&mut app, &ctx, 3600.0, true);
    let first = text_center(&output, "First notice").unwrap();
    let second = text_center(&output, "Second notice").unwrap();
    let third = text_center(&output, "Third notice").unwrap();
    assert!(first.y + 30.0 < second.y && second.y + 30.0 < third.y);
    let close_first = close_position(&output, "First notice");
    let close_second = close_position(&output, "Second notice");
    let close_third = close_position(&output, "Third notice");
    assert!((close_first.x - close_second.x).abs() < 1.0);
    assert!((close_second.x - close_third.x).abs() < 1.0);
    let position = close_position(&output, "Second notice");
    for (time, pressed) in [(3600.2, true), (3600.4, false)] {
        draw_events(
            &mut app,
            &ctx,
            time,
            true,
            vec![
                egui::Event::PointerMoved(position),
                egui::Event::PointerButton {
                    pos: position,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
    }
    assert_eq!(
        app.notice_toasts
            .iter()
            .map(|toast| toast.id)
            .collect::<Vec<_>>(),
        vec![ids[0], ids[2]]
    );
    let (output, _) = draw(&mut app, &ctx, 3600.6, true);
    assert!(text_center(&output, "First notice").is_some());
    assert!(text_center(&output, "Third notice").is_some());
    assert!(text_center(&output, "Second notice").is_none());
}

#[test]
fn hovering_one_notification_does_not_extend_the_lifetime_of_another() {
    let ctx = egui::Context::default();
    let mut app = Explorer::with_context(&ctx, PathBuf::from("/tmp"));
    app.post_notice("Hovered notice".into());
    app.post_notice("Unhovered notice".into());
    draw(&mut app, &ctx, 0.0, true);
    let (output, _) = draw(&mut app, &ctx, 0.1, true);
    let position = text_center(&output, "Hovered notice").unwrap();
    draw_events(
        &mut app,
        &ctx,
        5.0,
        true,
        vec![egui::Event::PointerMoved(position)],
    );
    draw(&mut app, &ctx, 6.1, true);
    assert_eq!(app.notice_toasts.len(), 1);
    assert_eq!(app.notice_toasts[0].text, "Hovered notice");
    draw_events(
        &mut app,
        &ctx,
        6.2,
        true,
        vec![egui::Event::PointerMoved(Pos2::ZERO)],
    );
    draw(&mut app, &ctx, 11.9, true);
    assert_eq!(app.notice_toasts.len(), 1);
    draw(&mut app, &ctx, 12.2, true);
    assert!(app.notice_toasts.is_empty());
}

#[test]
fn a_large_manual_stack_is_bounded_and_older_notifications_remain_accessible() {
    let ctx = egui::Context::default();
    let mut app = Explorer::with_context(&ctx, PathBuf::from("/tmp"));
    app.toast_options.success.auto_dismiss = false;
    for number in 0..30 {
        app.post_notice(format!("Notice {number:02}"));
    }
    draw(&mut app, &ctx, 0.0, true);
    draw(&mut app, &ctx, 0.1, true);
    let (output, _) = draw(&mut app, &ctx, 0.2, true);
    let area = ctx
        .memory(|memory| memory.area_rect(egui::Id::new("operation_notice_stack")))
        .unwrap();
    assert!(
        area.height() <= 420.0 && area.top() > 0.0 && area.bottom() <= 760.0,
        "{area:?}"
    );
    let latest = text_center(&output, "Notice 29").unwrap();
    assert!(area.contains(latest));
    assert_eq!(app.notice_toasts.len(), 30);
    draw_events(
        &mut app,
        &ctx,
        0.4,
        true,
        vec![
            egui::Event::PointerMoved(area.center()),
            egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: Vec2::new(0.0, 10000.0),
                modifiers: egui::Modifiers::NONE,
            },
        ],
    );
    let (output, _) = draw(&mut app, &ctx, 0.6, true);
    let oldest = text_center(&output, "Notice 00").unwrap();
    assert!(area.contains(oldest));
    let position = close_position(&output, "Notice 00");
    for (time, pressed) in [(0.8, true), (1.0, false)] {
        draw_events(
            &mut app,
            &ctx,
            time,
            true,
            vec![
                egui::Event::PointerMoved(position),
                egui::Event::PointerButton {
                    pos: position,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
    }
    assert_eq!(app.notice_toasts.len(), 29);
    assert_eq!(app.notice_toasts[0].text, "Notice 01");
    assert_eq!(app.notice_toasts.last().unwrap().text, "Notice 29");
}

#[test]
fn transfer_outcomes_distinguish_success_skips_cancellation_and_failures() {
    let ctx = egui::Context::default();
    let mut app = Explorer::with_context(&ctx, PathBuf::from("/tmp"));
    for (skipped, cancelled, errors, expected) in [
        (0, false, vec![], NoticeKind::Success),
        (1, false, vec![], NoticeKind::Warning),
        (0, true, vec![], NoticeKind::Warning),
        (
            0,
            false,
            vec!["Permission denied".into()],
            NoticeKind::Error,
        ),
        (1, true, vec!["Permission denied".into()], NoticeKind::Error),
    ] {
        app.finish_transfer(file_ops::Report {
            clipboard_id: 1,
            mode: file_ops::Mode::Copy,
            completed: vec![PathBuf::from("/source/copied")],
            skipped,
            cancelled,
            errors,
        });
        let notice = app.notice_toasts.last().unwrap();
        assert_eq!(notice.kind, expected);
        assert_eq!(
            notice.text.contains("Permission denied"),
            expected == NoticeKind::Error
        );
    }
}

#[test]
fn each_kind_uses_its_own_timeout_and_only_changed_kinds_restart() {
    let ctx = egui::Context::default();
    let mut app = Explorer::with_context(&ctx, PathBuf::from("/tmp"));
    let mut preferences = app.preferences();
    preferences.toast.success.duration_seconds = 2;
    preferences.toast.warning.duration_seconds = 4;
    app.apply_preferences(preferences.clone(), &ctx);
    app.post_typed_notice(NoticeKind::Success, "Success".into());
    app.post_typed_notice(NoticeKind::Warning, "Warning".into());
    app.post_typed_notice(NoticeKind::Error, "Error".into());
    draw(&mut app, &ctx, 0.0, true);
    draw(&mut app, &ctx, 0.1, true);

    preferences.toast.warning.duration_seconds = 8;
    app.apply_preferences(preferences.clone(), &ctx);
    draw(&mut app, &ctx, 1.0, true);
    draw(&mut app, &ctx, 2.1, true);
    assert_eq!(app.notice_toasts.len(), 2);
    assert_eq!(app.notice_toasts[0].kind, NoticeKind::Warning);
    assert_eq!(app.notice_toasts[1].kind, NoticeKind::Error);
    draw(&mut app, &ctx, 9.1, true);
    assert_eq!(app.notice_toasts.len(), 1);
    let (output, _) = draw(&mut app, &ctx, 3600.0, true);
    assert!(text_center(&output, "Error").is_some());

    preferences.toast.error.duration_seconds = 3;
    preferences.toast.error.auto_dismiss = true;
    app.apply_preferences(preferences, &ctx);
    draw(&mut app, &ctx, 3600.1, true);
    draw(&mut app, &ctx, 3602.9, true);
    assert_eq!(app.notice_toasts.len(), 1);
    draw(&mut app, &ctx, 3603.2, true);
    assert!(app.notice_toasts.is_empty());
}

#[test]
fn preview_uses_draft_timing_without_changing_or_following_global_preferences() {
    let ctx = egui::Context::default();
    let mut app = Explorer::with_context(&ctx, PathBuf::from("/tmp"));
    app.post_preview_notice(
        NoticeKind::Success,
        settings::ToastTiming {
            duration_seconds: 2,
            auto_dismiss: true,
        },
        &icons::Appearance::default(),
    );
    app.post_preview_notice(
        NoticeKind::Error,
        settings::ToastTiming {
            duration_seconds: 2,
            auto_dismiss: true,
        },
        &icons::Appearance::default(),
    );
    app.post_preview_notice(
        NoticeKind::Warning,
        settings::ToastTiming {
            duration_seconds: 1,
            auto_dismiss: false,
        },
        &icons::Appearance::default(),
    );
    draw(&mut app, &ctx, 0.0, true);
    draw(&mut app, &ctx, 0.1, true);
    let mut preferences = app.preferences();
    preferences.toast.success.duration_seconds = 30;
    preferences.toast.error.auto_dismiss = true;
    app.apply_preferences(preferences, &ctx);
    draw(&mut app, &ctx, 1.0, true);
    draw(&mut app, &ctx, 2.1, true);
    assert_eq!(app.notice_toasts.len(), 1);
    assert_eq!(app.notice_toasts[0].kind, NoticeKind::Warning);
    draw(&mut app, &ctx, 3600.0, true);
    assert_eq!(app.notice_toasts.len(), 1);
}
