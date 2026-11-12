//! Bounded native drag badges use bundled fonts and never read dragged files.
use eframe::egui::Color32;
use std::sync::{Arc, OnceLock};

#[derive(Clone)]
pub(crate) struct Image {
    pub width: u16,
    pub height: u16,
    pub rgba: Vec<u8>,
}
fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
fn color(color: Color32) -> String {
    format!("#{:02x}{:02x}{:02x}", color.r(), color.g(), color.b())
}
pub(crate) fn image(
    label: &str,
    glyph: char,
    icon: Color32,
    background: Color32,
    foreground: Color32,
    border: Color32,
) -> Option<Image> {
    static FONTS: OnceLock<Arc<resvg::usvg::fontdb::Database>> = OnceLock::new();
    let fonts = FONTS.get_or_init(|| {
        let mut db = resvg::usvg::fontdb::Database::new();
        for font in eframe::egui::FontDefinitions::default()
            .font_data
            .into_values()
        {
            db.load_font_data(font.font.to_vec());
        }
        db.load_font_data(include_bytes!("../../assets/fontawesome/fa-solid-900.otf").to_vec());
        Arc::new(db)
    });
    let options = resvg::usvg::Options {
        fontdb: fonts.clone(),
        font_family: "Ubuntu".into(),
        ..Default::default()
    };
    let mut chars = label.chars().filter(|c| !c.is_control());
    let mut label: String = chars.by_ref().take(27).collect();
    // Clip long labels as well as bounding the SVG input, regardless of script width.
    if chars.next().is_some() {
        label.pop();
        label.push('…');
    }
    let svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="240" height="52"><rect x="1" y="1" width="238" height="50" rx="6" fill="{}" stroke="{}"/><text x="12" y="35" font-family="Font Awesome 7 Free" font-size="27" fill="{}">{}</text><clipPath id="label"><rect x="51" y="6" width="178" height="40"/></clipPath><text x="53" y="30" font-family="Ubuntu" font-size="13" fill="{}" clip-path="url(#label)">{}</text></svg>"#,
        color(background),
        color(border),
        color(icon),
        escape(&glyph.to_string()),
        color(foreground),
        escape(&label)
    );
    let tree = resvg::usvg::Tree::from_str(&svg, &options).ok()?;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(240, 52)?;
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::identity(),
        &mut pixmap.as_mut(),
    );
    // tiny-skia is premultiplied; both the Wayland API and egui take straight RGBA.
    let rgba = pixmap
        .pixels()
        .iter()
        .flat_map(|pixel| {
            let c = pixel.demultiply();
            [c.red(), c.green(), c.blue(), c.alpha()]
        })
        .collect();
    Some(Image {
        width: 240,
        height: 52,
        rgba,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_badges_are_bounded_use_bundled_fonts_and_escape_filenames() {
        for label in [
            "image.png",
            "5 itens sendo arrastados",
            "<image href='file:///private'>& café \"hello\".txt",
        ] {
            let image = image(
                label,
                '\u{f15b}',
                Color32::BLUE,
                Color32::WHITE,
                Color32::BLACK,
                Color32::GRAY,
            )
            .unwrap();
            assert_eq!(
                image.rgba.len(),
                usize::from(image.width) * usize::from(image.height) * 4
            );
            assert!(image.width <= 256 && image.height <= 256);
            assert!(
                image
                    .rgba
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .any(|p| p[3] > 0 && p[0] < 100 && p[1] < 100 && p[2] < 100)
            );
        }
    }
}
