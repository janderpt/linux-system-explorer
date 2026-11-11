use ab_glyph::{Font, FontRef};
use eframe::egui::{Color32, IconData};
use std::{
    fs,
    process::{Command, Stdio},
};

pub const DEFAULT_ACCENT: Color32 = Color32::from_rgb(0, 103, 192);

/// Query with a time limit before creating the native window and its desktop identity.
pub fn accent_color() -> Color32 {
    // ReadOne is the current portal API; Read supports older desktop portals.
    for method in ["ReadOne", "Read"] {
        if let Ok(output) = Command::new("gdbus")
            .args([
                "call",
                "--session",
                "--timeout",
                "1",
                "--dest",
                "org.freedesktop.portal.Desktop",
                "--object-path",
                "/org/freedesktop/portal/desktop",
                "--method",
                &format!("org.freedesktop.portal.Settings.{method}"),
                "org.freedesktop.appearance",
                "accent-color",
            ])
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .output()
            && output.status.success()
            && let Some(color) = parse_portal_color(&String::from_utf8_lossy(&output.stdout))
        {
            return color;
        }
    }
    let kde = dirs::config_dir().and_then(|path| fs::read_to_string(path.join("kdeglobals")).ok());
    kde.as_deref()
        .and_then(parse_kde_color)
        .unwrap_or(DEFAULT_ACCENT)
}

fn parse_portal_color(text: &str) -> Option<Color32> {
    let values: Vec<f64> = text
        .split(|c: char| c.is_whitespace() || "<>() ,".contains(c))
        .filter_map(|part| part.parse().ok())
        .collect();
    if values.len() != 3
        || values
            .iter()
            .any(|value| !value.is_finite() || !(0.0..=1.0).contains(value))
    {
        return None;
    }
    Some(Color32::from_rgb(
        (values[0] * 255.0).round() as u8,
        (values[1] * 255.0).round() as u8,
        (values[2] * 255.0).round() as u8,
    ))
}

fn parse_kde_color(text: &str) -> Option<Color32> {
    let mut section = "";
    let mut selection = None;
    for line in text.lines().map(str::trim) {
        if line.starts_with('[') {
            section = line;
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            let key = key.trim();
            if (section == "[General]" && key == "AccentColor")
                || (section == "[Colors:Selection]" && key == "BackgroundNormal")
            {
                let rgb: Vec<u8> = value
                    .split(',')
                    .filter_map(|part| part.trim().parse().ok())
                    .collect();
                if let [r, g, b] = rgb.as_slice() {
                    let color = Color32::from_rgb(*r, *g, *b);
                    if section == "[General]" && color != Color32::BLACK {
                        return Some(color);
                    }
                    if section == "[Colors:Selection]" {
                        selection = Some(color);
                    }
                }
            }
        }
    }
    selection
}

/// Render the embedded Explorer folder glyph into a transparent native window icon.
pub fn application_icon(accent: Color32) -> IconData {
    const SIZE: u32 = 256;
    let font = FontRef::try_from_slice(include_bytes!("../assets/fontawesome/fa-solid-900.otf"))
        .expect("fonte Font Awesome incorporada inválida");
    let character = crate::icons::glyph(crate::icons::Icon::Folder.default_name())
        .expect("ícone de pasta ausente")
        .character();
    let glyph = font.glyph_id(character).with_scale(224.0);
    let outlined = font
        .outline_glyph(glyph)
        .expect("ícone de pasta sem contorno");
    let bounds = outlined.px_bounds();
    let left = (SIZE as f32 - bounds.width()) * 0.5;
    let top = (SIZE as f32 - bounds.height()) * 0.5;
    let mut icon = IconData {
        rgba: vec![0; (SIZE * SIZE * 4) as usize],
        width: SIZE,
        height: SIZE,
    };
    outlined.draw(|x, y, coverage| {
        let x = x + left as u32;
        let y = y + top as u32;
        if x < SIZE && y < SIZE {
            let offset = ((y * SIZE + x) * 4) as usize;
            icon.rgba[offset..offset + 4].copy_from_slice(&[
                accent.r(),
                accent.g(),
                accent.b(),
                (coverage * 255.0).round() as u8,
            ]);
        }
    });
    icon
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_portal_variants_and_kde_colors_with_validated_ranges() {
        let accent = Color32::from_rgb(61, 174, 233);
        assert_eq!(
            parse_portal_color(
                "(<<(0.23921568691730499, 0.68235296010971069, 0.91372549533843994)>>,)"
            ),
            Some(accent)
        );
        assert_eq!(
            parse_portal_color("(<(0.0, 0.5, 1.0)>,)"),
            Some(Color32::from_rgb(0, 128, 255))
        );
        for text in [
            "(<(-1.0, -1.0, -1.0)>,)",
            "(<(NaN, 0.0, 1.0)>,)",
            "(<(1.1, 0.0, 0.0)>,)",
            "invalid",
            "(0.2, 0.4)",
        ] {
            assert!(parse_portal_color(text).is_none());
        }
        assert_eq!(
            parse_kde_color(
                "[Colors:Selection]\nBackgroundNormal=61,174,233\n[General]\nAccentColor=0,0,0"
            ),
            Some(accent)
        );
        assert_eq!(
            parse_kde_color(
                "[Colors:Selection]\nBackgroundNormal=61,174,233\n[General]\nAccentColor=50,80,120"
            ),
            Some(Color32::from_rgb(50, 80, 120))
        );
    }

    #[test]
    fn native_icon_uses_folder_shape_and_requested_accent_with_transparency() {
        let accent = Color32::from_rgb(61, 174, 233);
        let icon = application_icon(accent);
        assert_eq!(icon.rgba.len(), (icon.width * icon.height * 4) as usize);
        let pixels = icon.rgba.as_chunks::<4>().0;
        assert!(pixels.iter().any(|pixel| pixel[3] == 0));
        assert!(pixels.iter().any(|pixel| pixel[3] == 255));
        for pixel in pixels.iter().filter(|pixel| pixel[3] > 0) {
            assert_eq!(&pixel[..3], &[61, 174, 233]);
        }
        assert!(pixels.iter().filter(|pixel| pixel[3] > 0).count() > 20_000);
    }
}
