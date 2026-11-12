use super::*;
use crate::app::{tests::complete_navigation, workspace::tests::workspace};
use egui::{Event, FullOutput, Modifiers, PointerButton, Pos2, RawInput, Rect};

fn render(
    app: &mut Workspace,
    ctx: &egui::Context,
    events: Vec<Event>,
    modifiers: Modifiers,
) -> FullOutput {
    ctx.run(
        RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1180.0, 760.0))),
            time: Some(ctx.input(|i| i.time) + 0.05),
            events,
            modifiers,
            ..Default::default()
        },
        |ctx| app.show(ctx),
    )
}
fn click(app: &mut Workspace, ctx: &egui::Context, position: Pos2) {
    for pressed in [true, false] {
        render(
            app,
            ctx,
            vec![
                Event::PointerMoved(position),
                Event::PointerButton {
                    pos: position,
                    button: PointerButton::Primary,
                    pressed,
                    modifiers: Modifiers::NONE,
                },
            ],
            Modifiers::NONE,
        );
    }
}
fn points(output: &FullOutput, text: &str) -> Vec<Pos2> {
    output
        .shapes
        .iter()
        .filter_map(|shape| match &shape.shape {
            egui::Shape::Text(t) if t.galley.text() == text => Some(t.pos + t.galley.size() * 0.5),
            _ => None,
        })
        .collect()
}
fn settle(app: &mut Workspace) {
    for tab in &mut app.tabs {
        complete_navigation(tab);
    }
}

#[test]
fn session_restores_pane_folders_active_side_and_resized_ratio_after_reordering() {
    let root = tempfile::tempdir().unwrap();
    let folders = ["Left", "Right", "Hidden"].map(|name| root.path().join(name));
    for path in &folders {
        std::fs::create_dir(path).unwrap();
    }
    let file = root.path().join("session.json");
    let ctx = egui::Context::default();
    let mut app = workspace(&ctx, folders[0].clone());
    app.session = crate::session::Store::at(Some(file.clone()));
    app.toggle_split(&ctx);
    settle(&mut app);
    let [left, right] = app.split_indices().unwrap();
    app.tabs[right].navigate(folders[1].clone(), Navigation::Visit);
    app.append_tab(folders[2].clone(), &ctx);
    settle(&mut app);
    let right_id = app.tabs[right].tab_id;
    app.reorder_tab(right_id, 0);
    app.activate(0, &ctx);
    render(&mut app, &ctx, vec![], Modifiers::NONE);
    let panes = app.split.as_ref().unwrap().rects.unwrap();
    let start = egui::pos2(panes[0].right() + 4.0, panes[0].center().y);
    render(
        &mut app,
        &ctx,
        vec![
            Event::PointerMoved(start),
            Event::PointerButton {
                pos: start,
                button: PointerButton::Primary,
                pressed: true,
                modifiers: Modifiers::NONE,
            },
        ],
        Modifiers::NONE,
    );
    let end = start + Vec2::new(90.0, 0.0);
    render(
        &mut app,
        &ctx,
        vec![Event::PointerMoved(end)],
        Modifiers::NONE,
    );
    render(
        &mut app,
        &ctx,
        vec![Event::PointerButton {
            pos: end,
            button: PointerButton::Primary,
            pressed: false,
            modifiers: Modifiers::NONE,
        }],
        Modifiers::NONE,
    );
    let original_ratio = app.split.as_ref().unwrap().ratio;
    assert!(original_ratio > 0.55);
    eframe::App::on_exit(&mut app, None);
    drop(app);

    let startup = crate::session::Startup::at(Some(file.clone()), true);
    let saved = startup.saved.unwrap();
    assert_eq!(
        saved.paths().unwrap(),
        vec![
            folders[1].clone(),
            folders[left].clone(),
            folders[2].clone()
        ]
    );
    assert_eq!(saved.split.unwrap().left, 1);
    assert_eq!(saved.split.unwrap().right, 0);
    let ctx = egui::Context::default();
    let mut restored = workspace(&ctx, saved.paths().unwrap()[0].clone());
    restored.session = startup.store;
    restored.restore_tabs(&saved, &ctx);
    assert_eq!(
        restored.tabs.len(),
        3,
        "restoring must not create an extra pane tab"
    );
    settle(&mut restored);
    render(&mut restored, &ctx, vec![], Modifiers::NONE);
    let [left, right] = restored.split_indices().unwrap();
    assert_eq!(restored.tabs[left].current, folders[0]);
    assert_eq!(restored.tabs[right].current, folders[1]);
    assert_eq!(restored.active, right);
    let split = restored.split.as_ref().unwrap();
    assert!((split.ratio - original_ratio).abs() < 0.0001);
    let panes = split.rects.unwrap();
    assert!(
        (panes[0].width() / (panes[0].width() + panes[1].width()) - original_ratio).abs() < 0.001
    );
    assert_eq!(restored.session_state(), saved);
}

