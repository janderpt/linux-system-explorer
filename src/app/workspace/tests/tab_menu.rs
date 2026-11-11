use super::tab_drag::{render, tab_rect};
use super::*;
use crate::app::tests::text_center;
use egui::{Event, FullOutput, Modifiers, PointerButton, Pos2};

fn click(app: &mut Workspace, ctx: &egui::Context, pos: Pos2, button: PointerButton) {
    for pressed in [true, false] {
        render(
            app,
            ctx,
            vec![
                Event::PointerMoved(pos),
                Event::PointerButton {
                    pos,
                    button,
                    pressed,
                    modifiers: Modifiers::NONE,
                },
            ],
        );
    }
}
fn menu(app: &mut Workspace, ctx: &egui::Context, title: &str) -> FullOutput {
    render(app, ctx, vec![]);
    let output = render(app, ctx, vec![]);
    let point = tab_rect(&output, title).left_center() + Vec2::new(60.0, 0.0);
    click(app, ctx, point, PointerButton::Secondary);
    render(app, ctx, vec![])
}
fn choose(app: &mut Workspace, ctx: &egui::Context, output: &FullOutput, label: &str) {
    let point =
        text_center(output, label).unwrap_or_else(|| panic!("missing tab menu item {label}"));
    click(app, ctx, point, PointerButton::Primary);
}
fn three_tabs(root: &Path, ctx: &egui::Context) -> Workspace {
    for name in ["Alpha", "Beta", "Gamma"] {
        std::fs::create_dir(root.join(name)).unwrap();
    }
    let mut app = workspace(ctx, root.join("Alpha"));
    for name in ["Beta", "Gamma"] {
        app.add_tab(root.join(name), ctx);
    }
    for tab in &mut app.tabs {
        complete_navigation(tab);
    }
    app.session = crate::session::Store::at(Some(root.join("session.json")));
    app
}

#[test]
fn duplicate_inactive_tab_opens_adjacent_independent_copy_with_browsing_context() {
    let root = tempfile::tempdir().unwrap();
    let ctx = egui::Context::default();
    let mut app = three_tabs(root.path(), &ctx);
    let file = root.path().join("Alpha/file.txt");
    std::fs::write(&file, "test").unwrap();
    app.tabs[0].refresh();
    complete_navigation(&mut app.tabs[0]);
    app.tabs[0].history = History::default();
    app.tabs[0]
        .history
        .commit(root.path().to_owned(), Navigation::Visit);
    app.tabs[0]
        .history
        .commit(root.path().join("Alpha"), Navigation::Visit);
    app.tabs[0].query = "file".into();
    app.tabs[0].view = View::Icons;
    app.tabs[0].item_scale = 1.75;
    app.tabs[0].selected.insert(file.clone());
    app.tabs[0].selection_anchor = Some(file.clone());
    app.tabs[0].selection_cursor = Some(file.clone());
    let source_id = app.tabs[0].tab_id;
    app.tabs[2]
        .appearance
        .overrides
        .insert(Icon::DuplicateTab, "star".into());
    let output = menu(&mut app, &ctx, "Alpha");
    let star = icons::glyph("star").unwrap().character().to_string();
    assert!(
        output
            .shapes
            .iter()
            .any(|shape| matches!(&shape.shape, egui::Shape::Text(t) if t.galley.text() == star)),
        "menu uses the latest global icon preference, including on inactive tabs"
    );
    assert_eq!(
        app.active, 2,
        "right click should not activate an inactive tab"
    );
    for label in [
        "Duplicar aba",
        "Fechar outras abas",
        "Fechar abas à direita",
    ] {
        assert!(text_center(&output, label).is_some());
    }
    choose(&mut app, &ctx, &output, "Duplicar aba");
    assert_eq!(app.tabs.len(), 4);
    assert_eq!(app.active, 1);
    complete_navigation(&mut app.tabs[1]);
    let copy = &app.tabs[1];
    assert_ne!(copy.tab_id, source_id);
    assert_eq!(copy.current, app.tabs[0].current);
    assert_eq!(copy.query, "file");
    assert!(copy.view == View::Icons);
    assert_eq!(copy.item_scale, 1.75);
    assert!(copy.selected.contains(&file));
    assert_eq!(copy.history.back(), Some(root.path()));
    assert!(copy.terminal.is_none() && !copy.busy());
    app.tabs[1].query = "independent".into();
    app.tabs[1]
        .history
        .commit(root.path().join("Beta"), Navigation::Visit);
    assert_eq!(app.tabs[0].query, "file");
    assert_eq!(app.tabs[0].history.back(), Some(root.path()));
    let saved = crate::session::State::load(&root.path().join("session.json"))
        .unwrap()
        .unwrap();
    assert_eq!(
        saved.paths().unwrap(),
        ["Alpha", "Alpha", "Beta", "Gamma"].map(|name| root.path().join(name))
    );
    assert_eq!(saved.active, 1);
}

