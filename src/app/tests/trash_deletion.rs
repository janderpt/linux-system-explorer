use super::*;

#[test]
fn del_and_context_delete_only_selected_trash_items_after_confirmation() {
    let Some(root) = std::env::var_os("EXPLORER_TRASH_SELECTION_DELETE_ROOT") else {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join("data")).unwrap();
        let output = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "app::tests::trash_deletion::del_and_context_delete_only_selected_trash_items_after_confirmation", "--nocapture"])
            .env("EXPLORER_TRASH_SELECTION_DELETE_ROOT", root.path())
            .env("XDG_DATA_HOME", root.path().join("data")).env("HOME", root.path()).output().unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            std::fs::read_to_string(root.path().join("preserved/keep")).unwrap(),
            "keep"
        );
        return;
    };
    let root = PathBuf::from(root);
    let source = root.join("source");
    std::fs::create_dir(&source).unwrap();
    let prefix = root.file_name().unwrap().to_string_lossy().into_owned();
    let file_name = format!("{prefix}-selected.pdf");
    let folder_name = format!("{prefix}-folder");
    let keep_name = format!("{prefix}-keep.txt");
    let file = source.join(&file_name);
    std::fs::write(&file, "selected").unwrap();
    let folder = source.join(&folder_name);
    std::fs::create_dir(&folder).unwrap();
    std::fs::write(folder.join(".hidden"), "hidden").unwrap();
    let preserved = root.join("preserved");
    std::fs::create_dir(&preserved).unwrap();
    std::fs::write(preserved.join("keep"), "keep").unwrap();
    std::os::unix::fs::symlink(&preserved, folder.join("link")).unwrap();
    let keep = source.join(&keep_name);
    std::fs::write(&keep, "unselected").unwrap();
    ::trash::delete_all([&file, &folder, &keep]).unwrap();
    let ctx = egui::Context::default();
    let mut app = Explorer::with_context(&ctx, crate::trashbin::location());
    complete_navigation(&mut app);
    app.confirm_trash = false;
    app.query = prefix.clone();
    let selected_ids: BTreeSet<_> = app
        .entries
        .iter()
        .filter(|entry| {
            entry.trash.as_ref().is_some_and(|record| {
                record.original_path == file || record.original_path == folder
            })
        })
        .map(|entry| entry.path.clone())
        .collect();
    assert_eq!(selected_ids.len(), 2);
    assert!(
        selected_ids
            .iter()
            .all(|path| path.starts_with(root.join("data/Trash/files")))
    );
    let receipts: Vec<_> = selected_ids
        .iter()
        .map(|path| {
            let mut name = path.file_name().unwrap().to_owned();
            name.push(".trashinfo");
            root.join("data/Trash/info").join(name)
        })
        .collect();
    browser_frame(&mut app, &ctx, vec![]);
    let output = browser_frame(&mut app, &ctx, vec![]);
    let point = text_center(&output, &file_name).unwrap();
    let output = browser_click(
        &mut app,
        &ctx,
        point,
        PointerButton::Primary,
        Modifiers::NONE,
    );
    let point = text_center(&output, &folder_name).unwrap();
    browser_click(
        &mut app,
        &ctx,
        point,
        PointerButton::Primary,
        Modifiers::CTRL,
    );
    assert_eq!(app.selected, selected_ids);
    transfer_key(&mut app, &ctx, delete_key(false));
    assert!(app.delete_confirmation.is_some() && app.active_deletion.is_none());
    let dialog = |app: &mut Explorer, ui: &mut egui::Ui| {
        app.deletion_dialog(ui.ctx());
        ui.min_rect()
    };
    frame(&mut app, &ctx, 1180.0, vec![], dialog);
    let (output, _) = frame(&mut app, &ctx, 1180.0, vec![], dialog);
    assert!(text_center(&output, "Excluir permanentemente?").is_some());
    assert!(
        text_center(&output, &file_name).is_some() && text_center(&output, &folder_name).is_some()
    );
    assert!(selected_ids.iter().all(|path| path.exists()));
    click_label(
        &mut app,
        &ctx,
        "Cancelar",
        Modifiers::NONE,
        PointerButton::Primary,
        dialog,
    );
    assert!(app.delete_confirmation.is_none() && selected_ids.iter().all(|path| path.exists()));
    assert!(receipts.iter().all(|path| path.exists()));
    let output = browser_frame(&mut app, &ctx, vec![]);
    let point = text_center(&output, &file_name).unwrap();
    let output = browser_click(
        &mut app,
        &ctx,
        point,
        PointerButton::Secondary,
        Modifiers::NONE,
    );
    assert_eq!(app.selected, selected_ids);
    assert!(text_center(&output, "Mover para a lixeira").is_none());
    assert!(text_center(&output, "Del").is_some());
    let remove = text_center(&output, "Excluir permanentemente…").unwrap();
    browser_click(
        &mut app,
        &ctx,
        remove,
        PointerButton::Primary,
        Modifiers::NONE,
    );
    assert!(app.delete_confirmation.is_some() && app.active_deletion.is_none());
    let late_name = format!("{prefix}-late.txt");
    let late = source.join(&late_name);
    std::fs::write(&late, "later").unwrap();
    ::trash::delete(&late).unwrap();
    assert_eq!(app.selected, selected_ids);
    browser_frame(&mut app, &ctx, vec![]);
    let output = browser_frame(&mut app, &ctx, vec![]);
    let confirm = text_center(&output, "Excluir permanentemente").unwrap();
    browser_click(
        &mut app,
        &ctx,
        confirm,
        PointerButton::Primary,
        Modifiers::NONE,
    );
    assert!(app.active_deletion.is_some());
    assert!(!app.can_restore() && !app.can_empty_trash());
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while app.active_deletion.is_some() && std::time::Instant::now() < deadline {
        app.poll();
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert!(app.active_deletion.is_none());
    app.process_transfer_refresh();
    complete_navigation(&mut app);
    assert!(app.error.is_none(), "{:?}", app.error);
    assert!(
        app.selected.is_empty() && app.selection_anchor.is_none() && app.selection_cursor.is_none()
    );
    assert!(
        selected_ids
            .iter()
            .all(|path| std::fs::symlink_metadata(path).is_err())
    );
    assert!(receipts.iter().all(|path| !path.exists()));
    let remaining: Vec<_> = app
        .entries
        .iter()
        .filter(|entry| {
            entry
                .trash
                .as_ref()
                .is_some_and(|record| record.original_path.starts_with(&source))
        })
        .collect();
    assert_eq!(remaining.len(), 2);
    for name in [&keep_name, &late_name] {
        assert!(
            remaining
                .iter()
                .any(|entry| entry.name == *name && entry.path.exists())
        );
    }
    assert_eq!(
        std::fs::read_to_string(preserved.join("keep")).unwrap(),
        "keep"
    );
    assert!(
        app.notice_toasts
            .last()
            .unwrap()
            .text
            .contains("2 itens excluídos permanentemente")
    );
}
