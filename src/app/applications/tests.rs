use super::*;
use crate::app::tests::{complete_navigation, folder_frame, text_center};
use egui::{Event as Input, Modifiers, PointerButton};
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    process::Command,
    time::{Duration, Instant},
};

#[test]
fn applications_sidebar_filter_double_click_launch_and_session_are_integrated() {
    let Some(root) = std::env::var_os("LSE_APPLICATIONS_TEST_ROOT") else {
        let root = tempfile::tempdir().unwrap();
        let data = root.path().join("data/applications");
        fs::create_dir_all(data.join("nested")).unwrap();
        fs::create_dir(root.path().join("bin")).unwrap();
        fs::write(root.path().join("app.svg"), r##"<svg xmlns="http://www.w3.org/2000/svg" width="32" height="32"><rect width="32" height="32" fill="#39a"/></svg>"##).unwrap();
        for (file, name) in [
            ("nested/tool.desktop", "Ferramenta Alpha"),
            ("beta.desktop", "Editor Beta"),
        ] {
            fs::write(data.join(file), format!("[Desktop Entry]\nType=Application\nName={name}\nExec=/bin/true\nIcon={}\nCategories=Utility;Development;\nComment=Uma descrição sem relação com o nome\n", root.path().join("app.svg").display())).unwrap();
        }
        let gio = root.path().join("bin/gio");
        fs::write(
            &gio,
            "#!/bin/sh\nprintf '%s\\0' \"$@\" > \"$LSE_MENU_CAPTURE\"\n",
        )
        .unwrap();
        fs::set_permissions(gio, fs::Permissions::from_mode(0o755)).unwrap();
        let result = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "app::applications::tests::applications_sidebar_filter_double_click_launch_and_session_are_integrated", "--nocapture"])
            .env("LSE_APPLICATIONS_TEST_ROOT", root.path())
            .env("LSE_MENU_CAPTURE", root.path().join("launched"))
            .env("HOME", root.path()).env("XDG_CONFIG_HOME", root.path().join("config"))
            .env("XDG_DATA_HOME", root.path().join("data"))
            .env("XDG_DATA_DIRS", root.path().join("empty-system-data"))
            .env("PATH", format!("{}:/usr/bin:/bin", root.path().join("bin").display()))
            .env_remove("FLATPAK_ID")
            .output().unwrap();
        assert!(
            result.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
        return;
    };
    crate::i18n::set_locale(crate::i18n::Locale::Portuguese);
    let root = PathBuf::from(root);
    let ctx = egui::Context::default();
    let mut app = Explorer::with_context(&ctx, root.clone());
    complete_navigation(&mut app);
    assert_eq!(app.sidebar_entries()[0].id, sidebar::Location::Applications);
    app.navigate(menu::location(), Navigation::Visit);
    complete_navigation(&mut app);
    assert!(app.is_applications());
    assert!(app.entries.is_empty());
    assert_eq!(app.applications.items.len(), 2);
    assert!(
        app.applications
            .items
            .iter()
            .all(|item| item.icon.is_some())
    );
    assert_eq!(app.display_title(&app.current), "Aplicativos");
    app.query = "ALPHA".into();
    assert_eq!(app.applications.visible(&app.query).len(), 1);
    app.query = "descrição".into();
    assert!(
        app.applications.visible(&app.query).is_empty(),
        "busca somente pelo nome"
    );
    app.query.clear();
    folder_frame(&mut app, &ctx, vec![]);
    let output = folder_frame(&mut app, &ctx, vec![]);
    let point = text_center(&output, "Ferramenta Alpha").unwrap();
    assert!(text_center(&output, "Editor Beta").is_some());
    assert!(text_center(&output, "Development").is_none());
    let click = || {
        vec![
            Input::PointerMoved(point),
            Input::PointerButton {
                pos: point,
                button: PointerButton::Primary,
                pressed: true,
                modifiers: Modifiers::NONE,
            },
            Input::PointerButton {
                pos: point,
                button: PointerButton::Primary,
                pressed: false,
                modifiers: Modifiers::NONE,
            },
        ]
    };
    folder_frame(&mut app, &ctx, click());
    assert_eq!(
        app.applications.selected.as_deref(),
        Some("nested-tool.desktop")
    );
    assert!(
        !root.join("launched").exists(),
        "clique simples só seleciona"
    );
    folder_frame(&mut app, &ctx, click());
    let deadline = Instant::now() + Duration::from_secs(3);
    while !root.join("launched").exists() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(5));
    }
    let launched = fs::read(root.join("launched")).expect("duplo clique deve iniciar aplicativo");
    let args: Vec<_> = launched
        .split(|b| *b == 0)
        .filter(|s| !s.is_empty())
        .collect();
    assert_eq!(
        args,
        vec![
            b"launch".as_slice(),
            root.join("data/applications/nested/tool.desktop")
                .as_os_str()
                .as_encoded_bytes()
        ]
    );
    let icon = app.applications.items[0].icon.clone().unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    while app.thumbnails.texture(&icon, &ctx).is_none() {
        assert!(
            Instant::now() < deadline,
            "ícone deve ser decodificado em segundo plano"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    fs::write(
        root.join("bin/gio"),
        "#!/bin/sh\necho Falha-do-launcher >&2\nexit 1\n",
    )
    .unwrap();
    crate::app::tests::frame(
        &mut app,
        &ctx,
        1180.0,
        vec![Input::Key {
            key: Key::Enter,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        }],
        |app, ui| {
            app.application_shortcuts(ui.ctx());
            ui.min_rect()
        },
    );
    let deadline = Instant::now() + Duration::from_secs(3);
    while app.notice_toasts.is_empty() && Instant::now() < deadline {
        app.poll();
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(
        app.error
            .as_deref()
            .is_some_and(|error| error.contains("Falha-do-launcher")),
        "Enter inicia e falhas usam o toast de erro"
    );
    assert!(!app.notice_toasts.is_empty());
    assert!(app.selected.is_empty());
    assert!(!app.can_paste());
    assert!(!app.file_drop_available());
    app.capture_files(file_ops::Mode::Cut, &ctx);
    app.request_delete(crate::deletions::Mode::Permanent, &ctx);
    assert!(app.file_clipboard.is_none());
    assert!(app.delete_confirmation.is_none());
    assert!(app.active_deletion.is_none());
    assert!(root.join("data/applications/nested/tool.desktop").exists());
    app.navigate(root.clone(), Navigation::Back);
    complete_navigation(&mut app);
    assert!(!app.is_applications());
    app.navigate(menu::location(), Navigation::Forward);
    complete_navigation(&mut app);
    assert!(app.is_applications());
    let state = crate::session::State::from_paths([root, menu::location()], 1);
    let restored: crate::session::State =
        serde_json::from_str(&serde_json::to_string(&state).unwrap()).unwrap();
    assert_eq!(restored.active_folder(), Some(menu::location()));
    let old: locations::Preferences = serde_json::from_str("{}").unwrap();
    assert!(old.show_applications);
    app.sidebar_options.show_applications = false;
    assert!(
        !app.sidebar_entries()
            .iter()
            .any(|entry| entry.id == sidebar::Location::Applications)
    );
}
