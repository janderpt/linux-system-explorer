use super::*;
use crate::app::tests::{complete_navigation, text_center};
use egui::{Event, Modifiers, PointerButton, Pos2, RawInput, Rect};

fn key(key: Key) -> Event {
    Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    }
}
fn render(app: &mut Explorer, ctx: &egui::Context, events: Vec<Event>) -> egui::FullOutput {
    ctx.run(
        RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1180.0, 760.0))),
            time: Some(ctx.input(|i| i.time) + 0.05),
            events,
            ..Default::default()
        },
        |ctx| app.show(ctx),
    )
}
fn wait(app: &mut Explorer, ctx: &egui::Context) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while app.preview.as_ref().is_some_and(|p| p.job.is_some()) {
        render(app, ctx, vec![]);
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
}
#[test]
fn space_opens_internal_text_preview_escape_closes_and_text_input_keeps_spaces() {
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join("note.txt");
    std::fs::write(&file, "Preview content without opening an editor").unwrap();
    let ctx = egui::Context::default();
    let mut app = Explorer::with_context(&ctx, root.path().into());
    complete_navigation(&mut app);
    app.selected.insert(file.clone());
    app.selection_cursor = Some(file.clone());
    render(&mut app, &ctx, vec![]);
    render(&mut app, &ctx, vec![key(Key::Space)]);
    wait(&mut app, &ctx);
    let output = render(&mut app, &ctx, vec![]);
    assert!(text_center(&output, "Preview content without opening an editor").is_some());
    assert!(app.has_dialog());
    assert!(!app.file_drop_available());
    render(&mut app, &ctx, vec![key(Key::Escape)]);
    assert!(app.preview.is_none());
    assert_eq!(
        std::fs::read_to_string(&file).unwrap(),
        "Preview content without opening an editor"
    );
    app.focus_search = true;
    render(&mut app, &ctx, vec![]);
    render(
        &mut app,
        &ctx,
        vec![key(Key::Space), Event::Text(" ".into())],
    );
    assert!(app.preview.is_none());
    assert_eq!(app.query, " ");
}
#[test]
fn context_menu_opens_an_image_with_zoom_and_folders_do_not_start_preview() {
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join("picture.png");
    image::RgbaImage::from_pixel(64, 32, image::Rgba([20, 40, 90, 255]))
        .save(&file)
        .unwrap();
    std::fs::create_dir(root.path().join("Folder")).unwrap();
    let ctx = egui::Context::default();
    let mut app = Explorer::with_context(&ctx, root.path().into());
    complete_navigation(&mut app);
    app.request_preview(root.path().join("Folder"), &ctx);
    assert!(app.preview.is_none());
    render(&mut app, &ctx, vec![]);
    let output = render(&mut app, &ctx, vec![]);
    let position = text_center(&output, "picture.png").unwrap();
    for pressed in [true, false] {
        render(
            &mut app,
            &ctx,
            vec![
                Event::PointerMoved(position),
                Event::PointerButton {
                    pos: position,
                    button: PointerButton::Secondary,
                    pressed,
                    modifiers: Modifiers::NONE,
                },
            ],
        );
    }
    let output = render(&mut app, &ctx, vec![]);
    let position = text_center(&output, "Pré-visualizar").unwrap();
    for pressed in [true, false] {
        render(
            &mut app,
            &ctx,
            vec![
                Event::PointerMoved(position),
                Event::PointerButton {
                    pos: position,
                    button: PointerButton::Primary,
                    pressed,
                    modifiers: Modifiers::NONE,
                },
            ],
        );
    }
    wait(&mut app, &ctx);
    assert_eq!(
        app.preview
            .as_ref()
            .unwrap()
            .texture
            .as_ref()
            .unwrap()
            .size(),
        [64, 32]
    );
    let output = render(&mut app, &ctx, vec![]);
    assert!(text_center(&output, "Ajustar à janela").is_some());
    render(&mut app, &ctx, vec![key(Key::Escape)]);
    assert!(app.preview.is_none());
}

#[test]
fn unsupported_and_failed_files_remain_in_the_preview_without_opening_external_apps() {
    let root = tempfile::tempdir().unwrap();
    let binary = root.path().join("unknown.bin");
    let bad = root.path().join("broken.png");
    std::fs::write(&binary, b"\0binary").unwrap();
    std::fs::write(&bad, "invalid PNG").unwrap();
    let ctx = egui::Context::default();
    let mut app = Explorer::with_context(&ctx, root.path().into());
    complete_navigation(&mut app);
    for file in [binary, bad] {
        app.request_preview(file, &ctx);
        wait(&mut app, &ctx);
        let output = render(&mut app, &ctx, vec![]);
        assert!(text_center(&output, "Não há pré-visualização disponível.").is_some());
        render(&mut app, &ctx, vec![key(Key::Escape)]);
    }
    assert!(
        app.notice_toasts
            .iter()
            .any(|toast| toast.kind == toast::NoticeKind::Error)
    );
}

#[test]
fn pdf_preview_changes_pages_and_keeps_the_source_unchanged() {
    if std::process::Command::new("pdfinfo")
        .arg("-v")
        .output()
        .is_err()
    {
        assert!(
            std::env::var_os("CI").is_none(),
            "Poppler is required in CI"
        );
        return;
    }
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join("document.pdf");
    crate::preview::tests::test_pdf(&file);
    let before = std::fs::read(&file).unwrap();
    let ctx = egui::Context::default();
    let mut app = Explorer::with_context(&ctx, root.path().into());
    complete_navigation(&mut app);
    app.request_preview(file.clone(), &ctx);
    wait(&mut app, &ctx);
    assert_eq!(app.preview.as_ref().unwrap().pages, 2);
    let output = render(&mut app, &ctx, vec![]);
    let position = text_center(&output, "Próxima página").unwrap();
    for pressed in [true, false] {
        render(
            &mut app,
            &ctx,
            vec![
                Event::PointerMoved(position),
                Event::PointerButton {
                    pos: position,
                    button: PointerButton::Primary,
                    pressed,
                    modifiers: Modifiers::NONE,
                },
            ],
        );
    }
    wait(&mut app, &ctx);
    assert_eq!(app.preview.as_ref().unwrap().page, 2);
    let size = app
        .preview
        .as_ref()
        .unwrap()
        .texture
        .as_ref()
        .unwrap()
        .size();
    assert!(size[1] > size[0]);
    render(&mut app, &ctx, vec![key(Key::Escape)]);
    assert_eq!(before, std::fs::read(&file).unwrap());
}
