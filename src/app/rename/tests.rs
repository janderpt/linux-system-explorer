use super::*;
use crate::app::tests::{complete_navigation, text_center};
use egui::{Event as InputEvent, FullOutput, Modifiers, PointerButton, Pos2, RawInput, Rect};
use std::{
    fs,
    time::{Duration, Instant},
};

fn frame(app: &mut Explorer, ctx: &egui::Context, events: Vec<InputEvent>) -> FullOutput {
    ctx.run(
        RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1100.0, 760.0))),
            time: Some(ctx.input(|i| i.time) + 0.2),
            events,
            ..Default::default()
        },
        |ctx| {
            app.poll();
            app.process_transfer_refresh();
            app.shortcuts(ctx);
            app.top_toolbar(ctx);
            let visible = filesystem::visible_entries(
                &app.entries,
                &app.query,
                app.show_hidden,
                app.sort,
                app.descending,
            );
            app.contents(ctx, &visible);
            app.rename_dialog(ctx);
        },
    )
}
fn key(key: Key) -> InputEvent {
    InputEvent::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    }
}
fn click(app: &mut Explorer, ctx: &egui::Context, pos: Pos2, button: PointerButton) {
    for pressed in [true, false] {
        frame(
            app,
            ctx,
            vec![
                InputEvent::PointerMoved(pos),
                InputEvent::PointerButton {
                    pos,
                    button,
                    pressed,
                    modifiers: Modifiers::NONE,
                },
            ],
        );
    }
}

#[test]
fn f2_context_enter_cancel_and_folder_preferences_work_in_all_views() {
    for view in [View::Details, View::List, View::Icons] {
        let root = tempfile::tempdir().unwrap();
        let file = root.path().join("old.txt");
        fs::write(&file, "original").unwrap();
        let ctx = egui::Context::default();
        let mut app = Explorer::with_context(&ctx, root.path().to_owned());
        complete_navigation(&mut app);
        app.view = view;
        frame(&mut app, &ctx, vec![]);
        let output = frame(&mut app, &ctx, vec![]);
        click(
            &mut app,
            &ctx,
            text_center(&output, "old.txt").unwrap(),
            PointerButton::Primary,
        );
        frame(&mut app, &ctx, vec![key(Key::F2)]);
        frame(&mut app, &ctx, vec![InputEvent::Text("new".into())]);
        assert_eq!(app.rename.as_ref().unwrap().name, "new.txt");
        frame(&mut app, &ctx, vec![key(Key::Enter)]);
        let deadline = Instant::now() + Duration::from_secs(3);
        while app.rename.is_some() {
            frame(&mut app, &ctx, vec![]);
            assert!(Instant::now() < deadline);
            thread::sleep(Duration::from_millis(1));
        }
        app.process_transfer_refresh();
        complete_navigation(&mut app);
        assert!(root.path().join("new.txt").exists() && !file.exists());
        assert!(app.selected.contains(&root.path().join("new.txt")));
        let output = frame(&mut app, &ctx, vec![]);
        click(
            &mut app,
            &ctx,
            text_center(&output, "new.txt").unwrap(),
            PointerButton::Secondary,
        );
        let output = frame(&mut app, &ctx, vec![]);
        click(
            &mut app,
            &ctx,
            text_center(&output, "Renomear").unwrap(),
            PointerButton::Primary,
        );
        assert!(app.rename.is_some());
        frame(&mut app, &ctx, vec![key(Key::Escape)]);
        assert!(app.rename.is_none());
        assert!(root.path().join("new.txt").exists());
    }
    let root = tempfile::tempdir().unwrap();
    let folder = root.path().join("old");
    fs::create_dir(&folder).unwrap();
    let ctx = egui::Context::default();
    let mut app = Explorer::with_context(&ctx, root.path().to_owned());
    complete_navigation(&mut app);
    app.sidebar_options
        .pinned
        .push(locations::Shortcut::new(folder.clone()));
    let custom = FolderLayout {
        item_scale: 1.75,
        ..Default::default()
    };
    app.folder_layouts
        .insert(settings::folder_key(&folder.join("nested")), custom.clone());
    app.selected.insert(folder.clone());
    app.capture_files(file_ops::Mode::Cut, &ctx);
    let entry = filesystem::read_entry(&folder).unwrap();
    let destination = crate::renames::rename(&entry, "new").unwrap();
    app.finish_rename(&entry, &destination, &ctx);
    assert!(app.selected.contains(&destination));
    assert_eq!(app.sidebar_options.pinned[0].path, destination);
    assert_eq!(
        app.file_clipboard.as_ref().unwrap().paths,
        vec![destination.clone()]
    );
    assert_eq!(
        app.folder_layouts[&settings::folder_key(&destination.join("nested"))].item_scale,
        custom.item_scale
    );
}
