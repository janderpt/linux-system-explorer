use super::*;
use crate::app::tests::text_center;
use egui::{Event, FullOutput, Modifiers, PointerButton, Pos2, RawInput, Rect, Shape};

pub(super) fn render(app: &mut Workspace, ctx: &egui::Context, events: Vec<Event>) -> FullOutput {
    ctx.run(
        RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(780.0, 760.0))),
            time: Some(ctx.input(|i| i.time) + 0.05),
            events,
            ..Default::default()
        },
        |ctx| app.show(ctx),
    )
}
fn pointer(pos: Pos2, pressed: bool) -> Event {
    Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    }
}
pub(super) fn tab_rect(output: &FullOutput, title: &str) -> Rect {
    let text = text_center(output, title).unwrap_or_else(|| panic!("missing tab {title}"));
    output
        .shapes
        .iter()
        .find_map(|s| {
            if let Shape::Rect(r) = &s.shape
                && r.corner_radius.nw == 7
                && r.rect.contains(text)
            {
                return Some(r.rect);
            }
            None
        })
        .unwrap()
}
fn begin_drag(app: &mut Workspace, ctx: &egui::Context, from: Pos2, to: Pos2) {
    render(
        app,
        ctx,
        vec![Event::PointerMoved(from), pointer(from, true)],
    );
    render(
        app,
        ctx,
        vec![Event::PointerMoved(from + Vec2::new(12.0, 0.0))],
    );
    assert!(
        egui::DragAndDrop::has_any_payload(ctx),
        "tab drag did not start"
    );
    render(app, ctx, vec![Event::PointerMoved(to)]);
}

#[test]
fn mouse_reorders_tabs_in_both_directions_and_restores_order_and_active_identity() {
    let root = tempfile::tempdir().unwrap();
    let paths: Vec<_> = ["Alpha", "Beta", "Gamma"]
        .into_iter()
        .map(|n| root.path().join(n))
        .collect();
    for path in &paths {
        std::fs::create_dir(path).unwrap();
    }
    let ctx = egui::Context::default();
    let mut app = workspace(&ctx, paths[0].clone());
    app.session = crate::session::Store::at(Some(root.path().join("session.json")));
    for path in &paths[1..] {
        app.add_tab(path.clone(), &ctx);
    }
    for tab in &mut app.tabs {
        complete_navigation(tab);
    }
    app.activate(1, &ctx);
    app.tabs[0].query = "retained in Alpha".into();
    let active_id = app.tabs[1].tab_id;
    render(&mut app, &ctx, vec![]);
    let output = render(&mut app, &ctx, vec![]);
    let from = tab_rect(&output, "Alpha").left_center() + Vec2::new(60.0, 0.0);
    let to = tab_rect(&output, "Gamma").right_center() - Vec2::new(3.0, 0.0);
    begin_drag(&mut app, &ctx, from, to);
    let preview = render(&mut app, &ctx, vec![]);
    assert!(
        preview
            .shapes
            .iter()
            .any(|s| matches!(&s.shape, Shape::LineSegment {points,stroke} if stroke.width==3.0 && points[0].x==points[1].x))
    );
    render(&mut app, &ctx, vec![pointer(to, false)]);
    assert_eq!(
        app.session_state().paths().unwrap(),
        vec![paths[1].clone(), paths[2].clone(), paths[0].clone()]
    );
    assert_eq!(app.active, 0);
    assert_eq!(app.tabs[app.active].tab_id, active_id);
    assert_eq!(app.tabs[2].query, "retained in Alpha");
    let saved = crate::session::State::load(&root.path().join("session.json"))
        .unwrap()
        .unwrap();
    let fresh_ctx = egui::Context::default();
    let mut reopened = workspace(&fresh_ctx, saved.paths().unwrap()[0].clone());
    reopened.restore_tabs(&saved, &fresh_ctx);
    assert_eq!(reopened.session_state(), saved);
    let output = render(&mut app, &ctx, vec![]);
    // Move the active Beta to the end, then back to the beginning.
    let from = tab_rect(&output, "Beta").left_center() + Vec2::new(60.0, 0.0);
    let to = tab_rect(&output, "Alpha").right_center() - Vec2::new(3.0, 0.0);
    begin_drag(&mut app, &ctx, from, to);
    render(&mut app, &ctx, vec![pointer(to, false)]);
    assert_eq!(app.active, 2);
    let output = render(&mut app, &ctx, vec![]);
    let from = tab_rect(&output, "Beta").left_center() + Vec2::new(60.0, 0.0);
    let to = tab_rect(&output, "Gamma").left_center() + Vec2::new(3.0, 0.0);
    begin_drag(&mut app, &ctx, from, to);
    render(&mut app, &ctx, vec![pointer(to, false)]);
    assert_eq!(app.active, 0);
    assert_eq!(app.tabs[app.active].tab_id, active_id);
}

