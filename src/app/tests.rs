use super::*;
use egui::{Event, FullOutput, Modifiers, PointerButton, Pos2, RawInput, Rect};
use std::process::Command;

fn explorer(ctx: &egui::Context) -> Explorer {
    let mut explorer = Explorer::with_context(ctx, PathBuf::from("/"));
    explorer.current = PathBuf::from("/uma/pasta/com/um/caminho/longo/para/conferir/o/layout");
    explorer.loading = false;
    explorer
}

pub(super) fn frame(
    app: &mut Explorer,
    ctx: &egui::Context,
    width: f32,
    events: Vec<Event>,
    mut draw: impl FnMut(&mut Explorer, &mut egui::Ui) -> Rect,
) -> (FullOutput, Rect) {
    let mut rect = Rect::NOTHING;
    let modifiers = events
        .iter()
        .rev()
        .find_map(|event| match event {
            Event::PointerButton { modifiers, .. } | Event::Key { modifiers, .. } => {
                Some(*modifiers)
            }
            _ => None,
        })
        .unwrap_or_default();
    let time = ctx.input(|i| i.time) + 0.2;
    let output = ctx.run(
        RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(width, 760.0))),
            events,
            modifiers,
            time: Some(time),
            ..Default::default()
        },
        |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| rect = draw(app, ui));
        },
    );
    (output, rect)
}

pub(super) fn click(
    app: &mut Explorer,
    ctx: &egui::Context,
    point: Pos2,
    draw: impl FnMut(&mut Explorer, &mut egui::Ui) -> Rect,
) -> FullOutput {
    modified_click(
        app,
        ctx,
        point,
        Modifiers::NONE,
        PointerButton::Primary,
        draw,
    )
}

fn modified_click(
    app: &mut Explorer,
    ctx: &egui::Context,
    point: Pos2,
    modifiers: Modifiers,
    button: PointerButton,
    mut draw: impl FnMut(&mut Explorer, &mut egui::Ui) -> Rect,
) -> FullOutput {
    for pressed in [true, false] {
        frame(
            app,
            ctx,
            1180.0,
            vec![
                Event::PointerMoved(point),
                Event::PointerButton {
                    pos: point,
                    button,
                    pressed,
                    modifiers,
                },
            ],
            &mut draw,
        );
    }
    frame(app, ctx, 1180.0, vec![], draw).0
}

pub(super) fn text_center(output: &FullOutput, label: &str) -> Option<Pos2> {
    output.shapes.iter().find_map(|shape| {
        if let egui::Shape::Text(text) = &shape.shape
            && text.galley.text() == label
        {
            Some(text.pos + text.galley.size() * 0.5)
        } else {
            None
        }
    })
}

#[test]
fn filter_stays_at_the_right_edge_with_vertically_centered_placeholder() {
    for dark in [false, true] {
        let ctx = egui::Context::default();
        let mut app = explorer(&ctx);
        ctx.set_visuals(if dark {
            egui::Visuals::dark()
        } else {
            egui::Visuals::light()
        });
        for width in [540.0, 940.0, 1260.0] {
            let draw = |app: &mut Explorer, ui: &mut egui::Ui| app.filter_field(ui).rect;
            frame(&mut app, &ctx, width, vec![], draw);
            let (output, field) = frame(&mut app, &ctx, width, vec![], draw);
            assert!((field.right() - (width - 8.0)).abs() <= 1.0);
            let placeholder = text_center(&output, "Pesquisar nesta pasta · Ctrl+F").unwrap();
            assert!((placeholder.y - field.center().y).abs() <= 1.0);
            app.editing_address = true;
            let (_, editing) = frame(&mut app, &ctx, width, vec![], draw);
            assert_eq!(field, editing);
            app.editing_address = false;
        }
    }
}

#[test]
fn settings_apply_column_changes_and_keep_unsaved_edits_separate() {
    let ctx = egui::Context::default();
    let mut app = explorer(&ctx);
    app.sort = SortBy::Kind;
    let draw = |app: &mut Explorer, ui: &mut egui::Ui| {
        let button = app.column_settings(ui).rect;
        app.settings_dialog(ui.ctx());
        button
    };
    frame(&mut app, &ctx, 1180.0, vec![], draw);
    let (_, button) = frame(&mut app, &ctx, 1180.0, vec![], draw);
    let output = click(&mut app, &ctx, button.center(), draw);
    let columns = text_center(&output, "Colunas").unwrap();
    let output = click(&mut app, &ctx, columns, draw);
    let checkbox = text_center(&output, "Tipo").expect("configurações de colunas abertas");
    let output = click(&mut app, &ctx, checkbox, draw);
    assert!(
        app.visible_columns.contains(&Column::Kind),
        "edição ainda não aplicada"
    );
    let apply = text_center(&output, "Aplicar").unwrap();
    let output = click(&mut app, &ctx, apply, draw);
    assert!(!app.visible_columns.contains(&Column::Kind));
    assert!(app.sort == SortBy::Name);
    let checkbox = text_center(&output, "Tipo").unwrap();
    let output = click(&mut app, &ctx, checkbox, draw);
    let cancel = text_center(&output, "Cancelar").unwrap();
    click(&mut app, &ctx, cancel, draw);
    assert!(
        !app.visible_columns.contains(&Column::Kind),
        "cancelar descarta a edição"
    );
    assert!(app.settings_editor.is_none());
}

#[test]
fn fontawesome_picker_applies_a_different_icon_and_color_mode() {
    let ctx = egui::Context::default();
    let mut app = explorer(&ctx);
    app.settings_editor = Some(settings::Editor::new(app.preferences()));
    let draw = |app: &mut Explorer, ui: &mut egui::Ui| {
        app.settings_dialog(ui.ctx());
        ui.min_rect()
    };
    frame(&mut app, &ctx, 1180.0, vec![], draw);
    let (output, _) = frame(&mut app, &ctx, 1180.0, vec![], draw);
    let tab = text_center(&output, "Aparência e ícones").unwrap();
    let output = click(&mut app, &ctx, tab, draw);
    let color_label = text_center(&output, "Cores dos ícones:").unwrap();
    let color_option = text_center(&output, "Coloridos").unwrap();
    assert!(
        (color_label.y - color_option.y).abs() <= 1.0,
        "rótulo e opções de cor centralizados na linha"
    );
    let target_label = text_center(&output, "Personalizar:").unwrap();
    let reset_button = text_center(&output, "Ícone padrão").unwrap();
    assert!(
        (target_label.y - reset_button.y).abs() <= 1.0,
        "rótulo e botão centralizados na linha"
    );
    let preview_label = text_center(&output, "fa-solid fa-house").unwrap();
    let preview_glyph = text_center(
        &output,
        &icons::glyph("house").unwrap().character().to_string(),
    )
    .unwrap();
    assert!(
        (preview_label.y - preview_glyph.y).abs() <= 1.0,
        "prévia e nome do ícone centralizados na linha"
    );
    let colored = text_center(&output, "Coloridos").unwrap();
    let output = click(&mut app, &ctx, colored, draw);
    let choice = text_center(&output, "2").expect("catálogo Font Awesome disponível");
    let output = click(&mut app, &ctx, choice, draw);
    let save = text_center(&output, "Salvar e fechar").unwrap();
    click(&mut app, &ctx, save, draw);
    assert_eq!(app.appearance.name(Icon::Home), "2");
    assert!(app.appearance.mode == icons::IconMode::Colored);
    assert!(app.settings_editor.is_none());
}

#[test]
fn clicking_column_headers_sorts_and_reverses_direction() {
    let ctx = egui::Context::default();
    let mut app = explorer(&ctx);
    let details = |app: &mut Explorer, ui: &mut egui::Ui| {
        app.details(ui, &[]);
        ui.min_rect()
    };
    let (output, _) = frame(&mut app, &ctx, 1180.0, vec![], details);
    let name = text_center(&output, "Nome").unwrap();
    click(&mut app, &ctx, name, details);
    assert!(app.descending);
    let output = click(&mut app, &ctx, name, details);
    assert!(!app.descending);
    let size = text_center(&output, "Tamanho").unwrap();
    click(&mut app, &ctx, size, details);
    assert!(app.sort == SortBy::Size);
    assert!(!app.descending);
    click(&mut app, &ctx, size, details);
    assert!(app.descending);
}

#[test]
fn abbreviated_breadcrumb_keeps_current_folder_and_hidden_ancestors_navigable() {
    let ctx = egui::Context::default();
    let mut app = explorer(&ctx);
    app.current = PathBuf::from(
        "/home/jander/Downloads/PEDIDO DE MEDIDA DE PROTECAO - POC 094-2026 1.odt/media",
    );
    let draw = |app: &mut Explorer, ui: &mut egui::Ui| {
        ui.allocate_ui(Vec2::new(300.0, 32.0), |ui| {
            if let Some(path) = breadcrumbs::show(ui, &app.current) {
                app.current = path;
            }
            ui.min_rect()
        })
        .inner
    };
    frame(&mut app, &ctx, 1180.0, vec![], draw);
    let (output, rect) = frame(&mut app, &ctx, 1180.0, vec![], draw);
    assert!(
        rect.width() <= 300.0,
        "breadcrumb excedeu a largura disponível: {rect:?}"
    );
    assert!(text_center(&output, "media").is_some());
    assert!(text_center(&output, "Downloads").is_none());
    let collapsed = text_center(&output, "…").expect("menu de ancestrais abreviado");
    let output = click(&mut app, &ctx, collapsed, draw);
    let downloads = text_center(&output, "Downloads").expect("ancestral disponível no menu");
    click(&mut app, &ctx, downloads, draw);
    assert_eq!(app.current, Path::new("/home/jander/Downloads"));

    app.current = PathBuf::from(format!(
        "/home/jander/{}",
        "Pasta com acentuação ".repeat(30)
    ));
    let original = app.current.clone();
    let (_, rect) = frame(&mut app, &ctx, 1180.0, vec![], draw);
    assert!(rect.width() <= 300.0, "nome longo deve ser abreviado");
    assert_eq!(
        app.current, original,
        "abreviação não altera o caminho real"
    );
}

fn pointer_button(point: Pos2, pressed: bool) -> Event {
    Event::PointerButton {
        pos: point,
        button: PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    }
}

fn drag(
    app: &mut Explorer,
    ctx: &egui::Context,
    start: Pos2,
    end: Pos2,
    mut draw: impl FnMut(&mut Explorer, &mut egui::Ui) -> Rect,
) {
    frame(
        app,
        ctx,
        1180.0,
        vec![Event::PointerMoved(start), pointer_button(start, true)],
        &mut draw,
    );
    frame(
        app,
        ctx,
        1180.0,
        vec![Event::PointerMoved(start.lerp(end, 0.5))],
        &mut draw,
    );
    frame(app, ctx, 1180.0, vec![Event::PointerMoved(end)], &mut draw);
    frame(
        app,
        ctx,
        1180.0,
        vec![pointer_button(end, false)],
        &mut draw,
    );
    frame(app, ctx, 1180.0, vec![], draw);
}

#[test]
fn header_resize_and_drag_reorder_are_saved_without_sorting() {
    let ctx = egui::Context::default();
    let mut app = explorer(&ctx);
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("preferences.json");
    app.settings_path = Some(path.clone());
    app.column_widths = BTreeMap::from([
        (Column::Name, 200.0),
        (Column::Modified, 180.0),
        (Column::Kind, 145.0),
        (Column::Size, 110.0),
    ]);
    let draw = |app: &mut Explorer, ui: &mut egui::Ui| {
        let columns = column_layout(
            ui.available_width(),
            ui.spacing().item_spacing.x,
            &app.visible_columns,
            &app.column_widths,
        );
        let rect = app.column_headers(ui, &columns);
        app.persist_preferences(ui.ctx());
        rect
    };
    frame(&mut app, &ctx, 1180.0, vec![], draw);
    let (_, rect) = frame(&mut app, &ctx, 1180.0, vec![], draw);
    let edge = egui::pos2(rect.left() + 200.0, rect.center().y);
    let before = std::fs::read(&path).unwrap();
    frame(
        &mut app,
        &ctx,
        1180.0,
        vec![Event::PointerMoved(edge), pointer_button(edge, true)],
        draw,
    );
    let wider = edge + Vec2::new(64.0, 0.0);
    frame(
        &mut app,
        &ctx,
        1180.0,
        vec![Event::PointerMoved(wider)],
        draw,
    );
    assert_eq!(
        std::fs::read(&path).unwrap(),
        before,
        "não gravar durante o arraste"
    );
    frame(
        &mut app,
        &ctx,
        1180.0,
        vec![pointer_button(wider, false)],
        draw,
    );
    assert_eq!(app.column_widths[&Column::Name], 264.0);
    let restored = Preferences::load(&path).unwrap();
    assert_eq!(
        restored.layout_for(&app.current).column_widths[&Column::Name],
        264.0
    );
    assert!(!app.descending, "redimensionar não deve ordenar");

    let (_, rect) = frame(&mut app, &ctx, 1180.0, vec![], draw);
    let source = rect.left_center() + Vec2::new(70.0, 0.0);
    let target = rect.right_center() - Vec2::new(15.0, 0.0);
    drag(&mut app, &ctx, source, target, draw);
    assert_eq!(
        app.visible_columns,
        vec![Column::Modified, Column::Kind, Column::Size, Column::Name]
    );
    assert!(app.sort == Column::Name && !app.descending);
    assert_eq!(
        Preferences::load(&path)
            .unwrap()
            .layout_for(&app.current)
            .columns,
        app.visible_columns
    );

    let (_, rect) = frame(&mut app, &ctx, 1180.0, vec![], draw);
    drag(
        &mut app,
        &ctx,
        rect.right_center() - Vec2::new(100.0, 0.0),
        rect.left_center() + Vec2::new(15.0, 0.0),
        draw,
    );
    assert_eq!(app.visible_columns, Column::DEFAULT);
}

