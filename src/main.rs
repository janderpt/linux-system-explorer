mod activation;
mod app;
mod applications;
mod cli;
mod columns;
mod deletions;
mod desktop;
mod file_manager;
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
    let mut selection = Vec::new();
    let mut properties = false;
    let path = match cli::parse(std::env::args_os().skip(1)) {
        Ok(cli::Command::Browse(path)) => path,
        Ok(cli::Command::Reveal(paths, show_properties)) => {
            selection = paths
                .into_iter()
                .map(|path| filesystem::absolute_path(&path))
                .collect::<std::io::Result<_>>()
                .map_err(|e| eframe::Error::AppCreation(Box::new(e)))?;
            let first = &selection[0];
            let folder = first.parent().unwrap_or(first).to_owned();
            if selection.iter().any(|p| p.parent().unwrap_or(p) != folder) {
                eprintln!("Selected items must belong to the same folder");
                std::process::exit(2);
            }
            properties = show_properties;
            Some(folder)
        }
        Ok(cli::Command::FileManagerService) => {
            if let Err(error) = file_manager::serve() {
                eprintln!("{error}");
                std::process::exit(1);
            }
            return Ok(());
        }
        Ok(cli::Command::ExportIcon(path)) => {
            std::fs::write(path, system_style::APPLICATION_ICON_PNG)
                .map_err(|e| eframe::Error::AppCreation(Box::new(e)))?;
            return Ok(());
        }
        Ok(cli::Command::Version) => {
            println!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
            return Ok(());
        }
        Ok(cli::Command::Help) => {
            print!("{}", cli::HELP);
            return Ok(());
        }
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(2);
        }
    };
    let language = settings::Preferences::path()
        .and_then(|path| settings::Preferences::load(&path).ok())
        .unwrap_or_default()
        .language;
    i18n::set_locale(language.resolve());
    let initial_path = path
        .or_else(dirs::home_dir)
        .unwrap_or_else(|| PathBuf::from("/"));
    if let Err(error) = desktop::register() {
        eprintln!("Could not register desktop identity: {error}");
    }
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1180.0, 760.0])
            .with_min_inner_size([780.0, 480.0])
            .with_icon(system_style::application_icon())
            .with_app_id(desktop::APP_ID),
        ..Default::default()
    };
    eframe::run_native(
        crate::i18n::tr("Explorador de Arquivos"),
        options,
        Box::new(move |cc| {
            let mut app = app::Explorer::new(cc, initial_path);
            app.reveal_external(selection, properties);
            Ok(Box::new(app))
        }),
    )
}
