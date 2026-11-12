use super::*;
use std::os::unix::{
    ffi::{OsStrExt, OsStringExt},
    fs::PermissionsExt,
};

fn render(app: &mut Explorer, ctx: &egui::Context, events: Vec<Event>) -> FullOutput {
    ctx.run(
        RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1180.0, 760.0))),
            time: Some(ctx.input(|input| input.time) + 0.03),
            events,
            ..Default::default()
        },
        |ctx| {
            app.shortcuts(ctx);
            app.top_toolbar(ctx);
            let visible = filesystem::visible_entries(
                &app.entries,
                &app.query,
                app.show_hidden,
                app.sort,
                app.descending,
            );
            app.contents(ctx, &visible);
        },
    )
}

fn pointer(app: &mut Explorer, ctx: &egui::Context, point: Pos2, button: PointerButton) {
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

fn captures(path: &Path) -> Vec<Vec<u8>> {
    std::fs::read(path)
        .unwrap_or_default()
        .split(|byte| *byte == 0)
        .filter(|part| !part.is_empty())
        .map(<[u8]>::to_vec)
        .collect()
}

#[test]
fn double_click_context_and_enter_use_the_system_launcher() {
    let Some(root) = std::env::var_os("EXPLORER_OPENER_TEST_ROOT") else {
        for case in ["success", "failure", "missing", "fallback"] {
            let root = tempfile::tempdir().unwrap();
            let output = Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "app::tests::opening::double_click_context_and_enter_use_the_system_launcher",
                    "--nocapture",
                ])
                .env("EXPLORER_OPENER_TEST_ROOT", root.path())
                .env("EXPLORER_OPENER_CASE", case)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{case}: {}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
        return;
    };
    let root = PathBuf::from(root);
    let case = std::env::var("EXPLORER_OPENER_CASE").unwrap();
    let bin = root.join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    let capture = root.join("arguments");
    if case != "missing" {
        let launcher = bin.join(if case == "fallback" {
            "xdg-open"
        } else {
            "gio"
        });
        std::fs::write(&launcher, "#!/bin/sh\nif [ \"$1\" = open ]; then shift; fi\nprintf '%s\\0' \"$@\" >> \"$EXPLORER_OPEN_CAPTURE\"\nprintf 'fixture launcher diagnostics\\n' >&2\nexit \"$EXPLORER_OPEN_EXIT\"\n").unwrap();
        std::fs::set_permissions(launcher, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    // A second child isolates PATH and launcher settings from concurrent Rust tests.
    if std::env::var_os("EXPLORER_OPENER_ISOLATED").is_none() {
        let output = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "app::tests::opening::double_click_context_and_enter_use_the_system_launcher",
                "--nocapture",
            ])
            .env("EXPLORER_OPENER_ISOLATED", "1")
            .env("PATH", &bin)
            .env("EXPLORER_OPEN_CAPTURE", &capture)
            .env(
                "EXPLORER_OPEN_EXIT",
                if case == "failure" { "7" } else { "0" },
            )
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }
    let source = root.join("source");
    std::fs::create_dir(&source).unwrap();
    let plain = source.join("note.txt");
    std::fs::write(&plain, "note").unwrap();
    let quoted = source.join("space ' $literal; [example].pdf");
    std::fs::write(&quoted, "pdf").unwrap();
    let raw = source.join(std::ffi::OsString::from_vec(b"raw-\xff.txt".to_vec()));
    std::fs::write(&raw, "raw").unwrap();
    let folder = source.join("folder");
    std::fs::create_dir(&folder).unwrap();
    let ctx = egui::Context::default();
    let mut app = Explorer::with_context(&ctx, source.clone());
    complete_navigation(&mut app);
    if case == "failure" || case == "missing" {
        app.selected = BTreeSet::from([plain.clone()]);
        app.selection_cursor = Some(plain.clone());
        render(
            &mut app,
            &ctx,
            vec![Event::Key {
                key: Key::Enter,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            }],
        );
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        while app.error.is_none() && std::time::Instant::now() < deadline {
            app.poll();
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        let error = app
            .error
            .as_ref()
            .expect("falha do abridor deve ser informada");
        assert!(
            error.contains(if case == "missing" { "xdg-open" } else { "gio" }),
            "{error}"
        );
        if case == "failure" {
            assert!(error.contains("fixture launcher diagnostics"));
            assert_eq!(
                captures(&capture),
                vec![plain.as_os_str().as_bytes().to_vec()]
            );
        } else {
            assert!(!capture.exists());
        }
        assert_eq!(app.current, source);
        assert!(plain.exists());
        return;
    }
    render(&mut app, &ctx, vec![]);
    let output = render(&mut app, &ctx, vec![]);
    let point = text_center(&output, "note.txt").unwrap();
    pointer(&mut app, &ctx, point, PointerButton::Primary);
    pointer(&mut app, &ctx, point, PointerButton::Primary);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    while captures(&capture).is_empty() && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert_eq!(
        captures(&capture),
        vec![plain.as_os_str().as_bytes().to_vec()]
    );
    for path in [&quoted, &raw] {
        let output = render(&mut app, &ctx, vec![]);
        let label = path.file_name().unwrap().to_string_lossy();
        let point = text_center(&output, &label).unwrap();
        pointer(&mut app, &ctx, point, PointerButton::Secondary);
        let output = render(&mut app, &ctx, vec![]);
        let open = text_center(&output, "Abrir").unwrap();
        pointer(&mut app, &ctx, open, PointerButton::Primary);
    }
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    while captures(&capture).len() < 3 && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    let captured = captures(&capture);
    assert_eq!(captured.len(), 3);
    for path in [&plain, &quoted, &raw] {
        assert!(captured.contains(&path.as_os_str().as_bytes().to_vec()));
    }
    let html = source.join("web page.html");
    let docx = source.join("document.docx");
    std::fs::write(&html, "<p>fixture</p>").unwrap();
    std::fs::write(&docx, "fixture").unwrap();
    app.refresh();
    complete_navigation(&mut app);
    let enter_paths = [&plain, &quoted, &raw, &html, &docx];
    for path in enter_paths {
        if let Some(id) = ctx.memory(|memory| memory.focused()) {
            ctx.memory_mut(|memory| memory.surrender_focus(id));
        }
        app.selected = BTreeSet::from([path.clone()]);
        app.selection_cursor = Some(path.clone());
        let before = captures(&capture).len();
        render(
            &mut app,
            &ctx,
            vec![Event::Key {
                key: Key::Enter,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            }],
        );
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        while captures(&capture).len() == before && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        let captured = captures(&capture);
        assert_eq!(captured.len(), before + 1);
        assert_eq!(
            captured.last().unwrap(),
            &path.as_os_str().as_bytes().to_vec()
        );
        assert_eq!(app.current, source);
        render(
            &mut app,
            &ctx,
            vec![Event::Key {
                key: Key::Enter,
                physical_key: None,
                pressed: false,
                repeat: false,
                modifiers: Modifiers::NONE,
            }],
        );
    }
    // Separate this double-click from the preceding popup gesture and its click count.
    for _ in 0..20 {
        render(&mut app, &ctx, vec![]);
    }
    let output = render(&mut app, &ctx, vec![]);
    let point = text_center(&output, "folder").unwrap();
    // Use a fresh click location after the popup has gone away.
    pointer(&mut app, &ctx, point, PointerButton::Primary);
    pointer(&mut app, &ctx, point, PointerButton::Primary);
    complete_navigation(&mut app);
    app.poll();
    assert_eq!(app.current, folder);
    assert_eq!(captures(&capture).len(), 3 + enter_paths.len());
    assert!(app.error.is_none());
}

#[test]
fn folder_context_menu_offers_tabs_windows_and_terminal_but_files_do_not() {
    let root = tempfile::tempdir().unwrap();
    let folder = root.path().join("folder");
    std::fs::create_dir(&folder).unwrap();
    std::fs::write(root.path().join("note.txt"), "note").unwrap();
    let ctx = egui::Context::default();
    let mut app = Explorer::with_context(&ctx, root.path().to_owned());
    complete_navigation(&mut app);
    render(&mut app, &ctx, vec![]);
    let output = render(&mut app, &ctx, vec![]);
    let point = text_center(&output, "folder").unwrap();
    pointer(&mut app, &ctx, point, PointerButton::Secondary);
    let output = render(&mut app, &ctx, vec![]);
    for label in [
        "Abrir em nova aba",
        "Abrir em nova janela",
        "Abrir terminal aqui",
    ] {
        assert!(text_center(&output, label).is_some(), "{label}");
    }
    let point = text_center(&output, "Abrir em nova aba").unwrap();
    pointer(&mut app, &ctx, point, PointerButton::Primary);
    assert_eq!(app.requested_tabs, vec![folder]);
    assert_eq!(app.current, root.path());
    for _ in 0..10 {
        render(&mut app, &ctx, vec![]);
    }
    let output = render(&mut app, &ctx, vec![]);
    let point = text_center(&output, "note.txt").unwrap();
    pointer(&mut app, &ctx, point, PointerButton::Secondary);
    let output = render(&mut app, &ctx, vec![]);
    for label in [
        "Abrir em nova aba",
        "Abrir em nova janela",
        "Abrir terminal aqui",
    ] {
        assert!(
            text_center(&output, label).is_none(),
            "file menu should not contain {label}"
        );
    }
}
