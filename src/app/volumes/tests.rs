use super::*;
use crate::app::tests::{complete_navigation, text_center};
fn volume(path: PathBuf) -> Volume {
    // Read-only fixture matching UDisks2's wire representation through the model tests.
    crate::volumes::test_volume(path)
}
fn frame(app: &mut Explorer, ctx: &egui::Context, events: Vec<egui::Event>) -> egui::FullOutput {
    ctx.run(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                Vec2::new(1180.0, 1100.0),
            )),
            events,
            ..Default::default()
        },
        |ctx| {
            let visible = app.visible_for_display();
            app.top_toolbar(ctx);
            app.sidebar(ctx);
            app.contents(ctx, &visible);
            app.settings_dialog(ctx);
        },
    )
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
fn sidebar_shows_capacity_opens_mounted_volumes_context_actions_and_saved_visibility() {
    let root = tempfile::tempdir().unwrap();
    let mount = root.path().join("disk");
    std::fs::create_dir(&mount).unwrap();
    let ctx = egui::Context::default();
    let mut app = Explorer::with_context(&ctx, root.path().into());
    complete_navigation(&mut app);
    app.sidebar_options.show_applications = false;
    app.sidebar_options.visible.clear();
    app.sidebar_options.show_trash = false;
    app.sidebar_options.show_pinned = false;
    app.sidebar_options.show_local_shares = false;
    app.sidebar_options.show_network = false;
    let v = volume(mount.clone());
    app.volume_sidebar.lock().unwrap().volumes = vec![v.clone()];
    frame(&mut app, &ctx, vec![]);
    let out = frame(&mut app, &ctx, vec![]);
    assert!(text_center(&out, "UNIDADES").is_some());
    assert!(text_center(&out, &v.capacity()).is_some());
    assert!(!v.capacity().contains('{'));
    let pos = text_center(&out, &v.name).unwrap();
    let out = click(&mut app, &ctx, pos, egui::PointerButton::Secondary);
    assert!(text_center(&out, "Desmontar").is_some());
    assert!(text_center(&out, "Ejetar com segurança").is_some());
    egui::Popup::close_all(&ctx);
    frame(&mut app, &ctx, vec![]);
    app.start_volume(&v, Action::Open, &ctx, |v, _| {
        Ok(Outcome::Opened(v.mount().unwrap().to_owned()))
    });
    let deadline = Instant::now() + Duration::from_secs(3);
    while app.volume_operation.is_some() {
        app.poll_volumes();
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
    complete_navigation(&mut app);
    assert_eq!(app.current, mount);
    assert!(app.volume_operation.is_none());
    let config = root.path().join("settings.json");
    app.settings_path = Some(config.clone());
    app.settings_editor = Some(settings::Editor::new(app.preferences()));
    frame(&mut app, &ctx, vec![]);
    for label in ["Barra lateral", "Exibir seção Unidades"] {
        frame(&mut app, &ctx, vec![]);
        let out = frame(&mut app, &ctx, vec![]);
        let point = text_center(&out, label).unwrap();
        click(&mut app, &ctx, point, egui::PointerButton::Primary);
    }
    assert!(
        app.sidebar_options.show_volumes,
        "settings draft is not applied yet"
    );
    let out = frame(&mut app, &ctx, vec![]);
    click(
        &mut app,
        &ctx,
        text_center(&out, "Salvar e fechar").unwrap(),
        egui::PointerButton::Primary,
    );
    assert!(!Preferences::load(&config).unwrap().sidebar.show_volumes);
    let out = frame(&mut app, &ctx, vec![]);
    assert!(text_center(&out, "UNIDADES").is_none());
    assert!(
        serde_json::from_str::<crate::sidebar::Preferences>("{}")
            .unwrap()
            .show_volumes
    );
}
#[test]
fn stale_scan_cannot_undo_a_volume_operation_and_errors_keep_last_snapshot() {
    let ctx = egui::Context::default();
    let mut cache = Sidebar::default();
    let old = volume("/media/test".into());
    cache.volumes = vec![old.clone()];
    let (sender, receiver) = mpsc::channel();
    cache.pending = Some((cache.revision, receiver));
    cache.needs_sync = true;
    cache.invalidate();
    sender.send(Ok(vec![])).unwrap();
    cache.refresh(&ctx);
    assert!(
        cache.needs_sync,
        "stale scans must not re-enable drops after a device operation"
    );
    assert_eq!(cache.volumes, vec![old.clone()]);
    let (sender, receiver) = mpsc::channel();
    cache.pending = Some((cache.revision, receiver));
    sender.send(Err("Service unavailable".into())).unwrap();
    assert!(cache.refresh(&ctx).is_some());
    assert_eq!(cache.volumes, vec![old]);
    assert!(cache.next > Instant::now());
}
#[test]
fn operation_failure_and_cancellation_leave_location_and_files_untouched() {
    let root = tempfile::tempdir().unwrap();
    let ctx = egui::Context::default();
    let mut app = Explorer::with_context(&ctx, root.path().into());
    complete_navigation(&mut app);
    std::fs::write(root.path().join("keep.txt"), "keep").unwrap();
    for result in [Ok(Outcome::Cancelled), Err("Device busy".into())] {
        let (sender, receiver) = mpsc::channel();
        app.volume_operation = Some(Pending {
            volume: volume(root.path().into()),
            result: receiver,
        });
        let success = result.is_ok();
        sender.send(result).unwrap();
        app.poll_volumes();
        assert_eq!(app.current, root.path());
        assert!(app.volume_operation.is_none());
        assert_eq!(app.notice_toasts.is_empty(), success);
    }
    assert_eq!(
        std::fs::read_to_string(root.path().join("keep.txt")).unwrap(),
        "keep"
    );
}

#[test]
fn shared_volume_removal_moves_both_panes_out_of_the_removed_mount() {
    let root = tempfile::tempdir().unwrap();
    let mount = root.path().join("disk");
    std::fs::create_dir(&mount).unwrap();
    let ctx = egui::Context::default();
    let mut left = Explorer::with_context(&ctx, mount.clone());
    complete_navigation(&mut left);
    let mut right = Explorer::with_context(&ctx, mount.join("."));
    complete_navigation(&mut right);
    right.volume_sidebar = left.volume_sidebar.clone();
    left.volume_sidebar
        .lock()
        .unwrap()
        .removed(vec![mount.clone()]);
    for app in [&mut left, &mut right] {
        app.poll_volumes();
        complete_navigation(app);
        assert_eq!(app.current, dirs::home_dir().unwrap());
    }
}

#[test]
fn unmounted_volumes_offer_mount_and_open_and_share_operation_lock() {
    let root = tempfile::tempdir().unwrap();
    let ctx = egui::Context::default();
    let mut app = Explorer::with_context(&ctx, root.path().into());
    complete_navigation(&mut app);
    app.sidebar_options.visible.clear();
    app.sidebar_options.show_applications = false;
    app.sidebar_options.show_trash = false;
    let mut v = volume("/media/test".into());
    v.mounts.clear();
    v.space = None;
    app.volume_sidebar.lock().unwrap().volumes = vec![v.clone()];
    frame(&mut app, &ctx, vec![]);
    let out = frame(&mut app, &ctx, vec![]);
    assert!(text_center(&out, &v.capacity()).is_some());
    let out = click(
        &mut app,
        &ctx,
        text_center(&out, &v.name).unwrap(),
        egui::PointerButton::Secondary,
    );
    assert!(text_center(&out, "Montar e abrir").is_some());
    assert!(text_center(&out, "Desmontar").is_none());
    app.volume_sidebar.lock().unwrap().busy = true;
    app.start_volume(&v, Action::Open, &ctx, |_, _| {
        panic!("another pane's operation must block a new request")
    });
    assert!(app.volume_operation.is_none());
}
