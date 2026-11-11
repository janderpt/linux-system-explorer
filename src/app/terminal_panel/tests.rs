use super::*;
#[test]
fn terminal_keys_preserve_control_sequences_and_application_cursor_mode() {
    assert_eq!(
        key_bytes(Key::C, egui::Modifiers::CTRL, false),
        Some(vec![3])
    );
    assert_eq!(
        key_bytes(Key::D, egui::Modifiers::CTRL, false),
        Some(vec![4])
    );
    assert_eq!(
        key_bytes(Key::ArrowUp, egui::Modifiers::NONE, false),
        Some(b"\x1b[A".to_vec())
    );
    assert_eq!(
        key_bytes(Key::ArrowUp, egui::Modifiers::NONE, true),
        Some(b"\x1bOA".to_vec())
    );
    assert_eq!(
        key_bytes(Key::Tab, egui::Modifiers::SHIFT, false),
        Some(b"\x1b[Z".to_vec())
    );
    assert_eq!(
        key_bytes(Key::Delete, egui::Modifiers::NONE, false),
        Some(b"\x1b[3~".to_vec())
    );
}
#[test]
fn terminal_dock_uses_requested_folder_and_leaves_explorer_untouched() {
    let root = tempfile::tempdir().unwrap();
    let folder = root.path().join("shell folder");
    std::fs::create_dir(&folder).unwrap();
    let ctx = egui::Context::default();
    let mut app = Explorer::with_context(&ctx, root.path().to_owned());
    crate::app::tests::complete_navigation(&mut app);
    app.open_terminal(folder.clone(), &ctx);
    let panel = app.terminal.as_ref().unwrap();
    assert_eq!(panel.sessions.len(), 1);
    assert_eq!(panel.sessions[0].cwd(), folder);
    assert_eq!(app.current, root.path());
    app.open_terminal(folder.clone(), &ctx);
    assert_eq!(app.terminal.as_ref().unwrap().sessions.len(), 1);
    let output = ctx.run(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                Vec2::new(1100.0, 700.0),
            )),
            ..Default::default()
        },
        |ctx| app.terminal_panel(ctx),
    );
    assert!(!output.shapes.is_empty());
    assert!(app.terminal_focused(&ctx));
    app.terminal.as_mut().unwrap().visible = false;
    assert!(!app.terminal_focused(&ctx));
    assert!(app.terminal.as_ref().unwrap().sessions[0].alive());
    app.open_terminal(root.path().to_owned(), &ctx);
    assert_eq!(app.terminal.as_ref().unwrap().sessions.len(), 2);
    assert_eq!(app.terminal.as_ref().unwrap().sessions[0].cwd(), folder);
}

#[test]
fn terminal_focus_keeps_delete_and_copy_out_of_explorer_shortcuts() {
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join("keep.txt");
    std::fs::write(&file, "keep").unwrap();
    let ctx = egui::Context::default();
    let mut app = Explorer::with_context(&ctx, root.path().to_owned());
    crate::app::tests::complete_navigation(&mut app);
    app.selected.insert(file.clone());
    app.open_terminal(root.path().to_owned(), &ctx);
    let _ = ctx.run(egui::RawInput::default(), |ctx| app.terminal_panel(ctx));
    for (key, modifiers) in [
        (Key::Delete, egui::Modifiers::NONE),
        (Key::C, egui::Modifiers::CTRL),
    ] {
        let _ = ctx.run(
            egui::RawInput {
                modifiers,
                events: vec![egui::Event::Key {
                    key,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers,
                }],
                ..Default::default()
            },
            |ctx| {
                app.shortcuts(ctx);
                app.terminal_panel(ctx);
            },
        );
        assert!(app.delete_confirmation.is_none());
        assert!(app.file_clipboard.is_none());
        assert!(file.exists());
    }
}
