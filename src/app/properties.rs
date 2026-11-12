use super::*;

impl Explorer {
    pub(super) fn properties_dialog(&mut self, ctx: &egui::Context) {
        if let Some(entry) = self.properties.clone() {
            let folder = entry.is_dir && !entry.is_symlink;
            let mut totals = None;
            let mut calculating = false;
            let mut failed = false;
            if folder {
                if self
                    .properties_usage
                    .as_ref()
                    .is_none_or(|job| job.path != entry.path)
                {
                    self.properties_usage =
                        Some(crate::disk_usage::Job::start(entry.path.clone(), ctx));
                }
                let job = self.properties_usage.as_mut().unwrap();
                let was_done = job.progress.done;
                job.poll();
                let error = (!was_done).then(|| job.progress.error.clone()).flatten();
                totals = Some(job.progress.totals.clone());
                calculating = !job.progress.done;
                failed = job.progress.error.is_some();
                if let Some(error) = error {
                    self.report_error(crate::i18n::message(
                        "Não foi possível calcular o tamanho: {}",
                        &[error],
                    ));
                }
                if calculating {
                    ctx.request_repaint_after(std::time::Duration::from_millis(100));
                }
            } else {
                self.properties_usage = None;
            }
            let allocation_hint = crate::i18n::tr(
                "Blocos alocados pelo Linux (512 bytes por bloco). Inclui as pastas e conta links físicos uma única vez; links simbólicos não são percorridos. Compressão e blocos compartilhados dependem do sistema de arquivos.",
            );
            let mut open = true;
            egui::Window::new(crate::i18n::tr("Propriedades"))
                .collapsible(false)
                .resizable(false)
                .default_width(420.0)
                .open(&mut open)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        self.file_icon(ui, &entry, 38.0);
                        ui.label(RichText::new(&entry.name).strong().size(18.0));
                    });
                    ui.separator();
                    ui.label(crate::i18n::message(
                        "Tipo: {}",
                        &[(entry.kind()).to_string()],
                    ));
                    ui.label(crate::i18n::message(
                        "Local: {}",
                        &[entry
                            .trash
                            .as_ref()
                            .map(|record| record.original_path.as_path())
                            .unwrap_or(&entry.path)
                            .display()
                            .to_string()],
                    ));
                    if failed {
                        ui.label(crate::i18n::message(
                            "Tamanho: {}",
                            &[crate::i18n::tr("Indisponível").into()],
                        ));
                        ui.label(crate::i18n::message(
                            "Tamanho em disco: {}",
                            &[crate::i18n::tr("Indisponível").into()],
                        ));
                    } else {
                        let size = totals.as_ref().map(|t| t.bytes).or(entry.size).unwrap_or(0);
                        let allocated = totals
                            .as_ref()
                            .map_or_else(|| entry.allocated(), |t| t.allocated);
                        ui.label(crate::i18n::message(
                            "Tamanho: {} ({size} bytes)",
                            &[filesystem::format_size(size), size.to_string()],
                        ));
                        ui.label(crate::i18n::message(
                            "Tamanho em disco: {} ({} bytes)",
                            &[filesystem::format_size(allocated), allocated.to_string()],
                        ))
                        .on_hover_text(allocation_hint);
                        if let Some(totals) = &totals {
                            ui.label(crate::i18n::message(
                                "Contém: {} arquivos/links e {} pastas",
                                &[totals.files.to_string(), totals.directories.to_string()],
                            ));
                            if totals.skipped > 0 {
                                ui.weak(crate::i18n::message(
                                    "Totais parciais: {} itens não puderam ser lidos.",
                                    &[totals.skipped.to_string()],
                                ));
                            }
                        }
                        if calculating {
                            ui.horizontal(|ui| {
                                ui.spinner();
                                ui.weak(crate::i18n::tr("Calculando tamanho…"));
                            });
                        }
                    }
                    ui.label(crate::i18n::message(
                        "Modificado: {}",
                        &[(modified_label(&entry)).to_string()],
                    ));
                    if entry.is_symlink
                        && let Ok(target) = std::fs::read_link(&entry.path)
                    {
                        ui.label(crate::i18n::message(
                            "Destino do link: {}",
                            &[(target.display()).to_string()],
                        ));
                    }
                    if ui
                        .button(if entry.trash.is_some() {
                            crate::i18n::tr("Copiar caminho original")
                        } else {
                            crate::i18n::tr("Copiar caminho")
                        })
                        .clicked()
                    {
                        self.file_clipboard = None;
                        ctx.copy_text(
                            entry
                                .trash
                                .as_ref()
                                .map(|record| record.original_path.as_path())
                                .unwrap_or(&entry.path)
                                .display()
                                .to_string(),
                        );
                    }
                });
            if !open {
                self.properties = None;
                self.properties_usage = None;
            }
        } else {
            self.properties_usage = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::tests::{complete_navigation, text_center};
    fn render(app: &mut Explorer, ctx: &egui::Context) -> egui::FullOutput {
        ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    Vec2::new(1180.0, 760.0),
                )),
                ..Default::default()
            },
            |ctx| app.properties_dialog(ctx),
        )
    }
    #[test]
    fn properties_show_apparent_and_allocated_sizes_for_sparse_files_and_folder_contents() {
        let root = tempfile::tempdir().unwrap();
        let file = root.path().join("sparse");
        std::fs::File::create(&file)
            .unwrap()
            .set_len(16 * 1024 * 1024)
            .unwrap();
        let ctx = egui::Context::default();
        let mut app = Explorer::with_context(&ctx, root.path().into());
        complete_navigation(&mut app);
        let entry = filesystem::read_entry(&file).unwrap();
        app.properties = Some(entry.clone());
        render(&mut app, &ctx);
        let out = render(&mut app, &ctx);
        assert!(
            text_center(
                &out,
                &crate::i18n::message(
                    "Tamanho em disco: {} ({} bytes)",
                    &[
                        filesystem::format_size(entry.allocated()),
                        entry.allocated().to_string()
                    ]
                )
            )
            .is_some()
        );
        assert!(
            text_center(
                &out,
                &crate::i18n::message(
                    "Tamanho: {} ({size} bytes)",
                    &[
                        filesystem::format_size(16 * 1024 * 1024),
                        (16 * 1024 * 1024).to_string()
                    ]
                )
            )
            .is_some()
        );
        app.properties = Some(filesystem::read_entry(root.path()).unwrap());
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            render(&mut app, &ctx);
            if app
                .properties_usage
                .as_ref()
                .is_some_and(|j| j.progress.done)
            {
                break;
            }
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        assert_eq!(
            app.properties_usage.as_ref().unwrap().progress.totals.bytes,
            16 * 1024 * 1024
        );
        let out = render(&mut app, &ctx);
        assert!(
            text_center(
                &out,
                &crate::i18n::message(
                    "Contém: {} arquivos/links e {} pastas",
                    &["1".into(), "0".into()]
                )
            )
            .is_some()
        );
        app.properties = None;
        render(&mut app, &ctx);
        assert!(
            app.properties_usage.is_none(),
            "closing releases the worker"
        );
    }
}
