use super::*;
use std::{fs, io};

type FolderResult = (PathBuf, io::Result<Vec<PathBuf>>);

pub(super) struct FolderPicker {
    current: PathBuf,
    folders: Vec<PathBuf>,
    loading: bool,
    readable: bool,
    error: Option<String>,
    requests: Sender<PathBuf>,
    results: Receiver<FolderResult>,
}

impl FolderPicker {
    fn new(initial: PathBuf, show_hidden: bool, ctx: &egui::Context) -> Self {
        let (requests, incoming) = mpsc::channel::<PathBuf>();
        let (sender, results) = mpsc::channel();
        let context = ctx.clone();
        let locale = crate::i18n::current();
        thread::spawn(move || {
            crate::i18n::set_locale(locale);
            while let Ok(path) = incoming.recv() {
                let result = list_folders(&path, show_hidden);
                if sender.send((path, result)).is_err() {
                    break;
                }
                context.request_repaint();
            }
        });
        let mut picker = Self {
            current: initial.clone(),
            folders: Vec::new(),
            loading: false,
            readable: false,
            error: None,
            requests,
            results,
        };
        picker.navigate(initial);
        picker
    }

    fn navigate(&mut self, path: PathBuf) {
        if self.loading {
            return;
        }
        if self.requests.send(path).is_ok() {
            self.loading = true;
            self.error = None;
        } else {
            self.error = Some(crate::i18n::tr("O serviço de leitura foi encerrado.").into());
        }
    }

    fn poll(&mut self) {
        while let Ok((path, result)) = self.results.try_recv() {
            self.loading = false;
            match result {
                Ok(folders) => {
                    self.current = path;
                    self.folders = folders;
                    self.readable = true;
                }
                Err(error) => {
                    self.error = Some(crate::i18n::message(
                        "Não foi possível abrir {}: {error}",
                        &[(path.display()).to_string(), (error).to_string()],
                    ))
                }
            }
        }
    }
}

fn list_folders(path: &Path, show_hidden: bool) -> io::Result<Vec<PathBuf>> {
    let mut folders = Vec::new();
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        if !show_hidden && entry.file_name().as_encoded_bytes().starts_with(b".") {
            continue;
        }
        // Follow directory links for navigation; preserve the original path in the selection.
        if entry.path().is_dir() {
            folders.push(entry.path());
        }
    }
    folders.sort_by_cached_key(|path| {
        path.file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_lowercase()
    });
    Ok(folders)
}

impl Explorer {
    pub(super) fn cancel_address_edit(&mut self, ctx: &egui::Context) {
        self.editing_address = false;
        self.focus_address = false;
        self.address = if self.is_trash() {
            "trash:///".into()
        } else {
            self.location().display().to_string()
        };
        ctx.memory_mut(|memory| {
            if let Some(id) = memory.focused() {
                memory.surrender_focus(id);
            }
        });
    }

    pub(super) fn open_folder_picker(&mut self, ctx: &egui::Context) {
        // Retain exact bytes for an unchanged non-UTF-8 address.
        let initial = if self.address == self.location().display().to_string() {
            self.location().to_owned()
        } else {
            self.expand_address()
        };
        let initial = if crate::trashbin::is_location(&initial) {
            dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"))
        } else {
            initial
        };
        self.folder_picker = Some(FolderPicker::new(initial, self.show_hidden, ctx));
    }

