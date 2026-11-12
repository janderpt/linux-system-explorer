use super::*;
use crate::app::tests::complete_navigation;
mod closed_tabs;
mod recent_tabs;
mod tab_drag;
mod tab_menu;
pub(super) fn workspace(ctx: &egui::Context, path: PathBuf) -> Workspace {
    let mut tab = Explorer::with_context(ctx, path);
    complete_navigation(&mut tab);
    let shared = tab.stored_preferences(&tab.preferences());
    Workspace {
        tabs: vec![tab],
        active: 0,
        next_id: 1,
        shared,
        settings_path: None,
        window: None,
        closing: false,
        session: crate::session::Store::at(None),
        closed_tabs: Default::default(),
        split: None,
    }
}

#[test]
fn remote_tab_titles_show_share_or_subfolder_instead_of_mount_directory() {
    let root = tempfile::tempdir().unwrap();
    let mount = root.path().join("internal-gvfs-mount");
    let child = mount.join("Reports");
    std::fs::create_dir_all(&child).unwrap();
    let ctx = egui::Context::default();
    let mut app = workspace(&ctx, mount.clone());
    app.tabs[0]
        .sidebar_options
        .remote
        .push(crate::network::Connection {
            url: "smb://server/Shared%20folder".into(),
            label: "Custom sidebar label".into(),
            mount: Some(mount),
        });
    for (path, title) in [
        (app.tabs[0].current.clone(), "Shared folder"),
        (child, "Reports"),
    ] {
        app.tabs[0].navigate(path, Navigation::Visit);
        complete_navigation(&mut app.tabs[0]);
        let render = || egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                Vec2::new(1180.0, 760.0),
            )),
            ..Default::default()
        };
        let _ = ctx.run(render(), |ctx| app.tab_bar(ctx));
        let output = ctx.run(render(), |ctx| app.tab_bar(ctx));
        assert!(crate::app::tests::text_center(&output, title).is_some());
        assert!(crate::app::tests::text_center(&output, "internal-gvfs-mount").is_none());
    }
}
#[test]
fn tabs_keep_independent_navigation_selection_filter_layout_and_share_clipboard() {
    let root = tempfile::tempdir().unwrap();
    let a = root.path().join("a");
    let b = root.path().join("b");
    std::fs::create_dir(&a).unwrap();
    std::fs::create_dir(&b).unwrap();
    let file = a.join("test.txt");
    std::fs::write(&file, "data").unwrap();
    let ctx = egui::Context::default();
    let mut app = workspace(&ctx, a.clone());
    app.tabs[0].selected.insert(file.clone());
    app.tabs[0].query = "test".into();
    app.tabs[0].view = View::Icons;
    app.tabs[0].item_scale = 1.5;
    app.tabs[0].capture_files(file_ops::Mode::Copy, &ctx);
    app.share();
    app.add_tab(b.clone(), &ctx);
    complete_navigation(&mut app.tabs[1]);
    assert_eq!(app.active, 1);
    assert_eq!(app.tabs[1].current, b);
    assert_eq!(
        app.tabs[1].file_clipboard.as_ref().unwrap().paths,
        vec![file.clone()]
    );
    app.tabs[1].query = "second".into();
    app.tabs[1].view = View::List;
    app.activate(0, &ctx);
    assert_eq!(app.tabs[0].current, a);
    assert_eq!(app.tabs[0].query, "test");
    assert!(app.tabs[0].selected.contains(&file));
    assert!(app.tabs[0].view == View::Icons);
    assert_eq!(app.tabs[0].item_scale, 1.5);
    app.activate(1, &ctx);
    assert_eq!(app.tabs[1].query, "second");
    assert!(app.tabs[1].view == View::List);
    app.add_tab(a.clone(), &ctx);
    complete_navigation(&mut app.tabs[2]);
    assert_eq!(app.tabs.len(), 3);
    assert_eq!(app.tabs[2].current, a);
    assert!(app.tabs[2].selected.is_empty());
    assert_ne!(app.tabs[0].tab_id, app.tabs[2].tab_id);
    app.close_tab(2, &ctx);
    assert_eq!(app.tabs.len(), 2);
    assert_eq!(app.active, 1);
}
#[test]
fn external_reveal_activates_an_existing_inactive_tab_without_creating_one() {
    let root = tempfile::tempdir().unwrap();
    let a = root.path().join("a");
    let b = root.path().join("b");
    std::fs::create_dir(&a).unwrap();
    std::fs::create_dir(&b).unwrap();
    let file = a.join("download.pdf");
    std::fs::write(&file, "pdf").unwrap();
    let ctx = egui::Context::default();
    let mut app = workspace(&ctx, a.clone());
    app.add_tab(b.clone(), &ctx);
    complete_navigation(&mut app.tabs[1]);
    let (request, reply) =
        crate::file_manager::window::Request::for_test(crate::file_manager::OpenRequest {
            folder: a,
            selected: vec![file.clone()],
            properties: false,
        });
    app.external(request, &ctx);
    assert!(reply.recv().unwrap());
    assert_eq!(app.active, 0);
    complete_navigation(&mut app.tabs[0]);
    assert!(app.tabs[0].selected.contains(&file));
    assert_eq!(app.tabs.len(), 2);
    assert_eq!(app.tabs[1].current, b);
    app.close_tab(0, &ctx);
    assert_eq!(app.active, 0);
    assert_eq!(app.tabs.len(), 1);
}
#[test]
fn global_preferences_follow_tab_switches_without_resetting_each_tabs_layout() {
    let root = tempfile::tempdir().unwrap();
    let ctx = egui::Context::default();
    let mut app = workspace(&ctx, root.path().to_owned());
    app.add_tab(root.path().to_owned(), &ctx);
    complete_navigation(&mut app.tabs[1]);
    app.tabs[1].dark = true;
    app.tabs[1].restore_tabs = false;
    app.tabs[1].view = View::List;
    app.activate(0, &ctx);
    assert!(app.tabs[0].dark);
    assert!(!app.tabs[0].restore_tabs);
    assert!(app.tabs[0].view == View::Details);
    app.activate(1, &ctx);
    assert!(app.tabs[1].dark);
    assert!(!app.shared.restore_tabs);
    assert!(app.tabs[1].view == View::List);
}

