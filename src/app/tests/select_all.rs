use super::*;

fn ctrl_a() -> Event {
    Event::Key {
        key: Key::A,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers {
            ctrl: true,
            command: true,
            ..Modifiers::NONE
        },
    }
}

#[test]
fn ctrl_a_selects_all_displayed_items_in_every_view_and_respects_hidden_and_search() {
    for view in [View::Details, View::List, View::Icons] {
        let root = tempfile::tempdir().unwrap();
        for i in 0..80 {
            std::fs::write(root.path().join(format!("match-{i:03}.txt")), "unchanged").unwrap();
        }
        std::fs::write(root.path().join("other.txt"), "other").unwrap();
        std::fs::write(root.path().join(".hidden"), "hidden").unwrap();
        let ctx = egui::Context::default();
        let mut app = Explorer::with_context(&ctx, root.path().into());
        complete_navigation(&mut app);
        app.view = view;
        transfer_key(&mut app, &ctx, ctrl_a());
        assert_eq!(app.selected.len(), 81, "selects virtualized rows too");
        assert!(!app.selected.contains(&root.path().join(".hidden")));
        app.show_hidden = true;
        transfer_key(&mut app, &ctx, ctrl_a());
        assert_eq!(app.selected.len(), 82);
        app.query = "match-0".into();
        transfer_key(&mut app, &ctx, ctrl_a());
        assert_eq!(app.selected.len(), 80);
        app.query = "absent".into();
        transfer_key(&mut app, &ctx, ctrl_a());
        assert!(app.selected.is_empty() && app.selection_cursor.is_none());
    }
}

#[test]
fn ctrl_a_keeps_text_editing_and_dialogs_separate_from_file_selection() {
    let root = tempfile::tempdir().unwrap();
    for name in ["a.txt", "b.txt"] {
        std::fs::write(root.path().join(name), "content").unwrap();
    }
    let ctx = egui::Context::default();
    let mut app = Explorer::with_context(&ctx, root.path().into());
    complete_navigation(&mut app);
    app.selected.insert(root.path().join("a.txt"));
    app.settings_editor = Some(settings::Editor::new(app.preferences()));
    transfer_key(&mut app, &ctx, ctrl_a());
    assert_eq!(app.selected.len(), 1);
    app.settings_editor = None;
    let id = egui::Id::new("selection_text_field");
    let mut text = "field text".to_owned();
    let mut draw = |app: &mut Explorer, ui: &mut egui::Ui| {
        ui.add(egui::TextEdit::singleline(&mut text).id(id))
            .request_focus();
        app.shortcuts(ui.ctx());
        ui.min_rect()
    };
    frame(&mut app, &ctx, 1180.0, vec![], &mut draw);
    frame(&mut app, &ctx, 1180.0, vec![ctrl_a()], &mut draw);
    assert_eq!(app.selected.len(), 1);
    let state = egui::TextEdit::load_state(&ctx, id).unwrap();
    assert_eq!(
        state.cursor.char_range().unwrap().as_sorted_char_range(),
        0..10
    );
}