#[test]
fn duplicate_pane_folders_restore_and_disabling_or_closing_a_pane_clears_saved_split() {
    for close in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let file = root.path().join("session.json");
        let ctx = egui::Context::default();
        let mut app = workspace(&ctx, root.path().into());
        app.session = crate::session::Store::at(Some(file.clone()));
        app.toggle_split(&ctx);
        settle(&mut app);
        render(&mut app, &ctx, vec![], Modifiers::NONE);
        let saved = crate::session::State::load(&file).unwrap().unwrap();
        let ctx = egui::Context::default();
        let mut restored = workspace(&ctx, root.path().into());
        restored.session = crate::session::Store::at(Some(file.clone()));
        restored.restore_tabs(&saved, &ctx);
        settle(&mut restored);
        assert_eq!(restored.tabs.len(), 2);
        assert!(restored.split.is_some());
        assert_ne!(restored.tabs[0].tab_id, restored.tabs[1].tab_id);
        if close {
            restored.close_tab(1, &ctx);
        } else {
            restored.toggle_split(&ctx);
        }
        render(&mut restored, &ctx, vec![], Modifiers::NONE);
        let saved = crate::session::State::load(&file).unwrap().unwrap();
        assert!(saved.split.is_none());
        assert_eq!(saved.tabs.len(), if close { 1 } else { 2 });
        let ctx = egui::Context::default();
        let mut reopened = workspace(&ctx, root.path().into());
        reopened.restore_tabs(&saved, &ctx);
        assert!(reopened.split.is_none());
    }
}

#[test]
fn toolbar_opens_two_equal_independent_panes_and_toggles_back_without_losing_folders() {
    let root = tempfile::tempdir().unwrap();
    let child = root.path().join("Child");
    std::fs::create_dir(&child).unwrap();
    let ctx = egui::Context::default();
    let mut app = workspace(&ctx, root.path().into());
    let _ = render(&mut app, &ctx, vec![], Modifiers::NONE);
    let output = render(&mut app, &ctx, vec![], Modifiers::NONE);
    let glyph = icons::glyph(Icon::SplitView.default_name())
        .unwrap()
        .character()
        .to_string();
    click(&mut app, &ctx, points(&output, &glyph)[0]);
    settle(&mut app);
    let output = render(&mut app, &ctx, vec![], Modifiers::NONE);
    assert!(app.split.is_some());
    let [left, right] = app.split_indices().unwrap();
    assert_eq!(app.tabs[left].current, app.tabs[right].current);
    let rects = app.split.as_ref().unwrap().rects.unwrap();
    assert!((rects[0].width() - rects[1].width()).abs() < 1.0);
    let names = points(&output, "Child");
    assert_eq!(names.len(), 2, "both panes render their folder listing");
    let right_name = *names.iter().max_by(|a, b| a.x.total_cmp(&b.x)).unwrap();
    click(
        &mut app,
        &ctx,
        egui::pos2(rects[1].right() - 90.0, rects[1].top() + 20.0),
    );
    render(
        &mut app,
        &ctx,
        vec![Event::Text("missing".into())],
        Modifiers::NONE,
    );
    assert_eq!(app.tabs[right].query, "missing");
    assert!(app.tabs[left].query.is_empty());
    app.tabs[right].query.clear();
    click(&mut app, &ctx, right_name);
    assert_eq!(app.active, right);
    assert!(app.tabs[right].selected.contains(&child));
    assert!(app.tabs[left].selected.is_empty());
    app.tabs[right].navigate(child.clone(), Navigation::Visit);
    settle(&mut app);
    assert_eq!(app.tabs[left].current, root.path());
    assert_eq!(app.tabs[right].current, child);
    let output = render(&mut app, &ctx, vec![], Modifiers::NONE);
    click(&mut app, &ctx, points(&output, &glyph)[0]);
    assert!(app.split.is_none());
    assert_eq!(app.tabs[app.active].current, child);
    assert_eq!(app.tabs[left].current, root.path());
}