#[test]
fn notification_stack_follows_active_tab_and_includes_background_completions() {
    let root = tempfile::tempdir().unwrap();
    let ctx = egui::Context::default();
    let mut app = workspace(&ctx, root.path().to_owned());
    app.tabs[0].post_notice("first".into());
    app.add_tab(root.path().to_owned(), &ctx);
    complete_navigation(&mut app.tabs[1]);
    assert_eq!(app.tabs[1].notice_toasts.len(), 1);
    assert!(app.tabs[0].notice_toasts.is_empty());
    app.tabs[0].post_notice("background completed".into());
    app.background();
    assert_eq!(app.tabs[1].notice_toasts.len(), 2);
    assert!(app.tabs[0].notice_toasts.is_empty());
    app.activate(0, &ctx);
    assert_eq!(app.tabs[0].notice_toasts.len(), 2);
}

#[test]
fn rendered_workspace_opens_tab_from_folder_menu_and_docks_terminal_below_file_grid() {
    use crate::app::tests::text_center;
    use egui::{Event, Modifiers, PointerButton, Pos2, RawInput, Rect};
    let root = tempfile::tempdir().unwrap();
    let child = root.path().join("child");
    std::fs::create_dir(&child).unwrap();
    std::fs::write(child.join("sample.txt"), "test").unwrap();
    let ctx = egui::Context::default();
    let mut app = workspace(&ctx, root.path().to_owned());
    fn render(app: &mut Workspace, ctx: &egui::Context, events: Vec<Event>) -> egui::FullOutput {
        ctx.run(
            RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1180.0, 760.0))),
                time: Some(ctx.input(|i| i.time) + 0.05),
                events,
                ..Default::default()
            },
            |ctx| app.show(ctx),
        )
    }
    fn click(app: &mut Workspace, ctx: &egui::Context, p: Pos2, button: PointerButton) {
        for pressed in [true, false] {
            render(
                app,
                ctx,
                vec![
                    Event::PointerMoved(p),
                    Event::PointerButton {
                        pos: p,
                        button,
                        pressed,
                        modifiers: Modifiers::NONE,
                    },
                ],
            );
        }
    }
    render(&mut app, &ctx, vec![]);
    let output = render(&mut app, &ctx, vec![]);
    let p = text_center(&output, "child").unwrap();
    click(&mut app, &ctx, p, PointerButton::Secondary);
    let output = render(&mut app, &ctx, vec![]);
    click(
        &mut app,
        &ctx,
        text_center(&output, "Abrir em nova aba").unwrap(),
        PointerButton::Primary,
    );
    assert_eq!(app.tabs.len(), 2);
    assert_eq!(app.active, 1);
    complete_navigation(&mut app.tabs[1]);
    assert_eq!(app.tabs[1].current, child);
    app.tabs[1].open_terminal(child.clone(), &ctx);
    render(&mut app, &ctx, vec![]);
    let output = render(&mut app, &ctx, vec![]);
    let file = text_center(&output, "sample.txt").unwrap();
    let terminal = text_center(&output, "Terminal").unwrap();
    assert!(
        file.y < terminal.y,
        "terminal belongs below the exploration area: {file:?}, {terminal:?}"
    );
    let output = render(
        &mut app,
        &ctx,
        vec![Event::Key {
            key: Key::Tab,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::CTRL,
        }],
    );
    drop(output);
    assert_eq!(app.active, 0);
    assert_eq!(app.tabs[0].current, root.path());
}