#[test]
fn list_mode_can_be_selected_saved_and_displays_selectable_names_without_headers() {
    let ctx = egui::Context::default();
    let mut app = explorer(&ctx);
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("preferences.json");
    app.settings_path = Some(path.clone());
    app.settings_editor = Some(settings::Editor::new(app.preferences()));
    let settings = |app: &mut Explorer, ui: &mut egui::Ui| {
        app.settings_dialog(ui.ctx());
        ui.min_rect()
    };
    frame(&mut app, &ctx, 1180.0, vec![], settings);
    let (output, _) = frame(&mut app, &ctx, 1180.0, vec![], settings);
    let general = text_center(&output, "Geral").unwrap();
    click(&mut app, &ctx, general, settings);
    // Folder view controls follow the per-type notification settings in this scrollable tab.
    frame(
        &mut app,
        &ctx,
        1180.0,
        vec![
            Event::PointerMoved(egui::pos2(600.0, 300.0)),
            Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: Vec2::new(0.0, -1000.0),
                modifiers: Modifiers::NONE,
            },
        ],
        settings,
    );
    let (output, _) = frame(&mut app, &ctx, 1180.0, vec![], settings);
    let list = text_center(&output, "Lista").unwrap();
    let output = click(&mut app, &ctx, list, settings);
    let save = text_center(&output, "Salvar e fechar").unwrap();
    click(&mut app, &ctx, save, settings);
    assert!(
        app.view == View::List
            && Preferences::load(&path)
                .unwrap()
                .layout_for(&app.current)
                .view
                == View::List
    );

    std::fs::write(directory.path().join("sample.txt"), "data").unwrap();
    app.entries = filesystem::read_directory(directory.path())
        .unwrap()
        .entries;
    let visible = filesystem::visible_entries(&app.entries, "sample", false, SortBy::Name, false);
    let listing = |app: &mut Explorer, ui: &mut egui::Ui| {
        app.list(ui, &visible);
        ui.min_rect()
    };
    frame(&mut app, &ctx, 1180.0, vec![], listing);
    let (output, _) = frame(&mut app, &ctx, 1180.0, vec![], listing);
    assert!(text_center(&output, "Data de modificação").is_none());
    let name = text_center(&output, "sample.txt").unwrap();
    click(&mut app, &ctx, name, listing);
    assert!(app.selected.contains(&directory.path().join("sample.txt")));
    for details in [true, false] {
        app.clear_selection();
        let draw = |app: &mut Explorer, ui: &mut egui::Ui| {
            if details {
                app.details(ui, &visible);
            } else {
                app.icons(ui, &visible);
            }
            ui.min_rect()
        };
        frame(&mut app, &ctx, 1180.0, vec![], draw);
        let (output, _) = frame(&mut app, &ctx, 1180.0, vec![], draw);
        let name = text_center(&output, "sample.txt").unwrap();
        click(&mut app, &ctx, name, draw);
        assert!(
            !app.selected.is_empty(),
            "clicar no nome deve selecionar em todos os modos"
        );
    }
}

pub(super) fn complete_navigation(app: &mut Explorer) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    while app.loading && std::time::Instant::now() < deadline {
        app.poll();
        if app.loading {
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    }
    assert!(!app.loading, "leitura da pasta não terminou");
}

#[test]
fn folders_restore_independent_layouts_through_history_refresh_errors_and_restart() {
    let directory = tempfile::tempdir().unwrap();
    let folder_a = directory.path().join("a");
    let folder_b = directory.path().join("b");
    std::fs::create_dir(&folder_a).unwrap();
    std::fs::create_dir(&folder_b).unwrap();
    let path = directory.path().join("preferences.json");
    let ctx = egui::Context::default();
    let legacy = Preferences {
        view: View::Icons,
        item_scale: 1.25,
        columns: vec![Column::Name, Column::Size],
        column_widths: BTreeMap::from([(Column::Name, 210.0)]),
        ..Default::default()
    };
    let mut app = Explorer::with_context(&ctx, folder_a.clone());
    app.apply_preferences(legacy.clone(), &ctx);
    app.settings_path = Some(path.clone());
    complete_navigation(&mut app);
    assert!(app.view == View::Icons);
    assert_eq!(app.item_scale, 1.25);
    assert_eq!(app.column_widths[&Column::Name], 210.0);

    app.view = View::List;
    app.visible_columns = vec![Column::Size, Column::Name];
    app.column_widths.insert(Column::Name, 320.0);
    app.zoom_items(0.5);
    assert_eq!(app.item_scale, 1.75);
    // Navigation must also remember changes that have not reached the periodic save yet.
    let layout_a = app.current_layout();
    app.navigate(folder_b.clone(), Navigation::Visit);
    complete_navigation(&mut app);
    assert!(
        app.view == View::Icons,
        "pasta nova usa o padrão anterior, não o layout da pasta A"
    );
    assert_eq!(
        app.item_scale, 1.25,
        "a pasta B deve usar seu próprio padrão"
    );
    assert_eq!(app.visible_columns, legacy.columns);
    assert_eq!(app.column_widths[&Column::Name], 210.0);
    app.view = View::Details;
    app.visible_columns = vec![Column::Name, Column::Owner];
    app.column_widths.insert(Column::Name, 440.0);
    app.dark = true;
    app.settings_editor = Some(settings::Editor::new(app.preferences()));
    app.settings_editor.as_mut().unwrap().draft.item_scale = 0.75;
    let draw = |app: &mut Explorer, ui: &mut egui::Ui| {
        app.settings_dialog(ui.ctx());
        ui.min_rect()
    };
    frame(&mut app, &ctx, 1180.0, vec![], draw);
    click_label(
        &mut app,
        &ctx,
        "Salvar e fechar",
        Modifiers::NONE,
        PointerButton::Primary,
        draw,
    );
    assert_eq!(app.item_scale, 0.75);
    assert_eq!(app.default_layout.item_scale, 1.25);
    let layout_b = app.current_layout();
    app.persist_preferences(&ctx);

    app.navigate(folder_a.clone(), Navigation::Back);
    complete_navigation(&mut app);
    assert!(app.current_layout() == layout_a);
    assert!(app.dark, "aparência continua global");
    app.refresh();
    complete_navigation(&mut app);
    assert!(app.current_layout() == layout_a);
    app.navigate(directory.path().join("missing"), Navigation::Visit);
    complete_navigation(&mut app);
    assert_eq!(app.current, folder_a);
    assert!(
        app.current_layout() == layout_a,
        "falha de navegação preserva o layout atual"
    );
    app.navigate(folder_b.clone(), Navigation::Forward);
    complete_navigation(&mut app);
    assert!(app.current_layout() == layout_b);

    let restored = Preferences::load(&path).unwrap();
    assert!(
        restored.layout() == legacy.layout(),
        "padrão geral não foi sobrescrito"
    );
    assert!(restored.layout_for(&folder_a) == layout_a);
    assert!(restored.layout_for(&folder_b) == layout_b);
    let other_context = egui::Context::default();
    let mut reopened = Explorer::with_context(&other_context, folder_a.clone());
    reopened.apply_preferences(restored, &other_context);
    complete_navigation(&mut reopened);
    assert!(reopened.current_layout() == layout_a);
    reopened.navigate(folder_b, Navigation::Visit);
    complete_navigation(&mut reopened);
    assert!(reopened.current_layout() == layout_b);
}

pub(super) fn click_label(
    app: &mut Explorer,
    ctx: &egui::Context,
    label: &str,
    modifiers: Modifiers,
    button: PointerButton,
    mut draw: impl FnMut(&mut Explorer, &mut egui::Ui) -> Rect,
) {
    let (output, _) = frame(app, ctx, 1180.0, vec![], &mut draw);
    let point = text_center(&output, label).unwrap_or_else(|| panic!("item não exibido: {label}"));
    modified_click(app, ctx, point, modifiers, button, draw);
}

#[test]
fn ctrl_and_shift_selection_follow_visible_order_in_all_three_views() {
    let directory = tempfile::tempdir().unwrap();
    for name in ["a.txt", "b.txt", "c.txt", "d.txt", "e.txt"] {
        std::fs::write(directory.path().join(name), name).unwrap();
    }
    let expected = |names: &[&str]| {
        names
            .iter()
            .map(|name| directory.path().join(name))
            .collect::<BTreeSet<_>>()
    };
    for view in [View::Details, View::List, View::Icons] {
        let ctx = egui::Context::default();
        let mut app = explorer(&ctx);
        app.current = directory.path().to_path_buf();
        app.entries = filesystem::read_directory(directory.path())
            .unwrap()
            .entries;
        app.view = view;
        let draw = |app: &mut Explorer, ui: &mut egui::Ui| {
            let visible = filesystem::visible_entries(
                &app.entries,
                &app.query,
                app.show_hidden,
                app.sort,
                app.descending,
            );
            let paths = visible
                .iter()
                .map(|&i| app.entries[i].path.clone())
                .collect();
            app.retain_selection(&paths);
            match app.view {
                View::Details => app.details(ui, &visible),
                View::List => app.list(ui, &visible),
                View::Icons => app.icons(ui, &visible),
            }
            ui.min_rect()
        };
        frame(&mut app, &ctx, 1180.0, vec![], draw);
        click_label(
            &mut app,
            &ctx,
            "a.txt",
            Modifiers::NONE,
            PointerButton::Primary,
            draw,
        );
        assert_eq!(app.selected, expected(&["a.txt"]));
        click_label(
            &mut app,
            &ctx,
            "c.txt",
            Modifiers::CTRL,
            PointerButton::Primary,
            draw,
        );
        click_label(
            &mut app,
            &ctx,
            "e.txt",
            Modifiers::CTRL,
            PointerButton::Primary,
            draw,
        );
        assert_eq!(app.selected, expected(&["a.txt", "c.txt", "e.txt"]));
        click_label(
            &mut app,
            &ctx,
            "c.txt",
            Modifiers::CTRL,
            PointerButton::Primary,
            draw,
        );
        assert_eq!(app.selected, expected(&["a.txt", "e.txt"]));
        click_label(
            &mut app,
            &ctx,
            "b.txt",
            Modifiers::NONE,
            PointerButton::Primary,
            draw,
        );
        click_label(
            &mut app,
            &ctx,
            "d.txt",
            Modifiers::SHIFT,
            PointerButton::Primary,
            draw,
        );
        assert_eq!(app.selected, expected(&["b.txt", "c.txt", "d.txt"]));
        click_label(
            &mut app,
            &ctx,
            "a.txt",
            Modifiers::SHIFT,
            PointerButton::Primary,
            draw,
        );
        assert_eq!(
            app.selected,
            expected(&["a.txt", "b.txt"]),
            "Shift preserva a âncora original"
        );
        let control_shift = Modifiers {
            ctrl: true,
            shift: true,
            ..Modifiers::NONE
        };
        click_label(
            &mut app,
            &ctx,
            "e.txt",
            control_shift,
            PointerButton::Primary,
            draw,
        );
        assert_eq!(app.selected.len(), 5);
        click_label(
            &mut app,
            &ctx,
            "c.txt",
            Modifiers::NONE,
            PointerButton::Secondary,
            draw,
        );
        assert_eq!(
            app.selected.len(),
            5,
            "clique direito em item selecionado preserva o grupo"
        );
        assert_eq!(app.selected_paths_text().lines().count(), 5);
        frame(
            &mut app,
            &ctx,
            1180.0,
            vec![Event::Key {
                key: Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            }],
            draw,
        );
        frame(&mut app, &ctx, 1180.0, vec![], draw);
        app.descending = true;
        click_label(
            &mut app,
            &ctx,
            "e.txt",
            Modifiers::NONE,
            PointerButton::Primary,
            draw,
        );
        click_label(
            &mut app,
            &ctx,
            "c.txt",
            Modifiers::SHIFT,
            PointerButton::Primary,
            draw,
        );
        assert_eq!(app.selected, expected(&["c.txt", "d.txt", "e.txt"]));
        let copied: Vec<_> = app
            .selected_paths_text()
            .lines()
            .map(str::to_owned)
            .collect();
        assert_eq!(
            copied,
            ["e.txt", "d.txt", "c.txt"].map(|name| directory
                .path()
                .join(name)
                .display()
                .to_string())
        );
        app.query = "d.txt".into();
        frame(&mut app, &ctx, 1180.0, vec![], draw);
        assert_eq!(app.selected, expected(&["d.txt"]));
        assert!(app.selection_anchor.is_none());
        click_label(
            &mut app,
            &ctx,
            "d.txt",
            Modifiers::SHIFT,
            PointerButton::Primary,
            draw,
        );
        assert_eq!(app.selected, expected(&["d.txt"]));
        app.query = "missing".into();
        frame(&mut app, &ctx, 1180.0, vec![], draw);
        assert!(app.selected.is_empty() && app.selection_anchor.is_none());
    }
}

