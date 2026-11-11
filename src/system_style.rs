use eframe::egui::{Color32, IconData};
use std::sync::OnceLock;

/// Cache the desktop accent independently from the application's light/dark mode.
pub fn accent_color(fallback: Color32) -> Color32 {
    static ACCENT: OnceLock<Option<Color32>> = OnceLock::new();
    ACCENT.get_or_init(detect_accent).unwrap_or(fallback)
}
fn detect_accent() -> Option<Color32> {
    if std::env::var("XDG_CURRENT_DESKTOP")
        .is_ok_and(|s| s.split(':').any(|s| s.eq_ignore_ascii_case("KDE")))
        && let Some(color) = dirs::config_dir()
            .and_then(|p| std::fs::read_to_string(p.join("kdeglobals")).ok())
            .and_then(|s| kde_accent(&s))
    {
        return Some(color);
    }
    zbus::block_on(async {
        let connection = zbus::connection::Builder::session()
            .method_timeout(std::time::Duration::from_millis(500))
            .build()
            .await
            .ok()?;
        let reply = connection
            .call_method(
                Some("org.freedesktop.portal.Desktop"),
                "/org/freedesktop/portal/desktop",
                Some("org.freedesktop.portal.Settings"),
                "Read",
                &("org.freedesktop.appearance", "accent-color"),
            )
            .await
            .ok()?;
        let body = reply.body();
        let value: zbus::Variant<'_> = body.deserialize().ok()?;
        let (r, g, b): (f64, f64, f64) = value.downcast().ok()?;
        if [r, g, b]
            .iter()
            .any(|v| !v.is_finite() || !(0.0..=1.0).contains(v))
        {
            return None;
        }
        Some(Color32::from_rgb(
            (r * 255.0).round() as u8,
            (g * 255.0).round() as u8,
            (b * 255.0).round() as u8,
        ))
    })
}
fn kde_accent(text: &str) -> Option<Color32> {
    let mut section = "";
    let mut selection = None;
    for line in text.lines().map(str::trim) {
        if line.starts_with('[') {
            section = line;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        if (section == "[General]" && key == "AccentColor")
            || (section == "[Colors:Selection]" && key == "BackgroundNormal")
        {
            let Ok(parts) = value
                .split(',')
                .map(|v| v.trim().parse::<u8>())
                .collect::<Result<Vec<_>, _>>()
            else {
                continue;
            };
            if let [r, g, b] = parts.as_slice() {
                let color = Color32::from_rgb(*r, *g, *b);
                if key == "AccentColor" {
                    return Some(color);
                }
                selection = Some(color);
            }
        }
    }
    selection
}

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
    fn kde_accent_prefers_explicit_color_and_falls_back_to_selection_palette() {
        let base = "[Colors:Selection]\nBackgroundNormal=61,174,233\n[General]\n";
        assert_eq!(kde_accent(base), Some(Color32::from_rgb(61, 174, 233)));
        assert_eq!(
            kde_accent(&format!("{base}AccentColor=200,80,160\n")),
            Some(Color32::from_rgb(200, 80, 160))
        );
        assert_eq!(
            kde_accent(&format!("{base}AccentColor=not a color\n")),
            Some(Color32::from_rgb(61, 174, 233))
        );
        assert_eq!(kde_accent("[General]\nAccentColor=999,0,0\n"), None);
    }

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