    pub(super) fn folder_picker_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut picker) = self.folder_picker.take() else {
            return;
        };
        picker.poll();
        let mut destination = None;
        let mut accept = false;
        let mut cancel = false;
        let modal = egui::Modal::new(egui::Id::new("folder_picker")).show(ctx, |ui| {
            ui.set_width((ctx.content_rect().width() - 80.0).clamp(350.0, 680.0));
            ui.spacing_mut().interact_size.y = 32.0;
            ui.heading(crate::i18n::tr("Selecionar pasta"));
            ui.label(crate::i18n::tr(
                "Clique nas pastas para navegar e confirme a pasta que deseja abrir.",
            ));
            ui.add_enabled_ui(!picker.loading, |ui| {
                ui.horizontal(|ui| {
                    if self
                        .navigation_button(
                            ui,
                            picker.current.parent().is_some(),
                            Icon::Up,
                            crate::i18n::tr("Pasta acima"),
                        )
                        .clicked()
                    {
                        destination = picker.current.parent().map(Path::to_owned);
                    }
                    if self
                        .icon_button(ui, Icon::Home, crate::i18n::tr("Pasta pessoal"))
                        .clicked()
                    {
                        destination = dirs::home_dir();
                    }
                    if self
                        .icon_button(ui, Icon::Drive, crate::i18n::tr("Sistema de arquivos"))
                        .clicked()
                    {
                        destination = Some(PathBuf::from("/"));
                    }
                    if self
                        .icon_button(ui, Icon::Refresh, crate::i18n::tr("Atualizar pastas"))
                        .clicked()
                    {
                        destination = Some(picker.current.clone());
                    }
                });
                if let Some(path) = breadcrumbs::show(ui, &picker.current) {
                    destination = Some(path);
                }
            });
            ui.separator();
            if let Some(error) = &picker.error {
                ui.colored_label(ui.visuals().error_fg_color, error);
            }
            let height = (ctx.content_rect().height() - 360.0).clamp(120.0, 340.0);
            egui::ScrollArea::vertical()
                .id_salt("folder_picker_list")
                .max_height(height)
                .min_scrolled_height(height)
                .show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    if picker.loading {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label(crate::i18n::tr("Carregando pastas…"));
                        });
                    } else if picker.readable && picker.folders.is_empty() {
                        ui.weak(crate::i18n::tr("Esta pasta não contém subpastas visíveis."));
                    }
                    ui.add_enabled_ui(!picker.loading, |ui| {
                        for path in &picker.folders {
                            let name = path.file_name().unwrap_or_default().to_string_lossy();
                            if self
                                .menu_item(ui, Icon::Folder, &name, "", true, false)
                                .on_hover_text(path.display().to_string())
                                .clicked()
                            {
                                destination = Some(path.clone());
                            }
                        }
                    });
                });
            ui.separator();
            ui.label(RichText::new(picker.current.display().to_string()).small())
                .on_hover_text(picker.current.display().to_string());
            ui.horizontal(|ui| {
                cancel = ui.button(crate::i18n::tr("Cancelar")).clicked();
                accept = ui
                    .add_enabled(
                        !self.loading
                            && !picker.loading
                            && picker.readable
                            && destination.is_none(),
                        egui::Button::new(crate::i18n::tr("Usar esta pasta")),
                    )
                    .clicked();
            });
        });
        if cancel || modal.should_close() {
            self.focus_address = true;
            return;
        }
        if accept {
            self.focus_address = false;
            self.navigate(picker.current, Navigation::Visit);
        } else {
            if let Some(path) = destination {
                picker.navigate(path);
            }
            self.folder_picker = Some(picker);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folder_list_only_shows_directories_and_respects_hidden_and_symlinks() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir(root.path().join("child")).unwrap();
        fs::create_dir(root.path().join(".hidden")).unwrap();
        fs::write(root.path().join("file"), "file").unwrap();
        std::os::unix::fs::symlink(root.path().join("child"), root.path().join("linked")).unwrap();
        std::os::unix::fs::symlink(root.path().join("missing"), root.path().join("broken"))
            .unwrap();
        assert_eq!(
            list_folders(root.path(), false).unwrap(),
            vec![root.path().join("child"), root.path().join("linked")]
        );
        assert_eq!(list_folders(root.path(), true).unwrap().len(), 3);
    }

    fn complete_picker(app: &mut Explorer) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        loop {
            let picker = app.folder_picker.as_mut().unwrap();
            picker.poll();
            if !picker.loading {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "Seletor não terminou a leitura"
            );
            thread::sleep(std::time::Duration::from_millis(1));
        }
    }

    #[test]
    fn address_buttons_are_centered_and_cancel_restores_breadcrumb_without_navigation() {
        use crate::app::tests::{click, complete_navigation, frame, text_center};
        let root = tempfile::tempdir().unwrap();
        let ctx = egui::Context::default();
        let mut app = Explorer::with_context(&ctx, root.path().to_owned());
        complete_navigation(&mut app);
        for width in [540.0, 940.0, 1260.0] {
            app.editing_address = true;
            app.appearance.size = 28.0;
            let draw = |app: &mut Explorer, ui: &mut egui::Ui| app.folder_path(ui).rect;
            frame(&mut app, &ctx, width, vec![], draw);
            let (output, row) = frame(&mut app, &ctx, width, vec![], draw);
            let cancel = text_center(
                &output,
                &icons::glyph("xmark").unwrap().character().to_string(),
            )
            .unwrap();
            let browse = text_center(
                &output,
                &icons::glyph("folder-tree").unwrap().character().to_string(),
            )
            .unwrap();
            assert!((cancel.y - browse.y).abs() < 1.0);
            assert!(cancel.x < browse.x && browse.x < row.right() && row.right() <= width - 7.0);
        }
        app.address = root.path().join("draft").display().to_string();
        let draw = |app: &mut Explorer, ui: &mut egui::Ui| app.folder_path(ui).rect;
        let (output, _) = frame(&mut app, &ctx, 1180.0, vec![], draw);
        let cancel = text_center(
            &output,
            &icons::glyph("xmark").unwrap().character().to_string(),
        )
        .unwrap();
        click(&mut app, &ctx, cancel, draw);
        assert!(!app.editing_address && !app.loading);
        assert_eq!(app.current, root.path());
        assert_eq!(app.address, root.path().display().to_string());
    }

    #[test]
    fn picker_button_browses_subfolders_and_only_changes_active_folder_on_confirmation() {
        use crate::app::tests::{click, click_label, complete_navigation, frame, text_center};
        let root = tempfile::tempdir().unwrap();
        let child = root.path().join("child");
        fs::create_dir(&child).unwrap();
        let ctx = egui::Context::default();
        let mut app = Explorer::with_context(&ctx, root.path().to_owned());
        complete_navigation(&mut app);
        app.editing_address = true;
        let path_row = |app: &mut Explorer, ui: &mut egui::Ui| app.folder_path(ui).rect;
        frame(&mut app, &ctx, 1180.0, vec![], path_row);
        let (output, _) = frame(&mut app, &ctx, 1180.0, vec![], path_row);
        let browse = text_center(
            &output,
            &icons::glyph("folder-tree").unwrap().character().to_string(),
        )
        .unwrap();
        click(&mut app, &ctx, browse, path_row);
        assert!(app.folder_picker.is_some());
        complete_picker(&mut app);
        let draw = |app: &mut Explorer, ui: &mut egui::Ui| {
            app.folder_picker_dialog(ui.ctx());
            ui.min_rect()
        };
        frame(&mut app, &ctx, 1180.0, vec![], draw);
        click_label(
            &mut app,
            &ctx,
            "child",
            egui::Modifiers::NONE,
            egui::PointerButton::Primary,
            draw,
        );
        complete_picker(&mut app);
        assert_eq!(app.folder_picker.as_ref().unwrap().current, child);
        assert_eq!(
            app.current,
            root.path(),
            "navegar no seletor não altera a pasta ativa"
        );
        click_label(
            &mut app,
            &ctx,
            "Cancelar",
            egui::Modifiers::NONE,
            egui::PointerButton::Primary,
            draw,
        );
        assert!(app.folder_picker.is_none() && app.editing_address && app.focus_address);
        assert_eq!(app.current, root.path());
        app.open_folder_picker(&ctx);
        complete_picker(&mut app);
        frame(&mut app, &ctx, 1180.0, vec![], draw);
        click_label(
            &mut app,
            &ctx,
            "child",
            egui::Modifiers::NONE,
            egui::PointerButton::Primary,
            draw,
        );
        complete_picker(&mut app);
        click_label(
            &mut app,
            &ctx,
            "Usar esta pasta",
            egui::Modifiers::NONE,
            egui::PointerButton::Primary,
            draw,
        );
        assert!(app.folder_picker.is_none());
        complete_navigation(&mut app);
        assert_eq!(app.current, child);
        assert!(!app.editing_address);
    }

    #[test]
    fn picker_preserves_non_utf8_paths_and_can_recover_from_a_failed_read() {
        use crate::app::tests::complete_navigation;
        use std::os::unix::ffi::OsStringExt;
        let root = tempfile::tempdir().unwrap();
        let child = root
            .path()
            .join(std::ffi::OsString::from_vec(vec![b'f', 0xff]));
        fs::create_dir(&child).unwrap();
        let ctx = egui::Context::default();
        let mut app = Explorer::with_context(&ctx, child.clone());
        complete_navigation(&mut app);
        app.open_folder_picker(&ctx);
        complete_picker(&mut app);
        assert_eq!(app.folder_picker.as_ref().unwrap().current, child);
        app.folder_picker
            .as_mut()
            .unwrap()
            .navigate(root.path().join("missing"));
        complete_picker(&mut app);
        let picker = app.folder_picker.as_ref().unwrap();
        assert!(picker.error.is_some() && picker.readable);
        assert_eq!(picker.current, child);
        app.folder_picker
            .as_mut()
            .unwrap()
            .navigate(root.path().to_owned());
        complete_picker(&mut app);
        let picker = app.folder_picker.as_ref().unwrap();
        assert!(picker.error.is_none());
        assert_eq!(picker.folders, vec![child]);
    }
}
