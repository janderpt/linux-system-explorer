use super::*;
use crate::app::tests::{complete_navigation, text_center};
use egui::{Event as Input, FullOutput, Modifiers, Pos2, RawInput, Rect};
fn render(app: &mut Explorer, ctx: &egui::Context, text: &[Input], advance: f64) -> FullOutput {
    ctx.run(
        RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1180.0, 760.0))),
            time: Some(ctx.input(|i| i.time) + advance),
            events: text.to_vec(),
            ..Default::default()
        },
        |ctx| app.show(ctx),
    )
}
fn text(value: &str) -> Input {
    Input::Text(value.into())
}
fn key(key: Key) -> Input {
    Input::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    }
}

#[test]
fn typing_selects_first_prefix_in_visible_order_without_filtering_or_opening_in_every_view() {
    for view in [View::Details, View::List, View::Icons] {
        let root = tempfile::tempdir().unwrap();
        for name in ["Abraao", "Aline", "Allan", "Argeu", "Bernard"] {
            std::fs::create_dir(root.path().join(name)).unwrap();
        }
        std::fs::write(root.path().join("Argeu.txt"), "file").unwrap();
        let ctx = egui::Context::default();
        let mut app = Explorer::with_context(&ctx, root.path().into());
        complete_navigation(&mut app);
        app.view = view;
        app.sidebar_options.remote.push(crate::network::Connection {
            url: "ftp://example.test/".into(),
            label: "FTP fixture".into(),
            mount: Some(root.path().into()),
        });
        app.selected.insert(root.path().join("Aline"));
        app.selection_cursor = Some(root.path().join("Aline"));
        render(&mut app, &ctx, &[], 0.05);
        render(&mut app, &ctx, &[text("aR")], 0.05);
        assert_eq!(app.selected, BTreeSet::from([root.path().join("Argeu")]));
        assert_eq!(app.selection_anchor, app.selection_cursor);
        assert_eq!(app.current, root.path());
        assert!(app.query.is_empty() && !app.loading && app.preview.is_none());
        render(&mut app, &ctx, &[text("geu.t")], 0.05);
        assert_eq!(app.selection_cursor, Some(root.path().join("Argeu.txt")));
        render(&mut app, &ctx, &[text("b")], 1.1);
        assert_eq!(app.selection_cursor, Some(root.path().join("Bernard")));
        render(&mut app, &ctx, &[text("zzz")], 1.1);
        assert_eq!(app.selection_cursor, Some(root.path().join("Bernard")));
        app.descending = true;
        render(&mut app, &ctx, &[text("al")], 1.1);
        assert_eq!(app.selection_cursor, Some(root.path().join("Allan")));
    }
}
#[test]
fn repeated_letters_cycle_and_space_opens_preview_immediately_after_type_selection() {
    let root = tempfile::tempdir().unwrap();
    for name in ["Beta-one.txt", "Beta-two.txt", "Hello world.txt"] {
        std::fs::write(root.path().join(name), "text preview").unwrap();
    }
    let ctx = egui::Context::default();
    let mut app = Explorer::with_context(&ctx, root.path().into());
    complete_navigation(&mut app);
    render(&mut app, &ctx, &[], 0.05);
    for name in ["Beta-one.txt", "Beta-two.txt", "Beta-one.txt"] {
        render(&mut app, &ctx, &[text("b")], 0.05);
        assert_eq!(app.selection_cursor, Some(root.path().join(name)));
    }
    render(&mut app, &ctx, &[text("hello world")], 1.1);
    assert_eq!(
        app.selection_cursor,
        Some(root.path().join("Hello world.txt"))
    );
    assert!(app.preview.is_none() && app.query.is_empty());
    render(&mut app, &ctx, &[key(Key::Space), text(" ")], 0.05);
    assert!(app.preview.is_some());
}
#[test]
fn type_selection_reveals_offscreen_items_respects_hidden_and_resets_for_clicks_and_navigation() {
    for view in [View::Details, View::List, View::Icons] {
        let root = tempfile::tempdir().unwrap();
        for i in 0..100 {
            std::fs::write(root.path().join(format!("file-{i:03}.txt")), "text").unwrap();
        }
        std::fs::write(root.path().join(".hidden.txt"), "hidden").unwrap();
        let ctx = egui::Context::default();
        let mut app = Explorer::with_context(&ctx, root.path().into());
        complete_navigation(&mut app);
        app.view = view;
        render(&mut app, &ctx, &[], 0.05);
        render(&mut app, &ctx, &[], 0.05);
        render(&mut app, &ctx, &[text("file-099")], 0.05);
        let mut output = render(&mut app, &ctx, &[], 0.05);
        for _ in 0..16 {
            output = render(&mut app, &ctx, &[], 0.05);
        }
        let position = text_center(&output, "file-099.txt").expect("selected item is revealed");
        assert!((0.0..760.0).contains(&position.y));
        let selected = app.selected.clone();
        render(&mut app, &ctx, &[text(".hi")], 1.1);
        assert_eq!(app.selected, selected);
        app.show_hidden = true;
        render(&mut app, &ctx, &[text(".hi")], 1.1);
        assert_eq!(app.selection_cursor, Some(root.path().join(".hidden.txt")));
        render(&mut app, &ctx, &[key(Key::Backspace)], 0.05);
        assert_eq!(app.type_ahead.prefix, ".h");
        render(&mut app, &ctx, &[key(Key::Escape)], 0.05);
        assert!(app.type_ahead.prefix.is_empty());
        render(&mut app, &ctx, &[text("file")], 0.05);
        let visible = app.visible_for_display();
        app.select_entry(&root.path().join("file-003.txt"), &visible, Modifiers::NONE);
        assert!(app.type_ahead.prefix.is_empty());
        render(&mut app, &ctx, &[text("file")], 0.05);
        app.refresh();
        assert!(app.type_ahead.prefix.is_empty());
    }
}
#[test]
fn focused_text_fields_keep_their_text_and_do_not_trigger_type_selection() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("alpha.txt"), "text").unwrap();
    let ctx = egui::Context::default();
    let mut app = Explorer::with_context(&ctx, root.path().into());
    complete_navigation(&mut app);
    app.focus_search = true;
    render(&mut app, &ctx, &[], 0.05);
    render(&mut app, &ctx, &[text("alpha")], 0.05);
    assert_eq!(app.query, "alpha");
    assert!(app.type_ahead.prefix.is_empty());
}