#[test]
fn reset_folder_button_removes_only_current_override_and_survives_restart() {
    let directory = tempfile::tempdir().unwrap();
    let folder_a = directory.path().join("a");
    let folder_b = directory.path().join("b");
    std::fs::create_dir(&folder_a).unwrap();
    std::fs::create_dir(&folder_b).unwrap();
    let path = directory.path().join("preferences.json");
    let ctx = egui::Context::default();
    let mut app = Explorer::with_context(&ctx, folder_a.clone());
    let mut preferences = Preferences {
        view: View::Icons,
        item_scale: 1.25,
        columns: vec![Column::Name, Column::Size],
        dark: true,
        ..Default::default()
    };
    preferences.column_widths.insert(Column::Name, 230.0);
    preferences.appearance.mode = icons::IconMode::Colored;
    let custom = FolderLayout {
        view: View::List,
        item_scale: 2.0,
        columns: vec![Column::Size, Column::Name],
        column_widths: BTreeMap::from([(Column::Name, 480.0)]),
        ..Default::default()
    };
    preferences
        .folders
        .insert(settings::folder_key(&folder_a), custom.clone());
    preferences
        .folders
        .insert(settings::folder_key(&folder_b), custom.clone());
    app.apply_preferences(preferences.clone(), &ctx);
    app.settings_path = Some(path.clone());
    complete_navigation(&mut app);
    assert!(app.view == View::List);
    assert_eq!(app.item_scale, 2.0);
    app.settings_editor = Some(settings::Editor::new(app.preferences()));
    let draw = |app: &mut Explorer, ui: &mut egui::Ui| {
        app.settings_dialog(ui.ctx());
        ui.min_rect()
    };
    frame(&mut app, &ctx, 1180.0, vec![], draw);
    click_label(
        &mut app,
        &ctx,
        "Colunas",
        Modifiers::NONE,
        PointerButton::Primary,
        draw,
    );
    click_label(
        &mut app,
        &ctx,
        "Restaurar padrão desta pasta",
        Modifiers::NONE,
        PointerButton::Primary,
        draw,
    );
    assert!(app.current_layout() == preferences.layout());
    assert!(app.dark && app.appearance == preferences.appearance);
    app.persist_preferences(&ctx);
    let stored = Preferences::load(&path).unwrap();
    assert!(
        !stored
            .folders
            .contains_key(&settings::folder_key(&folder_a)),
        "remoção não deve ser recriada ao salvar"
    );
    assert!(stored.layout_for(&folder_b) == custom);
    assert!(stored.layout() == preferences.layout());
    assert!(stored.appearance == preferences.appearance && stored.dark);
    assert!(app.settings_editor.as_ref().unwrap().draft.layout() == preferences.layout());
    let other_ctx = egui::Context::default();
    let mut reopened = Explorer::with_context(&other_ctx, folder_a);
    reopened.apply_preferences(stored, &other_ctx);
    complete_navigation(&mut reopened);
    assert!(reopened.current_layout() == preferences.layout());
    reopened.navigate(folder_b, Navigation::Visit);
    complete_navigation(&mut reopened);
    assert!(reopened.current_layout() == custom);
}

#[test]
fn failed_folder_reset_preserves_current_and_other_folder_layouts() {
    let ctx = egui::Context::default();
    let mut app = explorer(&ctx);
    app.view = View::List;
    app.remember_current_folder();
    app.folder_layouts
        .insert("/other".into(), FolderLayout::default());
    let layout = app.current_layout();
    let profiles = app.folder_layouts.clone();
    let directory = tempfile::tempdir().unwrap();
    // A directory cannot be replaced by the JSON preferences file.
    app.settings_path = Some(directory.path().to_path_buf());
    assert!(app.reset_current_folder().is_err());
    assert!(app.current_layout() == layout);
    assert!(app.folder_layouts == profiles);
}

#[test]
fn refresh_retains_remaining_selected_items_and_navigation_clears_the_anchor() {
    let directory = tempfile::tempdir().unwrap();
    let folder_a = directory.path().join("a");
    let folder_b = directory.path().join("b");
    std::fs::create_dir(&folder_a).unwrap();
    std::fs::create_dir(&folder_b).unwrap();
    let first = folder_a.join("first");
    let second = folder_a.join("second");
    std::fs::write(&first, "1").unwrap();
    std::fs::write(&second, "2").unwrap();
    let ctx = egui::Context::default();
    let mut app = Explorer::with_context(&ctx, folder_a.clone());
    complete_navigation(&mut app);
    app.selected.extend([first.clone(), second.clone()]);
    app.selection_anchor = Some(first.clone());
    std::fs::remove_file(&first).unwrap();
    app.refresh();
    complete_navigation(&mut app);
    assert_eq!(app.selected, BTreeSet::from([second.clone()]));
    assert!(app.selection_anchor.is_none());
    app.navigate(directory.path().join("missing"), Navigation::Visit);
    complete_navigation(&mut app);
    assert_eq!(app.selected, BTreeSet::from([second]));
    app.navigate(folder_b, Navigation::Visit);
    complete_navigation(&mut app);
    assert!(app.selected.is_empty() && app.selection_anchor.is_none());
}

fn complete_transfer(app: &mut Explorer) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while app.active_transfer.is_some() && std::time::Instant::now() < deadline {
        app.poll();
        assert!(
            app.transfer_conflict.is_none(),
            "conflito inesperado no teste"
        );
        if app.active_transfer.is_some() {
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    }
    assert!(app.active_transfer.is_none(), "transferência não terminou");
    app.process_transfer_refresh();
    complete_navigation(app);
}

fn transfer_key(app: &mut Explorer, ctx: &egui::Context, event: Event) {
    frame(app, ctx, 1180.0, vec![event], |app, ui| {
        app.shortcuts(ui.ctx());
        ui.min_rect()
    });
}

#[test]
fn native_clipboard_events_copy_repeatedly_then_cut_multiple_files_and_folders() {
    let root = tempfile::tempdir().unwrap();
    let folders: [PathBuf; 4] = ["a", "b", "c", "d"].map(|name| root.path().join(name));
    for folder in &folders {
        std::fs::create_dir(folder).unwrap();
    }
    std::fs::create_dir(folders[0].join("tree")).unwrap();
    std::fs::write(folders[0].join("tree/nested"), "nested").unwrap();
    std::fs::write(folders[0].join("plain"), "plain").unwrap();
    let ctx = egui::Context::default();
    let mut app = Explorer::with_context(&ctx, folders[0].clone());
    complete_navigation(&mut app);
    app.selected = app.entries.iter().map(|e| e.path.clone()).collect();
    transfer_key(&mut app, &ctx, Event::Copy);
    let marker = app.file_clipboard_text.clone();
    assert_eq!(app.file_clipboard.as_ref().unwrap().paths.len(), 2);
    for folder in &folders[1..3] {
        app.navigate(folder.clone(), Navigation::Visit);
        complete_navigation(&mut app);
        transfer_key(&mut app, &ctx, Event::Paste(marker.clone()));
        complete_transfer(&mut app);
        assert_eq!(
            std::fs::read_to_string(folder.join("tree/nested")).unwrap(),
            "nested"
        );
        assert_eq!(
            std::fs::read_to_string(folder.join("plain")).unwrap(),
            "plain"
        );
        assert!(app.file_clipboard.is_some(), "cópia continua disponível");
        assert!(
            app.entries.iter().any(|e| e.name == "plain"),
            "listagem atualizada"
        );
    }
    app.navigate(folders[1].clone(), Navigation::Visit);
    complete_navigation(&mut app);
    app.selected = app.entries.iter().map(|e| e.path.clone()).collect();
    transfer_key(&mut app, &ctx, Event::Cut);
    let marker = app.file_clipboard_text.clone();
    assert!(
        folders[1].join("plain").exists(),
        "recortar só move ao colar"
    );
    app.navigate(folders[3].clone(), Navigation::Visit);
    complete_navigation(&mut app);
    transfer_key(&mut app, &ctx, Event::Paste(marker));
    complete_transfer(&mut app);
    assert_eq!(std::fs::read_dir(&folders[1]).unwrap().count(), 0);
    assert!(folders[0].join("plain").exists() && folders[2].join("plain").exists());
    assert!(folders[3].join("tree/nested").exists());
    assert!(app.file_clipboard.is_none(), "recorte concluído é limpo");
}

fn folder_frame(app: &mut Explorer, ctx: &egui::Context, events: Vec<Event>) -> FullOutput {
    let visible = filesystem::visible_entries(
        &app.entries,
        &app.query,
        app.show_hidden,
        app.sort,
        app.descending,
    );
    let time = ctx.input(|i| i.time) + 0.2;
    ctx.run(
        RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1180.0, 760.0))),
            events,
            time: Some(time),
            ..Default::default()
        },
        |ctx| {
            app.top_toolbar(ctx);
            app.contents(ctx, &visible);
        },
    )
}

