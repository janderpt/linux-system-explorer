use super::*;
use crate::app::tests::{complete_navigation, text_center};
use egui::{Event, FullOutput, Modifiers, PointerButton, Pos2, RawInput, Rect};

fn render(app: &mut Explorer, ctx: &egui::Context, events: Vec<Event>) -> FullOutput {
    ctx.run(
        RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1180.0, 760.0))),
            time: Some(ctx.input(|i| i.time) + 0.2),
            events,
            ..Default::default()
        },
        |ctx| app.show(ctx),
    )
}
fn click(app: &mut Explorer, ctx: &egui::Context, point: Pos2, button: PointerButton) {
    for pressed in [true, false] {
        render(
            app,
            ctx,
            vec![
                Event::PointerMoved(point),
                Event::PointerButton {
                    pos: point,
                    button,
                    pressed,
                    modifiers: Modifiers::NONE,
                },
            ],
        );
    }
}

#[test]
fn sidebar_addition_connects_an_existing_mount_and_persists_its_name() {
    let root = tempfile::tempdir().unwrap();
    let mount = root.path().join("mounted share");
    std::fs::create_dir(&mount).unwrap();
    let ctx = egui::Context::default();
    let mut app = Explorer::with_context(&ctx, root.path().to_owned());
    complete_navigation(&mut app);
    let file = root.path().join("preferences.json");
    app.settings_path = Some(file.clone());
    app.sidebar_options.visible.clear();
    app.sidebar_options.show_pinned = false;
    app.sidebar_options.show_trash = false;
    render(&mut app, &ctx, vec![]);
    let output = render(&mut app, &ctx, vec![]);
    let p = text_center(&output, "Adicionar compartilhamento de rede").unwrap();
    click(&mut app, &ctx, p, PointerButton::Primary);
    let dialog = app.network_dialog.as_mut().unwrap();
    dialog.address = mount.to_string_lossy().into_owned();
    dialog.label = "Equipe".into();
    let output = render(&mut app, &ctx, vec![]);
    click(
        &mut app,
        &ctx,
        text_center(&output, "Conectar e adicionar").unwrap(),
        PointerButton::Primary,
    );
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    while app.network_dialog.is_some() {
        render(&mut app, &ctx, vec![]);
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    complete_navigation(&mut app);
    assert_eq!(app.current, mount);
    let prefs = Preferences::load(&file).unwrap();
    assert_eq!(prefs.sidebar.network[0].label, "Equipe");
    assert_eq!(prefs.sidebar.network[0].path, mount);
}

#[test]
fn remote_mount_result_is_persisted_and_cancelling_or_invalid_credentials_adds_nothing() {
    let root = tempfile::tempdir().unwrap();
    let ctx = egui::Context::default();
    let mut app = Explorer::with_context(&ctx, root.path().to_owned());
    complete_navigation(&mut app);
    let prefs_file = root.path().join("prefs.json");
    app.settings_path = Some(prefs_file.clone());
    let mut dialog = ConnectDialog::new("smb://user:secret@server/share".into(), "Private".into());
    Explorer::start_network_connection(&mut dialog, &ctx);
    assert!(dialog.result.is_none());
    assert!(dialog.error.is_some());
    app.network_dialog = Some(dialog);
    render(&mut app, &ctx, vec![]);
    let output = render(&mut app, &ctx, vec![]);
    click(
        &mut app,
        &ctx,
        text_center(&output, "Cancelar").unwrap(),
        PointerButton::Primary,
    );
    assert!(app.network_dialog.is_none());
    assert!(app.sidebar_options.remote.is_empty());
    let (sender, receiver) = mpsc::channel();
    sender.send(Ok(root.path().to_owned())).unwrap();
    let mut dialog = ConnectDialog::new("smb://server/share".into(), "Equipe remota".into());
    dialog.url = Some(dialog.address.clone());
    dialog.result = Some(receiver);
    app.network_dialog = Some(dialog);
    render(&mut app, &ctx, vec![]);
    complete_navigation(&mut app);
    assert!(app.network_dialog.is_none());
    let prefs = Preferences::load(&prefs_file).unwrap();
    assert_eq!(prefs.sidebar.remote.len(), 1);
    assert_eq!(prefs.sidebar.remote[0].url, "smb://server/share");
    assert_eq!(prefs.sidebar.remote[0].mount.as_deref(), Some(root.path()));
    assert_eq!(prefs.sidebar.remote[0].title(), "Equipe remota");
    assert!(
        !std::fs::read_to_string(&prefs_file)
            .unwrap()
            .contains("secret")
    );
    app.sidebar_options = prefs.sidebar;
    let entries = app.sidebar_entries();
    assert!(
        entries
            .iter()
            .any(|e| e.id == sidebar::Location::Remote(0) && e.label == "Equipe remota")
    );
}

#[test]
fn share_over_network_is_available_only_for_folders_in_item_context_menus() {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir(root.path().join("Folder")).unwrap();
    std::fs::write(root.path().join("file.txt"), "test").unwrap();
    for title in ["Folder", "file.txt"] {
        let ctx = egui::Context::default();
        let mut app = Explorer::with_context(&ctx, root.path().to_owned());
        complete_navigation(&mut app);
        render(&mut app, &ctx, vec![]);
        let output = render(&mut app, &ctx, vec![]);
        click(
            &mut app,
            &ctx,
            text_center(&output, title).unwrap(),
            PointerButton::Secondary,
        );
        let output = render(&mut app, &ctx, vec![]);
        assert_eq!(
            text_center(&output, "Compartilhar pela rede").is_some(),
            title == "Folder"
        );
    }
}

#[test]
fn smb_server_listing_requires_selection_and_only_saves_the_chosen_share() {
    let root = tempfile::tempdir().unwrap();
    let ctx = egui::Context::default();
    let mut app = Explorer::with_context(&ctx, root.path().into());
    complete_navigation(&mut app);
    let prefs = root.path().join("preferences.json");
    app.settings_path = Some(prefs.clone());
    let (sender, receiver) = mpsc::channel();
    sender
        .send(Ok(vec![
            crate::network::ServerShare {
                name: "Docs".into(),
                url: "smb://server/Docs".into(),
            },
            crate::network::ServerShare {
                name: "Images".into(),
                url: "smb://server/Images".into(),
            },
        ]))
        .unwrap();
    let mut dialog = ConnectDialog::new("smb://server/".into(), "Equipe".into());
    dialog.url = Some("smb://server/".into());
    dialog.browse_result = Some(receiver);
    app.network_dialog = Some(dialog);
    render(&mut app, &ctx, vec![]);
    let output = render(&mut app, &ctx, vec![]);
    assert!(text_center(&output, "Compartilhamentos disponíveis").is_some());
    assert!(app.sidebar_options.remote.is_empty());
    let point = text_center(&output, "Docs").unwrap();
    click(&mut app, &ctx, point, PointerButton::Primary);
    assert_eq!(app.network_dialog.as_ref().unwrap().selected_share, Some(0));
    assert!(app.sidebar_options.remote.is_empty());
    let dialog = app.network_dialog.as_mut().unwrap();
    let (sender, receiver) = mpsc::channel();
    sender.send(Ok(root.path().into())).unwrap();
    dialog.url = Some(dialog.shares.as_ref().unwrap().1[0].url.clone());
    dialog.result = Some(receiver);
    render(&mut app, &ctx, vec![]);
    complete_navigation(&mut app);
    let prefs = Preferences::load(&prefs).unwrap();
    assert_eq!(prefs.sidebar.remote.len(), 1);
    assert_eq!(prefs.sidebar.remote[0].url, "smb://server/Docs");
}

#[test]
fn cancelling_server_listing_cancels_work_and_does_not_create_a_shortcut() {
    let root = tempfile::tempdir().unwrap();
    let ctx = egui::Context::default();
    let mut app = Explorer::with_context(&ctx, root.path().into());
    complete_navigation(&mut app);
    let (_sender, receiver) = mpsc::channel();
    let mut dialog = ConnectDialog::new("smb://server/".into(), String::new());
    let cancel = dialog.cancel.clone();
    dialog.browse_result = Some(receiver);
    app.network_dialog = Some(dialog);
    render(&mut app, &ctx, vec![]);
    let output = render(&mut app, &ctx, vec![]);
    click(
        &mut app,
        &ctx,
        text_center(&output, "Cancelar").unwrap(),
        PointerButton::Primary,
    );
    assert!(app.network_dialog.is_none());
    assert!(app.sidebar_options.remote.is_empty());
    assert_eq!(
        crate::network::browse_server("smb://server/", &cancel)
            .unwrap_err()
            .kind(),
        io::ErrorKind::Interrupted
    );
}

#[test]
fn network_address_bar_routes_to_the_connection_dialog_and_rejects_uri_passwords() {
    let root = tempfile::tempdir().unwrap();
    let ctx = egui::Context::default();
    let mut app = Explorer::with_context(&ctx, root.path().into());
    complete_navigation(&mut app);
    app.editing_address = true;
    app.address = "smb://user:secret@server/".into();
    app.submit_address(&ctx);
    assert!(!app.editing_address);
    assert_eq!(app.current, root.path());
    let dialog = app.network_dialog.as_ref().unwrap();
    assert!(dialog.error.is_some());
    assert!(dialog.result.is_none());
    assert!(dialog.browse_result.is_none());
    assert!(app.sidebar_options.remote.is_empty());
}
