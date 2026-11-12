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

#[test]
fn dragging_to_a_network_mount_copies_by_default_and_shift_moves_with_matching_feedback() {
    for (modifiers, moved) in [
        (Modifiers::NONE, false),
        (Modifiers::CTRL, false),
        (Modifiers::SHIFT, true),
    ] {
        let root = tempfile::tempdir().unwrap();
        let folder = root.path().join("FTP destination");
        fs::create_dir(&folder).unwrap();
        let file = root.path().join("upload.txt");
        fs::write(&file, "preserve source unless Shift requests moving").unwrap();
        let ctx = egui::Context::default();
        let mut app = Explorer::with_context(&ctx, root.path().into());
        complete_navigation(&mut app);
        render(&mut app, &ctx, vec![], vec![], Modifiers::NONE);
        let output = render(&mut app, &ctx, vec![], vec![], Modifiers::NONE);
        let start = text_center(&output, "upload.txt").unwrap();
        let target = text_center(&output, "FTP destination").unwrap();
        pointer(&mut app, &ctx, start, Some(true), modifiers);
        pointer(
            &mut app,
            &ctx,
            start + Vec2::new(12.0, 0.0),
            None,
            modifiers,
        );
        assert_eq!(app.dnd.paths.as_slice(), std::slice::from_ref(&file));
        // The files are real temporary files; only the mount inventory is a
        // fixture, allowing CI to exercise the UI without a remote server.
        let escaped = folder.to_string_lossy().replace(' ', "\\040");
        app.dnd.mounts = Some(Arc::new(crate::native_dnd::policy::Mounts::parse(format!("1 0 8:1 / / rw - ext4 /dev/test rw\n2 1 0:77 / {escaped} rw - fuse.gvfsd-fuse gvfs rw\n").as_bytes())));
        pointer(&mut app, &ctx, target, None, modifiers);
        let output = render(&mut app, &ctx, vec![], vec![], modifiers);
        assert_eq!(
            output.platform_output.cursor_icon,
            if moved {
                egui::CursorIcon::Default
            } else {
                egui::CursorIcon::Copy
            }
        );
        assert!(text_center(&output, if moved { "Mover" } else { "Copiar" }).is_some());
        assert_eq!(app.dnd.preview.as_ref().unwrap().1, "upload.txt");
        pointer(&mut app, &ctx, target, Some(false), modifiers);
        assert_eq!(
            app.active_transfer.as_ref().unwrap().mode,
            if moved {
                file_ops::Mode::Cut
            } else {
                file_ops::Mode::Copy
            }
        );
        finish(&mut app);
        assert_eq!(file.exists(), !moved);
        assert_eq!(
            fs::read_to_string(folder.join("upload.txt")).unwrap(),
            "preserve source unless Shift requests moving"
        );
        assert!(app.dnd.preview.is_none());
    }
}

#[test]
fn drag_badge_follows_the_pointer_shows_the_selection_count_and_disappears_on_escape() {
    for view in [View::Details, View::List, View::Icons] {
        let root = tempfile::tempdir().unwrap();
        let mut paths = Vec::new();
        for i in 0..5 {
            let path = root.path().join(format!("file-{i}.txt"));
            fs::write(&path, "original").unwrap();
            paths.push(path);
        }
        let ctx = egui::Context::default();
        let mut app = Explorer::with_context(&ctx, root.path().into());
        complete_navigation(&mut app);
        app.view = view;
        app.selected = paths.iter().cloned().collect();
        render(&mut app, &ctx, vec![], vec![], Modifiers::NONE);
        let output = render(&mut app, &ctx, vec![], vec![], Modifiers::NONE);
        let start = text_center(&output, "file-0.txt").unwrap();
        pointer(&mut app, &ctx, start, Some(true), Modifiers::NONE);
        pointer(
            &mut app,
            &ctx,
            start + Vec2::new(12.0, 0.0),
            None,
            Modifiers::NONE,
        );
        assert_eq!(app.dnd.paths.len(), 5);
        let mut positions = Vec::new();
        for point in [egui::pos2(400.0, 400.0), egui::pos2(480.0, 460.0)] {
            pointer(&mut app, &ctx, point, None, Modifiers::NONE);
            let output = render(&mut app, &ctx, vec![], vec![], Modifiers::NONE);
            positions.push(
                text_center(&output, "5 itens sendo arrastados").expect("floating selection badge"),
            );
        }
        assert!((positions[1] - positions[0] - Vec2::new(80.0, 60.0)).length() < 2.0);
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
        let output = render(&mut app, &ctx, vec![], vec![], Modifiers::NONE);
        assert!(text_center(&output, "5 itens sendo arrastados").is_none());
        assert!(app.dnd.preview.is_none() && app.dnd.paths.is_empty());
        assert!(app.active_transfer.is_none());
        assert!(
            paths
                .iter()
                .all(|path| fs::read_to_string(path).unwrap() == "original")
        );
    }
}

#[test]
fn returning_native_drag_keeps_its_negotiated_copy_instead_of_promoting_it_to_move() {
    let root = tempfile::tempdir().unwrap();
    let target = root.path().join("Destination");
    fs::create_dir(&target).unwrap();
    let file = root.path().join("source.txt");
    fs::write(&file, "original").unwrap();
    let ctx = egui::Context::default();
    let mut app = Explorer::with_context(&ctx, root.path().into());
    complete_navigation(&mut app);
    render(&mut app, &ctx, vec![], vec![], Modifiers::NONE);
    let output = render(&mut app, &ctx, vec![], vec![], Modifiers::NONE);
    let position = text_center(&output, "Destination").unwrap();
    app.dnd.pending = Some(PendingDrop {
        position,
        paths: vec![file.clone()],
        internal: true,
        action: Some(Action::Copy),
        completion: Some(Completion::Wayland(1)),
    });
    render(&mut app, &ctx, vec![], vec![], Modifiers::NONE);
    assert_eq!(
        app.active_transfer.as_ref().unwrap().mode,
        file_ops::Mode::Copy
    );
    finish(&mut app);
    assert!(file.exists());
    assert_eq!(
        fs::read_to_string(target.join("source.txt")).unwrap(),
        "original"
    );
}
