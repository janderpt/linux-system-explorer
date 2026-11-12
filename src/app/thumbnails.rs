use super::*;
use crate::preview::{Content, Job, Kind, thumbnail};

const LIMIT: usize = 96;
#[derive(PartialEq, Clone)]
struct Stamp {
    modified: Option<std::time::SystemTime>,
    changed: Option<std::time::SystemTime>,
    size: Option<u64>,
    inode: u64,
    device: u64,
    kind: Kind,
}
impl Stamp {
    fn for_entry(e: &Entry) -> Self {
        Self {
            modified: e.modified,
            changed: e.changed,
            size: e.size,
            inode: e.inode,
            device: e.device,
            kind: Kind::for_path(e.display_path()),
        }
    }
}
enum State {
    Loading(Job),
    Ready(egui::TextureHandle),
    Failed,
}
struct Cached {
    stamp: Stamp,
    state: State,
    used: u64,
}
#[derive(Default)]
pub(super) struct Cache {
    entries: BTreeMap<PathBuf, Cached>,
}
impl Cache {
    pub(super) fn cancel_pending(&mut self) {
        self.entries
            .retain(|_, value| matches!(value.state, State::Ready(_)));
    }
    pub(super) fn cancel_unseen(&mut self, paths: &BTreeSet<PathBuf>) {
        self.entries.retain(|path, value| {
            !matches!(value.state, State::Loading(_)) || paths.contains(path)
        });
    }
    fn texture(&mut self, entry: &Entry, ctx: &egui::Context) -> Option<egui::TextureHandle> {
        let stamp = Stamp::for_entry(entry);
        if entry.is_dir
            || !thumbnail::supported(stamp.kind)
            || !matches!(entry.mode & 0o170000, 0o100000 | 0o120000)
        {
            return None;
        }
        let frame = ctx.cumulative_frame_nr();
        if self
            .entries
            .get(&entry.path)
            .is_some_and(|value| value.stamp != stamp)
        {
            self.entries.remove(&entry.path);
        }
        if !self.entries.contains_key(&entry.path) {
            let oldest = (self.entries.len() >= LIMIT)
                .then(|| {
                    self.entries
                        .iter()
                        .filter(|(_, v)| v.used != frame)
                        .min_by_key(|(_, v)| v.used)
                        .map(|(p, _)| p.clone())
                })
                .flatten();
            if self.entries.len() >= 512 && oldest.is_none() {
                return None;
            }
            let Some(job) = thumbnail::start(
                entry.path.clone(),
                entry.display_path().to_owned(),
                stamp.kind,
                ctx,
            ) else {
                ctx.request_repaint_after(std::time::Duration::from_millis(50));
                return None;
            };
            if let Some(path) = oldest {
                self.entries.remove(&path);
            }
            self.entries.insert(
                entry.path.clone(),
                Cached {
                    stamp,
                    state: State::Loading(job),
                    used: frame,
                },
            );
        }
        let value = self.entries.get_mut(&entry.path)?;
        value.used = frame;
        if let State::Loading(job) = &value.state {
            if let Some(result) = job.poll() {
                value.state = match result {
                    Ok(Content::Image(frame)) => State::Ready(ctx.load_texture(
                        "file_thumbnail",
                        egui::ColorImage::from_rgba_unmultiplied(
                            [frame.width as usize, frame.height as usize],
                            &frame.rgba,
                        ),
                        egui::TextureOptions::LINEAR,
                    )),
                    _ => State::Failed,
                };
            } else {
                ctx.request_repaint_after(std::time::Duration::from_millis(50));
            }
        }
        if let State::Ready(texture) = &value.state {
            Some(texture.clone())
        } else {
            None
        }
    }
}
impl Explorer {
    pub(super) fn grid_picture(&mut self, ui: &mut egui::Ui, entry: &Entry, scale: f32) {
        let (area, _) = ui.allocate_exact_size(
            Vec2::new(ui.available_width(), 56.0 * scale),
            Sense::hover(),
        );
        if let Some(texture) = self.thumbnails.texture(entry, ui.ctx()) {
            let size = texture.size_vec2();
            let shown = size * (area.width() / size.x).min(area.height() / size.y);
            egui::Image::new((texture.id(), shown))
                .paint_at(ui, egui::Rect::from_center_size(area.center(), shown));
            if entry.is_symlink {
                let badge = egui::Rect::from_min_size(
                    area.left_bottom() - Vec2::new(0.0, 18.0 * scale),
                    Vec2::splat(18.0 * scale),
                );
                ui.painter()
                    .rect_filled(badge, 2.0, ui.visuals().panel_fill);
                icons::paint(
                    ui.painter(),
                    badge,
                    Icon::Link,
                    ui.visuals().text_color(),
                    &self.appearance,
                );
            }
        } else {
            self.paint_file_icon(
                ui.painter(),
                egui::Rect::from_center_size(area.center(), Vec2::splat(46.0 * scale)),
                entry,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::tests::{complete_navigation, folder_frame};
    use std::process::Command;
    fn ready(app: &mut Explorer, ctx: &egui::Context, path: &Path) -> egui::TextureId {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
        loop {
            folder_frame(app, ctx, vec![]);
            if let Some(Cached {
                state: State::Ready(texture),
                ..
            }) = app.thumbnails.entries.get(path)
            {
                return texture.id();
            }
            assert!(
                std::time::Instant::now() < deadline,
                "thumbnail did not become ready"
            );
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
    }
    #[test]
    fn grid_paints_real_thumbnails_fits_them_and_invalidates_changed_files() {
        let root = tempfile::tempdir().unwrap();
        let file = root.path().join("picture.png");
        image::RgbaImage::from_pixel(120, 60, image::Rgba([20, 50, 90, 255]))
            .save(&file)
            .unwrap();
        let ctx = egui::Context::default();
        let mut app = Explorer::with_context(&ctx, root.path().into());
        complete_navigation(&mut app);
        app.view = View::Icons;
        let texture = ready(&mut app, &ctx, &file);
        let output = folder_frame(&mut app, &ctx, vec![]);
        let picture = output
            .shapes
            .iter()
            .find_map(|s| {
                if let egui::Shape::Rect(r) = &s.shape
                    && r.fill_texture_id() == texture
                {
                    Some(r.rect)
                } else {
                    None
                }
            })
            .unwrap();
        assert!((picture.aspect_ratio() - 2.0).abs() < 0.001);
        assert!(picture.width() <= 98.0 && picture.height() <= 56.0);
        assert_eq!(
            ready(&mut app, &ctx, &file),
            texture,
            "unchanged files reuse their texture"
        );
        app.file_clipboard = Some(file_ops::Clipboard {
            id: 1,
            mode: file_ops::Mode::Cut,
            paths: vec![file.clone()],
        });
        let cut = folder_frame(&mut app, &ctx, vec![]);
        let alpha = cut
            .shapes
            .iter()
            .find_map(|s| {
                if let egui::Shape::Rect(r) = &s.shape
                    && r.fill_texture_id() == texture
                {
                    Some(r.fill.a())
                } else {
                    None
                }
            })
            .unwrap();
        assert!(alpha < 255, "thumbnails preserve the pending-cut fade");
        app.file_clipboard = None;
        image::RgbaImage::from_pixel(60, 120, image::Rgba([90, 40, 20, 255]))
            .save(&file)
            .unwrap();
        app.entries = filesystem::read_directory(root.path()).unwrap().entries;
        assert_ne!(
            ready(&mut app, &ctx, &file),
            texture,
            "changed files replace their cached thumbnail"
        );
        app.view = View::Details;
        let output = folder_frame(&mut app, &ctx, vec![]);
        assert!(!output.shapes.iter().any(|s|matches!(&s.shape,egui::Shape::Rect(r) if r.fill_texture_id()==app.thumbnails.texture(&app.entries[0],&ctx).unwrap().id())),"details keep their compact type icons");
    }
    #[test]
    fn office_and_video_thumbnails_arrive_asynchronously_in_the_icon_grid() {
        if !crate::preview::tests::office_available() {
            return;
        }
        let root = tempfile::tempdir().unwrap();
        let office = root.path().join("report.docx");
        crate::preview::tests::test_docx(&office);
        let video = root.path().join("movie.webm");
        let result = Command::new("ffmpeg")
            .args([
                "-loglevel",
                "error",
                "-f",
                "lavfi",
                "-i",
                "color=c=blue:s=64x48:r=5",
                "-t",
                "0.4",
                "-c:v",
                "libvpx",
                "-an",
                "-y",
            ])
            .arg(&video)
            .status();
        if result.is_err() {
            assert!(std::env::var_os("CI").is_none(), "ffmpeg is required in CI");
            return;
        }
        assert!(result.unwrap().success());
        let ctx = egui::Context::default();
        let mut app = Explorer::with_context(&ctx, root.path().into());
        complete_navigation(&mut app);
        app.view = View::Icons;
        for path in [&office, &video] {
            let texture = ready(&mut app, &ctx, path);
            let output = folder_frame(&mut app, &ctx, vec![]);
            assert!(output.shapes.iter().any(
                |s| matches!(&s.shape, egui::Shape::Rect(r) if r.fill_texture_id() == texture)
            ));
        }
        assert!(app.notice_toasts.is_empty());
    }
    #[test]
    fn missing_document_tools_keep_office_icons_without_notices() {
        const CHILD: &str = "LSE_TEST_THUMBNAIL_NO_TOOLS";
        if std::env::var_os(CHILD).is_none() {
            let empty = tempfile::tempdir().unwrap();
            assert!(Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "app::thumbnails::tests::missing_document_tools_keep_office_icons_without_notices", "--nocapture"])
                .env(CHILD, "1").env("PATH", empty.path()).status().unwrap().success());
            return;
        }
        let root = tempfile::tempdir().unwrap();
        let file = root.path().join("report.docx");
        crate::preview::tests::test_docx(&file);
        let ctx = egui::Context::default();
        let mut app = Explorer::with_context(&ctx, root.path().into());
        complete_navigation(&mut app);
        app.view = View::Icons;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        loop {
            let output = folder_frame(&mut app, &ctx, vec![]);
            if matches!(
                app.thumbnails.entries.get(&file).map(|v| &v.state),
                Some(State::Failed)
            ) {
                assert!(
                    crate::app::tests::text_center(
                        &output,
                        &icons::glyph("file-word").unwrap().character().to_string()
                    )
                    .is_some()
                );
                assert!(app.notice_toasts.is_empty());
                break;
            }
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
    }
    #[test]
    fn unsupported_and_broken_files_keep_icons_without_notices_and_refresh_permits_retry() {
        let root = tempfile::tempdir().unwrap();
        for name in ["bad.png", "bad.mp4", "bad.docx", "report.unknown"] {
            std::fs::write(root.path().join(name), [0_u8, 255, 0, 255]).unwrap();
        }
        // A sparse, oversized document exercises the Office input-limit fallback
        // without relying on LibreOffice's tolerant text-format autodetection.
        std::fs::File::options()
            .write(true)
            .open(root.path().join("bad.docx"))
            .unwrap()
            .set_len(257 * 1024 * 1024)
            .unwrap();
        let ctx = egui::Context::default();
        let mut app = Explorer::with_context(&ctx, root.path().into());
        complete_navigation(&mut app);
        app.view = View::Icons;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
        while !["bad.png", "bad.mp4", "bad.docx"].iter().all(|name| {
            matches!(
                app.thumbnails
                    .entries
                    .get(&root.path().join(name))
                    .map(|v| &v.state),
                Some(State::Failed)
            )
        }) {
            folder_frame(&mut app, &ctx, vec![]);
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        assert!(
            !app.thumbnails
                .entries
                .contains_key(&root.path().join("report.unknown"))
        );
        assert!(app.notice_toasts.is_empty());
        let output = folder_frame(&mut app, &ctx, vec![]);
        assert!(
            crate::app::tests::text_center(
                &output,
                &icons::glyph("file-image").unwrap().character().to_string()
            )
            .is_some()
        );
        app.thumbnails.cancel_pending();
        assert!(
            app.thumbnails.entries.is_empty(),
            "refresh permits retrying failed thumbnails"
        );
    }
}

#[cfg(test)]
mod capacity_tests {
    use super::*;
    #[test]
    fn many_visible_thumbnails_are_not_evicted_in_a_loop_and_the_hard_limit_is_respected() {
        let root = tempfile::tempdir().unwrap();
        let file = root.path().join("image.png");
        image::RgbaImage::from_pixel(4, 4, image::Rgba([20, 40, 60, 255]))
            .save(&file)
            .unwrap();
        let entry = filesystem::read_entry(&file).unwrap();
        let ctx = egui::Context::default();
        let texture = ctx.load_texture(
            "fixture",
            egui::ColorImage::filled([1, 1], Color32::WHITE),
            egui::TextureOptions::LINEAR,
        );
        let mut cache = Cache::default();
        for i in 0..512 {
            cache.entries.insert(
                root.path().join(format!("{i}.png")),
                Cached {
                    stamp: Stamp::for_entry(&entry),
                    state: State::Ready(texture.clone()),
                    used: ctx.cumulative_frame_nr(),
                },
            );
        }
        assert!(cache.texture(&entry, &ctx).is_none());
        assert_eq!(cache.entries.len(), 512);
        let oldest = root.path().join("0.png");
        cache.entries.get_mut(&oldest).unwrap().used = u64::MAX;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while !cache.entries.contains_key(&file) {
            cache.texture(&entry, &ctx);
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        assert!(!cache.entries.contains_key(&oldest));
        assert_eq!(cache.entries.len(), 512);
    }
}