#[test]
fn tabs_have_shared_contours_centered_contents_and_distinct_active_style_in_both_themes() {
    let root = tempfile::tempdir().unwrap();
    let a = root.path().join("Short");
    let b = root
        .path()
        .join("Long directory name that needs truncation and an ellipsis");
    for path in [&a, &b] {
        std::fs::create_dir(path).unwrap();
    }
    for dark in [false, true] {
        let ctx = egui::Context::default();
        let mut app = workspace(&ctx, a.clone());
        app.add_tab(b.clone(), &ctx);
        complete_navigation(&mut app.tabs[1]);
        app.tabs[1].dark = dark;
        let preferences = app.tabs[1].preferences();
        app.tabs[1].apply_global_preferences(&preferences, &ctx);
        render(&mut app, &ctx, vec![]);
        let output = render(&mut app, &ctx, vec![]);
        let shapes: Vec<_> = output
            .shapes
            .iter()
            .filter_map(|s| {
                if let Shape::Rect(r) = &s.shape
                    && r.corner_radius.nw == 7
                {
                    Some(r)
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(shapes.len(), 3);
        assert_eq!(shapes[0].rect.height(), 30.0);
        assert_eq!(shapes[0].rect.height(), shapes[1].rect.height());
        assert_eq!(
            shapes[0].rect.right(),
            shapes[1].rect.left(),
            "tabs touch without a gap"
        );
        assert_eq!(
            shapes[1].rect.right(),
            shapes[2].rect.left(),
            "new-tab control also touches the strip"
        );
        assert_eq!(shapes[2].rect.size(), Vec2::new(36.0, 30.0));
        assert_eq!(
            shapes[2].fill.a(),
            255,
            "new-tab control has an opaque background"
        );
        let background = output
            .shapes
            .iter()
            .find_map(|s| {
                if let Shape::Rect(r) = &s.shape
                    && r.rect
                        .contains(shapes[2].rect.right_top() + Vec2::new(50.0, 1.0))
                    && r.rect.height() < 50.0
                {
                    Some(r.fill)
                } else {
                    None
                }
            })
            .unwrap();
        assert_eq!(
            background.a(),
            255,
            "tab strip must not expose the renderer's dark clear color"
        );
        assert_eq!(background, ctx.style().visuals.panel_fill);
        assert_ne!(shapes[0].fill, shapes[1].fill);
        assert_ne!(shapes[0].stroke.color, shapes[1].stroke.color);
        let title = text_center(&output, "Short").unwrap();
        assert!((title.y - shapes[0].rect.center().y).abs() < 0.5);
        assert!(
            output
                .shapes
                .iter()
                .any(|s| matches!(&s.shape, Shape::Text(t) if t.galley.elided))
        );
        let crosses: Vec<_> = output
            .shapes
            .iter()
            .filter_map(|s| {
                if let Shape::LineSegment { points, stroke } = &s.shape
                    && stroke.width == 1.5
                    && points[0].x != points[1].x
                {
                    Some(points)
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(crosses.len(), 4);
        for (cross, tab) in crosses.chunks(2).zip(&shapes) {
            assert!(
                cross
                    .iter()
                    .all(|p| tab.rect.contains(p[0]) && tab.rect.contains(p[1]))
            );
        }
    }
}

#[test]
fn escape_and_outside_drop_cancel_reordering_and_close_button_does_not_drag() {
    let root = tempfile::tempdir().unwrap();
    let a = root.path().join("Alpha");
    let b = root.path().join("Beta");
    for path in [&a, &b] {
        std::fs::create_dir(path).unwrap();
    }
    let ctx = egui::Context::default();
    let mut app = workspace(&ctx, a.clone());
    app.add_tab(b, &ctx);
    complete_navigation(&mut app.tabs[1]);
    render(&mut app, &ctx, vec![]);
    for escape in [true, false] {
        let output = render(&mut app, &ctx, vec![]);
        let from = tab_rect(&output, "Alpha").left_center() + Vec2::new(60.0, 0.0);
        let to = if escape {
            tab_rect(&output, "Beta").right_center()
        } else {
            Pos2::new(500.0, 300.0)
        };
        let before = app.session_state();
        begin_drag(&mut app, &ctx, from, to);
        if escape {
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
            );
        }
        render(&mut app, &ctx, vec![pointer(to, false)]);
        assert_eq!(app.session_state(), before);
        assert!(!egui::DragAndDrop::has_any_payload(&ctx));
    }
    let output = render(&mut app, &ctx, vec![]);
    let x = tab_rect(&output, "Alpha").right_center() - Vec2::new(16.5, 0.0);
    render(
        &mut app,
        &ctx,
        vec![Event::PointerMoved(x), pointer(x, true)],
    );
    assert!(!egui::DragAndDrop::has_any_payload(&ctx));
    render(&mut app, &ctx, vec![pointer(x, false)]);
    assert_eq!(app.tabs.len(), 1);
    assert_eq!(app.tabs[0].location().file_name().unwrap(), "Beta");
}

#[test]
fn dragging_near_the_strip_edge_scrolls_to_hidden_tabs() {
    let root = tempfile::tempdir().unwrap();
    let paths: Vec<_> = (0..12)
        .map(|n| root.path().join(format!("Folder_{n:02}")))
        .collect();
    for path in &paths {
        std::fs::create_dir(path).unwrap();
    }
    let ctx = egui::Context::default();
    let mut app = workspace(&ctx, paths[0].clone());
    for path in &paths[1..] {
        app.add_tab(path.clone(), &ctx);
    }
    for tab in &mut app.tabs {
        complete_navigation(tab);
    }
    app.activate(0, &ctx);
    render(&mut app, &ctx, vec![]);
    let output = render(&mut app, &ctx, vec![]);
    let from = tab_rect(&output, "Folder_00").left_center() + Vec2::new(60.0, 0.0);
    let edge = Pos2::new(765.0, from.y);
    begin_drag(&mut app, &ctx, from, edge);
    for _ in 0..30 {
        render(&mut app, &ctx, vec![]);
    }
    render(&mut app, &ctx, vec![pointer(edge, false)]);
    assert!(
        app.active >= 5,
        "drag should reach tabs that were initially offscreen: {}",
        app.active
    );
    assert_eq!(app.tabs[app.active].location(), paths[0]);
}
