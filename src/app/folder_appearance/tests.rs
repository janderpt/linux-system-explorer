use super::*;
use crate::app::tests::{complete_navigation, text_center};
use egui::{Event, Modifiers, PointerButton, Pos2, RawInput, Rect};
use std::fs;
fn frame(app: &mut Explorer, ctx: &egui::Context, events: Vec<Event>) -> egui::FullOutput {
    ctx.run(
        RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1180.0, 760.0))),
            time: Some(ctx.input(|i| i.time) + 0.2),
            events,
            ..Default::default()
        },
        |ctx| {
            let visible = app.visible_for_display();
            app.top_toolbar(ctx);
            app.contents(ctx, &visible);
            app.properties_dialog(ctx);
        },
    )
}
fn click(
    app: &mut Explorer,
    ctx: &egui::Context,
    pos: Pos2,
    button: PointerButton,
) -> egui::FullOutput {
    for pressed in [true, false] {
        frame(
            app,
            ctx,
            vec![
                Event::PointerMoved(pos),
                Event::PointerButton {
                    pos,
                    button,
                    pressed,
                    modifiers: Modifiers::NONE,
                },
            ],
        );
    }
    frame(app, ctx, vec![])
}
fn label(app: &mut Explorer, ctx: &egui::Context, name: &str) -> egui::FullOutput {
    let out = frame(app, ctx, vec![]);
    let point = text_center(&out, name).unwrap_or_else(|| panic!("missing {name}"));
    click(app, ctx, point, PointerButton::Primary)
}
#[test]
fn folder_properties_customize_apply_cancel_restore_and_failed_save_preserve_other_folders() {
    let root = tempfile::tempdir().unwrap();
    let folder = root.path().join("Pasta");
    let other = root.path().join("Outra");
    fs::create_dir(&folder).unwrap();
    fs::create_dir(&other).unwrap();
    fs::write(folder.join("keep.txt"), "original").unwrap();
    let ctx = egui::Context::default();
    let mut app = Explorer::with_context(&ctx, root.path().to_owned());
    complete_navigation(&mut app);
    let entry = app
        .entries
        .iter()
        .find(|e| e.path == folder)
        .unwrap()
        .clone();
    app.settings_path = Some(root.path().join("config/preferences.json"));
    let unrelated = Style {
        icon: "house".into(),
        color: [201, 35, 90],
    };
    app.folder_styles
        .insert(settings::folder_key(&other), unrelated.clone());
    frame(&mut app, &ctx, vec![]);
    let out = frame(&mut app, &ctx, vec![]);
    let point = out
        .shapes
        .iter()
        .find_map(|shape| {
            if let egui::Shape::Text(text) = &shape.shape
                && text.galley.text() == "Pasta"
                && text.pos.x < 500.0
            {
                Some(text.pos + text.galley.size() * 0.5)
            } else {
                None
            }
        })
        .unwrap();
    let out = click(&mut app, &ctx, point, PointerButton::Secondary);
    let point = text_center(&out, "Personalizar pasta…").unwrap();
    click(&mut app, &ctx, point, PointerButton::Primary);
    assert_eq!(app.folder_editor.as_ref().unwrap().path, folder);
    assert!(app.folder_editor.as_ref().unwrap().appearance);
    frame(&mut app, &ctx, vec![]);
    let out = frame(&mut app, &ctx, vec![]);
    let preset = out
        .shapes
        .iter()
        .find_map(|s| {
            if let egui::Shape::Rect(r) = &s.shape {
                (r.fill == Color32::from_rgb(36, 132, 204) && (r.rect.width() - 20.0).abs() < 1.0)
                    .then(|| r.rect.center())
            } else {
                None
            }
        })
        .unwrap();
    click(&mut app, &ctx, preset, PointerButton::Primary);
    assert_eq!(
        app.folder_editor
            .as_ref()
            .unwrap()
            .draft
            .as_ref()
            .unwrap()
            .color,
        [36, 132, 204]
    );
    app.folder_editor.as_mut().unwrap().search = "star".into();
    frame(&mut app, &ctx, vec![]);
    let out = frame(&mut app, &ctx, vec![]);
    let point = out
        .shapes
        .iter()
        .rev()
        .find_map(|shape| {
            if let egui::Shape::Text(text) = &shape.shape
                && text.galley.text() == "star"
            {
                Some(text.pos + text.galley.size() * 0.5)
            } else {
                None
            }
        })
        .unwrap();
    click(&mut app, &ctx, point, PointerButton::Primary);
    assert_eq!(
        app.folder_editor
            .as_ref()
            .unwrap()
            .draft
            .as_ref()
            .unwrap()
            .icon,
        "star"
    );
    label(&mut app, &ctx, "Aplicar");
    let expected = Style {
        icon: "star".into(),
        color: [36, 132, 204],
    };
    assert_eq!(
        app.folder_styles.get(&settings::folder_key(&folder)),
        Some(&expected)
    );
    let saved = Preferences::load(app.settings_path.as_ref().unwrap()).unwrap();
    assert_eq!(
        saved.folder_styles.get(&settings::folder_key(&folder)),
        Some(&expected)
    );
    assert_eq!(
        saved.folder_styles.get(&settings::folder_key(&other)),
        Some(&unrelated)
    );
    let appearance = frame(&mut app, &ctx, vec![]);
    let footer = text_center(&appearance, "Salvar e fechar").unwrap();
    let general = label(&mut app, &ctx, "Geral");
    assert!((text_center(&general, "Salvar e fechar").unwrap().y - footer.y).abs() < 1.0);
    label(&mut app, &ctx, "Aparência");
    label(&mut app, &ctx, "Restaurar padrão");
    assert!(app.folder_editor.as_ref().unwrap().draft.is_none());
    assert!(
        app.folder_styles
            .contains_key(&settings::folder_key(&folder))
    );
    label(&mut app, &ctx, "Cancelar");
    assert!(app.properties.is_none());
    assert_eq!(
        app.folder_styles.get(&settings::folder_key(&folder)),
        Some(&expected)
    );
    app.properties = Some(entry.clone());
    app.folder_editor = Some(Editor::new(folder.clone(), Some(expected.clone()), true));
    frame(&mut app, &ctx, vec![]);
    label(&mut app, &ctx, "Restaurar padrão");
    label(&mut app, &ctx, "Salvar e fechar");
    assert!(app.properties.is_none());
    let saved = Preferences::load(app.settings_path.as_ref().unwrap()).unwrap();
    assert!(
        !saved
            .folder_styles
            .contains_key(&settings::folder_key(&folder))
    );
    assert_eq!(
        saved.folder_styles.get(&settings::folder_key(&other)),
        Some(&unrelated)
    );
    app.settings_path = Some(root.path().to_owned());
    assert!(!app.save_folder_style(&folder, Some(expected)));
    assert!(
        !app.folder_styles
            .contains_key(&settings::folder_key(&folder))
    );
    assert!(!app.notice_toasts.is_empty());
    assert_eq!(
        fs::read_to_string(folder.join("keep.txt")).unwrap(),
        "original"
    );
}
#[test]
fn custom_glyph_and_exact_rgb_survive_all_file_views_global_icon_modes_and_light_dark_themes() {
    let root = tempfile::tempdir().unwrap();
    let folder = root.path().join("Pasta");
    fs::create_dir(&folder).unwrap();
    let ctx = egui::Context::default();
    let mut app = Explorer::with_context(&ctx, root.path().to_owned());
    complete_navigation(&mut app);
    let style = Style {
        icon: "star".into(),
        color: [211, 39, 117],
    };
    app.folder_styles
        .insert(settings::folder_key(&folder), style.clone());
    let symbol = icons::glyph("star").unwrap().character().to_string();
    for dark in [false, true] {
        for mode in [icons::IconMode::Monochrome, icons::IconMode::Colored] {
            app.dark = dark;
            app.appearance.mode = mode;
            ctx.set_visuals(if dark {
                egui::Visuals::dark()
            } else {
                egui::Visuals::light()
            });
            for view in [View::Details, View::List, View::Icons] {
                app.view = view;
                app.selected = BTreeSet::from([folder.clone()]);
                frame(&mut app, &ctx, vec![]);
                let out = frame(&mut app, &ctx, vec![]);
                assert!(out.shapes.iter().any(|s|matches!(&s.shape,egui::Shape::Text(t) if t.galley.text()==symbol&&t.galley.job.sections.iter().all(|section|section.format.color==Color32::from_rgb(211,39,117)))));
            }
        }
    }
    let mut preferences = app.preferences();
    preferences.dark = !app.dark;
    preferences.appearance.mode = icons::IconMode::Monochrome;
    app.apply_global_preferences(&preferences, &ctx);
    assert_eq!(
        app.folder_styles.get(&settings::folder_key(&folder)),
        Some(&style)
    );
}