#[test]
fn cut_and_paste_between_tabs_refreshes_both_directories_and_clears_shared_cut() {
    let root = tempfile::tempdir().unwrap();
    let a = root.path().join("a");
    let b = root.path().join("b");
    std::fs::create_dir(&a).unwrap();
    std::fs::create_dir(&b).unwrap();
    let source = a.join("move.txt");
    std::fs::write(&source, "contents").unwrap();
    let ctx = egui::Context::default();
    let mut app = workspace(&ctx, a.clone());
    app.tabs[0].selected.insert(source.clone());
    app.tabs[0].capture_files(file_ops::Mode::Cut, &ctx);
    app.share();
    app.add_tab(b.clone(), &ctx);
    complete_navigation(&mut app.tabs[1]);
    app.tabs[1].paste_files(&ctx);
    app.activate(0, &ctx);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    Vec2::new(1180.0, 760.0),
                )),
                ..Default::default()
            },
            |ctx| app.show(ctx),
        );
        if !app.tabs[1].busy()
            && !app.tabs[0].loading
            && !app.tabs[1].loading
            && app.tabs[0].entries.is_empty()
            && app.tabs[1]
                .entries
                .iter()
                .any(|entry| entry.path == b.join("move.txt"))
        {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "tabs did not finish refreshing"
        );
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    assert!(!source.exists());
    assert_eq!(
        std::fs::read_to_string(b.join("move.txt")).unwrap(),
        "contents"
    );
    assert!(app.tabs.iter().all(|tab| tab.file_clipboard.is_none()));
}

#[test]
fn session_restores_order_duplicates_active_tab_and_tracks_close_and_navigation() {
    let root = tempfile::tempdir().unwrap();
    let a = root.path().join("a");
    let b = root.path().join("b");
    let nested = a.join("nested");
    std::fs::create_dir_all(&nested).unwrap();
    std::fs::create_dir(&b).unwrap();
    let file = root.path().join("state/session.json");
    let ctx = egui::Context::default();
    let mut first = workspace(&ctx, a.clone());
    first.session = crate::session::Store::at(Some(file.clone()));
    first.add_tab(b.clone(), &ctx);
    complete_navigation(&mut first.tabs[1]);
    first.add_tab(a.clone(), &ctx);
    complete_navigation(&mut first.tabs[2]);
    first.activate(1, &ctx);
    first.persist_session(false);
    let saved = crate::session::State::load(&file).unwrap().unwrap();
    assert_eq!(
        saved.paths().unwrap(),
        vec![a.clone(), b.clone(), a.clone()]
    );
    assert_eq!(saved.active, 1);
    drop(first);
    let ctx = egui::Context::default();
    let mut reopened = workspace(&ctx, a.clone());
    reopened.session = crate::session::Store::at(Some(file.clone()));
    reopened.restore_tabs(&saved, &ctx);
    for tab in &mut reopened.tabs {
        complete_navigation(tab);
    }
    assert_eq!(reopened.active, 1);
    assert_eq!(
        reopened
            .tabs
            .iter()
            .map(|t| t.current.clone())
            .collect::<Vec<_>>(),
        vec![a.clone(), b, a.clone()]
    );
    assert!(reopened.tabs.iter().all(|t| t.terminal.is_none()));
    reopened.close_tab(1, &ctx);
    reopened.persist_session(false);
    let saved = crate::session::State::load(&file).unwrap().unwrap();
    assert_eq!(saved.paths().unwrap(), vec![a.clone(), a.clone()]);
    assert_eq!(saved.active, 0);
    reopened.tabs[0].navigate(nested.clone(), Navigation::Visit);
    assert_eq!(reopened.session_state().paths().unwrap()[0], a);
    complete_navigation(&mut reopened.tabs[0]);
    reopened.persist_session(false);
    assert_eq!(
        crate::session::State::load(&file)
            .unwrap()
            .unwrap()
            .paths()
            .unwrap()[0],
        nested
    );
    reopened.tabs[0].navigate(root.path().join("missing"), Navigation::Visit);
    complete_navigation(&mut reopened.tabs[0]);
    assert_eq!(reopened.session_state().paths().unwrap()[0], nested);
}

