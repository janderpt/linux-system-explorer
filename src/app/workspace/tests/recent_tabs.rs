use super::closed_tabs::reopen;
use super::tab_drag::render;
use super::tab_menu::{choose, click, menu, three_tabs};
use super::*;
use egui::{PointerButton, Shape};

fn recent_menu(app: &mut Workspace, ctx: &egui::Context, title: &str) -> egui::FullOutput {
    let output = menu(app, ctx, title);
    choose(app, ctx, &output, "Abas fechadas recentemente");
    render(app, ctx, vec![]);
    render(app, ctx, vec![])
}

#[test]
fn recent_menu_reopens_an_older_entry_without_consuming_the_latest_and_saves_it() {
    let root = tempfile::tempdir().unwrap();
    let ctx = egui::Context::default();
    let mut app = three_tabs(root.path(), &ctx);
    let output = menu(&mut app, &ctx, "Gamma");
    choose(&mut app, &ctx, &output, "Fechar outras abas");
    let output = recent_menu(&mut app, &ctx, "Gamma");
    let alpha = format!("Alpha\n{}", root.path().join("Alpha").display());
    let beta = format!("Beta\n{}", root.path().join("Beta").display());
    let a = crate::app::tests::text_center(&output, &alpha).expect("older tab should be listed");
    let b = crate::app::tests::text_center(&output, &beta).expect("newest tab should be listed");
    assert!(b.y < a.y, "most recently closed tab comes first");
    for shape in &output.shapes {
        if let Shape::Text(t) = &shape.shape
            && (t.galley.text() == alpha || t.galley.text() == beta)
        {
            assert_eq!(t.galley.rows.len(), 2, "folder and path use separate rows");
        }
    }
    click(&mut app, &ctx, a, PointerButton::Primary);
    assert_eq!(app.tabs[app.active].location(), root.path().join("Alpha"));
    assert_eq!(app.closed_tabs.len(), 1);
    assert_eq!(
        app.closed_tabs.back().unwrap().path,
        root.path().join("Beta")
    );
    assert!(!egui::Popup::is_any_open(&ctx));
    let saved = crate::session::State::load(&root.path().join("session.json"))
        .unwrap()
        .unwrap();
    assert_eq!(saved.closed_tabs.len(), 1);
    drop(app);
    let ctx = egui::Context::default();
    let mut app = workspace(&ctx, saved.paths().unwrap()[0].clone());
    app.restore_tabs(&saved, &ctx);
    reopen(&mut app, &ctx);
    assert_eq!(app.tabs[app.active].location(), root.path().join("Beta"));
    assert!(app.closed_tabs.is_empty());
}

#[test]
fn recent_entries_with_identical_paths_keep_their_independent_context() {
    let root = tempfile::tempdir().unwrap();
    let repeated = root.path().join("Repeated");
    std::fs::create_dir(&repeated).unwrap();
    let ctx = egui::Context::default();
    let mut app = workspace(&ctx, root.path().to_owned());
    for query in ["older context", "latest context"] {
        app.add_tab(repeated.clone(), &ctx);
        complete_navigation(&mut app.tabs[1]);
        app.tabs[1].query = query.into();
        app.close_tab(1, &ctx);
    }
    let title = root.path().file_name().unwrap().to_string_lossy();
    let output = recent_menu(&mut app, &ctx, &title);
    let label = format!("Repeated\n{}", repeated.display());
    let mut entries: Vec<_> = output
        .shapes
        .iter()
        .filter_map(|s| {
            if let Shape::Text(t) = &s.shape
                && t.galley.text() == label
            {
                Some(t.pos + t.galley.size() * 0.5)
            } else {
                None
            }
        })
        .collect();
    entries.sort_by(|a, b| a.y.total_cmp(&b.y));
    assert_eq!(entries.len(), 2);
    click(&mut app, &ctx, entries[1], PointerButton::Primary);
    assert_eq!(app.tabs[app.active].query, "older context");
    assert_eq!(
        app.closed_tabs
            .back()
            .unwrap()
            .context
            .as_ref()
            .unwrap()
            .query,
        "latest context"
    );
    reopen(&mut app, &ctx);
    assert_eq!(app.tabs[app.active].query, "latest context");
}

#[test]
fn recent_history_scrolls_to_older_entries_in_a_bounded_menu() {
    let root = tempfile::tempdir().unwrap();
    let ctx = egui::Context::default();
    let mut app = workspace(&ctx, root.path().to_owned());
    for i in 0..25 {
        let path = root.path().join(format!("Folder {i:02}"));
        std::fs::create_dir(&path).unwrap();
        app.add_tab(path, &ctx);
        complete_navigation(&mut app.tabs[1]);
        app.close_tab(1, &ctx);
    }
    let title = root.path().file_name().unwrap().to_string_lossy();
    let output = recent_menu(&mut app, &ctx, &title);
    let latest = format!("Folder 24\n{}", root.path().join("Folder 24").display());
    let point = crate::app::tests::text_center(&output, &latest).unwrap() + Vec2::new(0.0, 80.0);
    render(
        &mut app,
        &ctx,
        vec![
            egui::Event::PointerMoved(point),
            egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: Vec2::new(0.0, -10000.0),
                modifiers: egui::Modifiers::NONE,
            },
        ],
    );
    for _ in 0..15 {
        render(&mut app, &ctx, vec![]);
    }
    let output = render(&mut app, &ctx, vec![]);
    let oldest = format!("Folder 00\n{}", root.path().join("Folder 00").display());
    let center = crate::app::tests::text_center(&output, &oldest).unwrap();
    assert!(ctx.content_rect().contains(center));
    click(&mut app, &ctx, center, PointerButton::Primary);
    assert_eq!(
        app.tabs[app.active].location(),
        root.path().join("Folder 00")
    );
    assert_eq!(app.closed_tabs.len(), 24);
}