#[test]
fn enter_opens_the_folder_selected_by_typing_in_every_view_in_local_and_ftp_locations() {
    for view in [View::Details, View::List, View::Icons] {
        for ftp in [false, true] {
            let root = tempfile::tempdir().unwrap();
            let folder = root.path().join("Jander");
            std::fs::create_dir(&folder).unwrap();
            std::fs::write(folder.join("inside.txt"), "fixture").unwrap();
            let ctx = egui::Context::default();
            let mut app = Explorer::with_context(&ctx, root.path().into());
            complete_navigation(&mut app);
            app.view = view;
            if ftp {
                app.sidebar_options.remote.push(crate::network::Connection {
                    url: "ftp://example.test/usuarios/".into(),
                    label: "FTP fixture".into(),
                    mount: Some(root.path().into()),
                });
            }
            render(&mut app, &ctx, &[], 0.05);
            render(&mut app, &ctx, &[text("jand")], 0.05);
            assert_eq!(app.selection_cursor, Some(folder.clone()));
            render(&mut app, &ctx, &[key(Key::Enter)], 0.05);
            complete_navigation(&mut app);
            assert_eq!(app.current, folder);
            assert!(app.entries.iter().any(|entry| entry.name == "inside.txt"));
            assert!(app.query.is_empty());
            if ftp {
                assert_eq!(app.address, "ftp://example.test/usuarios/Jander");
            }
        }
    }
}

#[test]
fn enter_ignores_repeats_modifiers_text_fields_and_dialogs_and_uses_the_focused_selected_folder() {
    let root = tempfile::tempdir().unwrap();
    let first = root.path().join("First");
    let second = root.path().join("Second");
    for folder in [&first, &second] {
        std::fs::create_dir(folder).unwrap();
    }
    let ctx = egui::Context::default();
    let mut app = Explorer::with_context(&ctx, root.path().into());
    complete_navigation(&mut app);
    app.selected = BTreeSet::from([first.clone(), second.clone()]);
    app.selection_cursor = Some(second.clone());
    render(&mut app, &ctx, &[], 0.05);
    for modifiers in [Modifiers::CTRL, Modifiers::SHIFT, Modifiers::ALT] {
        let mut event = key(Key::Enter);
        if let Input::Key {
            modifiers: value, ..
        } = &mut event
        {
            *value = modifiers;
        }
        render(&mut app, &ctx, &[event], 0.05);
        assert_eq!(app.current, root.path());
    }
    let mut event = key(Key::Enter);
    if let Input::Key { repeat, .. } = &mut event {
        *repeat = true;
    }
    render(&mut app, &ctx, &[event], 0.05);
    assert_eq!(app.current, root.path());
    egui::Popup::open_id(&ctx, egui::Id::new("enter_popup_guard"));
    render(&mut app, &ctx, &[key(Key::Enter)], 0.05);
    assert_eq!(app.current, root.path());
    egui::Popup::close_all(&ctx);
    app.focus_search = true;
    render(&mut app, &ctx, &[], 0.05);
    render(&mut app, &ctx, &[key(Key::Enter)], 0.05);
    assert_eq!(app.current, root.path());
    if let Some(id) = ctx.memory(|memory| memory.focused()) {
        ctx.memory_mut(|memory| memory.surrender_focus(id));
    }
    app.properties = Some(filesystem::read_entry(&second).unwrap());
    render(&mut app, &ctx, &[key(Key::Enter)], 0.05);
    assert_eq!(app.current, root.path());
    app.properties = None;
    render(&mut app, &ctx, &[], 0.05);
    let mut release = key(Key::Enter);
    if let Input::Key { pressed, .. } = &mut release {
        *pressed = false;
    }
    render(&mut app, &ctx, &[release], 0.05);
    render(&mut app, &ctx, &[key(Key::Enter)], 0.05);
    complete_navigation(&mut app);
    assert_eq!(app.current, second);
}