#[test]
fn both_panes_render_all_views_without_crossing_the_divider_in_both_themes() {
    for dark in [false, true] {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("one.txt"), "data").unwrap();
        let ctx = egui::Context::default();
        let mut app = workspace(&ctx, root.path().into());
        app.tabs[0].dark = dark;
        app.toggle_split(&ctx);
        settle(&mut app);
        for view in [View::Details, View::List, View::Icons] {
            for tab in &mut app.tabs {
                tab.view = view;
            }
            render(&mut app, &ctx, vec![], Modifiers::NONE);
            let output = render(&mut app, &ctx, vec![], Modifiers::NONE);
            let rects = app.split.as_ref().unwrap().rects.unwrap();
            let background = ctx.style().visuals.panel_fill;
            assert_eq!(background.a(), 255);
            for rect in rects {
                assert!(
                    output.shapes.iter().any(|shape| {
                        matches!(&shape.shape, egui::Shape::Rect(paint)
                        if paint.fill == background
                            && paint.rect.contains_rect(rect)
                            && shape.clip_rect.contains(rect.center()))
                    }),
                    "each pane needs an opaque background from the selected theme"
                );
            }
            let names = points(&output, "one.txt");
            assert_eq!(names.len(), 2);
            assert!(rects[0].contains(names[0]));
            assert!(rects[1].contains(names[1]));
        }
    }
}

#[test]
fn splitter_resizes_and_tab_reordering_and_closing_preserve_valid_pane_ids() {
    let root = tempfile::tempdir().unwrap();
    let ctx = egui::Context::default();
    let mut app = workspace(&ctx, root.path().into());
    app.toggle_split(&ctx);
    settle(&mut app);
    render(&mut app, &ctx, vec![], Modifiers::NONE);
    let split = app.split.as_ref().unwrap();
    let old_ids = [split.left, split.right];
    let rects = split.rects.unwrap();
    let start = egui::pos2(rects[0].right() + 4.0, rects[0].center().y);
    render(
        &mut app,
        &ctx,
        vec![
            Event::PointerMoved(start),
            Event::PointerButton {
                pos: start,
                button: PointerButton::Primary,
                pressed: true,
                modifiers: Modifiers::NONE,
            },
        ],
        Modifiers::NONE,
    );
    let end = start + Vec2::new(100.0, 0.0);
    render(
        &mut app,
        &ctx,
        vec![Event::PointerMoved(end)],
        Modifiers::NONE,
    );
    render(
        &mut app,
        &ctx,
        vec![Event::PointerButton {
            pos: end,
            button: PointerButton::Primary,
            pressed: false,
            modifiers: Modifiers::NONE,
        }],
        Modifiers::NONE,
    );
    assert!(app.split.as_ref().unwrap().ratio > 0.55);
    app.reorder_tab(old_ids[0], app.tabs.len());
    assert_eq!(app.split.as_ref().unwrap().left, old_ids[0]);
    let index = app
        .tabs
        .iter()
        .position(|tab| tab.tab_id == old_ids[1])
        .unwrap();
    app.close_tab(index, &ctx);
    assert!(app.split.is_none());
    assert_eq!(app.tabs.len(), 1);
    render(&mut app, &ctx, vec![], Modifiers::NONE);
}