#[test]
fn unavailable_restored_folders_remain_saved_and_can_be_retried() {
    let root = tempfile::tempdir().unwrap();
    let available = root.path().join("available");
    let unavailable = root.path().join("temporarily-unmounted");
    std::fs::create_dir(&available).unwrap();
    let mut saved =
        crate::session::State::from_paths(vec![available.clone(), unavailable.clone()], 1);
    saved.split = Some(crate::session::Split {
        left: 0,
        right: 1,
        ratio: 0.6,
    });
    let ctx = egui::Context::default();
    let mut app = workspace(&ctx, available);
    app.restore_tabs(&saved, &ctx);
    assert!(app.split.is_some());
    complete_navigation(&mut app.tabs[1]);
    assert_eq!(app.active, 1);
    assert_eq!(app.tabs[1].location(), unavailable);
    assert!(app.tabs[1].error.is_some());
    assert_eq!(app.session_state(), saved);
    std::fs::create_dir(&unavailable).unwrap();
    app.tabs[1].refresh();
    complete_navigation(&mut app.tabs[1]);
    assert_eq!(app.tabs[1].current, unavailable);
    assert!(app.tabs[1].error.is_none());
}

#[test]
fn restored_session_is_saved_from_real_workspace_rendering_after_tab_switch() {
    let root = tempfile::tempdir().unwrap();
    let a = root.path().join("a");
    let b = root.path().join("b");
    std::fs::create_dir(&a).unwrap();
    std::fs::create_dir(&b).unwrap();
    let file = root.path().join("session.json");
    let ctx = egui::Context::default();
    let mut app = workspace(&ctx, a.clone());
    app.session = crate::session::Store::at(Some(file.clone()));
    app.add_tab(b.clone(), &ctx);
    complete_navigation(&mut app.tabs[1]);
    let _ = ctx.run(egui::RawInput::default(), |ctx| app.show(ctx));
    assert_eq!(
        crate::session::State::load(&file).unwrap().unwrap().active,
        1
    );
    let _ = ctx.run(
        egui::RawInput {
            events: vec![egui::Event::Key {
                key: Key::Tab,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::CTRL,
            }],
            ..Default::default()
        },
        |ctx| app.show(ctx),
    );
    let saved = crate::session::State::load(&file).unwrap().unwrap();
    assert_eq!(saved.active, 0);
    assert_eq!(saved.paths().unwrap(), vec![a, b]);
}

#[test]
fn tabs_share_one_local_share_sidebar_and_observe_renames_and_removals() {
    let root = tempfile::tempdir().unwrap();
    let ctx = egui::Context::default();
    let mut app = workspace(&ctx, root.path().into());
    let mut share = crate::local_shares::Share::new(root.path().into());
    share.name = "published".into();
    app.tabs[0].publish_local_share_sidebar(vec![share.clone()]);
    app.add_tab(root.path().into(), &ctx);
    assert!(std::sync::Arc::ptr_eq(
        &app.tabs[0].local_share_sidebar,
        &app.tabs[1].local_share_sidebar
    ));
    assert_eq!(app.tabs[1].local_share_entries(), vec![share.clone()]);
    share.name = "renamed".into();
    app.tabs[1].publish_local_share_sidebar(vec![share.clone()]);
    assert_eq!(app.tabs[0].local_share_entries(), vec![share]);
    app.tabs[0].publish_local_share_sidebar(vec![]);
    assert!(app.tabs[1].local_share_entries().is_empty());
}
