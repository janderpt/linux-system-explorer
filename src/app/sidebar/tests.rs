use super::*;
use crate::app::tests::complete_navigation;
use egui::{Event, FullOutput, Modifiers, Pos2, RawInput, Rect};

fn configure(app: &mut Explorer, root: &Path) {
    app.places = vec![(locations::Item::Home, root.into())];
    app.network_mounts.clear();
    app.sidebar_options.show_volumes = false;
    app.sidebar_options.show_local_shares = false;
    app.sidebar_options.pinned = (0..24)
        .map(|i| locations::Shortcut::new(root.join(format!("pin-{i:02}"))))
        .collect();
}

fn frame(app: &mut Explorer, ctx: &egui::Context, events: Vec<Event>) -> FullOutput {
    ctx.run(
        RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1000.0, 480.0))),
            time: Some(ctx.input(|i| i.time) + 0.1),
            events,
            ..Default::default()
        },
        |ctx| app.show(ctx),
    )
}

fn settle(app: &mut Explorer, ctx: &egui::Context) {
    for _ in 0..12 {
        frame(app, ctx, vec![]);
    }
}

fn selected_row(app: &Explorer) -> Rect {
    app.sidebar_scroll
        .target_rect
        .expect("selected ancestor has a row")
}

fn visible(app: &Explorer) -> bool {
    let row = selected_row(app);
    row.top() >= 48.0 && row.bottom() <= 450.0
}

#[test]
fn navigation_reveals_the_closest_pin_or_remote_without_overriding_manual_scroll() {
    for remote in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let parent = root.path().join("mounted-or-pinned");
        let child = parent.join("child");
        let deeper = child.join("deeper");
        std::fs::create_dir_all(&deeper).unwrap();
        let ctx = egui::Context::default();
        let mut app = Explorer::with_context(&ctx, root.path().into());
        complete_navigation(&mut app);
        configure(&mut app, root.path());
        let expected = if remote {
            app.sidebar_options.remote.push(crate::network::Connection {
                url: "ftp://example.invalid/".into(),
                label: "Test FTP".into(),
                mount: Some(parent.clone()),
            });
            Location::Remote(0)
        } else {
            app.sidebar_options
                .pinned
                .push(locations::Shortcut::new(parent.clone()));
            Location::Pinned(24)
        };
        settle(&mut app, &ctx);
        app.navigate(child.clone(), Navigation::Visit);
        complete_navigation(&mut app);
        settle(&mut app, &ctx);
        assert_eq!(
            closest(&app.current, &app.sidebar_entries()),
            Some(expected)
        );
        assert!(visible(&app), "{:?}", selected_row(&app));
        frame(
            &mut app,
            &ctx,
            vec![
                Event::PointerMoved(egui::pos2(60.0, 200.0)),
                Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: Vec2::new(0.0, 10000.0),
                    modifiers: Modifiers::NONE,
                },
            ],
        );
        settle(&mut app, &ctx);
        assert!(!visible(&app), "manual scrolling remains free");
        app.navigate(deeper.clone(), Navigation::Visit);
        complete_navigation(&mut app);
        settle(&mut app, &ctx);
        assert!(
            visible(&app),
            "entering another child reveals the same ancestor again"
        );
        app.navigate(root.path().into(), Navigation::Visit);
        complete_navigation(&mut app);
        settle(&mut app, &ctx);
        assert!(visible(&app), "navigation also scrolls back up to Home");
        let back = app.history.back().unwrap().to_owned();
        assert_eq!(back, deeper);
        app.navigate(back, Navigation::Back);
        complete_navigation(&mut app);
        settle(&mut app, &ctx);
        assert!(visible(&app), "Back reveals the remote/pin again");
        assert_eq!(app.current, deeper);
        assert!(app.selected.is_empty());
    }
}

#[test]
fn newly_discovered_network_ancestor_is_revealed_without_changing_the_folder() {
    let root = tempfile::tempdir().unwrap();
    let parent = root.path().join("mounted");
    let child = parent.join("child");
    std::fs::create_dir_all(&child).unwrap();
    let ctx = egui::Context::default();
    let mut app = Explorer::with_context(&ctx, child.clone());
    complete_navigation(&mut app);
    configure(&mut app, root.path());
    settle(&mut app, &ctx);
    assert!(visible(&app));
    app.network_mounts.push(locations::Shortcut::new(parent));
    settle(&mut app, &ctx);
    assert!(visible(&app));
    assert_eq!(
        closest(&app.current, &app.sidebar_entries()),
        Some(Location::Network(0))
    );
    app.sidebar_options.show_network = false;
    settle(&mut app, &ctx);
    assert!(visible(&app));
    assert_eq!(
        closest(&app.current, &app.sidebar_entries()),
        Some(Location::Standard(locations::Item::Home))
    );
    assert_eq!(app.current, child);
}

#[test]
fn volume_ancestor_uses_the_same_sidebar_reveal_behavior() {
    let root = tempfile::tempdir().unwrap();
    let mount = root.path().join("volume");
    let child = mount.join("child");
    std::fs::create_dir_all(&child).unwrap();
    let ctx = egui::Context::default();
    let mut app = Explorer::with_context(&ctx, child.clone());
    complete_navigation(&mut app);
    configure(&mut app, root.path());
    app.sidebar_options.show_volumes = true;
    app.places = locations::Item::ALL
        .into_iter()
        .map(|item| {
            (
                item,
                if item == locations::Item::Home {
                    root.path().into()
                } else {
                    PathBuf::from("/unrelated").join(format!("{item:?}"))
                },
            )
        })
        .collect();
    app.volume_sidebar.lock().unwrap().volumes = vec![crate::volumes::test_volume(mount)];
    settle(&mut app, &ctx);
    assert_eq!(
        closest(&app.current, &app.sidebar_entries()),
        Some(Location::Volume(0))
    );
    assert!(visible(&app));
    assert_eq!(selected_row(&app).height(), 54.0);
    assert_eq!(app.current, child);
}