fn folder_click(
    app: &mut Explorer,
    ctx: &egui::Context,
    point: Pos2,
    button: PointerButton,
) -> FullOutput {
    for pressed in [true, false] {
        folder_frame(
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
    folder_frame(app, ctx, vec![])
}

#[test]
fn context_menus_copy_paste_in_empty_space_and_paste_into_a_clicked_folder() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source");
    let destination = root.path().join("destination");
    std::fs::create_dir(&source).unwrap();
    std::fs::create_dir(&destination).unwrap();
    std::fs::write(source.join("sample"), "sample").unwrap();
    let ctx = egui::Context::default();
    let mut app = Explorer::with_context(&ctx, source);
    complete_navigation(&mut app);
    folder_frame(&mut app, &ctx, vec![]);
    let output = folder_frame(&mut app, &ctx, vec![]);
    let row = text_center(&output, "sample").unwrap();
    let output = folder_click(&mut app, &ctx, row, PointerButton::Secondary);
    assert!(text_center(&output, "Recortar").is_some());
    let copy = text_center(&output, "Copiar").unwrap();
    folder_click(&mut app, &ctx, copy, PointerButton::Primary);
    assert!(app.file_clipboard.is_some());
    app.navigate(destination.clone(), Navigation::Visit);
    complete_navigation(&mut app);
    folder_frame(&mut app, &ctx, vec![]);
    let output = folder_click(
        &mut app,
        &ctx,
        egui::pos2(700.0, 500.0),
        PointerButton::Secondary,
    );
    let paste = text_center(&output, "Colar").expect("menu da área vazia oferece Colar");
    folder_click(&mut app, &ctx, paste, PointerButton::Primary);
    complete_transfer(&mut app);
    assert_eq!(
        std::fs::read_to_string(destination.join("sample")).unwrap(),
        "sample"
    );

    std::fs::create_dir(destination.join("nested")).unwrap();
    app.refresh();
    complete_navigation(&mut app);
    folder_frame(&mut app, &ctx, vec![]);
    let output = folder_frame(&mut app, &ctx, vec![]);
    let row = text_center(&output, "nested").unwrap();
    let output = folder_click(&mut app, &ctx, row, PointerButton::Secondary);
    let paste = text_center(&output, "Colar").unwrap();
    folder_click(&mut app, &ctx, paste, PointerButton::Primary);
    complete_transfer(&mut app);
    assert_eq!(
        std::fs::read_to_string(destination.join("nested/sample")).unwrap(),
        "sample"
    );
}

#[test]
fn conflict_dialog_keeps_both_and_partial_cut_keeps_failed_items_for_retry() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source");
    let destination = root.path().join("destination");
    let retry = root.path().join("retry");
    for path in [&source, &destination, &retry] {
        std::fs::create_dir(path).unwrap();
    }
    std::fs::write(source.join("same"), "new").unwrap();
    std::fs::write(destination.join("same"), "old").unwrap();
    let ctx = egui::Context::default();
    let mut app = Explorer::with_context(&ctx, source.clone());
    complete_navigation(&mut app);
    app.selected.insert(source.join("same"));
    transfer_key(&mut app, &ctx, Event::Copy);
    app.navigate(destination.clone(), Navigation::Visit);
    complete_navigation(&mut app);
    app.paste_files(&ctx);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    while app.transfer_conflict.is_none() && std::time::Instant::now() < deadline {
        app.poll();
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert!(app.transfer_conflict.is_some());
    let dialog = |app: &mut Explorer, ui: &mut egui::Ui| {
        app.transfer_dialog(ui.ctx());
        ui.min_rect()
    };
    frame(&mut app, &ctx, 1180.0, vec![], dialog);
    click_label(
        &mut app,
        &ctx,
        "Manter ambos",
        Modifiers::NONE,
        PointerButton::Primary,
        dialog,
    );
    complete_transfer(&mut app);
    assert_eq!(
        std::fs::read_to_string(destination.join("same")).unwrap(),
        "old"
    );
    assert_eq!(
        std::fs::read_to_string(destination.join("same (cópia)")).unwrap(),
        "new"
    );

    std::fs::write(source.join("first"), "first").unwrap();
    std::fs::write(source.join("second"), "second").unwrap();
    app.navigate(source.clone(), Navigation::Visit);
    complete_navigation(&mut app);
    app.selected = BTreeSet::from([source.join("first"), source.join("second")]);
    transfer_key(&mut app, &ctx, Event::Cut);
    let marker = app.file_clipboard_text.clone();
    std::fs::remove_file(source.join("second")).unwrap();
    app.navigate(destination, Navigation::Visit);
    complete_navigation(&mut app);
    transfer_key(&mut app, &ctx, Event::Paste(marker.clone()));
    complete_transfer(&mut app);
    assert!(
        app.error.as_ref().is_some_and(|e| e.contains("second")),
        "erro persiste após atualizar a pasta"
    );
    assert_eq!(
        app.file_clipboard.as_ref().unwrap().paths,
        vec![source.join("second")]
    );
    std::fs::write(source.join("second"), "retry").unwrap();
    app.navigate(retry.clone(), Navigation::Visit);
    complete_navigation(&mut app);
    transfer_key(&mut app, &ctx, Event::Paste(marker));
    complete_transfer(&mut app);
    assert_eq!(
        std::fs::read_to_string(retry.join("second")).unwrap(),
        "retry"
    );
    assert!(app.file_clipboard.is_none());
}

#[test]
fn clipboard_shortcuts_in_text_fields_keep_normal_text_editing() {
    let ctx = egui::Context::default();
    let mut app = explorer(&ctx);
    app.file_clipboard = Some(file_ops::Clipboard {
        id: 1,
        mode: file_ops::Mode::Cut,
        paths: vec![PathBuf::from("/source")],
    });
    let edit = |app: &mut Explorer, ui: &mut egui::Ui| {
        app.shortcuts(ui.ctx());
        let field = ui.text_edit_singleline(&mut app.query);
        field.request_focus();
        field.rect
    };
    frame(&mut app, &ctx, 1180.0, vec![], edit);
    frame(&mut app, &ctx, 1180.0, vec![Event::Copy], edit);
    assert!(
        app.file_clipboard.is_none(),
        "copiar texto invalida o buffer de arquivos anterior"
    );
    frame(
        &mut app,
        &ctx,
        1180.0,
        vec![Event::Paste("normal text".into())],
        edit,
    );
    assert_eq!(app.query, "normal text");
    assert!(app.active_transfer.is_none());
}

#[test]
fn single_toolbar_centers_controls_without_repeating_the_application_heading() {
    for size in [16.0, 18.0, 28.0] {
        let ctx = egui::Context::default();
        let mut app = explorer(&ctx);
        app.appearance.size = size;
        let draw = |app: &mut Explorer, ui: &mut egui::Ui| {
            let row = app.folder_navigation(ui).rect;
            app.folder_path(ui);
            row
        };
        frame(&mut app, &ctx, 1180.0, vec![], draw);
        let (output, row) = frame(&mut app, &ctx, 1180.0, vec![], draw);
        assert!(text_center(&output, "Explorador de Arquivos").is_none());
        let filter = text_center(&output, "Pesquisar nesta pasta · Ctrl+F").unwrap();
        for name in ["arrow-left", "gear", "moon"] {
            let icon = text_center(
                &output,
                &icons::glyph(name).unwrap().character().to_string(),
            )
            .unwrap();
            assert!(
                (row.center().y - icon.y).abs() <= 1.0,
                "{name} no tamanho {size}"
            );
            assert!(filter.y > icon.y, "filtro deve estar na linha do caminho");
        }
    }
}

#[test]
fn sidebar_highlights_only_the_closest_visible_ancestor_including_pins_and_networks() {
    use locations::{Item, Shortcut};
    use sidebar::Location;
    let ctx = egui::Context::default();
    let mut app = explorer(&ctx);
    app.places = vec![
        (Item::Root, "/".into()),
        (Item::Home, "/home/test".into()),
        (Item::Downloads, "/home/test/Downloads".into()),
        (Item::Mounts, "/mnt".into()),
    ];
    app.current = "/home/test/Downloads/extracted/media".into();
    assert_eq!(
        sidebar::closest(&app.current, &app.sidebar_entries()),
        Some(Location::Standard(Item::Downloads))
    );
    app.sidebar_options
        .pinned
        .push(Shortcut::new("/home/test/Downloads/extracted".into()));
    assert_eq!(
        sidebar::closest(&app.current, &app.sidebar_entries()),
        Some(Location::Pinned(0))
    );
    app.sidebar_options.show_pinned = false;
    assert_eq!(
        sidebar::closest(&app.current, &app.sidebar_entries()),
        Some(Location::Standard(Item::Downloads))
    );
    app.sidebar_options.visible.remove(&Item::Downloads);
    assert_eq!(
        sidebar::closest(&app.current, &app.sidebar_entries()),
        Some(Location::Standard(Item::Home))
    );
    app.sidebar_options.visible.insert(Item::Downloads);
    app.current = "/home/test/Downloads-archive/file".into();
    assert_eq!(
        sidebar::closest(&app.current, &app.sidebar_entries()),
        Some(Location::Standard(Item::Home)),
        "prefixo textual não é ancestral"
    );
    app.network_mounts = vec![Shortcut::new("/mnt/server".into())];
    app.current = "/mnt/server/share/deep".into();
    assert_eq!(
        sidebar::closest(&app.current, &app.sidebar_entries()),
        Some(Location::Network(0))
    );
    app.sidebar_options.show_network = false;
    assert_eq!(
        sidebar::closest(&app.current, &app.sidebar_entries()),
        Some(Location::Standard(Item::Mounts))
    );
}

#[test]
fn sidebar_settings_can_hide_a_standard_location_and_persist_it() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("preferences.json");
    let ctx = egui::Context::default();
    let mut app = explorer(&ctx);
    app.settings_path = Some(path.clone());
    app.settings_editor = Some(settings::Editor::new(app.preferences()));
    let draw = |app: &mut Explorer, ui: &mut egui::Ui| {
        app.settings_dialog(ui.ctx());
        ui.min_rect()
    };
    frame(&mut app, &ctx, 1180.0, vec![], draw);
    click_label(
        &mut app,
        &ctx,
        "Barra lateral",
        Modifiers::NONE,
        PointerButton::Primary,
        draw,
    );
    click_label(
        &mut app,
        &ctx,
        "Sistema de arquivos",
        Modifiers::NONE,
        PointerButton::Primary,
        draw,
    );
    assert!(
        app.sidebar_options.visible.contains(&locations::Item::Root),
        "edição ainda não aplicada"
    );
    click_label(
        &mut app,
        &ctx,
        "Salvar e fechar",
        Modifiers::NONE,
        PointerButton::Primary,
        draw,
    );
    assert!(!app.sidebar_options.visible.contains(&locations::Item::Root));
    assert!(
        !app.sidebar_entries()
            .iter()
            .any(|entry| entry.id == sidebar::Location::Standard(locations::Item::Root))
    );
    assert!(
        !Preferences::load(&path)
            .unwrap()
            .sidebar
            .visible
            .contains(&locations::Item::Root)
    );
}

#[test]
fn folder_context_menu_pins_and_unpins_and_saved_pins_survive_restart() {
    let directory = tempfile::tempdir().unwrap();
    let child = directory.path().join("child");
    std::fs::create_dir(&child).unwrap();
    let path = directory.path().join("preferences.json");
    let ctx = egui::Context::default();
    let mut app = Explorer::with_context(&ctx, directory.path().to_path_buf());
    app.settings_path = Some(path.clone());
    complete_navigation(&mut app);
    folder_frame(&mut app, &ctx, vec![]);
    let output = folder_frame(&mut app, &ctx, vec![]);
    let point = text_center(&output, "child").unwrap();
    let output = folder_click(&mut app, &ctx, point, PointerButton::Secondary);
    let pin = text_center(&output, "Fixar na barra lateral")
        .expect("pasta oferece Fixar no menu de contexto");
    folder_click(&mut app, &ctx, pin, PointerButton::Primary);
    assert_eq!(app.sidebar_options.pinned.len(), 1);
    assert_eq!(app.sidebar_options.pinned[0].path, child);
    app.persist_preferences(&ctx);
    assert_eq!(
        Preferences::load(&path).unwrap().sidebar.pinned[0].path,
        child
    );
    let output = folder_frame(&mut app, &ctx, vec![]);
    let point = text_center(&output, "child").unwrap();
    let output = folder_click(&mut app, &ctx, point, PointerButton::Secondary);
    let unpin = text_center(&output, "Desafixar da barra lateral").unwrap();
    folder_click(&mut app, &ctx, unpin, PointerButton::Primary);
    assert!(app.sidebar_options.pinned.is_empty());
    app.pin_folder(child.clone());
    app.persist_preferences(&ctx);
    let other = egui::Context::default();
    let mut reopened = Explorer::with_context(&other, child.clone());
    reopened.apply_preferences(Preferences::load(&path).unwrap(), &other);
    complete_navigation(&mut reopened);
    assert_eq!(reopened.sidebar_options.pinned[0].path, child);
    assert_eq!(
        sidebar::closest(&reopened.current, &reopened.sidebar_entries()),
        Some(sidebar::Location::Pinned(0))
    );
}

#[test]
fn context_menu_uses_custom_fontawesome_icons_and_centers_them_with_labels() {
    let ctx = egui::Context::default();
    let mut app = explorer(&ctx);
    app.appearance.overrides.insert(Icon::Copy, "star".into());
    app.appearance.size = 28.0;
    let draw = |app: &mut Explorer, ui: &mut egui::Ui| {
        app.menu_item(ui, Icon::Copy, "Copiar", "Ctrl+C", true, false)
            .rect
    };
    frame(&mut app, &ctx, 1180.0, vec![], draw);
    let (output, _) = frame(&mut app, &ctx, 1180.0, vec![], draw);
    let label = text_center(&output, "Copiar").unwrap();
    let icon = text_center(
        &output,
        &icons::glyph("star").unwrap().character().to_string(),
    )
    .expect("menu usa ícone configurado");
    let shortcut = text_center(&output, "Ctrl+C").unwrap();
    assert!(icon.x < label.x && label.x < shortcut.x);
    assert!((icon.y - label.y).abs() <= 1.0 && (shortcut.y - label.y).abs() <= 1.0);
}

fn delete_key(shift: bool) -> Event {
    Event::Key {
        key: Key::Delete,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: if shift {
            Modifiers::SHIFT
        } else {
            Modifiers::NONE
        },
    }
}

