//! Native file exchange. Only local file URIs are accepted. Native incoming
//! transfers negotiate Copy/Move; our transfer engine owns incoming filesystem changes.
use eframe::egui::{self, Pos2};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::path::PathBuf;
pub(crate) mod policy;
pub(crate) mod visual;
mod x11;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Copy,
    Move,
}
#[derive(Clone, Copy, Debug)]
pub struct Actions {
    pub copy: bool,
    pub move_: bool,
}
impl Actions {
    pub const BOTH: Self = Self {
        copy: true,
        move_: true,
    };
    pub fn only(action: Action) -> Self {
        Self {
            copy: action == Action::Copy,
            move_: action == Action::Move,
        }
    }
    pub fn allows(self, action: Action) -> bool {
        match action {
            Action::Copy => self.copy,
            Action::Move => self.move_,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Completion {
    Wayland(u64),
    X11(u64),
}
pub fn modifier_action(modifiers: egui::Modifiers) -> Option<Action> {
    if modifiers.ctrl || modifiers.command {
        Some(Action::Copy)
    } else if modifiers.shift {
        Some(Action::Move)
    } else {
        None
    }
}

pub enum Event {
    Hover(Pos2),
    Left,
    Dropped {
        position: Pos2,
        paths: Vec<PathBuf>,
        internal: bool,
        action: Action,
        allowed: Actions,
        completion: Option<Completion>,
    },
    Started,
    Action(Option<Action>),
    Ended,
    Error(String),
}

pub struct Native {
    backend: Backend,
    outgoing: Vec<PathBuf>,
    outgoing_active: bool,
}
enum Backend {
    Wayland(guth_wayland_dnd::FileDnd),
    X11(x11::Bridge),
}

impl Native {
    pub fn attach(cc: &eframe::CreationContext<'_>) -> Result<Self, String> {
        let context = cc.egui_ctx.clone();
        let backend = if let Some(bridge) =
            guth_wayland_dnd::FileDnd::attach(cc, move || context.request_repaint())
        {
            bridge.set_incoming_drop_policy(true, guth_wayland_dnd::DropAction::Ask);
            // Both actions stay available throughout the native gesture. The
            // compositor/target can then honor modifier changes outside our window.
            bridge.set_outgoing_drag_action(None);
            Backend::Wayland(bridge)
        } else {
            let window =
                match cc
                    .window_handle()
                    .map_err(|error| error.to_string())?
                    .as_raw()
                {
                    RawWindowHandle::Xlib(handle) => handle.window as u32,
                    RawWindowHandle::Xcb(handle) => handle.window.get(),
                    _ => return Err(crate::i18n::tr(
                        "Não foi possível iniciar o arrastar e soltar nativo nesta sessão gráfica.",
                    )
                    .into()),
                };
            Backend::X11(x11::Bridge::attach(window, cc.egui_ctx.clone()))
        };
        Ok(Self {
            backend,
            outgoing: Vec::new(),
            outgoing_active: false,
        })
    }

    pub fn arm(&mut self, paths: Vec<PathBuf>, size: egui::Vec2, image: Option<visual::Image>) {
        self.outgoing = paths.clone();
        match &self.backend {
            Backend::Wayland(bridge) => {
                let icon = image.and_then(|image| {
                    guth_wayland_dnd::DragIcon::new(image.width, image.height, image.rgba)
                });
                bridge.arm_drag(paths, icon, (size.x as f64, size.y as f64))
            }
            Backend::X11(bridge) => bridge.arm(paths, image),
        }
    }
    pub fn cancel(&self) {
        match &self.backend {
            Backend::Wayland(bridge) => bridge.cancel_armed_drag(),
            Backend::X11(bridge) => bridge.cancel(),
        }
    }
    pub fn accept(&self, accepted: bool) {
        if let Backend::X11(bridge) = &self.backend {
            bridge.accept(accepted);
        }
    }
    pub fn incoming_policy(
        &self,
        accepted: bool,
        move_available: bool,
        _modifiers: egui::Modifiers,
    ) {
        self.accept(accepted);
        if let Backend::Wayland(bridge) = &self.backend {
            // Negotiate Ask before the drop when the source supports it, so the
            // final action can follow the user's explicit choice afterward.
            bridge.set_incoming_drop_policy(move_available, guth_wayland_dnd::DropAction::Ask);
        }
    }
    pub fn complete(&self, completion: Completion, action: Option<Action>) {
        match (&self.backend, completion) {
            (Backend::Wayland(bridge), Completion::Wayland(id)) => bridge
                .complete_drop_with_action(
                    id,
                    action.map(|action| match action {
                        Action::Copy => guth_wayland_dnd::DropCompletion::Copy,
                        Action::Move => guth_wayland_dnd::DropCompletion::Move,
                    }),
                ),
            (Backend::X11(bridge), Completion::X11(id)) => bridge.complete(id, action),
            _ => (),
        }
    }
    pub fn scale(&self, scale: f32) {
        if let Backend::X11(bridge) = &self.backend {
            bridge.scale(scale);
        }
    }
    pub fn poll(&mut self, modifiers: egui::Modifiers) -> Vec<Event> {
        match &self.backend {
            Backend::X11(bridge) => bridge.poll(),
            Backend::Wayland(bridge) => {
                use guth_wayland_dnd::{DndEvent, DropAction};
                bridge.poll().into_iter().filter_map(|event| match event {
                    DndEvent::Hovered { x,y } => Some(Event::Hover(egui::pos2(x,y))),
                    DndEvent::Left => Some(Event::Left),
                    DndEvent::IncomingAction {action}=>Some(Event::Action(action.map(|action|match action {DropAction::Copy=>Action::Copy,DropAction::Move=>Action::Move,DropAction::Ask=>modifier_action(modifiers).unwrap_or(Action::Copy)}))),
                    DndEvent::Dropped { x,y,paths,action,completion_id,actions } => {
                        let chosen=match action {DropAction::Copy=>Action::Copy,DropAction::Move=>Action::Move,DropAction::Ask=>modifier_action(modifiers).filter(|action| if *action==Action::Copy { actions.allow_copy } else { actions.allow_move }).unwrap_or(if actions.allow_copy {Action::Copy} else {Action::Move})};
                        if (chosen==Action::Move && !actions.allow_move) || (chosen==Action::Copy && !actions.allow_copy) {
                            if let Some(id) = completion_id { bridge.complete_drop(id,false); }
                            return Some(Event::Error(crate::i18n::tr("O aplicativo de origem não permite a operação solicitada para os arquivos arrastados.").into()));
                        }
                        let internal = self.outgoing_active && paths == self.outgoing;
                        let allowed = if action == DropAction::Ask { Actions { copy: actions.allow_copy, move_: actions.allow_move } } else { Actions::only(chosen) };
                        Some(Event::Dropped { position:egui::pos2(x,y),paths,internal,action:chosen,allowed,completion:completion_id.map(Completion::Wayland) })
                    }
                    DndEvent::DragStarted => { self.outgoing_active=true; Some(Event::Started) },
                    DndEvent::DragEnded { .. } => { self.outgoing_active=false; self.outgoing.clear(); Some(Event::Ended) },
                    DndEvent::WorkerUnavailable { reason } | DndEvent::DragStartFailed { reason } | DndEvent::DropFailed { reason } => {self.outgoing_active=false;self.outgoing.clear();Some(Event::Error(reason))},
                    _ => None,
                }).collect()
            }
        }
    }
}

const MAX_BYTES: usize = 16 * 1024 * 1024;
const MAX_PATHS: usize = 20_000;
fn uri_list(paths: &[PathBuf]) -> Option<Vec<u8>> {
    if paths.is_empty() || paths.len() > MAX_PATHS {
        return None;
    }
    let mut bytes = Vec::new();
    for path in paths {
        bytes.extend(url::Url::from_file_path(path).ok()?.as_str().as_bytes());
        bytes.extend(b"\r\n");
        if bytes.len() > MAX_BYTES {
            return None;
        }
    }
    Some(bytes)
}
fn parse_uris(bytes: &[u8]) -> Option<Vec<PathBuf>> {
    if bytes.len() > MAX_BYTES {
        return None;
    }
    let text = std::str::from_utf8(bytes).ok()?;
    let mut paths = Vec::new();
    for line in text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
    {
        let uri = url::Url::parse(line).ok()?;
        if uri.scheme() != "file"
            || uri
                .host_str()
                .is_some_and(|host| !host.is_empty() && host != "localhost")
        {
            return None;
        }
        let path = uri.to_file_path().ok()?;
        if !paths.contains(&path) {
            paths.push(path);
        }
        if paths.len() > MAX_PATHS {
            return None;
        }
    }
    (!paths.is_empty()).then_some(paths)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn local_uris_round_trip_spaces_unicode_and_raw_bytes_but_reject_remote_payloads() {
        use std::{ffi::OsString, os::unix::ffi::OsStringExt};
        let paths = vec![
            PathBuf::from("/tmp/space # ? ' café.txt"),
            PathBuf::from(OsString::from_vec(b"/tmp/raw-\xff".to_vec())),
        ];
        assert_eq!(parse_uris(&uri_list(&paths).unwrap()).unwrap(), paths);
        for data in [
            b"https://example.com/file".as_slice(),
            b"file://remote/tmp/file",
            b"garbage",
            b"# comment\n",
        ] {
            assert!(parse_uris(data).is_none());
        }
    }
}
