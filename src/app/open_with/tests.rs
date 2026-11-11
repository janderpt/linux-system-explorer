use super::*;
use crate::app::tests::text_center;
use egui::{Event as InputEvent, FullOutput, Modifiers, PointerButton, Pos2, RawInput, Rect};
use std::{
    fs,
    time::{Duration, Instant},
};

fn render(app: &mut Explorer, ctx: &egui::Context, events: Vec<InputEvent>) -> FullOutput {
    ctx.run(
        RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1000.0, 760.0))),
            time: Some(ctx.input(|input| input.time) + 0.2),
            events,
            ..Default::default()
        },
        |ctx| {
            app.top_toolbar(ctx);
            let visible = filesystem::visible_entries(
                &app.entries,
                &app.query,
                app.show_hidden,
                app.sort,
                app.descending,
            );
            app.contents(ctx, &visible);
            app.open_with_dialog(ctx);
        },
    )
}

fn click(app: &mut Explorer, ctx: &egui::Context, point: Pos2, button: PointerButton) {
    for pressed in [true, false] {
        render(
            app,
            ctx,
            vec![
                InputEvent::PointerMoved(point),
                InputEvent::PointerButton {
                    pos: point,
                    button,
                    pressed,
                    modifiers: Modifiers::NONE,
                },
            ],
        );
    }
}

fn fixture_chooser(path: PathBuf, applications: Vec<Application>) -> Chooser {
    let (sender, results) = mpsc::channel();
    sender
        .send(Ok(Applications {
            mime: Some("text/plain".into()),
            items: applications,
        }))
        .unwrap();
    Chooser {
        path,
        results,
        applications: vec![],
        mime: None,
        loading: true,
        error: None,
        search: String::new(),
        show_other: false,
        selected: None,
        focus_search: true,
        always_use: false,
    }
}

#[test]
fn context_chooser_filters_cancels_and_opens_only_the_chosen_file() {
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join("note.txt");
    fs::write(&file, "sample").unwrap();
    let ctx = egui::Context::default();
    let mut app = Explorer::with_context(&ctx, root.path().to_owned());
    let deadline = Instant::now() + Duration::from_secs(3);
    while app.loading {
        app.poll();
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(1));
    }
    render(&mut app, &ctx, vec![]);
    let output = render(&mut app, &ctx, vec![]);
    let point = text_center(&output, "note.txt").unwrap();
    click(&mut app, &ctx, point, PointerButton::Secondary);
    let output = render(&mut app, &ctx, vec![]);
    let point = text_center(&output, "Abrir com…").expect("file context menu must expose chooser");
    click(&mut app, &ctx, point, PointerButton::Primary);
    assert!(app.open_with.is_some());

    let capture = root.path().join("opened");
    let desktop = root.path().join("alternate.desktop");
    let script = root.path().join("capture.sh");
    fs::write(
        &script,
        format!("#!/bin/sh\nprintf '%s' \"$1\" > '{}'\n", capture.display()),
    )
    .unwrap();
    fs::write(
        &desktop,
        format!(
            "[Desktop Entry]\nType=Application\nName=Alternate Editor\nExec=/bin/sh \"{}\" %f\n",
            script.display()
        ),
    )
    .unwrap();
    let alternate = Application {
        id: "alternate.desktop".into(),
        path: desktop,
        name: "Alternate Editor".into(),
        description: "An alternate application".into(),
        recommended: true,
        is_default: false,
    };
    let mut other = alternate.clone();
    other.name = "Other Viewer".into();
    other.recommended = false;
    app.open_with = Some(fixture_chooser(
        file.clone(),
        vec![alternate.clone(), other],
    ));
    render(&mut app, &ctx, vec![]);
    let output = render(&mut app, &ctx, vec![]);
    assert!(text_center(&output, "Alternate Editor").is_some());
    assert!(text_center(&output, "Other Viewer").is_none());
    let show_other = text_center(&output, "Mostrar outros aplicativos").unwrap();
    click(&mut app, &ctx, show_other, PointerButton::Primary);
    let output = render(&mut app, &ctx, vec![]);
    assert!(text_center(&output, "Other Viewer").is_some());
    app.open_with.as_mut().unwrap().search = "absent".into();
    let output = render(&mut app, &ctx, vec![]);
    assert!(text_center(&output, "Nenhum aplicativo encontrado.").is_some());
    assert!(!capture.exists());
    let cancel = text_center(&output, "Cancelar").unwrap();
    click(&mut app, &ctx, cancel, PointerButton::Primary);
    assert!(app.open_with.is_none());
    assert!(!capture.exists());

    app.open_with = Some(fixture_chooser(file.clone(), vec![alternate]));
    render(&mut app, &ctx, vec![]);
    let output = render(&mut app, &ctx, vec![]);
    let editor = text_center(&output, "Alternate Editor").unwrap();
    click(&mut app, &ctx, editor, PointerButton::Primary);
    assert!(app.open_with.as_ref().unwrap().selected.is_some());
    let output = render(&mut app, &ctx, vec![]);
    let open = text_center(&output, "Abrir").unwrap();
    click(&mut app, &ctx, open, PointerButton::Primary);
    assert!(app.open_with.is_none());
    let deadline = Instant::now() + Duration::from_secs(3);
    while !capture.exists() {
        app.poll();
        assert!(app.error.is_none(), "{:?}", app.error);
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(fs::read_to_string(capture).unwrap(), file.to_str().unwrap());
    assert_eq!(app.current, root.path());
}