#[test]
fn shift_delete_requires_confirmation_cancel_preserves_and_confirm_removes_selection() {
    let root = tempfile::tempdir().unwrap();
    let a = root.path().join("first.txt");
    let b = root.path().join("folder");
    std::fs::write(&a, "first").unwrap();
    std::fs::create_dir(&b).unwrap();
    std::fs::write(b.join("nested"), "nested").unwrap();
    let ctx = egui::Context::default();
    let mut app = Explorer::with_context(&ctx, root.path().to_owned());
    complete_navigation(&mut app);
    app.selected = BTreeSet::from([a.clone(), b.clone()]);
    app.selection_anchor = Some(a.clone());
    app.confirm_trash = false;
    app.capture_files(file_ops::Mode::Cut, &ctx);
    transfer_key(&mut app, &ctx, delete_key(true));
    assert!(app.delete_confirmation.is_some() && app.active_deletion.is_none());
    assert!(!app.can_paste());
    let draw = |app: &mut Explorer, ui: &mut egui::Ui| {
        app.deletion_dialog(ui.ctx());
        ui.min_rect()
    };
    frame(&mut app, &ctx, 1180.0, vec![], draw);
    let (output, _) = frame(&mut app, &ctx, 1180.0, vec![], draw);
    assert!(
        text_center(&output, "first.txt").is_some() && text_center(&output, "folder").is_some()
    );
    click_label(
        &mut app,
        &ctx,
        "Cancelar",
        Modifiers::NONE,
        PointerButton::Primary,
        draw,
    );
    assert!(app.delete_confirmation.is_none() && a.exists() && b.exists());
    // The keyboard focus belongs to the removed Cancel button for one frame.
    ctx.memory_mut(|memory| {
        if let Some(id) = memory.focused() {
            memory.surrender_focus(id);
        }
    });
    let mut release = delete_key(true);
    if let Event::Key { pressed, .. } = &mut release {
        *pressed = false;
    }
    transfer_key(&mut app, &ctx, release);
    transfer_key(&mut app, &ctx, delete_key(true));
    assert!(app.delete_confirmation.is_some());
    frame(&mut app, &ctx, 1180.0, vec![], draw);
    click_label(
        &mut app,
        &ctx,
        "Excluir permanentemente",
        Modifiers::NONE,
        PointerButton::Primary,
        draw,
    );
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while app.active_deletion.is_some() && std::time::Instant::now() < deadline {
        app.poll();
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert!(app.active_deletion.is_none());
    app.process_transfer_refresh();
    complete_navigation(&mut app);
    assert!(app.error.is_none(), "{:?}", app.error);
    assert!(!a.exists() && !b.exists() && app.entries.is_empty());
    assert!(
        app.selected.is_empty() && app.selection_anchor.is_none() && app.file_clipboard.is_none()
    );
}

#[test]
fn delete_in_text_fields_and_with_other_modifiers_does_not_remove_files() {
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join("keep");
    std::fs::write(&file, "keep").unwrap();
    let ctx = egui::Context::default();
    let mut app = Explorer::with_context(&ctx, root.path().to_owned());
    complete_navigation(&mut app);
    app.selected.insert(file.clone());
    let edit = |app: &mut Explorer, ui: &mut egui::Ui| {
        app.shortcuts(ui.ctx());
        let field = ui.text_edit_singleline(&mut app.address);
        field.request_focus();
        field.rect
    };
    frame(&mut app, &ctx, 1180.0, vec![], edit);
    frame(&mut app, &ctx, 1180.0, vec![delete_key(false)], edit);
    frame(&mut app, &ctx, 1180.0, vec![delete_key(true)], edit);
    assert!(app.active_deletion.is_none() && app.delete_confirmation.is_none() && file.exists());
    ctx.memory_mut(|memory| {
        if let Some(id) = memory.focused() {
            memory.surrender_focus(id);
        }
    });
    let mut event = delete_key(false);
    if let Event::Key { modifiers, .. } = &mut event {
        *modifiers = Modifiers::CTRL;
    }
    transfer_key(&mut app, &ctx, event);
    assert!(app.active_deletion.is_none() && app.delete_confirmation.is_none() && file.exists());
}

#[test]
fn deletion_report_prunes_only_successful_items_and_disables_conflicting_operations() {
    let ctx = egui::Context::default();
    let mut app = explorer(&ctx);
    let a = PathBuf::from("/source/a");
    let b = PathBuf::from("/source/b");
    app.selected = BTreeSet::from([a.clone(), b.clone()]);
    app.selection_anchor = Some(b.clone());
    app.file_clipboard = Some(file_ops::Clipboard {
        id: 1,
        mode: file_ops::Mode::Copy,
        paths: vec![a.join("child"), b.clone()],
    });
    app.finish_deletion(crate::deletions::Report {
        mode: crate::deletions::Mode::Trash,
        completed: vec![a],
        errors: vec!["Falha em b".into()],
        cancelled: false,
    });
    assert_eq!(app.selected, BTreeSet::from([b.clone()]));
    assert_eq!(app.selection_anchor, Some(b.clone()));
    assert_eq!(app.file_clipboard.as_ref().unwrap().paths, vec![b]);
    assert!(app.error.as_ref().unwrap().contains("Falha em b"));
    let notice = app.notice_toasts.last().unwrap();
    assert_eq!(notice.kind, super::toast::NoticeKind::Error);
    assert!(notice.text.contains("Falha em b"));
    assert!(app.refresh_after_transfer);
}

#[test]
fn delete_key_trashes_in_isolated_child() {
    let Some(root) = std::env::var_os("EXPLORER_DELETE_KEY_TEST_ROOT") else {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join("data")).unwrap();
        let output = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "app::tests::delete_key_trashes_in_isolated_child",
                "--nocapture",
            ])
            .env("EXPLORER_DELETE_KEY_TEST_ROOT", root.path())
            .env("XDG_DATA_HOME", root.path().join("data"))
            .env("HOME", root.path())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(root.path().join("data/Trash/files/first").exists());
        assert!(root.path().join("data/Trash/files/second").exists());
        return;
    };
    let root = PathBuf::from(root);
    let source = root.join("source");
    std::fs::create_dir(&source).unwrap();
    let a = source.join("first");
    let b = source.join("second");
    std::fs::write(&a, "a").unwrap();
    std::fs::write(&b, "b").unwrap();
    let ctx = egui::Context::default();
    let mut app = Explorer::with_context(&ctx, source);
    complete_navigation(&mut app);
    app.selected = BTreeSet::from([a.clone(), b.clone()]);
    app.capture_files(file_ops::Mode::Copy, &ctx);
    transfer_key(&mut app, &ctx, delete_key(false));
    assert!(app.active_deletion.is_none() && app.delete_confirmation.is_some());
    assert!(a.exists() && b.exists());
    assert!(!app.can_paste());
    let draw = |app: &mut Explorer, ui: &mut egui::Ui| {
        app.deletion_dialog(ui.ctx());
        ui.min_rect()
    };
    frame(&mut app, &ctx, 1180.0, vec![], draw);
    let (output, _) = frame(&mut app, &ctx, 1180.0, vec![], draw);
    assert!(text_center(&output, "Enviar para a lixeira?").is_some());
    assert!(text_center(&output, "first").is_some() && text_center(&output, "second").is_some());
    click_label(
        &mut app,
        &ctx,
        "Cancelar",
        Modifiers::NONE,
        PointerButton::Primary,
        draw,
    );
    assert!(app.delete_confirmation.is_none() && a.exists() && b.exists());
    app.request_delete(crate::deletions::Mode::Trash, &ctx);
    frame(&mut app, &ctx, 1180.0, vec![], draw);
    click_label(
        &mut app,
        &ctx,
        "Mover para a lixeira",
        Modifiers::NONE,
        PointerButton::Primary,
        draw,
    );
    assert!(app.active_deletion.is_some() && app.delete_confirmation.is_none());
    assert!(!app.can_paste());
    app.request_delete(crate::deletions::Mode::Permanent, &ctx);
    assert!(
        app.delete_confirmation.is_none(),
        "exclusões não devem se sobrepor"
    );
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while app.active_deletion.is_some() && std::time::Instant::now() < deadline {
        app.poll();
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert!(app.active_deletion.is_none());
    app.process_transfer_refresh();
    complete_navigation(&mut app);
    assert!(app.error.is_none(), "{:?}", app.error);
    assert!(!a.exists() && !b.exists() && app.entries.is_empty() && app.file_clipboard.is_none());
    assert!(
        app.notice_toasts
            .last()
            .unwrap()
            .text
            .contains("2 itens enviados à lixeira")
    );
    // With the preference disabled, Del still uses the trash and starts immediately.
    let third = app.current.join("third");
    std::fs::write(&third, "third").unwrap();
    app.refresh();
    complete_navigation(&mut app);
    let mut preferences = app.preferences();
    preferences.confirm_trash = false;
    app.apply_preferences(preferences, &ctx);
    app.selected.insert(third.clone());
    let mut release = delete_key(false);
    if let Event::Key { pressed, .. } = &mut release {
        *pressed = false;
    }
    transfer_key(&mut app, &ctx, release);
    transfer_key(&mut app, &ctx, delete_key(false));
    assert!(app.active_deletion.is_some() && app.delete_confirmation.is_none());
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while app.active_deletion.is_some() && std::time::Instant::now() < deadline {
        app.poll();
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert!(app.active_deletion.is_none() && !third.exists());
    assert!(app.error.is_none(), "{:?}", app.error);
    assert!(root.join("data/Trash/files/third").exists());
}

#[test]
fn permanent_context_menu_uses_selection_and_escape_cancels() {
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join("keep");
    std::fs::write(&file, "keep").unwrap();
    let ctx = egui::Context::default();
    let mut app = Explorer::with_context(&ctx, root.path().to_owned());
    complete_navigation(&mut app);
    folder_frame(&mut app, &ctx, vec![]);
    let output = folder_frame(&mut app, &ctx, vec![]);
    let point = text_center(&output, "keep").unwrap();
    let output = folder_click(&mut app, &ctx, point, PointerButton::Secondary);
    assert!(text_center(&output, "Mover para a lixeira").is_some());
    let point = text_center(&output, "Excluir permanentemente…").unwrap();
    folder_click(&mut app, &ctx, point, PointerButton::Primary);
    assert!(app.delete_confirmation.is_some() && app.active_deletion.is_none());
    let draw = |app: &mut Explorer, ui: &mut egui::Ui| {
        app.deletion_dialog(ui.ctx());
        ui.min_rect()
    };
    frame(&mut app, &ctx, 1180.0, vec![], draw);
    frame(
        &mut app,
        &ctx,
        1180.0,
        vec![Event::Key {
            key: Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        }],
        draw,
    );
    assert!(app.delete_confirmation.is_none() && app.active_deletion.is_none() && file.exists());
}

fn shift_arrow(key: Key) -> Event {
    Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::SHIFT,
    }
}

#[test]
fn shift_arrows_extend_shrink_and_cross_the_anchor_in_all_views() {
    let root = tempfile::tempdir().unwrap();
    for name in ["a", "b", "c", "d", "e"] {
        std::fs::write(root.path().join(name), name).unwrap();
    }
    for view in [View::Details, View::List, View::Icons] {
        let ctx = egui::Context::default();
        let mut app = Explorer::with_context(&ctx, root.path().to_owned());
        complete_navigation(&mut app);
        app.view = view;
        let draw = |app: &mut Explorer, ui: &mut egui::Ui| {
            let visible = filesystem::visible_entries(
                &app.entries,
                &app.query,
                app.show_hidden,
                app.sort,
                app.descending,
            );
            match app.view {
                View::Details => app.details(ui, &visible),
                View::List => app.list(ui, &visible),
                View::Icons => app.icons(ui, &visible),
            }
            ui.min_rect()
        };
        frame(&mut app, &ctx, 1180.0, vec![], draw);
        click_label(
            &mut app,
            &ctx,
            "b",
            Modifiers::NONE,
            PointerButton::Primary,
            draw,
        );
        let expected = |names: &[&str]| {
            names
                .iter()
                .map(|name| root.path().join(name))
                .collect::<BTreeSet<_>>()
        };
        for (key, names) in [
            (Key::ArrowDown, vec!["b", "c"]),
            (Key::ArrowDown, vec!["b", "c", "d"]),
            (Key::ArrowUp, vec!["b", "c"]),
            (Key::ArrowUp, vec!["b"]),
            (Key::ArrowUp, vec!["a", "b"]),
            (Key::ArrowUp, vec!["a", "b"]),
        ] {
            transfer_key(&mut app, &ctx, shift_arrow(key));
            assert_eq!(app.selected, expected(&names));
            assert_eq!(app.selection_anchor, Some(root.path().join("b")));
        }
        click_label(
            &mut app,
            &ctx,
            "e",
            Modifiers::SHIFT,
            PointerButton::Primary,
            draw,
        );
        assert_eq!(app.selected, expected(&["b", "c", "d", "e"]));
        transfer_key(&mut app, &ctx, shift_arrow(Key::ArrowUp));
        assert_eq!(app.selected, expected(&["b", "c", "d"]));
    }
}

#[test]
fn shift_arrows_follow_filter_sort_and_boundaries_and_leave_text_selection_alone() {
    let root = tempfile::tempdir().unwrap();
    for name in ["a.txt", "b.txt", "c.txt", "z.bin", ".hidden.txt"] {
        std::fs::write(root.path().join(name), name).unwrap();
    }
    let ctx = egui::Context::default();
    let mut app = Explorer::with_context(&ctx, root.path().to_owned());
    complete_navigation(&mut app);
    app.query = ".txt".into();
    app.descending = true;
    for _ in 0..5 {
        transfer_key(&mut app, &ctx, shift_arrow(Key::ArrowDown));
    }
    assert_eq!(
        app.selected,
        ["a.txt", "b.txt", "c.txt"]
            .map(|name| root.path().join(name))
            .into()
    );
    assert_eq!(app.selection_anchor, Some(root.path().join("c.txt")));
    transfer_key(&mut app, &ctx, shift_arrow(Key::ArrowUp));
    assert_eq!(
        app.selected,
        ["b.txt", "c.txt"].map(|name| root.path().join(name)).into()
    );
    app.query = "a.txt".into();
    transfer_key(&mut app, &ctx, shift_arrow(Key::ArrowDown));
    assert_eq!(app.selected, BTreeSet::from([root.path().join("a.txt")]));
    let expected = app.selected.clone();
    let edit = |app: &mut Explorer, ui: &mut egui::Ui| {
        app.shortcuts(ui.ctx());
        let field = ui.text_edit_singleline(&mut app.address);
        field.request_focus();
        field.rect
    };
    frame(&mut app, &ctx, 1180.0, vec![], edit);
    frame(
        &mut app,
        &ctx,
        1180.0,
        vec![shift_arrow(Key::ArrowUp)],
        edit,
    );
    frame(
        &mut app,
        &ctx,
        1180.0,
        vec![shift_arrow(Key::ArrowDown)],
        edit,
    );
    assert_eq!(app.selected, expected);
    app.clear_selection();
    assert!(app.selection_cursor.is_none() && !app.reveal_selection);
}

#[test]
fn keyboard_selection_scrolls_to_items_beyond_the_visible_rows() {
    let root = tempfile::tempdir().unwrap();
    for index in 0..60 {
        std::fs::write(root.path().join(format!("file{index:02}")), "file").unwrap();
    }
    let ctx = egui::Context::default();
    let mut app = Explorer::with_context(&ctx, root.path().to_owned());
    complete_navigation(&mut app);
    app.zoom_items(1.0);
    let draw = |app: &mut Explorer, ui: &mut egui::Ui| {
        let visible = filesystem::visible_entries(
            &app.entries,
            &app.query,
            app.show_hidden,
            app.sort,
            app.descending,
        );
        app.details(ui, &visible);
        ui.min_rect()
    };
    frame(&mut app, &ctx, 1180.0, vec![], draw);
    for _ in 0..45 {
        transfer_key(&mut app, &ctx, shift_arrow(Key::ArrowDown));
    }
    assert_eq!(app.selection_cursor, Some(root.path().join("file44")));
    for _ in 0..4 {
        frame(&mut app, &ctx, 1180.0, vec![], draw);
    }
    let (output, _) = frame(&mut app, &ctx, 1180.0, vec![], draw);
    let selected =
        text_center(&output, "file44").expect("último item selecionado deve ficar visível");
    assert!((0.0..760.0).contains(&selected.y));
}

