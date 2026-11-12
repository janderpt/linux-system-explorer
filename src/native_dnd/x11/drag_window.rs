//! Override-redirect badge with an empty input region: never steals a drop.
use super::*;
use x11rb::protocol::shape::{self, ConnectionExt as _};

pub(super) struct DragWindow(Window);
impl DragWindow {
    pub fn new(
        conn: &RustConnection,
        root: Window,
        image: &crate::native_dnd::visual::Image,
        scale: f32,
    ) -> Option<Self> {
        Self::create(conn, root, image, scale).ok()
    }
    fn create(
        conn: &RustConnection,
        root: Window,
        image: &crate::native_dnd::visual::Image,
        scale: f32,
    ) -> Result<Self> {
        let screen = conn
            .setup()
            .roots
            .iter()
            .find(|screen| screen.root == root)
            .ok_or("missing screen")?;
        let format = conn
            .setup()
            .pixmap_formats
            .iter()
            .find(|format| format.depth == screen.root_depth)
            .ok_or("missing image format")?;
        let visual = screen
            .allowed_depths
            .iter()
            .flat_map(|depth| &depth.visuals)
            .find(|visual| visual.visual_id == screen.root_visual)
            .ok_or("missing visual")?;
        let scale = scale.clamp(0.5, 2.0);
        let width = (f32::from(image.width) * scale).round() as u16;
        let height = (f32::from(image.height) * scale).round() as u16;
        let source =
            image::RgbaImage::from_raw(image.width.into(), image.height.into(), image.rgba.clone())
                .ok_or("invalid badge")?;
        let pixels = image::imageops::resize(
            &source,
            width.into(),
            height.into(),
            image::imageops::FilterType::Triangle,
        );
        let data = encode(&pixels, format, visual, conn.setup().image_byte_order)?;
        let window = conn.generate_id()?;
        conn.create_window(
            screen.root_depth,
            window,
            root,
            0,
            0,
            width,
            height,
            0,
            WindowClass::INPUT_OUTPUT,
            screen.root_visual,
            &CreateWindowAux::new().override_redirect(1),
        )?
        .check()?;
        let badge = Self(window);
        let result = (|| -> Result<()> {
            let mut silhouette = Vec::new();
            for (y, row) in pixels.rows().enumerate() {
                let row: Vec<_> = row.collect();
                if let (Some(first), Some(last)) = (
                    row.iter().position(|p| p[3] > 20),
                    row.iter().rposition(|p| p[3] > 20),
                ) {
                    silhouette.push(Rectangle {
                        x: first as i16,
                        y: y as i16,
                        width: (last - first + 1) as u16,
                        height: 1,
                    });
                }
            }
            conn.shape_rectangles(
                shape::SO::SET,
                shape::SK::BOUNDING,
                ClipOrdering::Y_SORTED,
                window,
                0,
                0,
                &silhouette,
            )?
            .check()?;
            conn.shape_rectangles(
                shape::SO::SET,
                shape::SK::INPUT,
                ClipOrdering::UNSORTED,
                window,
                0,
                0,
                &[],
            )?
            .check()?;
            // A background pixmap survives expose events without another paint loop.
            let pixmap = conn.generate_id()?;
            conn.create_pixmap(screen.root_depth, pixmap, window, width, height)?
                .check()?;
            let gc = conn.generate_id()?;
            let painted = (|| -> Result<()> {
                conn.create_gc(gc, pixmap, &CreateGCAux::new())?.check()?;
                let result = conn
                    .put_image(
                        ImageFormat::Z_PIXMAP,
                        pixmap,
                        gc,
                        width,
                        height,
                        0,
                        0,
                        0,
                        screen.root_depth,
                        &data,
                    )?
                    .check();
                let _ = conn.free_gc(gc);
                result?;
                conn.change_window_attributes(
                    window,
                    &ChangeWindowAttributesAux::new().background_pixmap(pixmap),
                )?
                .check()?;
                Ok(())
            })();
            let _ = conn.free_pixmap(pixmap);
            painted?;
            // Mapping is deferred until the pointer position is known.
            Ok(())
        })();
        if let Err(error) = result {
            badge.destroy(conn);
            return Err(error);
        }
        Ok(badge)
    }
    pub fn follow(&self, conn: &RustConnection, x: i16, y: i16) {
        let _ = conn.configure_window(
            self.0,
            &ConfigureWindowAux::new()
                .x(i32::from(x) + 16)
                .y(i32::from(y) + 18)
                .stack_mode(StackMode::ABOVE),
        );
        let _ = conn.map_window(self.0);
    }
    pub fn destroy(self, conn: &RustConnection) {
        let _ = conn.destroy_window(self.0);
    }
}
fn encode(
    image: &image::RgbaImage,
    format: &Format,
    visual: &Visualtype,
    order: ImageOrder,
) -> Result<Vec<u8>> {
    if ![16, 24, 32].contains(&format.bits_per_pixel) || format.scanline_pad == 0 {
        return Err("unsupported image format".into());
    }
    let bytes = usize::from(format.bits_per_pixel / 8);
    let pad = usize::from(format.scanline_pad / 8).max(1);
    let stride = (image.width() as usize * bytes).div_ceil(pad) * pad;
    let mut result = vec![0; stride * image.height() as usize];
    fn channel(value: u8, mask: u32) -> u32 {
        if mask == 0 {
            return 0;
        }
        let shift = mask.trailing_zeros();
        (((u32::from(value) * (mask >> shift) + 127) / 255) << shift) & mask
    }
    let background = image.get_pixel(image.width() / 2, 5.min(image.height() - 1));
    for (x, y, pixel) in image.enumerate_pixels() {
        let blend = |index| {
            ((u32::from(pixel[index]) * u32::from(pixel[3])
                + u32::from(background[index]) * (255 - u32::from(pixel[3]))
                + 127)
                / 255) as u8
        };
        let value = channel(blend(0), visual.red_mask)
            | channel(blend(1), visual.green_mask)
            | channel(blend(2), visual.blue_mask);
        let packed = if order == ImageOrder::LSB_FIRST {
            value.to_le_bytes()
        } else {
            value.to_be_bytes()
        };
        let packed = if order == ImageOrder::LSB_FIRST {
            &packed[..bytes]
        } else {
            &packed[4 - bytes..]
        };
        let offset = y as usize * stride + x as usize * bytes;
        result[offset..offset + bytes].copy_from_slice(packed);
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_badge_is_input_transparent_follows_the_pointer_and_is_destroyed() {
        let Ok((conn, screen)) = x11rb::connect(None) else {
            return;
        };
        let root = conn.setup().roots[screen].root;
        let image = crate::native_dnd::visual::image(
            "5 items",
            '\u{f15b}',
            egui::Color32::BLUE,
            egui::Color32::WHITE,
            egui::Color32::BLACK,
            egui::Color32::GRAY,
        )
        .unwrap();
        let badge = DragWindow::new(&conn, root, &image, 1.0).expect("native badge");
        let window = badge.0;
        badge.follow(&conn, 30, 40);
        let geometry = conn.get_geometry(window).unwrap().reply().unwrap();
        assert_eq!((geometry.x, geometry.y), (46, 58));
        assert_eq!((geometry.width, geometry.height), (240, 52));
        assert!(
            conn.shape_get_rectangles(window, shape::SK::INPUT)
                .unwrap()
                .reply()
                .unwrap()
                .rectangles
                .is_empty()
        );
        badge.follow(&conn, 130, 140);
        let geometry = conn.get_geometry(window).unwrap().reply().unwrap();
        assert_eq!((geometry.x, geometry.y), (146, 158));
        badge.destroy(&conn);
        assert!(conn.get_geometry(window).unwrap().reply().is_err());
    }
}
