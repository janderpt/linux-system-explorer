use super::*;

fn alpha(output: &FullOutput, label: &str) -> u8 {
    output
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Text(text) if text.galley.text() == label => {
                Some(text.override_text_color.unwrap_or(text.fallback_color).a())
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("missing painted text: {label}"))
}

fn escape() -> Event {
    Event::Key {
        key: Key::Escape,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    }
}

#[test]
fn cut_items_fade_in_all_views_and_escape_restores_them_without_touching_files() {
    for view in [View::Details, View::List, View::Icons] {
        for mode in [icons::IconMode::Monochrome, icons::IconMode::Colored] {
            for dark in [false, true] {
                let root = tempfile::tempdir().unwrap();
                let file = root.path().join("cut.pdf");
                let folder = root.path().join("Folder");
                std::fs::write(&file, "unchanged content").unwrap();
                std::fs::write(root.path().join("normal.txt"), "other").unwrap();
                std::fs::create_dir(&folder).unwrap();
                let ctx = egui::Context::default();
                let mut app = Explorer::with_context(&ctx, root.path().into());
                complete_navigation(&mut app);
                app.view = view;
                app.appearance.mode = mode;
                app.dark = dark;
                ctx.set_visuals(if dark {
                    egui::Visuals::dark()
                } else {
                    egui::Visuals::light()
                });
                app.selected = BTreeSet::from([file.clone(), folder.clone()]);
                folder_frame(&mut app, &ctx, vec![]);
                let normal = folder_frame(&mut app, &ctx, vec![]);
                transfer_key(&mut app, &ctx, Event::Cut);
                let faded = folder_frame(&mut app, &ctx, vec![]);
                for label in ["cut.pdf", "Folder"] {
                    assert!(alpha(&faded, label) < alpha(&normal, label));
                }
                let glyph = icons::glyph("file-pdf").unwrap().character().to_string();
                assert!(alpha(&faded, &glyph) < alpha(&normal, &glyph));
                assert_eq!(alpha(&faded, "normal.txt"), alpha(&normal, "normal.txt"));
                let marker = app.file_clipboard_text.clone();
                transfer_key(&mut app, &ctx, escape());
                assert!(app.file_clipboard.is_none());
                assert!(!app.can_paste());
                let restored = folder_frame(&mut app, &ctx, vec![]);
                assert_eq!(alpha(&restored, "cut.pdf"), alpha(&normal, "cut.pdf"));
                assert_eq!(alpha(&restored, &glyph), alpha(&normal, &glyph));
                transfer_key(&mut app, &ctx, Event::Paste(marker));
                assert!(app.active_transfer.is_none());
                assert_eq!(std::fs::read_to_string(&file).unwrap(), "unchanged content");
                assert!(folder.is_dir());
                // Copy replaces the pending cut and Escape leaves copying available.
                transfer_key(&mut app, &ctx, Event::Cut);
                transfer_key(&mut app, &ctx, Event::Copy);
                transfer_key(&mut app, &ctx, escape());
                assert_eq!(
                    app.file_clipboard.as_ref().unwrap().mode,
                    file_ops::Mode::Copy
                );
                assert!(app.can_paste());
                let copied = folder_frame(&mut app, &ctx, vec![]);
                assert_eq!(alpha(&copied, "cut.pdf"), alpha(&normal, "cut.pdf"));
            }
        }
    }
}