#[test]
fn page_home_end_follow_viewport_sort_and_shift_selection_in_all_views() {
    let root = tempfile::tempdir().unwrap();
    for index in 0..120 {
        std::fs::write(root.path().join(format!("file{index:03}.txt")), "file").unwrap();
    }
    std::fs::write(root.path().join("excluded.bin"), "ignored").unwrap();
    for view in [View::Details, View::List, View::Icons] {
        let ctx = egui::Context::default();
        let mut app = Explorer::with_context(&ctx, root.path().to_owned());
        complete_navigation(&mut app);
        app.view = view;
        app.item_scale = 1.5;
        app.query = ".txt".into();
        app.descending = true;
        let draw = |app: &mut Explorer, ui: &mut egui::Ui| {
            let visible = filesystem::visible_entries(
                &app.entries,
                &app.query,
                app.show_hidden,
                app.sort,
                app.descending,
            );
            match app.view {
                View::Details => app.details(ui, &visible),
                View::List => app.list(ui, &visible),
                View::Icons => app.icons(ui, &visible),
            }
            ui.min_rect()
        };
        let key = |key, shift| Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: true,
            modifiers: if shift {
                Modifiers::SHIFT
            } else {
                Modifiers::NONE
            },
        };
        frame(&mut app, &ctx, 1180.0, vec![], draw);
        let page = app.navigation_page;
        assert!(page > 1 && page < 120);
        transfer_key(&mut app, &ctx, key(Key::Home, false));
        assert_eq!(app.selection_cursor, Some(root.path().join("file119.txt")));
        transfer_key(&mut app, &ctx, key(Key::PageDown, false));
        assert_eq!(
            app.selected,
            BTreeSet::from([root.path().join(format!("file{:03}.txt", 119 - page))])
        );
        transfer_key(&mut app, &ctx, key(Key::PageUp, false));
        assert_eq!(app.selection_cursor, Some(root.path().join("file119.txt")));
        transfer_key(&mut app, &ctx, key(Key::PageDown, true));
        assert_eq!(app.selected.len(), page + 1);
        transfer_key(&mut app, &ctx, key(Key::PageUp, true));
        assert_eq!(app.selected.len(), 1);
        transfer_key(&mut app, &ctx, key(Key::End, true));
        assert_eq!(app.selected.len(), 120);
        for _ in 0..6 {
            frame(&mut app, &ctx, 1180.0, vec![], draw);
        }
        let (output, _) = frame(&mut app, &ctx, 1180.0, vec![], draw);
        let end = text_center(&output, "file000.txt").expect("End must reveal last item");
        assert!((0.0..760.0).contains(&end.y));
        transfer_key(&mut app, &ctx, key(Key::Home, false));
        assert_eq!(app.selected.len(), 1);
        for _ in 0..6 {
            frame(&mut app, &ctx, 1180.0, vec![], draw);
        }
        let (output, _) = frame(&mut app, &ctx, 1180.0, vec![], draw);
        assert!((0.0..760.0).contains(&text_center(&output, "file119.txt").unwrap().y));
        // Navigation keys belong to the text editor while an address/search is focused.
        let expected = app.selected.clone();
        let edit = |app: &mut Explorer, ui: &mut egui::Ui| {
            app.shortcuts(ui.ctx());
            let response = ui.text_edit_singleline(&mut app.address);
            response.request_focus();
            response.rect
        };
        frame(&mut app, &ctx, 1180.0, vec![], edit);
        for pressed in [Key::PageDown, Key::PageUp, Key::Home, Key::End] {
            frame(&mut app, &ctx, 1180.0, vec![key(pressed, false)], edit);
        }
        assert_eq!(app.selected, expected);
    }
}

#[test]
fn trash_confirmation_setting_can_be_cancelled_applied_and_restored_after_restart() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source");
    std::fs::create_dir(&source).unwrap();
    let ctx = egui::Context::default();
    let mut app = Explorer::with_context(&ctx, source.clone());
    complete_navigation(&mut app);
    let path = root.path().join("config/preferences.json");
    app.settings_path = Some(path.clone());
    assert!(app.confirm_trash);
    let draw = |app: &mut Explorer, ui: &mut egui::Ui| {
        app.settings_dialog(ui.ctx());
        ui.min_rect()
    };
    let label = "Pedir confirmação antes de enviar itens à lixeira";
    for save in [false, true] {
        app.settings_editor = Some(settings::Editor::new(app.preferences()));
        frame(&mut app, &ctx, 1180.0, vec![], draw);
        click_label(
            &mut app,
            &ctx,
            "Geral",
            Modifiers::NONE,
            PointerButton::Primary,
            draw,
        );
        click_label(
            &mut app,
            &ctx,
            label,
            Modifiers::NONE,
            PointerButton::Primary,
            draw,
        );
        assert!(app.confirm_trash, "a edição ainda não foi aplicada");
        click_label(
            &mut app,
            &ctx,
            if save { "Salvar e fechar" } else { "Cancelar" },
            Modifiers::NONE,
            PointerButton::Primary,
            draw,
        );
        assert!(app.settings_editor.is_none());
        assert_eq!(app.confirm_trash, !save);
    }
    let preferences = Preferences::load(&path).unwrap();
    assert!(!preferences.confirm_trash);
    let restarted_ctx = egui::Context::default();
    let mut restarted = Explorer::with_context(&restarted_ctx, source);
    complete_navigation(&mut restarted);
    restarted.apply_preferences(preferences, &restarted_ctx);
    assert!(!restarted.confirm_trash);
    assert!(!restarted.preferences().confirm_trash);
}

#[test]
fn general_toast_controls_can_be_cancelled_saved_and_restored() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("preferences.json");
    let ctx = egui::Context::default();
    let mut app = Explorer::with_context(&ctx, root.path().to_owned());
    complete_navigation(&mut app);
    app.settings_path = Some(path.clone());
    let draw = |app: &mut Explorer, ui: &mut egui::Ui| {
        app.settings_dialog(ui.ctx());
        ui.min_rect()
    };
    for save in [false, true] {
        app.settings_editor = Some(settings::Editor::new(app.preferences()));
        frame(&mut app, &ctx, 1180.0, vec![], draw);
        click_label(
            &mut app,
            &ctx,
            "6 s",
            Modifiers::NONE,
            PointerButton::Primary,
            draw,
        );
        frame(
            &mut app,
            &ctx,
            1180.0,
            vec![
                Event::Key {
                    key: Key::A,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers {
                        ctrl: true,
                        command: true,
                        ..Default::default()
                    },
                },
                Event::Text("27".into()),
            ],
            draw,
        );
        frame(
            &mut app,
            &ctx,
            1180.0,
            vec![Event::Key {
                key: Key::Enter,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            }],
            draw,
        );
        assert_eq!(
            app.settings_editor
                .as_ref()
                .unwrap()
                .draft
                .toast
                .success
                .duration_seconds,
            27
        );
        click_label(
            &mut app,
            &ctx,
            "Fechamento manual",
            Modifiers::NONE,
            PointerButton::Primary,
            draw,
        );
        assert!(
            !app.settings_editor
                .as_ref()
                .unwrap()
                .draft
                .toast
                .success
                .auto_dismiss
        );
        assert_eq!(
            app.toast_options,
            settings::ToastOptions::default(),
            "draft must not apply early"
        );
        click_label(
            &mut app,
            &ctx,
            if save { "Salvar e fechar" } else { "Cancelar" },
            Modifiers::NONE,
            PointerButton::Primary,
            draw,
        );
        assert!(app.settings_editor.is_none());
        if !save {
            assert_eq!(app.toast_options, settings::ToastOptions::default());
        }
    }
    let saved = Preferences::load(&path).unwrap();
    assert_eq!(
        saved.toast,
        settings::ToastOptions {
            success: settings::ToastTiming {
                duration_seconds: 27,
                auto_dismiss: false
            },
            ..Default::default()
        }
    );
    let second_ctx = egui::Context::default();
    let mut second = Explorer::with_context(&second_ctx, root.path().to_owned());
    complete_navigation(&mut second);
    second.apply_preferences(saved, &second_ctx);
    assert_eq!(second.toast_options, app.toast_options);
    app.settings_editor = Some(settings::Editor::new(app.preferences()));
    frame(&mut app, &ctx, 1180.0, vec![], draw);
    click_label(
        &mut app,
        &ctx,
        "Restaurar padrão",
        Modifiers::NONE,
        PointerButton::Primary,
        draw,
    );
    assert!(
        !app.toast_options.success.auto_dismiss,
        "restore remains a draft until applied"
    );
    click_label(
        &mut app,
        &ctx,
        "Aplicar",
        Modifiers::NONE,
        PointerButton::Primary,
        draw,
    );
    assert!(app.settings_editor.is_some());
    assert_eq!(app.toast_options, settings::ToastOptions::default());
    assert_eq!(
        Preferences::load(&path).unwrap().toast,
        settings::ToastOptions::default()
    );
}

#[test]
fn language_can_be_selected_applied_without_restart_and_saved() {
    use crate::i18n::{self, Language, Locale};
    i18n::with_locale(Locale::Portuguese, || {
        let root = tempfile::tempdir().unwrap();
        let ctx = egui::Context::default();
        let mut app = explorer(&ctx);
        let path = root.path().join("preferences.json");
        app.settings_path = Some(path.clone());
        app.settings_editor = Some(settings::Editor::new(app.preferences()));
        let draw = |app: &mut Explorer, ui: &mut egui::Ui| {
            app.settings_dialog(ui.ctx());
            ui.min_rect()
        };
        frame(&mut app, &ctx, 1180.0, vec![], draw);
        click_label(
            &mut app,
            &ctx,
            "Seguir idioma do sistema",
            Modifiers::NONE,
            PointerButton::Primary,
            draw,
        );
        click_label(
            &mut app,
            &ctx,
            "English",
            Modifiers::NONE,
            PointerButton::Primary,
            draw,
        );
        assert_eq!(
            app.language,
            Language::System,
            "a edição ainda não foi aplicada"
        );
        click_label(
            &mut app,
            &ctx,
            "Aplicar",
            Modifiers::NONE,
            PointerButton::Primary,
            draw,
        );
        assert_eq!(app.language, Language::English);
        let (output, _) = frame(&mut app, &ctx, 1180.0, vec![], draw);
        for label in [
            "Settings",
            "General",
            "Sidebar",
            "Columns",
            "Appearance and icons",
            "Language:",
            "Ask for confirmation before moving items to Trash",
        ] {
            assert!(
                text_center(&output, label).is_some(),
                "missing English text: {label}"
            );
        }
        assert_eq!(Column::Modified.label(), "Date modified");
        assert_eq!(locations::Item::Home.label(), "Home");
        assert_eq!(Icon::Copy.label(), "Copy files");
        assert_eq!(filesystem::format_size(1536), "1.5 KiB");
        assert!(app.sidebar_options == locations::Preferences::default());
        let saved = Preferences::load(&path).unwrap();
        assert_eq!(saved.language, Language::English);
        click_label(
            &mut app,
            &ctx,
            "Save and close",
            Modifiers::NONE,
            PointerButton::Primary,
            draw,
        );
        assert!(app.settings_editor.is_none());
        let new_ctx = egui::Context::default();
        let mut reopened = explorer(&new_ctx);
        reopened.apply_preferences(saved, &new_ctx);
        assert_eq!(reopened.language, Language::English);
        assert_eq!(i18n::tr("Explorador de Arquivos"), "File Explorer");
        app.settings_editor = Some(settings::Editor::new(app.preferences()));
        frame(&mut app, &ctx, 1180.0, vec![], draw);
        click_label(
            &mut app,
            &ctx,
            "English",
            Modifiers::NONE,
            PointerButton::Primary,
            draw,
        );
        click_label(
            &mut app,
            &ctx,
            "Português",
            Modifiers::NONE,
            PointerButton::Primary,
            draw,
        );
        click_label(
            &mut app,
            &ctx,
            "Apply",
            Modifiers::NONE,
            PointerButton::Primary,
            draw,
        );
        assert_eq!(app.language, Language::Portuguese);
        assert_eq!(Column::Modified.label(), "Data de modificação");
        assert_eq!(filesystem::format_size(1536), "1,5 KiB");
        // General settings can now scroll; return to the language row after applying.
        frame(
            &mut app,
            &ctx,
            1180.0,
            vec![
                Event::PointerMoved(egui::pos2(600.0, 300.0)),
                Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: Vec2::new(0.0, 1000.0),
                    modifiers: Modifiers::NONE,
                },
            ],
            draw,
        );
        frame(&mut app, &ctx, 1180.0, vec![], draw);
        click_label(
            &mut app,
            &ctx,
            "Português",
            Modifiers::NONE,
            PointerButton::Primary,
            draw,
        );
        click_label(
            &mut app,
            &ctx,
            "Seguir idioma do sistema",
            Modifiers::NONE,
            PointerButton::Primary,
            draw,
        );
        click_label(
            &mut app,
            &ctx,
            "Salvar e fechar",
            Modifiers::NONE,
            PointerButton::Primary,
            draw,
        );
        assert_eq!(app.language, Language::System);
        assert_eq!(Preferences::load(&path).unwrap().language, Language::System);
    });
}

#[test]
fn settings_tabs_have_requested_order_and_keep_footer_at_the_same_height_in_both_languages() {
    use crate::i18n::{self, Locale};
    for locale in [Locale::Portuguese, Locale::English] {
        i18n::with_locale(locale, || {
            for width in [780.0, 1180.0] {
                let ctx = egui::Context::default();
                let mut app = explorer(&ctx);
                app.settings_editor = Some(settings::Editor::new(app.preferences()));
                let draw = |app: &mut Explorer, ui: &mut egui::Ui| {
                    app.settings_dialog(ui.ctx());
                    ui.min_rect()
                };
                frame(&mut app, &ctx, width, vec![], draw);
                let (output, _) = frame(&mut app, &ctx, width, vec![], draw);
                let tabs = [
                    i18n::tr("Geral"),
                    i18n::tr("Barra lateral"),
                    i18n::tr("Colunas"),
                    i18n::tr("Aparência e ícones"),
                ];
                let positions: Vec<_> = tabs
                    .iter()
                    .map(|label| text_center(&output, label).unwrap())
                    .collect();
                assert!(positions.windows(2).all(|pair| pair[0].x < pair[1].x));
                let baseline = text_center(&output, i18n::tr("Salvar e fechar")).unwrap();
                for tab in tabs {
                    click_label(
                        &mut app,
                        &ctx,
                        tab,
                        Modifiers::NONE,
                        PointerButton::Primary,
                        draw,
                    );
                    let (output, _) = frame(&mut app, &ctx, width, vec![], draw);
                    let footer = text_center(&output, i18n::tr("Salvar e fechar")).unwrap();
                    assert!(
                        (footer.y - baseline.y).abs() <= 1.0,
                        "{tab}: footer {footer:?}, baseline {baseline:?}"
                    );
                }
            }
        });
    }
}