#[test]
fn each_pane_has_its_own_sidebar_that_navigates_only_that_pane() {
    let root = tempfile::tempdir().unwrap();
    let target = root.path().join("Other");
    std::fs::create_dir(&target).unwrap();
    let ctx = egui::Context::default();
    let mut app = workspace(&ctx, root.path().into());
    let sidebar = &mut app.tabs[0].sidebar_options;
    sidebar.visible.clear();
    sidebar.show_trash = false;
    sidebar.show_local_shares = false;
    sidebar.show_network = false;
    sidebar.pinned = vec![locations::Shortcut {
        path: target.clone(),
        label: "Destination shortcut".into(),
    }];
    app.toggle_split(&ctx);
    settle(&mut app);
    render(&mut app, &ctx, vec![], Modifiers::NONE);
    let output = render(&mut app, &ctx, vec![], Modifiers::NONE);
    let [left, right] = app.split_indices().unwrap();
    let rects = app.split.as_ref().unwrap().rects.unwrap();
    let shortcuts = points(&output, "Destination shortcut");
    assert_eq!(shortcuts.len(), 2);
    assert!(rects[0].contains(shortcuts[0]));
    assert!(rects[1].contains(shortcuts[1]));
    click(&mut app, &ctx, shortcuts[1]);
    settle(&mut app);
    assert_eq!(app.active, right);
    assert_eq!(app.tabs[right].current, target);
    assert_eq!(app.tabs[left].current, root.path());
}

#[test]
fn keyboard_copy_paste_and_file_drag_work_between_the_two_visible_panes() {
    for drag in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("Source");
        let target = root.path().join("Target");
        std::fs::create_dir(&source).unwrap();
        std::fs::create_dir(&target).unwrap();
        let file = source.join("report.txt");
        std::fs::write(&file, "content").unwrap();
        let ctx = egui::Context::default();
        let mut app = workspace(&ctx, source.clone());
        app.toggle_split(&ctx);
        let [left, right] = app.split_indices().unwrap();
        settle(&mut app);
        app.tabs[right].navigate(target.clone(), Navigation::Visit);
        settle(&mut app);
        render(&mut app, &ctx, vec![], Modifiers::NONE);
        let output = render(&mut app, &ctx, vec![], Modifiers::NONE);
        let from = points(&output, "report.txt")[0];
        let rects = app.split.as_ref().unwrap().rects.unwrap();
        let to = rects[1].center_bottom() - Vec2::new(0.0, 80.0);
        if drag {
            render(
                &mut app,
                &ctx,
                vec![
                    Event::PointerMoved(from),
                    Event::PointerButton {
                        pos: from,
                        button: PointerButton::Primary,
                        pressed: true,
                        modifiers: Modifiers::NONE,
                    },
                ],
                Modifiers::NONE,
            );
            render(
                &mut app,
                &ctx,
                vec![Event::PointerMoved(from + Vec2::new(14.0, 0.0))],
                Modifiers::NONE,
            );
            render(
                &mut app,
                &ctx,
                vec![Event::PointerMoved(to)],
                Modifiers::NONE,
            );
            render(
                &mut app,
                &ctx,
                vec![Event::PointerButton {
                    pos: to,
                    button: PointerButton::Primary,
                    pressed: false,
                    modifiers: Modifiers::NONE,
                }],
                Modifiers::NONE,
            );
        } else {
            click(&mut app, &ctx, from);
            let command = Modifiers {
                ctrl: true,
                command: true,
                ..Default::default()
            };
            let key = |key| Event::Key {
                key,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: command,
            };
            render(&mut app, &ctx, vec![key(Key::C)], command);
            click(&mut app, &ctx, to);
            assert_eq!(app.active, right);
            render(&mut app, &ctx, vec![key(Key::V)], command);
        }
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        while app.tabs.iter().any(Explorer::busy) {
            for tab in &mut app.tabs {
                tab.poll();
            }
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        assert_eq!(
            std::fs::read_to_string(target.join("report.txt")).unwrap(),
            "content"
        );
        assert_eq!(file.exists(), !drag, "drag moves; Ctrl+C/Ctrl+V copies");
        assert_eq!(app.tabs[left].current, source);
        assert_eq!(app.tabs[right].current, target);
    }
}