#[test]
fn always_use_saves_only_after_confirmation_and_the_system_reuses_the_association() {
    let Some(root) = std::env::var_os("EXPLORER_ASSOCIATION_TEST_ROOT") else {
        let root = tempfile::tempdir().unwrap();
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "app::open_with::tests::always_use_saves_only_after_confirmation_and_the_system_reuses_the_association", "--nocapture"])
            .env("EXPLORER_ASSOCIATION_TEST_ROOT", root.path())
            .env("HOME", root.path())
            .env("XDG_DATA_HOME", root.path().join("data"))
            .env("XDG_DATA_DIRS", root.path().join("system-data"))
            .env("XDG_CONFIG_HOME", root.path().join("config"))
            .env("XDG_CONFIG_DIRS", root.path().join("system-config"))
            .env("XDG_CACHE_HOME", root.path().join("cache"))
            .env("XDG_CURRENT_DESKTOP", "")
            .env("DESKTOP_SESSION", "")
            .env("XDG_SESSION_DESKTOP", "")
            .env_remove("KDE_FULL_SESSION")
            .env_remove("KDE_SESSION_VERSION")
            .output().unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    };
    // All writes by GIO, including mimeapps.list, are confined to this child.
    let root = PathBuf::from(root);
    let source = root.join("source");
    let registry = root.join("data/applications");
    for directory in [
        &source,
        &registry,
        &root.join("config"),
        &root.join("system-data"),
        &root.join("system-config"),
        &root.join("cache"),
    ] {
        fs::create_dir_all(directory).unwrap();
    }
    // Keep the installed MIME database readable, while all desktop entries and
    // association writes remain confined to the temporary registry/config.
    std::os::unix::fs::symlink("/usr/share/mime", root.join("system-data/mime")).unwrap();
    let file = source.join("note.txt");
    fs::write(&file, "plain text\n").unwrap();
    let capture = root.join("opened");
    let script = root.join("capture.sh");
    fs::write(
        &script,
        format!(
            "#!/bin/sh\nprintf '%s\\0' \"$1\" >> '{}'\n",
            capture.display()
        ),
    )
    .unwrap();
    let desktop = registry.join("alternate.desktop");
    fs::write(&desktop, format!("[Desktop Entry]\nType=Application\nName=Alternate Editor\nExec=/bin/sh \"{}\" %f\nMimeType=text/plain;\n", script.display())).unwrap();
    fs::write(registry.join("previous.desktop"), "[Desktop Entry]\nType=Application\nName=Previous Editor\nExec=/bin/true %f\nMimeType=text/plain;image/png;\n").unwrap();
    let config = root.join("config/mimeapps.list");
    let initial =
        "[Default Applications]\ntext/plain=previous.desktop\nimage/png=previous.desktop\n";
    fs::write(&config, initial).unwrap();
    let alternate = Application {
        id: "alternate.desktop".into(),
        path: desktop,
        name: "Alternate Editor".into(),
        description: String::new(),
        recommended: true,
        is_default: false,
    };
    let ctx = egui::Context::default();
    let mut app = Explorer::with_context(&ctx, source);
    crate::app::tests::complete_navigation(&mut app);
    let label = "Usar sempre este aplicativo para este tipo de arquivo";
    let begin = |app: &mut Explorer| {
        app.open_with = Some(fixture_chooser(file.clone(), vec![alternate.clone()]));
        render(app, &ctx, vec![]);
        render(app, &ctx, vec![])
    };
    let count = || {
        fs::read(&capture)
            .unwrap_or_default()
            .iter()
            .filter(|byte| **byte == 0)
            .count()
    };
    let wait = |app: &mut Explorer, expected| {
        let deadline = Instant::now() + Duration::from_secs(3);
        while count() != expected {
            app.poll();
            assert!(app.error.is_none(), "{:?}", app.error);
            assert!(Instant::now() < deadline, "opening did not finish");
            thread::sleep(Duration::from_millis(5));
        }
    };
    // Unchecked: launch once and preserve both existing associations.
    let output = begin(&mut app);
    assert!(!app.open_with.as_ref().unwrap().always_use);
    click(
        &mut app,
        &ctx,
        text_center(&output, "Alternate Editor").unwrap(),
        PointerButton::Primary,
    );
    let output = render(&mut app, &ctx, vec![]);
    click(
        &mut app,
        &ctx,
        text_center(&output, "Abrir").unwrap(),
        PointerButton::Primary,
    );
    wait(&mut app, 1);
    assert_eq!(fs::read_to_string(&config).unwrap(), initial);

    // Checking does not write anything; cancelling still preserves the settings.
    let output = begin(&mut app);
    click(
        &mut app,
        &ctx,
        text_center(&output, label).unwrap(),
        PointerButton::Primary,
    );
    assert!(app.open_with.as_ref().unwrap().always_use);
    assert_eq!(fs::read_to_string(&config).unwrap(), initial);
    let output = render(&mut app, &ctx, vec![]);
    assert!(
        text_center(
            &output,
            "A associação padrão do sistema será alterada para este tipo de arquivo."
        )
        .is_some()
    );
    click(
        &mut app,
        &ctx,
        text_center(&output, "Cancelar").unwrap(),
        PointerButton::Primary,
    );
    assert!(app.open_with.is_none());
    assert_eq!(count(), 1);
    assert_eq!(fs::read_to_string(&config).unwrap(), initial);

    // Confirm through the actual UI, then query the persisted default in a new process.
    let output = begin(&mut app);
    click(
        &mut app,
        &ctx,
        text_center(&output, label).unwrap(),
        PointerButton::Primary,
    );
    let output = render(&mut app, &ctx, vec![]);
    click(
        &mut app,
        &ctx,
        text_center(&output, "Alternate Editor").unwrap(),
        PointerButton::Primary,
    );
    let output = render(&mut app, &ctx, vec![]);
    click(
        &mut app,
        &ctx,
        text_center(&output, "Abrir").unwrap(),
        PointerButton::Primary,
    );
    wait(&mut app, 2);
    let default = |mime| {
        let output = std::process::Command::new("xdg-mime")
            .args(["query", "default", mime])
            .output()
            .unwrap();
        assert!(output.status.success());
        String::from_utf8(output.stdout).unwrap().trim().to_owned()
    };
    assert_eq!(default("text/plain"), "alternate.desktop");
    assert_eq!(default("image/png"), "previous.desktop");
    let discovered = applications::discover(&file).unwrap();
    assert!(
        discovered
            .items
            .iter()
            .any(|app| app.id == "alternate.desktop" && app.is_default),
        "mime={:?}, apps={:?}",
        discovered.mime,
        discovered.items
    );
    applications::open_default(&file).unwrap();
    wait(&mut app, 3);

    // A rejected association reports the failure and does not launch the file.
    let before = fs::read(&config).unwrap();
    let mut missing = alternate;
    missing.id = "missing-application.desktop".into();
    app.open_with_application(missing, file, Some("text/plain".into()), &ctx);
    let deadline = Instant::now() + Duration::from_secs(3);
    while app.error.is_none() {
        app.poll();
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(5));
    }
    assert!(
        app.error
            .as_ref()
            .unwrap()
            .contains("Não foi possível definir")
    );
    assert_eq!(count(), 3);
    assert_eq!(fs::read(config).unwrap(), before);
}

#[test]
fn unknown_mime_disables_saving_the_association() {
    let root = tempfile::tempdir().unwrap();
    let ctx = egui::Context::default();
    let mut app = Explorer::with_context(&ctx, root.path().to_owned());
    let mut chooser = fixture_chooser(root.path().join("unknown"), vec![]);
    chooser.poll();
    chooser.mime = None;
    app.open_with = Some(chooser);
    render(&mut app, &ctx, vec![]);
    let output = render(&mut app, &ctx, vec![]);
    let point = text_center(
        &output,
        "Usar sempre este aplicativo para este tipo de arquivo",
    )
    .unwrap();
    click(&mut app, &ctx, point, PointerButton::Primary);
    assert!(!app.open_with.as_ref().unwrap().always_use);
    for mime in [
        "",
        "--help",
        "text/plain\nimage/png",
        "text/",
        "/plain",
        "text/*",
    ] {
        assert!(!applications::valid_mime(mime), "{mime}");
    }
}
