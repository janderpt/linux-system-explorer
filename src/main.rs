mod app;
mod applications;
mod columns;
mod deletions;
mod desktop;
mod filesystem;
mod filetypes;
mod i18n;
mod icons;
mod native_dnd;
mod navigation;
mod renames;
mod settings;
mod sidebar;
mod system_style;
mod transfers;
mod trashbin;

use std::path::PathBuf;

fn main() -> eframe::Result {
    let language = settings::Preferences::path()
        .and_then(|path| settings::Preferences::load(&path).ok())
        .unwrap_or_default()
        .language;
    i18n::set_locale(language.resolve());
    let initial_path = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .or_else(dirs::home_dir)
        .unwrap_or_else(|| PathBuf::from("/"));
    let accent = system_style::accent_color();
    if let Err(error) = desktop::register(accent) {
        eprintln!("Could not register desktop identity: {error}");
    }
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1180.0, 760.0])
            .with_min_inner_size([780.0, 480.0])
            .with_icon(system_style::application_icon(accent))
            .with_app_id(desktop::APP_ID),
        ..Default::default()
    };
    eframe::run_native(
        crate::i18n::tr("Explorador de Arquivos"),
        options,
        Box::new(move |cc| Ok(Box::new(app::Explorer::new(cc, initial_path)))),
    )
}
