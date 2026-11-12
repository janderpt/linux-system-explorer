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
            time: Some(ctx.input(|input| input.time) + 0.2),
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

fn click(
    app: &mut Explorer,
    ctx: &egui::Context,
    pos: egui::Pos2,
    button: egui::PointerButton,
) -> egui::FullOutput {
    for pressed in [true, false] {
        frame(
            app,
            ctx,
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
    }
    frame(app, ctx, vec![])
}

#[test]
fn result_context_menu_opens_parent_selects_only_clicked_item_and_scrolls_to_it_in_every_view() {
    for view in [View::Details, View::List, View::Icons] {
        let root = tempfile::tempdir().unwrap();
        for name in ["A", "B"] {
            fs::create_dir(root.path().join(name)).unwrap();
            fs::write(root.path().join(name).join("zzz-hit"), name).unwrap();
        }
        for i in 0..75 {
            fs::write(root.path().join("A").join(format!("item-{i:03}")), "keep").unwrap();
        }
        let target = root.path().join("A/zzz-hit");
        let other = root.path().join("B/zzz-hit");
        let ctx = egui::Context::default();
        let mut app = Explorer::with_context(&ctx, root.path().into());
        complete_navigation(&mut app);
        app.view = view;
        let parent_layout = FolderLayout {
            view,
            ..Default::default()
        };
        app.folder_layouts.insert(
            settings::folder_key(&root.path().join("A")),
            parent_layout.clone(),
        );
        app.query = "hit".into();
        finish(&mut app);
        app.selected = BTreeSet::from([target.clone(), other.clone()]);
        frame(&mut app, &ctx, vec![]);
        let output = frame(&mut app, &ctx, vec![]);
        let point = text_center(&output, "A/zzz-hit").unwrap();
        let output = click(&mut app, &ctx, point, egui::PointerButton::Secondary);
        let point = text_center(&output, "Abrir local do arquivo")
            .expect("search results offer their containing folder");
        click(&mut app, &ctx, point, egui::PointerButton::Primary);
        complete_navigation(&mut app);
        assert_eq!(app.current, root.path().join("A"));
        assert!(app.query.is_empty());
        assert!(!app.recursive_search.active());
        assert_eq!(app.selected, BTreeSet::from([target.clone()]));
        assert_eq!(app.selection_cursor, Some(target.clone()));
        assert!(app.current_layout() == parent_layout);
        assert!(app.reveal_selection);
        for _ in 0..3 {
            frame(&mut app, &ctx, vec![]);
        }
        let output = frame(&mut app, &ctx, vec![]);
        assert!(output.shapes.iter().any(|shape|matches!(&shape.shape,egui::Shape::Text(text) if text.galley.text()=="zzz-hit" && shape.clip_rect.contains(text.pos+text.galley.size()*0.5))),"selected item must scroll into view");
        let point = text_center(&output, "zzz-hit").unwrap();
        let output = click(&mut app, &ctx, point, egui::PointerButton::Secondary);
        assert!(
            text_center(&output, "Abrir local do arquivo").is_none(),
            "ordinary folder menus keep their existing actions"
        );
        assert_eq!(fs::read_to_string(target).unwrap(), "A");
        assert_eq!(fs::read_to_string(other).unwrap(), "B");
    }
}

#[test]
fn result_location_handles_same_folder_directories_hidden_raw_names_and_failed_navigation() {
    use std::{ffi::OsString, os::unix::ffi::OsStringExt};
    let root = tempfile::tempdir().unwrap();
    let parent = root.path().join("parent");
    fs::create_dir_all(parent.join("hit-folder")).unwrap();
    fs::write(root.path().join("hit-root"), "root").unwrap();
    let raw = parent.join(OsString::from_vec(b".hit-\xff".to_vec()));
    fs::write(&raw, "raw").unwrap();
    let ctx = egui::Context::default();
    let mut app = Explorer::with_context(&ctx, root.path().into());
    complete_navigation(&mut app);
    app.query = "hit".into();
    finish(&mut app);
    let entry = app
        .entries
        .iter()
        .find(|e| e.path == root.path().join("hit-root"))
        .unwrap()
        .clone();
    app.open_result_location(&entry, &ctx);
    complete_navigation(&mut app);
    assert_eq!(app.current, root.path());
    assert!(app.query.is_empty());
    assert_eq!(app.selected, BTreeSet::from([entry.path]));
    app.query = "hit-folder".into();
    finish(&mut app);
    let directory = app.entries.iter().find(|e| e.is_dir).unwrap().clone();
    app.open_result_location(&directory, &ctx);
    complete_navigation(&mut app);
    assert_eq!(app.current, parent);
    assert_eq!(app.selected, BTreeSet::from([directory.path]));
    app.show_hidden = true;
    app.query = "hit".into();
    finish(&mut app);
    let entry = app.entries.iter().find(|e| e.path == raw).unwrap().clone();
    app.open_result_location(&entry, &ctx);
    app.show_hidden = false;
    complete_navigation(&mut app);
    assert_eq!(app.selected, BTreeSet::from([raw.clone()]));
    assert!(app.reveal_hidden && app.hidden_visible() && !app.show_hidden);
    assert!(app.query.is_empty());
    app.navigate(root.path().into(), Navigation::Visit);
    complete_navigation(&mut app);
    let missing_dir = root.path().join("missing");
    fs::create_dir(&missing_dir).unwrap();
    fs::write(missing_dir.join("hit-lost"), "lost").unwrap();
    app.refresh();
    complete_navigation(&mut app);
    app.query = "lost".into();
    finish(&mut app);
    let entry = app.entries[0].clone();
    fs::remove_file(&entry.path).unwrap();
    fs::remove_dir(&missing_dir).unwrap();
    app.open_result_location(&entry, &ctx);
    complete_navigation(&mut app);
    assert_eq!(app.current, root.path());
    assert_eq!(app.query, "lost");
    assert!(app.pending_external.is_none());
    assert!(!app.notice_toasts.is_empty());
    assert_eq!(fs::read_to_string(raw).unwrap(), "raw");
}
