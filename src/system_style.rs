use eframe::egui::IconData;

/// Generated at build time from the very same SVG installed in the desktop icon theme.
pub const APPLICATION_ICON_PNG: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/application-icon.png"));

pub fn application_icon() -> IconData {
    eframe::icon_data::from_png_bytes(APPLICATION_ICON_PNG)
        .expect("invalid embedded application icon")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_icon_has_transparency_and_the_svg_brand_colors() {
        let icon = application_icon();
        assert_eq!((icon.width, icon.height), (256, 256));
        let pixels = icon.rgba.as_chunks::<4>().0;
        assert!(pixels.iter().any(|pixel| pixel[3] == 0));
        for color in [[0, 84, 158, 255], [22, 137, 223, 255], [234, 246, 255, 255]] {
            assert!(pixels.contains(&color), "missing SVG color: {color:?}");
        }
    }
}
