use super::*;

#[test]
fn typed_icons_and_custom_overrides_are_used_in_every_view() {
    let root = tempfile::tempdir().unwrap();
    for name in [
        "report.pdf",
        "document.docx",
        "sheet.csv",
        "package.deb",
        "photo.jpg",
    ] {
        std::fs::write(root.path().join(name), "content").unwrap();
    }
    for view in [View::Details, View::List, View::Icons] {
        let ctx = egui::Context::default();
        let mut app = Explorer::with_context(&ctx, root.path().to_owned());
        complete_navigation(&mut app);
        app.view = view;
        app.appearance.mode = icons::IconMode::Colored;
        folder_frame(&mut app, &ctx, vec![]);
        let output = folder_frame(&mut app, &ctx, vec![]);
        for glyph in [
            "file-pdf",
            "file-word",
            "file-excel",
            "box-open",
            "file-image",
        ] {
            assert!(
                text_center(
                    &output,
                    &icons::glyph(glyph).unwrap().character().to_string()
                )
                .is_some(),
                "ícone de tipo ausente: {glyph}"
            );
        }
        app.appearance
            .overrides
            .insert(Icon::FilePdf, "star".into());
        app.appearance.mode = icons::IconMode::Monochrome;
        let output = folder_frame(&mut app, &ctx, vec![]);
        assert!(
            text_center(
                &output,
                &icons::glyph("star").unwrap().character().to_string()
            )
            .is_some()
        );
        assert!(
            text_center(
                &output,
                &icons::glyph("file-pdf").unwrap().character().to_string()
            )
            .is_none()
        );
    }
}
