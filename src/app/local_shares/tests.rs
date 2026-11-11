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
fn click(app: &mut Explorer, ctx: &egui::Context, point: Pos2) -> FullOutput {
    render(
        app,
        ctx,
        vec![
            Event::PointerMoved(point),
            Event::PointerButton {
                pos: point,
                button: PointerButton::Primary,
                pressed: true,
                modifiers: Modifiers::NONE,
            },
        ],
    );
    render(
        app,
        ctx,
        vec![
            Event::PointerMoved(point),
            Event::PointerButton {
                pos: point,
                button: PointerButton::Primary,
                pressed: false,
                modifiers: Modifiers::NONE,
            },
        ],
    )
}
fn setup() -> (tempfile::TempDir, egui::Context, Explorer) {
    let root = tempfile::tempdir().unwrap();
    let ctx = egui::Context::default();
    let mut app = Explorer::with_context(&ctx, root.path().into());
    complete_navigation(&mut app);
    let mut share = Share::new(root.path().into());
    share.name = "Saved share".into();
    let mut manager = Manager::new(None);
    manager.accept(vec![share], vec!["10.1.1.30".into()]);
    app.local_shares = Some(manager);
    render(&mut app, &ctx, vec![]);
    render(&mut app, &ctx, vec![]);
    (root, ctx, app)
}
#[test]
fn manager_copies_persisted_addresses_instead_of_unsaved_name() {
    let (_root, ctx, mut app) = setup();
    app.local_shares
        .as_mut()
        .unwrap()
        .draft
        .as_mut()
        .unwrap()
        .name = "Unsaved".into();
    for (button, expected) in [
        ("Copiar endereço SMB", "smb://10.1.1.30/Saved%20share"),
        ("Copiar endereço Windows", "\\\\10.1.1.30\\Saved share"),
    ] {
        let output = render(&mut app, &ctx, vec![]);
        let point = text_center(&output, button).unwrap();
        let output = click(&mut app, &ctx, point);
        assert!(output.platform_output.commands.iter().any(
            |command| matches!(command,egui::OutputCommand::CopyText(text) if text==expected)
        ));
    }
}
#[test]
fn stopping_requires_confirmation_and_cancel_keeps_share_and_files() {
    let (root, ctx, mut app) = setup();
    let marker = root.path().join("keep.txt");
    std::fs::write(&marker, "keep").unwrap();
    let output = render(&mut app, &ctx, vec![]);
    click(
        &mut app,
        &ctx,
        text_center(&output, "Parar de compartilhar").unwrap(),
    );
    assert!(app.local_shares.as_ref().unwrap().confirm_stop);
    let output = render(&mut app, &ctx, vec![]);
    assert!(text_center(&output, "Parar de compartilhar esta pasta?").is_some());
    click(&mut app, &ctx, text_center(&output, "Cancelar").unwrap());
    let manager = app.local_shares.as_ref().unwrap();
    assert!(!manager.confirm_stop);
    assert!(manager.pending.is_none());
    assert_eq!(manager.shares.len(), 1);
    assert_eq!(std::fs::read_to_string(marker).unwrap(), "keep");
}
#[test]
fn other_owners_are_read_only_and_invalid_edits_never_start_an_operation() {
    let (_root, ctx, mut app) = setup();
    app.local_shares
        .as_mut()
        .unwrap()
        .draft
        .as_mut()
        .unwrap()
        .editable = false;
    let output = render(&mut app, &ctx, vec![]);
    click(
        &mut app,
        &ctx,
        text_center(&output, "Salvar compartilhamento").unwrap(),
    );
    assert!(app.local_shares.as_ref().unwrap().pending.is_none());
    let draft = app.local_shares.as_mut().unwrap().draft.as_mut().unwrap();
    draft.editable = true;
    draft.name = "../invalid".into();
    let output = render(&mut app, &ctx, vec![]);
    click(
        &mut app,
        &ctx,
        text_center(&output, "Salvar compartilhamento").unwrap(),
    );
    let manager = app.local_shares.as_ref().unwrap();
    assert!(manager.error.is_some());
    assert!(manager.pending.is_none());
}
#[test]
fn requested_folder_selects_existing_share_or_prepares_private_new_share() {
    let root = tempfile::tempdir().unwrap();
    let mut share = Share::new(root.path().into());
    share.name = "Existing".into();
    let mut manager = Manager::new(Some(root.path().into()));
    manager.accept(vec![share.clone()], vec!["localhost".into()]);
    assert_eq!(manager.original, Some(share));
    let folder = root.path().join("New folder");
    std::fs::create_dir(&folder).unwrap();
    manager.requested_folder = Some(folder.clone());
    manager.accept(vec![], vec!["localhost".into()]);
    assert!(manager.original.is_none());
    let draft = manager.draft.unwrap();
    assert_eq!(draft.path, folder);
    assert!(!draft.guests);
    assert_eq!(draft.rules.len(), 1);
    assert_eq!(draft.rules[0].access, Access::Write);
}

