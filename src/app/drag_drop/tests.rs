use super::*;
use crate::app::tests::{complete_navigation, text_center};
use egui::{Event as InputEvent, FullOutput, Modifiers, PointerButton, Pos2, RawInput, Rect};
use std::{
    fs,
    time::{Duration, Instant},
};

fn render(
    app: &mut Explorer,
    ctx: &egui::Context,
    events: Vec<InputEvent>,
    files: Vec<egui::DroppedFile>,
    modifiers: Modifiers,
) -> FullOutput {
    ctx.run(
        RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1100.0, 760.0))),
            time: Some(ctx.input(|i| i.time) + 0.2),
            events,
            dropped_files: files,
            modifiers,
            ..Default::default()
        },
        |ctx| {
            app.poll();
            app.begin_drag_drop(ctx);
            app.process_transfer_refresh();
            app.shortcuts(ctx);
            app.top_toolbar(ctx);
            app.sidebar(ctx);
            let visible = filesystem::visible_entries(
                &app.entries,
                &app.query,
                app.show_hidden,
                app.sort,
                app.descending,
            );
            app.contents(ctx, &visible);
            app.finish_drag_drop(ctx);
        },
    )
}
fn pointer(
    app: &mut Explorer,
    ctx: &egui::Context,
    position: Pos2,
    pressed: Option<bool>,
    modifiers: Modifiers,
) {
    let mut events = vec![InputEvent::PointerMoved(position)];
    if let Some(pressed) = pressed {
        events.push(InputEvent::PointerButton {
            pos: position,
            button: PointerButton::Primary,
            pressed,
            modifiers,
        });
    }
    render(app, ctx, events, vec![], modifiers);
}
fn finish(app: &mut Explorer) {
    let deadline = Instant::now() + Duration::from_secs(3);
    while app.active_transfer.is_some() {
        app.poll();
        assert!(app.transfer_conflict.is_none());
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(1));
    }
    app.process_transfer_refresh();
    complete_navigation(app);
    assert!(app.error.is_none(), "{:?}", app.error);
}

#[test]
fn dragging_a_selection_copies_to_rows_and_moves_to_sidebar_in_every_view() {
    for view in [View::Details, View::List, View::Icons] {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source");
        let outside = root.path().join("Outside");
        fs::create_dir_all(source.join("Destination")).unwrap();
        fs::create_dir(&outside).unwrap();
        let first = source.join("first.txt");
        let second = source.join("second.txt");
        fs::write(&first, "first").unwrap();
        fs::write(&second, "second").unwrap();
        let ctx = egui::Context::default();
        let mut app = Explorer::with_context(&ctx, source.clone());
        complete_navigation(&mut app);
        app.view = view;
        app.places = vec![(locations::Item::Home, source.clone())];
        // Refresh repopulates places: keep the pinned drop target in the viewport.
        app.sidebar_options.visible = BTreeSet::from([locations::Item::Home]);
        app.sidebar_options
            .pinned
            .push(locations::Shortcut::new(outside.clone()));
        app.selected = BTreeSet::from([first.clone(), second.clone()]);
        render(&mut app, &ctx, vec![], vec![], Modifiers::NONE);
        let output = render(&mut app, &ctx, vec![], vec![], Modifiers::NONE);
        let start = text_center(&output, "first.txt").unwrap();
        let destination = text_center(&output, "Destination").unwrap();
        pointer(&mut app, &ctx, start, Some(true), Modifiers::CTRL);
        pointer(
            &mut app,
            &ctx,
            start + Vec2::new(12.0, 0.0),
            None,
            Modifiers::CTRL,
        );
        assert_eq!(app.dnd.paths.len(), 2, "view drag selection");
        pointer(&mut app, &ctx, destination, None, Modifiers::CTRL);
        pointer(&mut app, &ctx, destination, Some(false), Modifiers::CTRL);
        assert!(app.active_transfer.is_some());
        finish(&mut app);
        assert!(first.exists() && second.exists());
        assert_eq!(
            fs::read_to_string(source.join("Destination/second.txt")).unwrap(),
            "second"
        );
        let output = render(&mut app, &ctx, vec![], vec![], Modifiers::NONE);
        let start = text_center(&output, "first.txt").unwrap();
        let destination = text_center(&output, "Outside").unwrap();
        pointer(&mut app, &ctx, start, Some(true), Modifiers::NONE);
        pointer(
            &mut app,
            &ctx,
            start + Vec2::new(12.0, 0.0),
            None,
            Modifiers::NONE,
        );
        pointer(&mut app, &ctx, destination, None, Modifiers::NONE);
        pointer(&mut app, &ctx, destination, Some(false), Modifiers::NONE);
        assert!(app.active_transfer.is_some());
        finish(&mut app);
        assert!(!first.exists() && !second.exists());
        assert!(outside.join("first.txt").exists() && outside.join("second.txt").exists());
    }
}