#[test]
fn copying_then_right_clicking_blank_space_in_the_other_pane_offers_paste_in_every_view() {
    for view in [View::Details, View::List, View::Icons] {
        for source_side in [0, 1] {
            let root = tempfile::tempdir().unwrap();
            let source = root.path().join("Source");
            let target = root.path().join("Target");
            std::fs::create_dir(&source).unwrap();
            std::fs::create_dir(&target).unwrap();
            let file = source.join("clipboard.txt");
            std::fs::write(&file, "copied content").unwrap();
            std::fs::write(target.join("existing.txt"), "existing content").unwrap();
            let ctx = egui::Context::default();
            let mut app = workspace(&ctx, source.clone());
            app.toggle_split(&ctx);
            settle(&mut app);
            let indices = app.split_indices().unwrap();
            let destination = indices[1 - source_side];
            app.tabs[destination].navigate(target.clone(), Navigation::Visit);
            settle(&mut app);
            for tab in &mut app.tabs {
                tab.view = view;
            }
            render(&mut app, &ctx, vec![], Modifiers::NONE);
            let output = render(&mut app, &ctx, vec![], Modifiers::NONE);
            click(&mut app, &ctx, points(&output, "clipboard.txt")[0]);
            let command = Modifiers {
                ctrl: true,
                command: true,
                ..Default::default()
            };
            render(
                &mut app,
                &ctx,
                vec![Event::Key {
                    key: Key::C,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: command,
                }],
                command,
            );
            let rect = app.split.as_ref().unwrap().rects.unwrap()[1 - source_side];
            let empty = egui::pos2(rect.right() - 80.0, rect.top() + 260.0);
            for pressed in [true, false] {
                render(
                    &mut app,
                    &ctx,
                    vec![
                        Event::PointerMoved(empty),
                        Event::PointerButton {
                            pos: empty,
                            button: PointerButton::Secondary,
                            pressed,
                            modifiers: Modifiers::NONE,
                        },
                    ],
                    Modifiers::NONE,
                );
            }
            assert_eq!(app.active, destination);
            let output = render(&mut app, &ctx, vec![], Modifiers::NONE);
            let paste = points(&output, "Colar");
            assert_eq!(paste.len(), 1, "blank-space menu must offer Paste");
            click(&mut app, &ctx, paste[0]);
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
            while app.tabs.iter().any(Explorer::busy) {
                for tab in &mut app.tabs {
                    tab.poll();
                }
                assert!(std::time::Instant::now() < deadline);
                std::thread::sleep(std::time::Duration::from_millis(2));
            }
            assert_eq!(
                std::fs::read_to_string(target.join("clipboard.txt")).unwrap(),
                "copied content"
            );
            assert_eq!(std::fs::read_to_string(file).unwrap(), "copied content");
        }
    }
}

