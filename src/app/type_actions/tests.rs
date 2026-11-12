use super::*;
use crate::app::tests::{complete_navigation, folder_frame, text_center};
use egui::{Event as Input, Modifiers, PointerButton, Pos2};
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    process::Command,
    time::{Duration, Instant},
};
fn pointer(point: Pos2, button: PointerButton, pressed: bool) -> Vec<Input> {
    vec![
        Input::PointerMoved(point),
        Input::PointerButton {
            pos: point,
            button,
            pressed,
            modifiers: Modifiers::NONE,
        },
    ]
}
#[test]
fn type_specific_archive_section_respects_selection_in_every_view_and_invokes_only_selected_archives()
 {
    let Some(root) = std::env::var_os("LSE_TYPE_MENU_TEST") else {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir(root.path().join("bin")).unwrap();
        let program = root.path().join("bin/ark");
        fs::write(
            &program,
            "#!/bin/sh\nprintf '%s\\0' \"$@\" > \"$LSE_TYPE_MENU_CAPTURE\"\n",
        )
        .unwrap();
        fs::set_permissions(program, fs::Permissions::from_mode(0o755)).unwrap();
        let result=Command::new(std::env::current_exe().unwrap()).args(["--exact","app::type_actions::tests::type_specific_archive_section_respects_selection_in_every_view_and_invokes_only_selected_archives","--nocapture"])
            .env("LSE_TYPE_MENU_TEST",root.path()).env("LSE_TYPE_MENU_CAPTURE",root.path().join("captured"))
            .env("PATH",root.path().join("bin")).env("XDG_CURRENT_DESKTOP","KDE").env_remove("FLATPAK_ID").output().unwrap();
        assert!(
            result.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
        return;
    };
    let root = PathBuf::from(root);
    fs::write(root.join("one.zip"), "one").unwrap();
    fs::write(root.join("two.tar.gz"), "two").unwrap();
    fs::write(root.join("note.txt"), "text").unwrap();
    fs::create_dir(root.join("folder.zip")).unwrap();
    let ctx = egui::Context::default();
    let mut app = Explorer::with_context(&ctx, root.clone());
    complete_navigation(&mut app);
    let archive = app
        .entries
        .iter()
        .find(|e| e.name == "one.zip")
        .unwrap()
        .clone();
    let folder = app
        .entries
        .iter()
        .find(|e| e.name == "folder.zip")
        .unwrap()
        .clone();
    app.selected = BTreeSet::from([archive.path.clone()]);
    assert_eq!(app.selected_archives(&archive), vec![archive.path.clone()]);
    assert!(app.selected_archives(&folder).is_empty());
    app.selected.insert(root.join("note.txt"));
    assert!(
        app.selected_archives(&archive).is_empty(),
        "seleção mista não recebe extração parcial"
    );
    app.selected = BTreeSet::from([archive.path.clone(), root.join("two.tar.gz")]);
    assert_eq!(app.selected_archives(&archive).len(), 2);
    for view in [View::Details, View::List, View::Icons] {
        app.view = view;
        folder_frame(&mut app, &ctx, vec![]);
        let output = folder_frame(&mut app, &ctx, vec![]);
        let point = text_center(&output, "one.zip").unwrap();
        for pressed in [true, false] {
            folder_frame(
                &mut app,
                &ctx,
                pointer(point, PointerButton::Secondary, pressed),
            );
        }
        let output = folder_frame(&mut app, &ctx, vec![]);
        assert!(text_center(&output, "Arquivos compactados").is_some());
        assert!(text_center(&output, "Extrair aqui").is_some());
        assert!(text_center(&output, "Extrair em…").is_some());
        assert!(text_center(&output, "Ver conteúdo deste arquivo").is_some());
        assert!(
            text_center(&output, "Pré-visualizar").is_none(),
            "sem preview duplicado"
        );
        assert!(text_center(&output, "Copiar").is_some());
        assert!(text_center(&output, "Renomear").is_some());
        egui::Popup::close_all(&ctx);
        folder_frame(&mut app, &ctx, vec![]);
    }
    app.view = View::Details;
    folder_frame(&mut app, &ctx, vec![]);
    let output = folder_frame(&mut app, &ctx, vec![]);
    let point = text_center(&output, "one.zip").unwrap();
    for pressed in [true, false] {
        folder_frame(
            &mut app,
            &ctx,
            pointer(point, PointerButton::Secondary, pressed),
        );
    }
    let output = folder_frame(&mut app, &ctx, vec![]);
    let point = text_center(&output, "Extrair aqui").unwrap();
    for pressed in [true, false] {
        folder_frame(
            &mut app,
            &ctx,
            pointer(point, PointerButton::Primary, pressed),
        );
    }
    let deadline = Instant::now() + Duration::from_secs(3);
    while !app.refresh_after_transfer && Instant::now() < deadline {
        app.poll();
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(app.refresh_after_transfer);
    let bytes = fs::read(root.join("captured")).unwrap();
    let args: Vec<_> = bytes.split(|b| *b == 0).filter(|s| !s.is_empty()).collect();
    assert_eq!(args.len(), 6);
    assert_eq!(args[0], b"--batch");
    let received: BTreeSet<_> = args[4..].iter().map(|p| p.to_vec()).collect();
    let expected = BTreeSet::from([
        archive.path.as_os_str().as_encoded_bytes().to_vec(),
        root.join("two.tar.gz")
            .as_os_str()
            .as_encoded_bytes()
            .to_vec(),
    ]);
    assert_eq!(received, expected);
    assert!(!bytes.windows(b"note.txt".len()).any(|b| b == b"note.txt"));
    assert_eq!(fs::read_to_string(&archive.path).unwrap(), "one");
    assert!(
        app.notice_toasts.is_empty(),
        "solicitar extração externa não inventa aviso de sucesso"
    );
    app.current = crate::trashbin::location();
    assert!(app.selected_archives(&archive).is_empty());
    app.current = crate::applications::menu::location();
    assert!(app.selected_archives(&archive).is_empty());
}
