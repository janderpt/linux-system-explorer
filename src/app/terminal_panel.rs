use super::*;
use crate::terminal::Session;

pub(super) struct Panel {
    sessions: Vec<Session>,
    active: usize,
    pub visible: bool,
    focus: bool,
    selection: Option<((u16, u16), (u16, u16))>,
    error: Option<String>,
}
impl Panel {
    pub(super) fn poll(&mut self) {
        for session in &mut self.sessions {
            session.poll();
        }
    }
    pub(super) fn toggle(&mut self, ctx: &egui::Context, tab: u64) {
        self.visible = !self.visible;
        self.focus = self.visible;
        if !self.visible {
            ctx.memory_mut(|m| m.surrender_focus(Self::id(tab)));
        }
    }
    fn new() -> Self {
        Self {
            sessions: vec![],
            active: 0,
            visible: true,
            focus: true,
            selection: None,
            error: None,
        }
    }
    fn open(&mut self, path: &Path, ctx: &egui::Context) -> std::io::Result<()> {
        if let Some(index) = self
            .sessions
            .iter()
            .position(|s| s.alive() && s.cwd() == path)
        {
            self.active = index;
        } else {
            self.sessions.push(Session::start(path, ctx)?);
            self.active = self.sessions.len() - 1;
        }
        self.visible = true;
        self.focus = true;
        self.selection = None;
        Ok(())
    }
    fn id(tab: u64) -> egui::Id {
        egui::Id::new(("terminal_input", tab))
    }
    pub fn focused(&self, ctx: &egui::Context, tab: u64) -> bool {
        self.visible && ctx.memory(|m| m.has_focus(Self::id(tab)))
    }
    fn copy_selection(&self, ctx: &egui::Context) {
        if let Some((a, b)) = self.selection {
            let (a, b) = if a <= b { (a, b) } else { (b, a) };
            if let Ok(parser) = self.sessions[self.active].parser.lock() {
                ctx.copy_text(parser.screen().contents_between(
                    a.0,
                    a.1,
                    b.0,
                    b.1.saturating_add(1),
                ));
            }
        }
    }
    fn show(&mut self, ctx: &egui::Context, tab: u64, keyboard: bool) {
        if !self.visible || self.sessions.is_empty() {
            return;
        }
        egui::TopBottomPanel::bottom(egui::Id::new(("terminal_dock", tab)))
            .resizable(true)
            .default_height(240.0)
            .min_height(110.0)
            .max_height((ctx.content_rect().height() * 0.7).max(110.0))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.strong(crate::i18n::tr("Terminal"));
                    if self.sessions.len() > 1 {
                        egui::ComboBox::from_id_salt(("terminal_sessions", tab))
                            .selected_text(format!("{}", self.active + 1))
                            .show_ui(ui, |ui| {
                                for (index, session) in self.sessions.iter().enumerate() {
                                    if ui
                                        .selectable_value(
                                            &mut self.active,
                                            index,
                                            format!("{} · {}", index + 1, session.cwd().display()),
                                        )
                                        .clicked()
                                    {
                                        self.focus = true;
                                        self.selection = None;
                                    }
                                }
                            });
                    }
                    ui.label(
                        egui::RichText::new(self.sessions[self.active].cwd().display().to_string())
                            .small(),
                    )
                    .on_hover_text(crate::i18n::tr(
                        "Pasta do terminal independente da navegação",
                    ));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui
                            .small_button("×")
                            .on_hover_text(crate::i18n::tr("Ocultar terminal · F4"))
                            .clicked()
                        {
                            self.visible = false;
                            ui.memory_mut(|m| m.surrender_focus(Self::id(tab)));
                        }
                    });
                });
                let font = FontId::monospace(14.0);
                let cell_width = ui.fonts_mut(|f| f.glyph_width(&font, 'M')).max(6.0);
                let cell_height = 19.0;
                let size = ui.available_size().max(Vec2::new(50.0, cell_height));
                let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
                let response = ui.interact(rect, Self::id(tab), Sense::click_and_drag());
                response.widget_info(|| {
                    egui::WidgetInfo::labeled(
                        egui::WidgetType::TextEdit,
                        true,
                        crate::i18n::tr("Terminal"),
                    )
                });
                if self.focus || response.clicked() {
                    response.request_focus();
                    self.focus = false;
                }
                let rows = (rect.height() / cell_height).floor().max(1.0) as u16;
                let cols = (rect.width() / cell_width).floor().max(1.0) as u16;
                let session = &mut self.sessions[self.active];
                session.poll();
                if let Err(error) = session.resize(rows, cols) {
                    self.error = Some(error.to_string());
                }
                if response.hovered() {
                    let scroll = ui.input(|i| i.smooth_scroll_delta.y);
                    if scroll.abs() > 0.5 {
                        if let Ok(mut p) = session.parser.lock() {
                            let offset = p.screen().scrollback() as isize
                                + (scroll / cell_height).round() as isize;
                            p.screen_mut().set_scrollback(offset.max(0) as usize);
                        }
                        self.selection = None;
                    }
                }
                let position = |point: egui::Pos2| {
                    (
                        ((point.y - rect.top()) / cell_height)
                            .floor()
                            .clamp(0.0, (rows - 1) as f32) as u16,
                        ((point.x - rect.left()) / cell_width)
                            .floor()
                            .clamp(0.0, (cols - 1) as f32) as u16,
                    )
                };
                if response.drag_started()
                    && let Some(point) = response.interact_pointer_pos()
                {
                    let p = position(point);
                    self.selection = Some((p, p));
                    response.request_focus();
                }
                if response.dragged()
                    && let Some(point) = response.interact_pointer_pos()
                    && let Some((_, end)) = &mut self.selection
                {
                    *end = position(point);
                }
                if response.clicked() {
                    self.selection = None;
                }
                if response.has_focus() && keyboard && !egui::Popup::is_any_open(ctx) {
                    ctx.send_viewport_cmd(egui::ViewportCommand::IMEAllowed(true));
                    ctx.send_viewport_cmd(egui::ViewportCommand::IMERect(rect));
                    self.input(ctx);
                }
                let session = &self.sessions[self.active];
                let parser = session.parser.lock().unwrap();
                let screen = parser.screen();
                let painter = ui.painter().with_clip_rect(rect);
                painter.rect_filled(rect, 0.0, Color32::from_rgb(20, 23, 28));
                let (height, width) = screen.size();
                for row in 0..height.get() {
                    for col in 0..width.get() {
                        let Some(cell) = screen.cell(row, col) else {
                            continue;
                        };
                        if cell.is_wide_continuation() {
                            continue;
                        }
                        let pos =
                            rect.min + Vec2::new(col as f32 * cell_width, row as f32 * cell_height);
                        let area = egui::Rect::from_min_size(
                            pos,
                            Vec2::new(
                                cell_width * if cell.is_wide() { 2.0 } else { 1.0 },
                                cell_height,
                            ),
                        );
                        let mut fg = color(cell.fgcolor(), Color32::from_rgb(225, 228, 232));
                        let mut bg = color(cell.bgcolor(), Color32::from_rgb(20, 23, 28));
                        if cell.inverse() {
                            std::mem::swap(&mut fg, &mut bg);
                        }
                        if let Some((a, b)) = self.selection {
                            let (a, b) = if a <= b { (a, b) } else { (b, a) };
                            if (row, col) >= a && (row, col) <= b {
                                bg = Color32::from_rgb(40, 80, 120);
                            }
                        }
                        painter.rect_filled(area, 0.0, bg);
                        if !cell.contents().is_empty() {
                            painter.text(
                                area.left_center(),
                                egui::Align2::LEFT_CENTER,
                                cell.contents(),
                                font.clone(),
                                fg,
                            );
                        }
                        if cell.underline() {
                            painter.line_segment(
                                [area.left_bottom(), area.right_bottom()],
                                egui::Stroke::new(1.0_f32, fg),
                            );
                        }
                    }
                }
                if response.has_focus() && !screen.hide_cursor() && screen.scrollback() == 0 {
                    let (row, col) = screen.cursor_position();
                    let pos =
                        rect.min + Vec2::new(col as f32 * cell_width, row as f32 * cell_height);
                    painter.rect_stroke(
                        egui::Rect::from_min_size(pos, Vec2::new(cell_width, cell_height)),
                        0.0,
                        egui::Stroke::new(1.0_f32, Color32::LIGHT_GRAY),
                        egui::StrokeKind::Inside,
                    );
                }
                drop(parser);
                if let Some(code) = session.exit_code {
                    painter.text(
                        rect.left_bottom(),
                        egui::Align2::LEFT_BOTTOM,
                        crate::i18n::message("Sessão encerrada · código {}", &[code.to_string()]),
                        FontId::monospace(12.0),
                        Color32::GRAY,
                    );
                }
                if let Some(error) = &self.error {
                    response.on_hover_text(error);
                }
            });
    }
    fn input(&mut self, ctx: &egui::Context) {
        let modifiers = ctx.input(|i| i.modifiers);
        let events = ctx.input_mut(|i| {
            let mut events = vec![];
            i.events.retain(|e| {
                if matches!(
                    e,
                    egui::Event::Key { .. }
                        | egui::Event::Text(_)
                        | egui::Event::Paste(_)
                        | egui::Event::Copy
                        | egui::Event::Cut
                        | egui::Event::Ime(_)
                ) {
                    events.push(e.clone());
                    false
                } else {
                    true
                }
            });
            events
        });
        let paste = events.iter().any(|e| matches!(e, egui::Event::Paste(_)));
        let key_c=events.iter().any(|e|matches!(e,egui::Event::Key{key:Key::C,pressed:true,modifiers,..} if modifiers.ctrl));
        for event in events {
            let parser = self.sessions[self.active].parser.lock().unwrap();
            let application = parser.screen().application_cursor();
            let bracketed = parser.screen().bracketed_paste();
            drop(parser);
            let bytes = match event {
                egui::Event::Copy if modifiers.shift => {
                    self.copy_selection(ctx);
                    None
                }
                egui::Event::Copy if !key_c => Some(vec![3]),
                egui::Event::Key {
                    key: Key::C,
                    pressed: true,
                    modifiers,
                    ..
                } if modifiers.ctrl && modifiers.shift => {
                    self.copy_selection(ctx);
                    None
                }
                egui::Event::Key {
                    key: Key::V,
                    modifiers,
                    ..
                } if modifiers.ctrl && paste => None,
                egui::Event::Key {
                    key,
                    pressed: true,
                    modifiers,
                    ..
                } => key_bytes(key, modifiers, application),
                egui::Event::Text(text) | egui::Event::Ime(egui::ImeEvent::Commit(text))
                    if !modifiers.ctrl || modifiers.alt =>
                {
                    let mut bytes = text.into_bytes();
                    if modifiers.alt && !modifiers.ctrl {
                        bytes.insert(0, 27);
                    }
                    Some(bytes)
                }
                egui::Event::Paste(text) => Some(if bracketed {
                    format!("\x1b[200~{}\x1b[201~", text.replace("\r\n", "\n")).into_bytes()
                } else {
                    text.replace("\r\n", "\n").replace('\n', "\r").into_bytes()
                }),
                _ => None,
            };
            if let Some(bytes) = bytes {
                self.sessions[self.active]
                    .parser
                    .lock()
                    .unwrap()
                    .screen_mut()
                    .set_scrollback(0);
                self.selection = None;
                if let Err(error) = self.sessions[self.active].write(bytes) {
                    self.error = Some(error.to_string());
                }
            }
        }
    }
}
fn key_bytes(key: Key, m: egui::Modifiers, application: bool) -> Option<Vec<u8>> {
    if m.ctrl && !m.alt {
        let name = key.name();
        if name.len() == 1 {
            let b = name.as_bytes()[0].to_ascii_uppercase();
            if (b'@'..=b'_').contains(&b) {
                return Some(vec![b & 31]);
            }
        }
        if key == Key::Space {
            return Some(vec![0]);
        }
    }
    let arrow = match key {
        Key::ArrowUp => Some('A'),
        Key::ArrowDown => Some('B'),
        Key::ArrowRight => Some('C'),
        Key::ArrowLeft => Some('D'),
        Key::Home => Some('H'),
        Key::End => Some('F'),
        _ => None,
    };
    if let Some(suffix) = arrow {
        let modifier = 1 + u8::from(m.shift) + 2 * u8::from(m.alt) + 4 * u8::from(m.ctrl);
        return Some(
            if modifier > 1 {
                format!("\x1b[1;{modifier}{suffix}")
            } else if application {
                format!("\x1bO{suffix}")
            } else {
                format!("\x1b[{suffix}")
            }
            .into_bytes(),
        );
    }
    let text = match key {
        Key::Enter => "\r",
        Key::Backspace => "\x7f",
        Key::Escape => "\x1b",
        Key::Tab if m.shift => "\x1b[Z",
        Key::Tab => "\t",
        Key::Insert => "\x1b[2~",
        Key::Delete => "\x1b[3~",
        Key::PageUp => "\x1b[5~",
        Key::PageDown => "\x1b[6~",
        Key::F1 => "\x1bOP",
        Key::F2 => "\x1bOQ",
        Key::F3 => "\x1bOR",
        Key::F4 => "\x1bOS",
        Key::F5 => "\x1b[15~",
        Key::F6 => "\x1b[17~",
        Key::F7 => "\x1b[18~",
        Key::F8 => "\x1b[19~",
        Key::F9 => "\x1b[20~",
        Key::F10 => "\x1b[21~",
        Key::F11 => "\x1b[23~",
        Key::F12 => "\x1b[24~",
        _ => return None,
    };
    Some(text.as_bytes().to_vec())
}
fn color(value: vt100::Color, default: Color32) -> Color32 {
    match value {
        vt100::Color::Default => default,
        vt100::Color::Rgb(r, g, b) => Color32::from_rgb(r, g, b),
        vt100::Color::Idx(i) => {
            const ANSI: [[u8; 3]; 16] = [
                [0, 0, 0],
                [205, 49, 49],
                [13, 188, 121],
                [229, 229, 16],
                [36, 114, 200],
                [188, 63, 188],
                [17, 168, 205],
                [229, 229, 229],
                [102, 102, 102],
                [241, 76, 76],
                [35, 209, 139],
                [245, 245, 67],
                [59, 142, 234],
                [214, 112, 214],
                [41, 184, 219],
                [255, 255, 255],
            ];
            let [r, g, b] = if i < 16 {
                ANSI[i as usize]
            } else if i < 232 {
                let n = i - 16;
                let component = |v: u8| if v == 0 { 0 } else { 55 + v * 40 };
                [component(n / 36), component(n / 6 % 6), component(n % 6)]
            } else {
                let v = 8 + (i - 232) * 10;
                [v, v, v]
            };
            Color32::from_rgb(r, g, b)
        }
    }
}
impl Explorer {
    pub(super) fn open_terminal(&mut self, path: PathBuf, ctx: &egui::Context) {
        let panel = self.terminal.get_or_insert_with(Panel::new);
        if let Err(error) = panel.open(&path, ctx) {
            self.report_error(crate::i18n::message(
                "Não foi possível abrir o terminal: {error}",
                &[error.to_string()],
            ));
        }
    }
    pub(super) fn terminal_focused(&self, ctx: &egui::Context) -> bool {
        self.terminal
            .as_ref()
            .is_some_and(|p| p.focused(ctx, self.tab_id))
    }
    pub(super) fn terminal_panel(&mut self, ctx: &egui::Context) {
        let keyboard = !self.has_dialog();
        if let Some(panel) = &mut self.terminal {
            panel.show(ctx, self.tab_id, keyboard);
        }
    }
}
#[cfg(test)]
mod tests;