#[test]
fn manager_footer_stays_visible_in_small_windows_and_does_not_grow_each_frame() {
    for size in [Vec2::new(1180.0, 760.0), Vec2::new(780.0, 480.0)] {
        let (_root, ctx, mut app) = setup();
        for confirm in [false, true] {
            app.local_shares.as_mut().unwrap().confirm_stop = confirm;
            for _ in 0..8 {
                let _ = ctx.run(
                    RawInput {
                        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
                        ..Default::default()
                    },
                    |ctx| app.show(ctx),
                );
            }
            let output = ctx.run(
                RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
                    ..Default::default()
                },
                |ctx| app.show(ctx),
            );
            for label in [
                "Fechar",
                "Salvar compartilhamento",
                "Configurar pelo sistema",
            ] {
                let point = text_center(&output, label)
                    .unwrap_or_else(|| panic!("{label}: {size:?}, confirm={confirm}"));
                assert!(
                    Rect::from_min_size(Pos2::ZERO, size).contains(point),
                    "{label}: {point:?}"
                );
            }
            if confirm {
                assert!(text_center(&output, "Confirmar remoção do compartilhamento").is_some());
            }
        }
    }
}

#[test]
fn pending_changes_finish_when_polled_in_background() {
    let (_root, _ctx, mut app) = setup();
    let (sender, receiver) = mpsc::channel();
    let manager = app.local_shares.as_mut().unwrap();
    let shares = manager.shares.clone();
    manager.pending = Some(receiver);
    assert!(app.local_shares_pending());
    sender.send(Ok((shares, vec!["localhost".into()]))).unwrap();
    app.poll();
    assert!(!app.local_shares_pending());
    assert!(app.local_shares.as_ref().unwrap().pending.is_none());
    assert_eq!(app.local_shares.as_ref().unwrap().shares.len(), 1);
}

#[test]
fn sidebar_separates_local_shares_from_remote_connections_and_opens_local_folder() {
    let root = tempfile::tempdir().unwrap();
    let folder = root.path().join("shared");
    std::fs::create_dir(&folder).unwrap();
    let ctx = egui::Context::default();
    let mut app = Explorer::with_context(&ctx, root.path().into());
    complete_navigation(&mut app);
    app.sidebar_options.visible.clear();
    app.sidebar_options.show_trash = false;
    app.sidebar_options.show_pinned = false;
    let mut share = Share::new(folder.clone());
    share.name = "Publicado aqui".into();
    app.publish_local_share_sidebar(vec![share]);
    render(&mut app, &ctx, vec![]);
    let output = render(&mut app, &ctx, vec![]);
    assert!(text_center(&output, "Compartilhamentos locais").is_some());
    assert!(text_center(&output, "REDE").is_some());
    assert!(text_center(&output, "Nenhuma conexão de rede configurada").is_some());
    let point = text_center(&output, "Publicado aqui").unwrap();
    click(&mut app, &ctx, point);
    complete_navigation(&mut app);
    assert_eq!(app.current, folder);
    app.sidebar_options.show_local_shares = false;
    assert!(
        !app.sidebar_entries()
            .iter()
            .any(|e| matches!(e.id, sidebar::Location::LocalShare(_)))
    );
    let output = render(&mut app, &ctx, vec![]);
    assert!(text_center(&output, "Compartilhamentos locais").is_none());
}

#[test]
fn manager_results_immediately_replace_sidebar_entries_after_create_rename_and_stop() {
    let (_root, _ctx, mut app) = setup();
    let mut share = app.local_shares.as_ref().unwrap().shares[0].clone();
    for name in [Some("created"), Some("renamed"), None] {
        let list = if let Some(name) = name {
            share.name = name.into();
            vec![share.clone()]
        } else {
            vec![]
        };
        let (sender, receiver) = mpsc::channel();
        app.local_shares.as_mut().unwrap().pending = Some(receiver);
        sender
            .send(Ok((list.clone(), vec!["localhost".into()])))
            .unwrap();
        app.poll();
        assert_eq!(app.local_share_entries(), list);
        let labels = app
            .sidebar_entries()
            .into_iter()
            .filter(|e| matches!(e.id, sidebar::Location::LocalShare(_)))
            .map(|e| e.label)
            .collect::<Vec<_>>();
        assert_eq!(
            labels,
            name.into_iter().map(str::to_owned).collect::<Vec<_>>()
        );
    }
}

#[test]
fn local_share_visibility_defaults_on_and_persists_independently_of_network() {
    let mut prefs: locations::Preferences =
        serde_json::from_str(r#"{"show_network":false}"#).unwrap();
    assert!(prefs.show_local_shares);
    assert!(!prefs.show_network);
    prefs.show_local_shares = false;
    prefs.show_network = true;
    let restored: locations::Preferences =
        serde_json::from_str(&serde_json::to_string(&prefs).unwrap()).unwrap();
    assert!(!restored.show_local_shares);
    assert!(restored.show_network);
}