#[test]
fn external_files_use_the_pointed_folder_and_esc_and_invalid_targets_change_nothing() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source");
    let file = root.path().join("external.txt");
    fs::create_dir_all(source.join("Destination")).unwrap();
    fs::write(&file, "external").unwrap();
    let ctx = egui::Context::default();
    let mut app = Explorer::with_context(&ctx, source.clone());
    complete_navigation(&mut app);
    render(&mut app, &ctx, vec![], vec![], Modifiers::NONE);
    let output = render(&mut app, &ctx, vec![], vec![], Modifiers::NONE);
    let destination = text_center(&output, "Destination").unwrap();
    render(
        &mut app,
        &ctx,
        vec![InputEvent::PointerMoved(destination)],
        vec![egui::DroppedFile {
            path: Some(file.clone()),
            ..Default::default()
        }],
        Modifiers::NONE,
    );
    assert!(app.active_transfer.is_some());
    finish(&mut app);
    assert!(file.exists());
    assert_eq!(
        fs::read_to_string(source.join("Destination/external.txt")).unwrap(),
        "external"
    );
    assert!(!source.join("external.txt").exists());
    app.drop_files(
        vec![source.join("Destination")],
        source.join("Destination/child"),
        file_ops::Mode::Cut,
        &ctx,
    );
    assert!(app.active_transfer.is_none());
    assert!(source.join("Destination/external.txt").exists());
    app.dnd.paths = vec![file.clone()];
    render(
        &mut app,
        &ctx,
        vec![InputEvent::Key {
            key: Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        }],
        vec![],
        Modifiers::NONE,
    );
    assert!(app.dnd.paths.is_empty());
    assert!(file.exists());
    app.drop_files(
        vec![file.clone()],
        crate::trashbin::location(),
        file_ops::Mode::Cut,
        &ctx,
    );
    let deadline = Instant::now() + Duration::from_secs(3);
    while app.dnd.trash_prepare.is_some() {
        app.begin_drag_drop(&ctx);
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(1));
    }
    assert!(app.delete_confirmation.is_some());
    assert!(file.exists());
    app.delete_confirmation = None;
}

#[test]
fn shift_moves_external_files_and_ctrl_copies_and_drag_cursors_match_the_operation() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source");
    fs::create_dir_all(source.join("Destination")).unwrap();
    let ctx = egui::Context::default();
    let mut app = Explorer::with_context(&ctx, source.clone());
    complete_navigation(&mut app);
    for (name, modifiers, moved) in [
        ("move.txt", Modifiers::SHIFT, true),
        ("copy.txt", Modifiers::CTRL, false),
    ] {
        let file = root.path().join(name);
        fs::write(&file, "content").unwrap();
        let output = render(&mut app, &ctx, vec![], vec![], Modifiers::NONE);
        let point = text_center(&output, "Destination").unwrap();
        render(
            &mut app,
            &ctx,
            vec![InputEvent::PointerMoved(point)],
            vec![egui::DroppedFile {
                path: Some(file.clone()),
                ..Default::default()
            }],
            modifiers,
        );
        finish(&mut app);
        assert_eq!(file.exists(), !moved);
        assert!(source.join("Destination").join(name).exists());
    }
    let file = source.join("local.txt");
    fs::write(&file, "local").unwrap();
    app.refresh();
    complete_navigation(&mut app);
    let output = render(&mut app, &ctx, vec![], vec![], Modifiers::NONE);
    let destination = text_center(&output, "Destination").unwrap();
    for (modifiers, expected) in [
        (Modifiers::NONE, egui::CursorIcon::Default),
        (Modifiers::SHIFT, egui::CursorIcon::Default),
        (Modifiers::CTRL, egui::CursorIcon::Copy),
    ] {
        app.dnd.paths = vec![file.clone()];
        let output = render(
            &mut app,
            &ctx,
            vec![InputEvent::PointerMoved(destination)],
            vec![],
            modifiers,
        );
        assert_eq!(output.platform_output.cursor_icon, expected);
    }
    let output = render(
        &mut app,
        &ctx,
        vec![InputEvent::PointerMoved(egui::pos2(4.0, 4.0))],
        vec![],
        Modifiers::NONE,
    );
    assert_eq!(output.platform_output.cursor_icon, egui::CursorIcon::NoDrop);
}
