use super::tab_drag::render;
use super::tab_menu::{choose, click, menu, three_tabs};
use super::*;
use egui::{Event, Modifiers, PointerButton, Shape};

fn key(app: &mut Workspace, ctx: &egui::Context, key: Key, modifiers: Modifiers) {
    for pressed in [true, false] {
        render(
            app,
            ctx,
            vec![Event::Key {
                key,
                physical_key: None,
                pressed,
                repeat: false,
                modifiers,
            }],
        );
    }
}
fn reopen(app: &mut Workspace, ctx: &egui::Context) {
    key(app, ctx, Key::T, Modifiers::CTRL | Modifiers::SHIFT);
}

#[test]
fn reopen_shortcut_restores_position_context_and_session_without_creating_extra_home_tab() {
    let root = tempfile::tempdir().unwrap();
    let ctx = egui::Context::default();
    let mut app = three_tabs(root.path(), &ctx);
    let file = root.path().join("Beta/file.txt");
    std::fs::write(&file, "test").unwrap();
    app.activate(1, &ctx);
    app.tabs[1].refresh();
    complete_navigation(&mut app.tabs[1]);
    app.tabs[1].query = "file".into();
    app.tabs[1].view = View::Icons;
    app.tabs[1].item_scale = 1.5;
    app.tabs[1].selected.insert(file.clone());
    app.tabs[1].selection_anchor = Some(file.clone());
    app.tabs[1].selection_cursor = Some(file.clone());
    let id = app.tabs[1].tab_id;
    key(&mut app, &ctx, Key::W, Modifiers::CTRL);
    assert_eq!(app.tabs.len(), 2);
    assert_eq!(app.closed_tabs.len(), 1);
    reopen(&mut app, &ctx);
    assert_eq!(app.tabs.len(), 3);
    assert_eq!(app.active, 1);
    assert_eq!(app.tabs[1].tab_id, id);
    complete_navigation(&mut app.tabs[1]);
    assert_eq!(app.tabs[1].query, "file");
    assert!(app.tabs[1].view == View::Icons);
    assert_eq!(app.tabs[1].item_scale, 1.5);
    assert!(app.tabs[1].selected.contains(&file));
    assert!(app.tabs[1].terminal.is_none() && !app.tabs[1].busy());
    let saved = crate::session::State::load(&root.path().join("session.json"))
        .unwrap()
        .unwrap();
    assert_eq!(
        saved.paths().unwrap(),
        ["Alpha", "Beta", "Gamma"].map(|s| root.path().join(s))
    );
    assert_eq!(saved.active, 1);
    reopen(&mut app, &ctx);
    assert_eq!(
        app.tabs.len(),
        3,
        "empty history must not fall through to Ctrl+T"
    );
}

#[test]
fn reopening_bulk_closed_tabs_uses_lifo_and_recovers_original_neighbor_order() {
    for label in ["Fechar outras abas", "Fechar abas à direita"] {
        let root = tempfile::tempdir().unwrap();
        let ctx = egui::Context::default();
        let mut app = three_tabs(root.path(), &ctx);
        let target = if label == "Fechar outras abas" {
            "Gamma"
        } else {
            "Alpha"
        };
        let output = menu(&mut app, &ctx, target);
        choose(&mut app, &ctx, &output, label);
        assert_eq!(app.tabs.len(), 1);
        assert_eq!(app.closed_tabs.len(), 2);
        let output = menu(&mut app, &ctx, target);
        choose(&mut app, &ctx, &output, "Reabrir aba fechada");
        assert_eq!(app.tabs.len(), 2);
        assert_eq!(
            app.tabs[app.active].location().file_name().unwrap(),
            if target == "Gamma" { "Beta" } else { "Gamma" }
        );
        reopen(&mut app, &ctx);
        assert_eq!(
            app.session_state().paths().unwrap(),
            ["Alpha", "Beta", "Gamma"].map(|s| root.path().join(s))
        );
        assert!(app.closed_tabs.is_empty());
    }
}

#[test]
fn plus_is_clickable_and_its_context_menu_reopens_closed_tabs() {
    let root = tempfile::tempdir().unwrap();
    let ctx = egui::Context::default();
    let mut app = three_tabs(root.path(), &ctx);
    app.close_tab(2, &ctx);
    let output = render(&mut app, &ctx, vec![]);
    let plus = output
        .shapes
        .iter()
        .find_map(|s| {
            if let Shape::Rect(r) = &s.shape
                && r.corner_radius.nw == 7
                && r.rect.width() == 36.0
            {
                Some(r.rect.center())
            } else {
                None
            }
        })
        .unwrap();
    click(&mut app, &ctx, plus, PointerButton::Secondary);
    let output = render(&mut app, &ctx, vec![]);
    choose(&mut app, &ctx, &output, "Reabrir aba fechada");
    assert_eq!(app.tabs.len(), 3);
    assert_eq!(app.tabs[app.active].location(), root.path().join("Gamma"));
    let output = render(&mut app, &ctx, vec![]);
    let plus = output
        .shapes
        .iter()
        .find_map(|s| {
            if let Shape::Rect(r) = &s.shape
                && r.corner_radius.nw == 7
                && r.rect.width() == 36.0
            {
                Some(r.rect.center())
            } else {
                None
            }
        })
        .unwrap();
    click(&mut app, &ctx, plus, PointerButton::Primary);
    assert_eq!(app.tabs.len(), 4);
    assert_eq!(app.tabs[app.active].location(), dirs::home_dir().unwrap());
}

#[test]
fn unavailable_non_utf8_closed_folder_can_be_reopened_without_losing_its_path() {
    use std::os::unix::ffi::OsStringExt;
    let root = tempfile::tempdir().unwrap();
    let raw = root
        .path()
        .join(std::ffi::OsString::from_vec(b"folder-\xff".to_vec()));
    std::fs::create_dir(&raw).unwrap();
    let ctx = egui::Context::default();
    let mut app = workspace(&ctx, raw.clone());
    app.add_tab(root.path().to_owned(), &ctx);
    complete_navigation(&mut app.tabs[1]);
    app.close_tab(0, &ctx);
    std::fs::remove_dir(&raw).unwrap();
    reopen(&mut app, &ctx);
    complete_navigation(&mut app.tabs[0]);
    assert_eq!(app.active, 0);
    assert_eq!(app.tabs[0].location(), raw);
    assert!(app.tabs[0].error.is_some());
    std::fs::create_dir(&raw).unwrap();
    app.tabs[0].refresh();
    complete_navigation(&mut app.tabs[0]);
    assert_eq!(app.tabs[0].current, raw);
    assert!(app.tabs[0].error.is_none());
}