#[test]
fn context_closures_use_clicked_tab_and_persist_remaining_order_and_active_tab() {
    for (action, expected) in [
        ("Fechar outras abas", vec!["Beta"]),
        ("Fechar abas à direita", vec!["Alpha", "Beta"]),
    ] {
        let root = tempfile::tempdir().unwrap();
        let ctx = egui::Context::default();
        let mut app = three_tabs(root.path(), &ctx);
        let target_id = app.tabs[1].tab_id;
        let output = menu(&mut app, &ctx, "Beta");
        choose(&mut app, &ctx, &output, action);
        assert_eq!(app.tabs[app.active].tab_id, target_id);
        let saved = crate::session::State::load(&root.path().join("session.json"))
            .unwrap()
            .unwrap();
        assert_eq!(
            saved.paths().unwrap(),
            expected
                .iter()
                .map(|name| root.path().join(name))
                .collect::<Vec<_>>()
        );
        let fresh = egui::Context::default();
        let mut restored = workspace(&fresh, saved.paths().unwrap()[0].clone());
        restored.restore_tabs(&saved, &fresh);
        assert_eq!(restored.session_state(), saved);
    }
}

#[test]
fn close_right_preserves_active_tab_on_the_left_and_inapplicable_actions_are_disabled() {
    let root = tempfile::tempdir().unwrap();
    let ctx = egui::Context::default();
    let mut app = three_tabs(root.path(), &ctx);
    app.activate(0, &ctx);
    let output = menu(&mut app, &ctx, "Beta");
    choose(&mut app, &ctx, &output, "Fechar abas à direita");
    assert_eq!(app.active, 0);
    let output = menu(&mut app, &ctx, "Beta");
    let before = app.session_state();
    choose(&mut app, &ctx, &output, "Fechar abas à direita");
    assert_eq!(app.session_state(), before);
    // Close others, leaving Beta alone; both bulk-close actions should now do nothing.
    let output = menu(&mut app, &ctx, "Beta");
    choose(&mut app, &ctx, &output, "Fechar outras abas");
    for label in ["Fechar outras abas", "Fechar abas à direita"] {
        let output = menu(&mut app, &ctx, "Beta");
        choose(&mut app, &ctx, &output, label);
        assert_eq!(app.tabs.len(), 1);
    }
}

#[test]
fn bulk_close_keeps_busy_tabs_and_does_not_cancel_their_file_operations() {
    let root = tempfile::tempdir().unwrap();
    let ctx = egui::Context::default();
    let mut app = three_tabs(root.path(), &ctx);
    let source = root.path().join("Beta/conflict.txt");
    std::fs::write(&source, "source").unwrap();
    std::fs::write(root.path().join("Gamma/conflict.txt"), "existing").unwrap();
    app.tabs[1].start_file_transfer(
        file_ops::Clipboard {
            id: 1,
            mode: file_ops::Mode::Copy,
            paths: vec![source],
        },
        root.path().join("Gamma"),
        &ctx,
    );
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while app.tabs[1].transfer_conflict.is_none() {
        app.tabs[1].poll();
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    let busy_id = app.tabs[1].tab_id;
    let output = menu(&mut app, &ctx, "Alpha");
    choose(&mut app, &ctx, &output, "Fechar outras abas");
    assert_eq!(app.tabs.len(), 2);
    assert_eq!(app.tabs[1].tab_id, busy_id);
    assert!(app.tabs[1].busy());
    let prompt = app.tabs[1].transfer_conflict.take().unwrap();
    prompt
        .reply
        .send(file_ops::Decision {
            choice: file_ops::Choice::Skip,
            apply_all: false,
        })
        .unwrap();
    while app.tabs[1].busy() {
        app.tabs[1].poll();
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    assert_eq!(
        std::fs::read_to_string(root.path().join("Gamma/conflict.txt")).unwrap(),
        "existing"
    );
}