#[test]
fn zoom_toolbar_buttons_resize_items_in_all_views_and_preserve_selection_and_columns() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("example.txt"), "example").unwrap();
    for view in [View::Details, View::List, View::Icons] {
        let ctx = egui::Context::default();
        let mut app = Explorer::with_context(&ctx, root.path().to_owned());
        complete_navigation(&mut app);
        app.view = view;
        let path = root.path().join("example.txt");
        app.selected.insert(path.clone());
        app.column_widths.insert(Column::Name, 280.0);
        let widths = app.column_widths.clone();
        folder_frame(&mut app, &ctx, vec![]);
        let output = folder_frame(&mut app, &ctx, vec![]);
        let height = |output: &FullOutput| {
            output
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) if text.galley.text() == "example.txt" => {
                        Some(text.galley.size().y)
                    }
                    _ => None,
                })
                .unwrap()
        };
        let normal_height = height(&output);
        let plus = icons::glyph("magnifying-glass-plus")
            .unwrap()
            .character()
            .to_string();
        let minus = icons::glyph("magnifying-glass-minus")
            .unwrap()
            .character()
            .to_string();
        let point = text_center(&output, &plus).expect("botão de aumentar na toolbar");
        let output = folder_click(&mut app, &ctx, point, PointerButton::Primary);
        assert_eq!(app.item_scale, 1.25);
        assert!(height(&output) > normal_height);
        assert_eq!(app.selected, BTreeSet::from([path]));
        assert_eq!(app.column_widths, widths);
        let point = text_center(&output, &minus).expect("botão de diminuir na toolbar");
        folder_click(&mut app, &ctx, point, PointerButton::Primary);
        assert_eq!(app.item_scale, 1.0);
        app.zoom_items(100.0);
        assert_eq!(app.item_scale, 2.0);
        let output = folder_frame(&mut app, &ctx, vec![]);
        let point = text_center(&output, &plus).unwrap();
        folder_click(&mut app, &ctx, point, PointerButton::Primary);
        assert_eq!(app.item_scale, 2.0, "botão desativado no limite superior");
        app.zoom_items(-100.0);
        assert_eq!(app.item_scale, 0.75);
        let output = folder_frame(&mut app, &ctx, vec![]);
        let point = text_center(&output, &minus).unwrap();
        folder_click(&mut app, &ctx, point, PointerButton::Primary);
        assert_eq!(app.item_scale, 0.75, "botão desativado no limite inferior");
        app.settings_path = Some(root.path().join("preferences.json"));
        app.persist_preferences(&ctx);
        let saved = Preferences::load(app.settings_path.as_ref().unwrap()).unwrap();
        let mut reopened = Explorer::with_context(&ctx, root.path().to_owned());
        complete_navigation(&mut reopened);
        reopened.apply_preferences(saved, &ctx);
        assert_eq!(reopened.item_scale, 0.75);
    }
}

fn browser_frame(app: &mut Explorer, ctx: &egui::Context, events: Vec<Event>) -> FullOutput {
    let modifiers = events
        .iter()
        .rev()
        .find_map(|event| match event {
            Event::PointerButton { modifiers, .. } | Event::Key { modifiers, .. } => {
                Some(*modifiers)
            }
            _ => None,
        })
        .unwrap_or_default();
    ctx.run(
        RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1180.0, 760.0))),
            time: Some(ctx.input(|input| input.time) + 0.2),
            modifiers,
            events,
            ..Default::default()
        },
        |ctx| {
            app.top_toolbar(ctx);
            app.sidebar(ctx);
            let visible = filesystem::visible_entries(
                &app.entries,
                &app.query,
                app.show_hidden,
                app.sort,
                app.descending,
            );
            app.contents(ctx, &visible);
            app.transfer_dialog(ctx);
            app.deletion_dialog(ctx);
            app.empty_trash_dialog(ctx);
        },
    )
}

fn browser_click(
    app: &mut Explorer,
    ctx: &egui::Context,
    point: Pos2,
    button: PointerButton,
    modifiers: Modifiers,
) -> FullOutput {
    for pressed in [true, false] {
        browser_frame(
            app,
            ctx,
            vec![
                Event::PointerMoved(point),
                Event::PointerButton {
                    pos: point,
                    button,
                    pressed,
                    modifiers,
                },
            ],
        );
    }
    browser_frame(app, ctx, vec![])
}

fn complete_restore(app: &mut Explorer) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while app.active_restore.is_some() && std::time::Instant::now() < deadline {
        app.poll();
        assert!(app.transfer_conflict.is_none(), "conflito inesperado");
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert!(app.active_restore.is_none());
    app.process_transfer_refresh();
    complete_navigation(app);
}

fn await_restore_conflict(app: &mut Explorer) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while app.transfer_conflict.is_none() && std::time::Instant::now() < deadline {
        app.poll();
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert!(app.transfer_conflict.is_some());
}

#[test]
fn trash_sidebar_and_context_menu_restore_real_selections_in_an_isolated_child() {
    let Some(root) = std::env::var_os("EXPLORER_RESTORE_TEST_ROOT") else {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join("data")).unwrap();
        let output = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "app::tests::trash_sidebar_and_context_menu_restore_real_selections_in_an_isolated_child", "--nocapture"])
            .env("EXPLORER_RESTORE_TEST_ROOT", root.path()).env("XDG_DATA_HOME", root.path().join("data")).env("HOME", root.path())
            .output().unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            std::fs::read_to_string(root.path().join("source/a.txt")).unwrap(),
            "a"
        );
        assert_eq!(
            std::fs::read_to_string(root.path().join("source/b.txt")).unwrap(),
            "b"
        );
        assert!(root.path().join("source/folder/.hidden").exists());
        return;
    };
    let root = PathBuf::from(root);
    let source = root.join("source");
    std::fs::create_dir(&source).unwrap();
    let a = source.join("a.txt");
    let b = source.join("b.txt");
    let folder = source.join("folder");
    std::fs::write(&a, "a").unwrap();
    std::fs::write(&b, "b").unwrap();
    std::fs::create_dir(&folder).unwrap();
    std::fs::write(folder.join(".hidden"), "hidden").unwrap();
    ::trash::delete_all([&a, &b, &folder]).unwrap();
    let ctx = egui::Context::default();
    let mut app = Explorer::with_context(&ctx, source.clone());
    complete_navigation(&mut app);
    browser_frame(&mut app, &ctx, vec![]);
    let output = browser_frame(&mut app, &ctx, vec![]);
    let trash = text_center(&output, "Lixeira").expect("atalho da lixeira visível");
    browser_click(
        &mut app,
        &ctx,
        trash,
        PointerButton::Primary,
        Modifiers::NONE,
    );
    complete_navigation(&mut app);
    assert!(app.is_trash());
    assert!(
        app.visible_columns.contains(&Column::OriginalPath)
            && app.visible_columns.contains(&Column::Deleted)
    );
    let own: Vec<_> = app
        .entries
        .iter()
        .filter(|entry| {
            entry
                .trash
                .as_ref()
                .is_some_and(|record| record.original_path.starts_with(&source))
        })
        .collect();
    assert_eq!(own.len(), 3);
    assert_eq!(
        sidebar::closest(&app.current, &app.sidebar_entries()),
        Some(sidebar::Location::Trash)
    );
    let tree_entry = app
        .entries
        .iter()
        .find(|entry| {
            entry.name == "folder" && entry.trash.as_ref().unwrap().original_path == folder
        })
        .unwrap()
        .clone();
    app.open(&tree_entry, &ctx);
    assert!(
        app.is_trash() && !app.loading,
        "abrir não deve entrar fisicamente nos dados da lixeira"
    );
    app.selected.insert(tree_entry.path.clone());
    app.capture_files(file_ops::Mode::Cut, &ctx);
    assert!(app.file_clipboard.is_none());
    app.request_delete(crate::deletions::Mode::Permanent, &ctx);
    assert!(app.delete_confirmation.is_some() && app.active_deletion.is_none());
    let dialog = |app: &mut Explorer, ui: &mut egui::Ui| {
        app.deletion_dialog(ui.ctx());
        ui.min_rect()
    };
    frame(&mut app, &ctx, 1180.0, vec![], dialog);
    click_label(
        &mut app,
        &ctx,
        "Cancelar",
        Modifiers::NONE,
        PointerButton::Primary,
        dialog,
    );
    assert!(app.delete_confirmation.is_none() && tree_entry.path.exists());
    app.file_clipboard = Some(file_ops::Clipboard {
        id: 1,
        mode: file_ops::Mode::Copy,
        paths: vec![source.clone()],
    });
    assert!(!app.can_paste());
    app.file_clipboard = None;
    let output = browser_frame(&mut app, &ctx, vec![]);
    let point = text_center(&output, "a.txt").unwrap();
    let output = browser_click(
        &mut app,
        &ctx,
        point,
        PointerButton::Primary,
        Modifiers::NONE,
    );
    let point = text_center(&output, "b.txt").unwrap();
    browser_click(
        &mut app,
        &ctx,
        point,
        PointerButton::Primary,
        Modifiers::CTRL,
    );
    assert_eq!(app.selected.len(), 2);
    let output = browser_click(
        &mut app,
        &ctx,
        point,
        PointerButton::Secondary,
        Modifiers::NONE,
    );
    assert_eq!(
        app.selected.len(),
        2,
        "clique direito deve preservar o grupo"
    );
    assert!(text_center(&output, "Recortar").is_none());
    let restore = text_center(&output, "Restaurar").unwrap();
    browser_click(
        &mut app,
        &ctx,
        restore,
        PointerButton::Primary,
        Modifiers::NONE,
    );
    assert!(app.active_restore.is_some());
    complete_restore(&mut app);
    assert!(app.error.is_none(), "{:?}", app.error);
    assert!(
        app.selected.is_empty() && app.selection_anchor.is_none() && app.selection_cursor.is_none()
    );
    assert!(a.exists() && b.exists());
    let output = browser_frame(&mut app, &ctx, vec![]);
    let point = text_center(&output, "folder").unwrap();
    let output = browser_click(
        &mut app,
        &ctx,
        point,
        PointerButton::Secondary,
        Modifiers::NONE,
    );
    let restore = text_center(&output, "Restaurar").unwrap();
    browser_click(
        &mut app,
        &ctx,
        restore,
        PointerButton::Primary,
        Modifiers::NONE,
    );
    complete_restore(&mut app);
    assert!(folder.join(".hidden").exists());

    let conflict = source.join("conflict.txt");
    std::fs::write(&conflict, "trashed").unwrap();
    ::trash::delete(&conflict).unwrap();
    std::fs::write(&conflict, "existing").unwrap();
    app.refresh();
    complete_navigation(&mut app);
    let entry = app
        .entries
        .iter()
        .find(|entry| {
            entry
                .trash
                .as_ref()
                .is_some_and(|record| record.original_path == conflict)
        })
        .unwrap()
        .clone();
    app.selected = BTreeSet::from([entry.path.clone()]);
    app.restore_selected(&ctx);
    await_restore_conflict(&mut app);
    browser_frame(&mut app, &ctx, vec![]);
    let output = browser_frame(&mut app, &ctx, vec![]);
    assert!(
        text_center(
            &output,
            "O destino original já existe. Manter ambos restaura com outro nome."
        )
        .is_some()
    );
    let keep = text_center(&output, "Manter ambos").unwrap();
    browser_click(
        &mut app,
        &ctx,
        keep,
        PointerButton::Primary,
        Modifiers::NONE,
    );
    complete_restore(&mut app);
    assert!(app.error.is_none(), "{:?}", app.error);
    assert_eq!(std::fs::read_to_string(&conflict).unwrap(), "existing");
    assert_eq!(
        std::fs::read_to_string(source.join("conflict (restaurado).txt")).unwrap(),
        "trashed"
    );
    std::fs::write(&conflict, "cancelled").unwrap();
    ::trash::delete(&conflict).unwrap();
    std::fs::write(&conflict, "preserved").unwrap();
    app.refresh();
    complete_navigation(&mut app);
    let entry = app
        .entries
        .iter()
        .find(|entry| {
            entry
                .trash
                .as_ref()
                .is_some_and(|record| record.original_path == conflict)
        })
        .unwrap()
        .clone();
    app.selected = BTreeSet::from([entry.path.clone()]);
    app.restore_selected(&ctx);
    await_restore_conflict(&mut app);
    app.cancel_restore();
    complete_restore(&mut app);
    assert!(entry.path.exists() && app.selected.contains(&entry.path));
    assert_eq!(std::fs::read_to_string(&conflict).unwrap(), "preserved");

    app.view = View::List;
    app.zoom_items(0.25);
    app.settings_path = Some(root.join("preferences.json"));
    app.persist_preferences(&ctx);
    let layout = app.current_layout();
    app.navigate(source.clone(), Navigation::Back);
    complete_navigation(&mut app);
    assert!(!app.is_trash() && app.item_scale == 1.0);
    app.navigate(crate::trashbin::location(), Navigation::Forward);
    complete_navigation(&mut app);
    assert!(app.current_layout() == layout);
    let stored = Preferences::load(app.settings_path.as_ref().unwrap()).unwrap();
    let other = egui::Context::default();
    let mut reopened = Explorer::with_context(&other, source);
    complete_navigation(&mut reopened);
    reopened.apply_preferences(stored, &other);
    reopened.navigate(crate::trashbin::location(), Navigation::Visit);
    complete_navigation(&mut reopened);
    assert!(reopened.is_trash() && reopened.current_layout() == layout);
}

