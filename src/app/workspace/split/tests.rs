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
