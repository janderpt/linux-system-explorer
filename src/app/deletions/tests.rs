use super::*;
use crate::app::tests::{click_label, complete_navigation, frame, text_center};
use egui::{Event as Input, Modifiers, PointerButton};

fn ftp_connection(path: &Path) -> crate::network::Connection {
    crate::network::Connection {
        url: "ftp://example.test/".into(),
        label: "Test FTP".into(),
        mount: Some(path.to_owned()),
    }
}
fn dialog(app: &mut Explorer, ui: &mut egui::Ui) -> egui::Rect {
    app.deletion_dialog(ui.ctx());
    ui.min_rect()
}

#[test]
fn ftp_del_and_shift_del_always_confirm_permanent_deletion_and_cancel_preserves_files() {
    for shift in [false, true] {
        for confirm_trash in [false, true] {
            let root = tempfile::tempdir().unwrap();
            let file = root.path().join("remote.txt");
            std::fs::write(&file, "preserve before confirmation").unwrap();
            let ctx = egui::Context::default();
            let mut app = Explorer::with_context(&ctx, root.path().into());
            complete_navigation(&mut app);
            app.sidebar_options.remote = vec![ftp_connection(root.path())];
            app.confirm_trash = confirm_trash;
            app.selected.insert(file.clone());
            let modifiers = if shift {
                Modifiers::SHIFT
            } else {
                Modifiers::NONE
            };
            frame(
                &mut app,
                &ctx,
                1180.0,
                vec![Input::Key {
                    key: Key::Delete,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers,
                }],
                |app, ui| {
                    app.deletion_shortcuts(ui.ctx());
                    ui.min_rect()
                },
            );
            assert_eq!(
                app.delete_confirmation.as_ref().unwrap().mode,
                removal::Mode::Permanent
            );
            assert!(app.active_deletion.is_none() && file.exists());
            frame(&mut app, &ctx, 1180.0, vec![], dialog);
            let (output, _) = frame(&mut app, &ctx, 1180.0, vec![], dialog);
            assert!(text_center(&output, "Excluir permanentemente?").is_some());
            assert!(text_center(&output, "Enviar para a lixeira?").is_none());
            assert!(
                text_center(
                    &output,
                    "Os itens FTP serão excluídos diretamente do servidor, sem passar pela lixeira."
                )
                .is_some()
            );
            click_label(
                &mut app,
                &ctx,
                "Cancelar",
                Modifiers::NONE,
                PointerButton::Primary,
                dialog,
            );
            assert!(app.delete_confirmation.is_none() && app.active_deletion.is_none());
            assert_eq!(
                std::fs::read_to_string(&file).unwrap(),
                "preserve before confirmation"
            );
        }
    }
}

#[test]
fn ftp_menu_offers_only_permanent_deletion_and_confirmation_removes_only_selected_items() {
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join("remote.txt");
    let folder = root.path().join("folder");
    let keep = root.path().join("keep.txt");
    std::fs::write(&file, "remove").unwrap();
    std::fs::create_dir(&folder).unwrap();
    std::fs::write(folder.join("nested.txt"), "remove").unwrap();
    std::fs::write(&keep, "keep").unwrap();
    let ctx = egui::Context::default();
    let mut app = Explorer::with_context(&ctx, root.path().into());
    complete_navigation(&mut app);
    app.sidebar_options.remote = vec![ftp_connection(root.path())];
    app.confirm_trash = false;
    app.selected = BTreeSet::from([file.clone(), folder.clone()]);
    let menu = |app: &mut Explorer, ui: &mut egui::Ui| {
        app.deletion_menu(ui);
        ui.min_rect()
    };
    frame(&mut app, &ctx, 1180.0, vec![], menu);
    let (output, _) = frame(&mut app, &ctx, 1180.0, vec![], menu);
    assert!(text_center(&output, "Mover para a lixeira").is_none());
    assert!(text_center(&output, "Excluir permanentemente…").is_some());
    assert!(text_center(&output, "Del / Shift+Del").is_some());
    click_label(
        &mut app,
        &ctx,
        "Excluir permanentemente…",
        Modifiers::NONE,
        PointerButton::Primary,
        menu,
    );
    assert!(app.active_deletion.is_none() && file.exists() && folder.exists());
    frame(&mut app, &ctx, 1180.0, vec![], dialog);
    click_label(
        &mut app,
        &ctx,
        "Excluir permanentemente",
        Modifiers::NONE,
        PointerButton::Primary,
        dialog,
    );
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    while app.active_deletion.is_some() {
        assert!(std::time::Instant::now() < deadline);
        app.poll();
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert!(!file.exists() && !folder.exists());
    assert_eq!(std::fs::read_to_string(keep).unwrap(), "keep");
    assert!(app.error.is_none(), "{:?}", app.error);
}

#[test]
fn mixed_ftp_and_local_del_does_not_silently_promote_local_files_to_permanent_deletion() {
    let root = tempfile::tempdir().unwrap();
    let remote = root.path().join("FTP");
    std::fs::create_dir(&remote).unwrap();
    let local = root.path().join("local.txt");
    let ftp_file = remote.join("remote.txt");
    std::fs::write(&local, "keep local").unwrap();
    std::fs::write(&ftp_file, "keep remote").unwrap();
    let ctx = egui::Context::default();
    let mut app = Explorer::with_context(&ctx, root.path().into());
    complete_navigation(&mut app);
    app.entries.push(filesystem::read_entry(&ftp_file).unwrap());
    app.sidebar_options.remote = vec![ftp_connection(&remote)];
    app.selected = BTreeSet::from([local.clone(), ftp_file.clone()]);
    app.confirm_trash = false;
    app.request_delete(removal::Mode::Trash, &ctx);
    assert!(app.active_deletion.is_none() && app.delete_confirmation.is_none());
    assert!(
        app.notice_toasts
            .iter()
            .any(|notice| notice.kind == crate::app::toast::NoticeKind::Warning)
    );
    assert!(local.exists() && ftp_file.exists());
    app.request_delete(removal::Mode::Permanent, &ctx);
    assert_eq!(
        app.delete_confirmation.as_ref().unwrap().mode,
        removal::Mode::Permanent
    );
    assert!(app.active_deletion.is_none());
}

#[test]
fn dropping_ftp_items_on_trash_requires_explicit_permanent_confirmation() {
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join("remote.txt");
    std::fs::write(&file, "keep").unwrap();
    let ctx = egui::Context::default();
    let mut app = Explorer::with_context(&ctx, root.path().into());
    complete_navigation(&mut app);
    app.sidebar_options.remote = vec![ftp_connection(root.path())];
    app.confirm_trash = false;
    app.delete_dropped_entries(vec![filesystem::read_entry(&file).unwrap()], &ctx);
    assert_eq!(
        app.delete_confirmation.as_ref().unwrap().mode,
        removal::Mode::Permanent
    );
    assert!(app.active_deletion.is_none() && file.exists());
}