#[test]
fn trash_sidebar_visibility_is_configurable_and_saved() {
    let root = tempfile::tempdir().unwrap();
    let ctx = egui::Context::default();
    let mut app = explorer(&ctx);
    app.settings_path = Some(root.path().join("preferences.json"));
    assert!(
        app.sidebar_entries()
            .iter()
            .any(|entry| entry.id == sidebar::Location::Trash)
    );
    app.settings_editor = Some(settings::Editor::new(app.preferences()));
    let draw = |app: &mut Explorer, ui: &mut egui::Ui| {
        app.settings_dialog(ui.ctx());
        ui.min_rect()
    };
    frame(&mut app, &ctx, 1180.0, vec![], draw);
    click_label(
        &mut app,
        &ctx,
        "Barra lateral",
        Modifiers::NONE,
        PointerButton::Primary,
        draw,
    );
    click_label(
        &mut app,
        &ctx,
        "Exibir Lixeira",
        Modifiers::NONE,
        PointerButton::Primary,
        draw,
    );
    assert!(app.sidebar_options.show_trash);
    click_label(
        &mut app,
        &ctx,
        "Salvar e fechar",
        Modifiers::NONE,
        PointerButton::Primary,
        draw,
    );
    assert!(!app.sidebar_options.show_trash);
    assert!(
        !app.sidebar_entries()
            .iter()
            .any(|entry| entry.id == sidebar::Location::Trash)
    );
    assert!(
        !Preferences::load(app.settings_path.as_ref().unwrap())
            .unwrap()
            .sidebar
            .show_trash
    );
}

#[test]
fn toolbar_spans_above_the_sidebar_and_the_folder_body() {
    for width in [780.0, 1180.0] {
        let ctx = egui::Context::default();
        let mut app = explorer(&ctx);
        app.places = vec![(locations::Item::Home, PathBuf::from("/home/test"))];
        let mut render = || {
            ctx.run(
                RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(width, 760.0))),
                    ..Default::default()
                },
                |ctx| {
                    app.top_toolbar(ctx);
                    app.sidebar(ctx);
                    app.contents(ctx, &[]);
                },
            )
        };
        render();
        let output = render();
        let back = text_center(
            &output,
            &icons::glyph("arrow-left").unwrap().character().to_string(),
        )
        .unwrap();
        let settings = text_center(
            &output,
            &icons::glyph("gear").unwrap().character().to_string(),
        )
        .unwrap();
        let home = text_center(&output, "Pasta pessoal").unwrap();
        let search = text_center(&output, "Pesquisar nesta pasta · Ctrl+F").unwrap();
        assert!(back.x < home.x, "navegação começa acima da barra lateral");
        assert!(back.y < home.y && settings.y < search.y);
        assert!((settings.y - back.y).abs() <= 1.0);
        assert!(
            settings.x > width - 100.0,
            "configuração permanece no extremo direito"
        );
        assert!(text_center(&output, "Explorador de Arquivos").is_none());
    }
}

fn confirmed_test_trash(app: &mut Explorer, private_root: &Path) -> Vec<PathBuf> {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while app
        .empty_trash_prompt
        .as_ref()
        .is_some_and(|prompt| prompt.listing.is_none() && prompt.error.is_none())
        && std::time::Instant::now() < deadline
    {
        app.poll();
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    let prompt = app.empty_trash_prompt.as_mut().unwrap();
    assert!(prompt.error.is_none(), "{:?}", prompt.error);
    let listing = prompt.listing.as_mut().expect("listagem não terminou");
    // A private data home does not hide mounted-volume trash. Never purge those records in a test.
    let private_files = private_root.join("data/Trash/files");
    let private_originals = private_root.join("source");
    listing.entries.retain(|entry| {
        entry.path.starts_with(&private_files)
            && entry
                .trash
                .as_ref()
                .is_some_and(|record| record.original_path.starts_with(&private_originals))
    });
    listing.skipped = 0;
    assert!(
        listing
            .entries
            .iter()
            .all(|entry| entry.path.starts_with(&private_files))
    );
    listing
        .entries
        .iter()
        .map(|entry| entry.path.clone())
        .collect()
}

#[test]
fn empty_trash_context_menu_confirms_and_only_purges_the_confirmed_snapshot_in_an_isolated_child() {
    let Some(root) = std::env::var_os("EXPLORER_EMPTY_TRASH_TEST_ROOT") else {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join("data")).unwrap();
        let output = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "app::tests::empty_trash_context_menu_confirms_and_only_purges_the_confirmed_snapshot_in_an_isolated_child", "--nocapture"])
            .env("EXPLORER_EMPTY_TRASH_TEST_ROOT", root.path()).env("XDG_DATA_HOME", root.path().join("data")).env("HOME", root.path())
            .output().unwrap();
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
    let file = source.join("file.txt");
    std::fs::write(&file, "permanently removed").unwrap();
    let folder = source.join("folder");
    std::fs::create_dir(&folder).unwrap();
    std::fs::write(folder.join(".hidden"), "hidden").unwrap();
    let preserved = root.join("preserved");
    std::fs::create_dir(&preserved).unwrap();
    std::fs::write(preserved.join("keep"), "keep").unwrap();
    std::os::unix::fs::symlink(&preserved, folder.join("link")).unwrap();
    let link = source.join("link");
    std::os::unix::fs::symlink(&preserved, &link).unwrap();
    ::trash::delete_all([&file, &folder, &link]).unwrap();
    let ctx = egui::Context::default();
    let mut app = Explorer::with_context(&ctx, source.clone());
    complete_navigation(&mut app);
    app.confirm_trash = false;
    browser_frame(&mut app, &ctx, vec![]);
    let output = browser_frame(&mut app, &ctx, vec![]);
    let trash_point = text_center(&output, "Lixeira").unwrap();
    let output = browser_click(
        &mut app,
        &ctx,
        trash_point,
        PointerButton::Secondary,
        Modifiers::NONE,
    );
    let empty = text_center(&output, "Esvaziar lixeira").unwrap();
    browser_click(
        &mut app,
        &ctx,
        empty,
        PointerButton::Primary,
        Modifiers::NONE,
    );
    assert!(app.empty_trash_prompt.is_some() && app.active_empty_trash.is_none());
    let snapshot = confirmed_test_trash(&mut app, &root);
    assert_eq!(snapshot.len(), 3);
    assert!(
        snapshot
            .iter()
            .all(|path| std::fs::symlink_metadata(path).is_ok())
    );
    assert!(!app.can_empty_trash());
    app.request_delete(crate::deletions::Mode::Permanent, &ctx);
    assert!(app.delete_confirmation.is_none());
    browser_frame(&mut app, &ctx, vec![]);
    let output = browser_frame(&mut app, &ctx, vec![]);
    assert!(text_center(&output, "Esvaziar lixeira?").is_some());
    let cancel = text_center(&output, "Cancelar").unwrap();
    browser_click(
        &mut app,
        &ctx,
        cancel,
        PointerButton::Primary,
        Modifiers::NONE,
    );
    assert!(app.empty_trash_prompt.is_none() && app.active_empty_trash.is_none());
    assert!(
        snapshot
            .iter()
            .all(|path| std::fs::symlink_metadata(path).is_ok())
    );
    let output = browser_frame(&mut app, &ctx, vec![]);
    let trash_point = text_center(&output, "Lixeira").unwrap();
    let output = browser_click(
        &mut app,
        &ctx,
        trash_point,
        PointerButton::Secondary,
        Modifiers::NONE,
    );
    let empty = text_center(&output, "Esvaziar lixeira").unwrap();
    browser_click(
        &mut app,
        &ctx,
        empty,
        PointerButton::Primary,
        Modifiers::NONE,
    );
    let snapshot = confirmed_test_trash(&mut app, &root);
    assert_eq!(snapshot.len(), 3);
    let late = source.join("late.txt");
    std::fs::write(&late, "preserve later arrival").unwrap();
    ::trash::delete(&late).unwrap();
    browser_frame(&mut app, &ctx, vec![]);
    let output = browser_frame(&mut app, &ctx, vec![]);
    let confirm = text_center(&output, "Esvaziar lixeira").unwrap();
    browser_click(
        &mut app,
        &ctx,
        confirm,
        PointerButton::Primary,
        Modifiers::NONE,
    );
    assert!(app.empty_trash_prompt.is_none() && app.active_empty_trash.is_some());
    assert!(!app.can_empty_trash());
    app.request_delete(crate::deletions::Mode::Permanent, &ctx);
    assert!(app.delete_confirmation.is_none());
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while app.active_empty_trash.is_some() && std::time::Instant::now() < deadline {
        app.poll();
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert!(app.active_empty_trash.is_none());
    app.process_transfer_refresh();
    complete_navigation(&mut app);
    assert!(app.error.is_none(), "{:?}", app.error);
    assert!(
        snapshot
            .iter()
            .all(|path| std::fs::symlink_metadata(path).is_err())
    );
    assert!(!file.exists() && !folder.exists() && std::fs::symlink_metadata(link).is_err());
    assert_eq!(
        std::fs::read_to_string(preserved.join("keep")).unwrap(),
        "keep"
    );
    let listing = crate::trashbin::read().unwrap();
    let own: Vec<_> = listing
        .entries
        .iter()
        .filter(|entry| {
            entry
                .trash
                .as_ref()
                .is_some_and(|record| record.original_path.starts_with(&source))
        })
        .collect();
    assert_eq!(own.len(), 1);
    assert_eq!(own[0].name, "late.txt");
    assert_eq!(
        std::fs::read_to_string(&own[0].path).unwrap(),
        "preserve later arrival"
    );
    assert!(
        app.notice_toasts
            .last()
            .unwrap()
            .text
            .contains("3 itens excluídos permanentemente")
    );
}

mod file_icons;
mod opening;
mod trash_deletion;

#[test]
fn general_test_buttons_preview_each_draft_kind_without_saving_preferences() {
    let root = tempfile::tempdir().unwrap();
    let ctx = egui::Context::default();
    let mut app = explorer(&ctx);
    let path = root.path().join("preferences.json");
    app.settings_path = Some(path.clone());
    let original = app.toast_options;
    app.settings_editor = Some(settings::Editor::new(app.preferences()));
    let appearance = app.appearance.clone();
    for icon in [Icon::NoticeSuccess, Icon::NoticeWarning, Icon::NoticeError] {
        app.settings_editor
            .as_mut()
            .unwrap()
            .draft
            .appearance
            .overrides
            .insert(icon, "bell".into());
    }

    let draw = |app: &mut Explorer, ui: &mut egui::Ui| {
        app.settings_dialog(ui.ctx());
        app.show_notice_toast(ui.ctx());
        ui.min_rect()
    };
    for (kind, label, timing) in [
        (
            settings::ToastKind::Success,
            "Testar sucesso",
            settings::ToastTiming {
                duration_seconds: 17,
                auto_dismiss: true,
            },
        ),
        (
            settings::ToastKind::Warning,
            "Testar aviso",
            settings::ToastTiming {
                duration_seconds: 25,
                auto_dismiss: true,
            },
        ),
        (
            settings::ToastKind::Error,
            "Testar erro",
            settings::ToastTiming {
                duration_seconds: 60,
                auto_dismiss: false,
            },
        ),
    ] {
        let draft = &mut app.settings_editor.as_mut().unwrap().draft.toast;
        match kind {
            settings::ToastKind::Success => draft.success = timing,
            settings::ToastKind::Warning => draft.warning = timing,
            settings::ToastKind::Error => draft.error = timing,
        }
        frame(&mut app, &ctx, 1180.0, vec![], draw);
        click_label(
            &mut app,
            &ctx,
            label,
            Modifiers::NONE,
            PointerButton::Primary,
            draw,
        );
        let preview = app.notice_toasts.last().unwrap();
        assert_eq!(preview.kind, kind);
        assert_eq!(preview.timing_override, Some(timing));
        assert_eq!(preview.icon_override.as_deref(), Some("bell"));
        assert!(app.appearance == appearance);
        assert_eq!(app.toast_options, original);
        assert!(!path.exists(), "testing must not save settings");
        assert!(app.settings_editor.is_some());
    }
    assert_eq!(app.notice_toasts.len(), 3);
    let (output, _) = frame(&mut app, &ctx, 1180.0, vec![], draw);
    let close = text_center(&output, "×").unwrap();
    click(&mut app, &ctx, close, draw);
    assert_eq!(app.notice_toasts.len(), 2);
    assert!(
        app.settings_editor.is_some(),
        "closing a preview must keep settings open"
    );
    click_label(
        &mut app,
        &ctx,
        "Cancelar",
        Modifiers::NONE,
        PointerButton::Primary,
        draw,
    );
    assert!(app.settings_editor.is_none());
    assert_eq!(app.toast_options, original);
    assert!(!path.exists());
}