#[test]
fn escape_in_the_other_pane_cancels_cut_and_restores_both_folder_views() {
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join("cut.txt");
    std::fs::write(&file, "preserved").unwrap();
    let ctx = egui::Context::default();
    let mut app = workspace(&ctx, root.path().into());
    app.toggle_split(&ctx);
    settle(&mut app);
    render(&mut app, &ctx, vec![], Modifiers::NONE);
    let output = render(&mut app, &ctx, vec![], Modifiers::NONE);
    click(&mut app, &ctx, points(&output, "cut.txt")[0]);
    let command = Modifiers {
        ctrl: true,
        command: true,
        ..Default::default()
    };
    render(
        &mut app,
        &ctx,
        vec![Event::Key {
            key: Key::X,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: command,
        }],
        command,
    );
    let faded = render(&mut app, &ctx, vec![], Modifiers::NONE);
    let alphas = |output: &FullOutput| {
        output
            .shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.text() == "cut.txt" => {
                    Some(text.fallback_color.a())
                }
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    assert!(alphas(&faded).iter().all(|a| *a < 200));
    let rect = app.split.as_ref().unwrap().rects.unwrap()[1];
    click(
        &mut app,
        &ctx,
        egui::pos2(rect.right() - 80.0, rect.top() + 260.0),
    );
    assert_eq!(app.active, app.split_indices().unwrap()[1]);
    render(
        &mut app,
        &ctx,
        vec![Event::Key {
            key: Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        }],
        Modifiers::NONE,
    );
    assert!(
        app.tabs
            .iter()
            .all(|tab| tab.file_clipboard.is_none() && !tab.can_paste())
    );
    let output = render(&mut app, &ctx, vec![], Modifiers::NONE);
    assert_eq!(alphas(&output), vec![255, 255]);
    assert_eq!(std::fs::read_to_string(file).unwrap(), "preserved");
}

#[test]
fn escape_does_not_clear_the_clipboard_of_an_already_started_move_in_another_pane() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("Source");
    let target = root.path().join("Target");
    std::fs::create_dir(&source).unwrap();
    std::fs::create_dir(&target).unwrap();
    let file = source.join("conflict.txt");
    std::fs::write(&file, "source").unwrap();
    std::fs::write(target.join("conflict.txt"), "target").unwrap();
    let ctx = egui::Context::default();
    let mut app = workspace(&ctx, source);
    app.toggle_split(&ctx);
    settle(&mut app);
    let [left, right] = app.split_indices().unwrap();
    app.tabs[left].selected.insert(file.clone());
    app.tabs[left].capture_files(file_ops::Mode::Cut, &ctx);
    app.share();
    let clipboard = app.tabs[left].file_clipboard.clone().unwrap();
    app.tabs[right].start_file_transfer(clipboard.clone(), target.clone(), &ctx);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    while app.tabs[right].transfer_conflict.is_none() {
        app.tabs[right].poll();
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    render(
        &mut app,
        &ctx,
        vec![Event::Key {
            key: Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        }],
        Modifiers::NONE,
    );
    assert!(app.tabs[left].file_clipboard.as_ref() == Some(&clipboard));
    assert!(app.tabs[right].active_transfer.is_some());
    app.tabs[right].cancel_transfer();
    while app.tabs[right].active_transfer.is_some() {
        app.tabs[right].poll();
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert_eq!(std::fs::read_to_string(file).unwrap(), "source");
    assert_eq!(
        std::fs::read_to_string(target.join("conflict.txt")).unwrap(),
        "target"
    );
}

#[test]
fn space_previews_the_selected_file_from_the_active_pane() {
    let root = tempfile::tempdir().unwrap();
    let a = root.path().join("Left");
    let b = root.path().join("Right");
    std::fs::create_dir(&a).unwrap();
    std::fs::create_dir(&b).unwrap();
    std::fs::write(a.join("left.txt"), "LEFT CONTENT").unwrap();
    std::fs::write(b.join("right.txt"), "RIGHT CONTENT").unwrap();
    let ctx = egui::Context::default();
    let mut app = workspace(&ctx, a);
    app.toggle_split(&ctx);
    settle(&mut app);
    let [left, right] = app.split_indices().unwrap();
    app.tabs[right].navigate(b, Navigation::Visit);
    settle(&mut app);
    render(&mut app, &ctx, vec![], Modifiers::NONE);
    let output = render(&mut app, &ctx, vec![], Modifiers::NONE);
    click(&mut app, &ctx, points(&output, "left.txt")[0]);
    click(&mut app, &ctx, points(&output, "right.txt")[0]);
    render(
        &mut app,
        &ctx,
        vec![Event::Key {
            key: Key::Space,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        }],
        Modifiers::NONE,
    );
    assert!(app.tabs[right].preview.is_some());
    assert!(app.tabs[left].preview.is_none());
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    loop {
        let output = render(&mut app, &ctx, vec![], Modifiers::NONE);
        if !points(&output, "RIGHT CONTENT").is_empty() {
            assert!(points(&output, "LEFT CONTENT").is_empty());
            break;
        }
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    render(
        &mut app,
        &ctx,
        vec![Event::Key {
            key: Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        }],
        Modifiers::NONE,
    );
    assert!(app.tabs[right].preview.is_none());
}
