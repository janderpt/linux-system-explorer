use super::*;
use crate::app::tests::{complete_navigation, text_center};
use std::fs;
fn finish(app: &mut Explorer) {
    app.update_search();
    app.recursive_search.due = Instant::now();
    let deadline = Instant::now() + Duration::from_secs(4);
    while app.recursive_search.running() {
        app.update_search();
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
}
fn frame(app: &mut Explorer, ctx: &egui::Context, events: Vec<egui::Event>) -> egui::FullOutput {
    ctx.run(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                Vec2::new(1180.0, 760.0),
            )),
            events,
            ..Default::default()
        },
        |ctx| {
            let visible = app.visible_for_display();
            app.top_toolbar(ctx);
            app.contents(ctx, &visible);
            app.status_bar(ctx, &visible);
        },
    )
}
#[test]
fn recursive_results_show_relative_paths_in_all_views_and_keep_real_paths_for_copy_and_open() {
    let root = tempfile::tempdir().unwrap();
    for dir in ["one", "two"] {
        fs::create_dir(root.path().join(dir)).unwrap();
        fs::write(root.path().join(dir).join("report.txt"), dir).unwrap();
    }
    let ctx = egui::Context::default();
    let mut app = Explorer::with_context(&ctx, root.path().into());
    complete_navigation(&mut app);
    app.query = "REPORT".into();
    finish(&mut app);
    assert_eq!(app.entries.len(), 2);
    let chosen = root.path().join("one/report.txt");
    assert!(app.entries.iter().all(|e| e.name == "report.txt"));
    for view in [View::Details, View::List, View::Icons] {
        app.view = view;
        frame(&mut app, &ctx, vec![]);
        let out = frame(&mut app, &ctx, vec![]);
        assert!(text_center(&out, "one/report.txt").is_some());
        assert!(text_center(&out, "two/report.txt").is_some());
    }
    app.selected = BTreeSet::from([chosen.clone()]);
    app.capture_files(file_ops::Mode::Copy, &ctx);
    assert_eq!(
        app.file_clipboard.as_ref().unwrap().paths,
        vec![chosen.clone()]
    );
    assert_eq!(fs::read_to_string(chosen).unwrap(), "one");
    app.query = "one".into();
    finish(&mut app);
    let entry = app.entries.iter().find(|e| e.is_dir).unwrap().clone();
    app.open(&entry, &ctx);
    complete_navigation(&mut app);
    assert_eq!(app.current, root.path().join("one"));
    assert!(app.query.is_empty());
    assert!(!app.recursive_search.active());
    assert_eq!(app.entries[0].name, "report.txt");
}
#[test]
fn query_changes_hidden_switch_clear_and_refresh_never_mix_previous_results_or_change_layout() {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir_all(root.path().join("sub/.secret")).unwrap();
    for file in ["alpha.txt", "sub/beta.txt", "sub/.secret/beta.txt"] {
        fs::write(root.path().join(file), "keep").unwrap();
    }
    let ctx = egui::Context::default();
    let mut app = Explorer::with_context(&ctx, root.path().into());
    complete_navigation(&mut app);
    let layout = app.current_layout();
    let base: Vec<_> = app.entries.iter().map(|e| e.path.clone()).collect();
    app.query = "beta".into();
    finish(&mut app);
    assert_eq!(app.entries.len(), 1);
    app.show_hidden = true;
    finish(&mut app);
    assert_eq!(app.entries.len(), 2);
    app.query = "alpha".into();
    app.update_search();
    assert!(app.entries.iter().all(|e| e.name == "alpha.txt"));
    finish(&mut app);
    assert_eq!(app.entries.len(), 1);
    app.query.clear();
    app.update_search();
    assert!(!app.recursive_search.active());
    let restored: Vec<_> = app.entries.iter().map(|e| e.path.clone()).collect();
    assert_eq!(base, restored);
    assert!(app.current_layout() == layout);
    app.query = "beta".into();
    finish(&mut app);
    fs::remove_file(root.path().join("sub/beta.txt")).unwrap();
    app.refresh();
    complete_navigation(&mut app);
    finish(&mut app);
    assert_eq!(app.entries.len(), 1);
    assert_eq!(
        app.entries[0].path,
        root.path().join("sub/.secret/beta.txt")
    );
    app.query = "never".into();
    app.update_search();
    app.query = "beta".into();
    finish(&mut app);
    assert_eq!(app.entries.len(), 1);
}
#[test]
fn escape_in_filter_restores_direct_listing_and_new_folder_cancels_a_pending_search() {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir(root.path().join("sub")).unwrap();
    fs::write(root.path().join("sub/result.txt"), "data").unwrap();
    let ctx = egui::Context::default();
    let mut app = Explorer::with_context(&ctx, root.path().into());
    complete_navigation(&mut app);
    app.query = "result".into();
    finish(&mut app);
    app.focus_search = true;
    frame(&mut app, &ctx, vec![]);
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::Key {
            key: Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }],
    );
    app.update_search();
    assert!(app.query.is_empty());
    assert_eq!(app.entries.len(), 1);
    assert!(app.entries[0].is_dir);
    app.query = "old".into();
    app.update_search();
    app.navigate(root.path().join("sub"), Navigation::Visit);
    complete_navigation(&mut app);
    finish(&mut app);
    assert!(app.query.is_empty());
    assert_eq!(app.entries[0].name, "result.txt");
}
