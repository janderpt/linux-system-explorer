use super::*;
use crate::app::tests::{complete_navigation, text_center};
use egui::{Event as Input, Modifiers, PointerButton, Pos2, RawInput, Rect};
use std::{
    fs,
    time::{Duration, Instant},
};
fn frame(app: &mut Explorer, ctx: &egui::Context, events: Vec<Input>) -> egui::FullOutput {
    ctx.run(
        RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1180.0, 760.0))),
            events,
            time: Some(ctx.input(|i| i.time) + 0.2),
            ..Default::default()
        },
        |ctx| {
            let visible = app.visible_for_display();
            app.top_toolbar(ctx);
            app.contents(ctx, &visible);
            app.image_dialog(ctx);
        },
    )
}
fn click(
    app: &mut Explorer,
    ctx: &egui::Context,
    pos: Pos2,
    button: PointerButton,
) -> egui::FullOutput {
    for pressed in [true, false] {
        frame(
            app,
            ctx,
            vec![
                Input::PointerMoved(pos),
                Input::PointerButton {
                    pos,
                    button,
                    pressed,
                    modifiers: Modifiers::NONE,
                },
            ],
        );
    }
    frame(app, ctx, vec![])
}
fn ready(app: &mut Explorer, ctx: &egui::Context) {
    let deadline = Instant::now() + Duration::from_secs(3);
    while app.image_editor.as_ref().is_some_and(|e| e.info.is_none()) {
        frame(app, ctx, vec![]);
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(2));
    }
}
#[test]
fn image_context_actions_create_new_files_from_dialogs_and_preserve_sources_in_all_views() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("photo.png");
    image::RgbaImage::from_pixel(8, 4, image::Rgba([120, 40, 80, 255]))
        .save(&path)
        .unwrap();
    let before = fs::read(&path).unwrap();
    let ctx = egui::Context::default();
    let mut app = Explorer::with_context(&ctx, root.path().to_owned());
    complete_navigation(&mut app);
    let source = app.entries.iter().find(|e| e.path == path).unwrap().clone();
    for (view, action, label) in [
        (View::Details, Action::Rotate, "Girar…"),
        (View::List, Action::Resize, "Redimensionar…"),
        (View::Icons, Action::Convert, "Converter…"),
    ] {
        app.view = view;
        app.selected = BTreeSet::from([path.clone()]);
        frame(&mut app, &ctx, vec![]);
        let output = frame(&mut app, &ctx, vec![]);
        let pos = text_center(&output, "photo.png").unwrap();
        let output = click(&mut app, &ctx, pos, PointerButton::Secondary);
        for name in [
            "Imagem",
            "Girar…",
            "Redimensionar…",
            "Converter…",
            "Imprimir…",
        ] {
            assert!(text_center(&output, name).is_some(), "{name}");
        }
        let pos = text_center(&output, label).unwrap();
        click(&mut app, &ctx, pos, PointerButton::Primary);
        ready(&mut app, &ctx);
        assert!(app.image_editor.is_some());
        let editor = app.image_editor.as_mut().unwrap();
        match action {
            Action::Rotate => editor.rotation = Rotation::Right,
            Action::Resize => {
                editor.width = 4;
                editor.height = 4;
                editor.proportional = true;
            }
            Action::Convert => editor.format = Some(Format::Jpeg),
        }
        let output = frame(&mut app, &ctx, vec![]);
        let create = text_center(&output, "Criar cópia").unwrap();
        let cancel = text_center(&output, "Cancelar").unwrap();
        assert!((create.y - cancel.y).abs() < 1.0 && create.x > cancel.x);
        click(&mut app, &ctx, create, PointerButton::Primary);
        let deadline = Instant::now() + Duration::from_secs(3);
        while app.image_pending() {
            app.poll();
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(2));
        }
        assert!(app.image_editor.is_none());
        app.process_transfer_refresh();
        complete_navigation(&mut app);
        let new = app.selected.iter().next().unwrap();
        assert_ne!(new, &path);
        let image = image::open(new).unwrap();
        assert_eq!(
            (image.width(), image.height()),
            match action {
                Action::Rotate => (4, 8),
                Action::Resize => (4, 2),
                Action::Convert => (8, 4),
            }
        );
        assert_eq!(fs::read(&path).unwrap(), before);
    }
    let count = fs::read_dir(root.path()).unwrap().count();
    app.request_image_action(vec![source.clone()], Action::Rotate, &ctx);
    ready(&mut app, &ctx);
    let output = frame(&mut app, &ctx, vec![]);
    click(
        &mut app,
        &ctx,
        text_center(&output, "Cancelar").unwrap(),
        PointerButton::Primary,
    );
    assert!(app.image_editor.is_none());
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), count);
    app.selected = BTreeSet::from([path.clone()]);
    assert_eq!(app.selected_images(&source).len(), 1);
    fs::write(root.path().join("note.txt"), "text").unwrap();
    app.refresh();
    complete_navigation(&mut app);
    app.selected.insert(root.path().join("note.txt"));
    assert!(app.selected_images(&source).is_empty());
}
