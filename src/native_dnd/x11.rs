//! XDND v5 on a separate connection. A validated XdndProxy receives precise
//! native positions without competing with winit's X11 event reader.
use super::{Action, Completion, Event as AppEvent, MAX_BYTES, parse_uris, uri_list};
use eframe::egui;
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};
use x11rb::{
    connection::Connection,
    protocol::{Event, xproto::*},
    rust_connection::RustConnection,
    wrapper::ConnectionExt as _,
};

x11rb::atom_manager! {
    Atoms: AtomsCookie {
        XdndAware, XdndProxy, XdndEnter, XdndLeave, XdndPosition, XdndStatus,
        XdndDrop, XdndFinished, XdndSelection, XdndActionCopy, XdndActionMove, XdndActionList, XdndTypeList,
        TARGETS, INCR, URI: b"text/uri-list", TRANSFER: b"_LSE_DND_TRANSFER",
    }
}

enum Command {
    Arm(Vec<PathBuf>),
    Cancel,
    Accept(bool),
    Scale(f32),
    Complete(u64, Option<Action>),
    Stop,
}
pub(super) struct Bridge {
    commands: mpsc::Sender<Command>,
    events: mpsc::Receiver<AppEvent>,
    worker: Option<thread::JoinHandle<()>>,
}
impl Bridge {
    pub fn attach(window: Window, ctx: egui::Context) -> Self {
        Self::with_selection(window, ctx, None)
    }
    fn with_selection(window: Window, ctx: egui::Context, selection: Option<Atom>) -> Self {
        let (commands, incoming) = mpsc::channel();
        let (sender, events) = mpsc::channel();
        let worker = thread::spawn(move || {
            if let Err(error) = run(window, incoming, &sender, &ctx, selection) {
                let _ = sender.send(AppEvent::Error(format!("XDND: {error}")));
                ctx.request_repaint();
            }
        });
        Self {
            commands,
            events,
            worker: Some(worker),
        }
    }
    pub fn arm(&self, paths: Vec<PathBuf>) {
        let _ = self.commands.send(Command::Arm(paths));
    }
    pub fn cancel(&self) {
        let _ = self.commands.send(Command::Cancel);
    }
    pub fn accept(&self, yes: bool) {
        let _ = self.commands.send(Command::Accept(yes));
    }
    pub fn complete(&self, id: u64, action: Option<Action>) {
        let _ = self.commands.send(Command::Complete(id, action));
    }
    pub fn scale(&self, scale: f32) {
        let _ = self.commands.send(Command::Scale(scale));
    }
    pub fn poll(&self) -> Vec<AppEvent> {
        self.events.try_iter().collect()
    }
}
impl Drop for Bridge {
    fn drop(&mut self) {
        let _ = self.commands.send(Command::Stop);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

struct Outgoing {
    bytes: Vec<u8>,
    started: bool,
    target: Window,
    receiver: Window,
    accepted: bool,
    dropped: Option<Instant>,
    released: Option<Instant>,
    requested: Action,
}
struct Incoming {
    source: Window,
    version: u32,
    action: Option<Action>,
    allow_move: bool,
    supported: bool,
    position: egui::Pos2,
    data: Vec<u8>,
    incremental: bool,
    deadline: Instant,
}
struct PendingFinish {
    source: Window,
    action: Action,
    deadline: Instant,
}
struct Chunk {
    data: Vec<u8>,
    offset: usize,
    deadline: Instant,
}
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
struct Worker {
    conn: RustConnection,
    root: Window,
    window: Window,
    proxy: Window,
    atoms: Atoms,
    sender: mpsc::Sender<AppEvent>,
    ctx: egui::Context,
    scale: f32,
    accepted: bool,
    incoming: Option<Incoming>,
    outgoing: Option<Outgoing>,
    chunks: BTreeMap<(Window, Atom), Chunk>,
    finishes: BTreeMap<u64, PendingFinish>,
    next_finish: u64,
}
impl Worker {
    fn action_atom(&self, action: Action) -> Atom {
        match action {
            Action::Copy => self.atoms.XdndActionCopy,
            Action::Move => self.atoms.XdndActionMove,
        }
    }
    fn incoming_action(&self) -> Option<Action> {
        let input = self.incoming.as_ref()?;
        // The source owns the keyboard grab; its negotiated action is authoritative.
        let action = input.action?;
        (action == Action::Copy || input.allow_move).then_some(action)
    }
    fn complete(&mut self, id: u64, action: Option<Action>) {
        if let Some(pending) = self.finishes.remove(&id) {
            let success = action == Some(pending.action);
            self.message(
                pending.source,
                self.atoms.XdndFinished,
                [
                    self.window,
                    u32::from(success),
                    if success {
                        self.action_atom(pending.action)
                    } else {
                        0
                    },
                    0,
                    0,
                ],
            );
        }
    }

    fn emit(&self, event: AppEvent) {
        let _ = self.sender.send(event);
        self.ctx.request_repaint();
    }
    fn message(&self, receiver: Window, event: Atom, data: [u32; 5]) {
        let _ = self.conn.send_event(
            false,
            receiver,
            EventMask::NO_EVENT,
            ClientMessageEvent::new(32, receiver, event, data),
        );
    }
    fn target_message(&self, receiver: Window, target: Window, event: Atom, data: [u32; 5]) {
        let _ = self.conn.send_event(
            false,
            receiver,
            EventMask::NO_EVENT,
            ClientMessageEvent::new(32, target, event, data),
        );
    }
    fn property(&self, window: Window, property: Atom, kind: Atom) -> Vec<u32> {
        self.conn
            .get_property(false, window, property, kind, 0, 64)
            .ok()
            .and_then(|cookie| cookie.reply().ok())
            .and_then(|reply| reply.value32().map(Iterator::collect))
            .unwrap_or_default()
    }
    fn status(&self) {
        if let Some(input) = &self.incoming {
            let action = self.incoming_action();
            let yes = input.supported && self.accepted && action.is_some();
            self.message(
                input.source,
                self.atoms.XdndStatus,
                [
                    self.window,
                    if yes { 3 } else { 2 },
                    0,
                    0,
                    if yes {
                        self.action_atom(self.incoming_action().unwrap_or(Action::Copy))
                    } else {
                        0
                    },
                ],
            );
        }
    }
    fn incoming_done(&mut self, yes: bool) {
        if let Some(input) = self.incoming.take() {
            self.message(
                input.source,
                self.atoms.XdndFinished,
                [
                    self.window,
                    u32::from(yes),
                    if yes {
                        self.action_atom(self.incoming_action().unwrap_or(Action::Copy))
                    } else {
                        0
                    },
                    0,
                    0,
                ],
            );
        }
        self.emit(AppEvent::Left);
    }
    fn deliver(&mut self, bytes: Vec<u8>) {
        let Some(paths) = parse_uris(&bytes) else {
            self.incoming_done(false);
            self.emit(AppEvent::Error(
                crate::i18n::tr("O arraste não contém uma lista válida de arquivos locais.").into(),
            ));
            return;
        };
        let Some(action) = self.incoming_action() else {
            self.incoming_done(false);
            return;
        };
        if let Some(input) = self.incoming.take() {
            self.next_finish = self.next_finish.wrapping_add(1);
            let id = self.next_finish;
            self.finishes.insert(
                id,
                PendingFinish {
                    source: input.source,
                    action,
                    deadline: Instant::now() + Duration::from_secs(300),
                },
            );
            self.emit(AppEvent::Dropped {
                position: input.position,
                paths,
                internal: input.source == self.proxy,
                action,
                completion: Some(Completion::X11(id)),
            });
            self.emit(AppEvent::Left);
        }
    }

    fn find_target(&self, root_x: i16, root_y: i16) -> (Window, Window) {
        let mut window = self.root;
        let mut candidates = Vec::new();
        for _ in 0..64 {
            let Ok(cookie) = self
                .conn
                .translate_coordinates(self.root, window, root_x, root_y)
            else {
                break;
            };
            let Ok(reply) = cookie.reply() else { break };
            if reply.child == 0 || reply.child == window {
                break;
            };
            window = reply.child;
            candidates.push(window);
        }
        for target in candidates.into_iter().rev() {
            let proxy = self
                .property(target, self.atoms.XdndProxy, AtomEnum::WINDOW.into())
                .first()
                .copied()
                .unwrap_or(target);
            let receiver = if proxy != target
                && self
                    .property(proxy, self.atoms.XdndProxy, AtomEnum::WINDOW.into())
                    .first()
                    == Some(&proxy)
            {
                proxy
            } else {
                target
            };
            if !self
                .property(receiver, self.atoms.XdndAware, AtomEnum::ATOM.into())
                .is_empty()
            {
                return (target, receiver);
            }
        }
        (0, 0)
    }
    fn outgoing_end(&mut self) {
        if let Some(output) = self.outgoing.take()
            && output.started
        {
            if output.target != 0 && output.dropped.is_none() {
                self.target_message(
                    output.receiver,
                    output.target,
                    self.atoms.XdndLeave,
                    [self.proxy, 0, 0, 0, 0],
                );
            }
            self.emit(AppEvent::Ended);
        }
    }
    fn sample_pointer(&mut self) {
        let Some(output) = &self.outgoing else { return };
        if let Some(since) = output.dropped {
            if since.elapsed() > Duration::from_secs(300) {
                self.outgoing_end();
            }
            return;
        }
        // Freeze the release position/action. Wait for the target's reply to the
        // final Position before sending Drop; never use a stale Copy/Move reply.
        if let Some(released) = output.released {
            if output.accepted {
                self.target_message(
                    output.receiver,
                    output.target,
                    self.atoms.XdndDrop,
                    [self.proxy, 0, x11rb::CURRENT_TIME, 0, 0],
                );
                self.outgoing.as_mut().unwrap().dropped = Some(Instant::now());
            } else if released.elapsed() > Duration::from_millis(500) {
                self.outgoing_end();
            }
            return;
        }
        let Some(pointer) = self
            .conn
            .query_pointer(self.window)
            .ok()
            .and_then(|c| c.reply().ok())
        else {
            self.outgoing_end();
            return;
        };
        let held = pointer.mask.contains(KeyButMask::BUTTON1);
        let geometry = self
            .conn
            .get_geometry(self.window)
            .ok()
            .and_then(|c| c.reply().ok());
        let outside = geometry.is_some_and(|g| {
            pointer.win_x < 0
                || pointer.win_y < 0
                || i32::from(pointer.win_x) >= i32::from(g.width)
                || i32::from(pointer.win_y) >= i32::from(g.height)
        });
        if !self.outgoing.as_ref().unwrap().started {
            if !held {
                self.outgoing = None;
                return;
            }
            if !outside {
                return;
            }
            let _ = self.conn.set_selection_owner(
                self.proxy,
                self.atoms.XdndSelection,
                x11rb::CURRENT_TIME,
            );
            self.outgoing.as_mut().unwrap().started = true;
            self.emit(AppEvent::Started);
        }
        let requested = if pointer.mask.contains(KeyButMask::CONTROL) {
            Action::Copy
        } else if pointer.mask.contains(KeyButMask::SHIFT) {
            Action::Move
        } else {
            Action::Copy
        };
        let (target, receiver) = self.find_target(pointer.root_x, pointer.root_y);
        let old = self.outgoing.as_ref().unwrap();
        let changed = target != old.target || requested != old.requested;
        if target != old.target {
            if old.target != 0 {
                self.target_message(
                    old.receiver,
                    old.target,
                    self.atoms.XdndLeave,
                    [self.proxy, 0, 0, 0, 0],
                );
            }
            if target != 0 {
                self.target_message(
                    receiver,
                    target,
                    self.atoms.XdndEnter,
                    [self.proxy, 5 << 24, self.atoms.URI, 0, 0],
                );
            }
        }
        let output = self.outgoing.as_mut().unwrap();
        output.target = target;
        output.receiver = receiver;
        output.requested = requested;
        if changed {
            output.accepted = false;
            self.emit(AppEvent::Action(None));
        }
        if target != 0 {
            let packed =
                (u32::from(pointer.root_x as u16) << 16) | u32::from(pointer.root_y as u16);
            self.target_message(
                receiver,
                target,
                self.atoms.XdndPosition,
                [
                    self.proxy,
                    0,
                    packed,
                    x11rb::CURRENT_TIME,
                    self.action_atom(requested),
                ],
            );
        }
        if !held {
            if target == 0 {
                self.outgoing_end();
            } else {
                let output = self.outgoing.as_mut().unwrap();
                output.accepted = false;
                output.released = Some(Instant::now());
            }
        }
    }
    fn selection_request(&mut self, request: SelectionRequestEvent) {
        let property = if request.property == 0 {
            request.target
        } else {
            request.property
        };
        let mut accepted = false;
        if request.selection == self.atoms.XdndSelection {
            if request.target == self.atoms.TARGETS {
                let _ = self.conn.change_property32(
                    PropMode::REPLACE,
                    request.requestor,
                    property,
                    AtomEnum::ATOM,
                    &[self.atoms.TARGETS, self.atoms.URI],
                );
                accepted = true;
            } else if request.target == self.atoms.URI
                && let Some(output) = &self.outgoing
            {
                if output.bytes.len() > 60_000 {
                    let _ = self.conn.change_window_attributes(
                        request.requestor,
                        &ChangeWindowAttributesAux::new().event_mask(EventMask::PROPERTY_CHANGE),
                    );
                    let _ = self.conn.change_property32(
                        PropMode::REPLACE,
                        request.requestor,
                        property,
                        self.atoms.INCR,
                        &[output.bytes.len() as u32],
                    );
                    if self.chunks.len() < 16 {
                        self.chunks.insert(
                            (request.requestor, property),
                            Chunk {
                                data: output.bytes.clone(),
                                offset: 0,
                                deadline: Instant::now() + Duration::from_secs(300),
                            },
                        );
                        accepted = true;
                    }
                } else {
                    let _ = self.conn.change_property8(
                        PropMode::REPLACE,
                        request.requestor,
                        property,
                        self.atoms.URI,
                        &output.bytes,
                    );
                    accepted = true;
                }
            }
        }
        let reply = SelectionNotifyEvent {
            response_type: SELECTION_NOTIFY_EVENT,
            sequence: 0,
            time: request.time,
            requestor: request.requestor,
            selection: request.selection,
            target: request.target,
            property: if accepted { property } else { 0 },
        };
        let _ = self
            .conn
            .send_event(false, request.requestor, EventMask::NO_EVENT, reply);
    }
    fn event(&mut self, event: Event) {
        match event {
            Event::ClientMessage(message) => {
                let data = message.data.as_data32();
                if message.type_ == self.atoms.XdndEnter {
                    let types = if data[1] & 1 != 0 {
                        self.property(data[0], self.atoms.XdndTypeList, AtomEnum::ATOM.into())
                    } else {
                        data[2..5].to_vec()
                    };
                    self.incoming = Some(Incoming {
                        source: data[0],
                        version: (data[1] >> 24).min(5),
                        action: Some(Action::Copy),
                        allow_move: self
                            .property(data[0], self.atoms.XdndActionList, AtomEnum::ATOM.into())
                            .contains(&self.atoms.XdndActionMove),
                        supported: types.contains(&self.atoms.URI),
                        position: egui::Pos2::ZERO,
                        data: Vec::new(),
                        incremental: false,
                        deadline: Instant::now() + Duration::from_secs(300),
                    });
                } else if message.type_ == self.atoms.XdndPosition {
                    let x = (data[2] >> 16) as i16;
                    let y = data[2] as i16;
                    if let Some(reply) = self
                        .conn
                        .translate_coordinates(self.root, self.window, x, y)
                        .ok()
                        .and_then(|c| c.reply().ok())
                        && let Some(input) = &mut self.incoming
                        && input.source == data[0]
                    {
                        input.action = if data[4] == self.atoms.XdndActionMove {
                            input.allow_move = true;
                            Some(Action::Move)
                        } else if data[4] == 0 || data[4] == self.atoms.XdndActionCopy {
                            Some(Action::Copy)
                        } else {
                            None
                        };
                        input.position = egui::pos2(
                            f32::from(reply.dst_x) / self.scale,
                            f32::from(reply.dst_y) / self.scale,
                        );
                        let position = input.position;
                        self.emit(AppEvent::Hover(position));
                        self.emit(AppEvent::Action(self.incoming_action()));
                    }
                    self.status();
                } else if message.type_ == self.atoms.XdndLeave {
                    if self.incoming.as_ref().is_some_and(|i| i.source == data[0]) {
                        self.incoming = None;
                        self.emit(AppEvent::Left);
                    }
                } else if message.type_ == self.atoms.XdndDrop {
                    if self.incoming.as_ref().is_some_and(|i| {
                        i.source == data[0]
                            && i.supported
                            && self.accepted
                            && i.version >= 3
                            && self.incoming_action().is_some()
                    }) {
                        let _ = self.conn.convert_selection(
                            self.proxy,
                            self.atoms.XdndSelection,
                            self.atoms.URI,
                            self.atoms.TRANSFER,
                            data[2],
                        );
                    } else {
                        self.incoming_done(false);
                    }
                } else if message.type_ == self.atoms.XdndStatus {
                    if let Some(output) = &mut self.outgoing
                        && output.target == data[0]
                    {
                        let action = if data[4] == self.atoms.XdndActionMove {
                            Some(Action::Move)
                        } else if data[4] == self.atoms.XdndActionCopy {
                            Some(Action::Copy)
                        } else {
                            None
                        };
                        let accepted = data[1] & 1 != 0 && action == Some(output.requested);
                        if output.accepted != accepted {
                            output.accepted = accepted;
                            self.emit(AppEvent::Action(if accepted { action } else { None }));
                        }
                    }
                } else if message.type_ == self.atoms.XdndFinished
                    && self.outgoing.as_ref().is_some_and(|o| o.target == data[0])
                {
                    self.outgoing_end();
                }
            }
            Event::SelectionRequest(request) => self.selection_request(request),
            Event::SelectionNotify(notify)
                if notify.requestor == self.proxy && self.incoming.is_some() =>
            {
                if notify.property == 0 {
                    self.incoming_done(false);
                    return;
                }
                if let Some(reply) = self
                    .conn
                    .get_property(
                        true,
                        self.proxy,
                        self.atoms.TRANSFER,
                        AtomEnum::ANY,
                        0,
                        (MAX_BYTES / 4 + 1) as u32,
                    )
                    .ok()
                    .and_then(|c| c.reply().ok())
                {
                    if reply.type_ == self.atoms.INCR {
                        self.incoming.as_mut().unwrap().incremental = true;
                    } else if reply.type_ == self.atoms.URI
                        && reply.bytes_after == 0
                        && reply.value.len() <= MAX_BYTES
                    {
                        self.deliver(reply.value);
                    } else {
                        self.incoming_done(false);
                    }
                } else {
                    self.incoming_done(false);
                }
            }
            Event::PropertyNotify(notify) => {
                if notify.window == self.proxy
                    && notify.atom == self.atoms.TRANSFER
                    && notify.state == Property::NEW_VALUE
                    && self.incoming.as_ref().is_some_and(|i| i.incremental)
                {
                    if let Some(reply) = self
                        .conn
                        .get_property(
                            true,
                            self.proxy,
                            self.atoms.TRANSFER,
                            AtomEnum::ANY,
                            0,
                            (MAX_BYTES / 4 + 1) as u32,
                        )
                        .ok()
                        .and_then(|c| c.reply().ok())
                    {
                        let input = self.incoming.as_mut().unwrap();
                        if reply.value.is_empty() {
                            let bytes = std::mem::take(&mut input.data);
                            self.deliver(bytes);
                        } else if reply.type_ == self.atoms.URI
                            && reply.bytes_after == 0
                            && input.data.len() + reply.value.len() <= MAX_BYTES
                        {
                            input.data.extend(reply.value);
                        } else {
                            self.incoming_done(false);
                        }
                    }
                } else if notify.state == Property::DELETE
                    && let Some(chunk) = self.chunks.get_mut(&(notify.window, notify.atom))
                {
                    let end = (chunk.offset + 60_000).min(chunk.data.len());
                    let _ = self.conn.change_property8(
                        PropMode::REPLACE,
                        notify.window,
                        notify.atom,
                        self.atoms.URI,
                        &chunk.data[chunk.offset..end],
                    );
                    let finished = chunk.offset == chunk.data.len();
                    chunk.offset = end;
                    if finished {
                        self.chunks.remove(&(notify.window, notify.atom));
                    }
                }
            }
            _ => (),
        }
    }
}
fn run(
    window: Window,
    commands: mpsc::Receiver<Command>,
    sender: &mpsc::Sender<AppEvent>,
    ctx: &egui::Context,
    selection: Option<Atom>,
) -> Result<()> {
    let (conn, screen) = x11rb::connect(None)?;
    let root = conn.setup().roots[screen].root;
    let mut atoms = Atoms::new(&conn)?.reply()?;
    if let Some(selection) = selection {
        atoms.XdndSelection = selection;
    }
    let proxy = conn.generate_id()?;
    conn.create_window(
        0,
        proxy,
        root,
        0,
        0,
        1,
        1,
        0,
        WindowClass::INPUT_ONLY,
        0,
        &CreateWindowAux::new().event_mask(EventMask::PROPERTY_CHANGE),
    )?
    .check()?;
    conn.change_property32(
        PropMode::REPLACE,
        proxy,
        atoms.XdndAware,
        AtomEnum::ATOM,
        &[5],
    )?;
    conn.change_property32(
        PropMode::REPLACE,
        proxy,
        atoms.XdndProxy,
        AtomEnum::WINDOW,
        &[proxy],
    )?;
    conn.change_property32(
        PropMode::REPLACE,
        window,
        atoms.XdndProxy,
        AtomEnum::WINDOW,
        &[proxy],
    )?
    .check()?;
    conn.change_property32(
        PropMode::REPLACE,
        proxy,
        atoms.XdndActionList,
        AtomEnum::ATOM,
        &[atoms.XdndActionCopy, atoms.XdndActionMove],
    )?;
    conn.flush()?;
    let mut worker = Worker {
        conn,
        root,
        window,
        proxy,
        atoms,
        sender: sender.clone(),
        ctx: ctx.clone(),
        scale: 1.0,
        accepted: false,
        incoming: None,
        outgoing: None,
        chunks: BTreeMap::new(),
        finishes: BTreeMap::new(),
        next_finish: 0,
    };
    let mut stop = false;
    while !stop {
        for command in commands.try_iter() {
            match command {
                Command::Stop => stop = true,
                Command::Complete(id, action) => worker.complete(id, action),
                Command::Scale(scale) => worker.scale = scale.max(0.1),
                Command::Accept(yes) => {
                    if worker.accepted != yes {
                        worker.accepted = yes;
                        worker.status();
                    }
                }
                Command::Cancel => worker.outgoing_end(),
                Command::Arm(paths) => {
                    worker.outgoing_end();
                    if let Some(bytes) = uri_list(&paths) {
                        worker.outgoing = Some(Outgoing {
                            bytes,
                            started: false,
                            target: 0,
                            receiver: 0,
                            accepted: false,
                            dropped: None,
                            released: None,
                            requested: Action::Copy,
                        });
                    } else {
                        worker.emit(AppEvent::Error(
                            crate::i18n::tr("A seleção excede os limites do arrastar e soltar.")
                                .into(),
                        ));
                    }
                }
            }
        }
        for _ in 0..128 {
            match worker.conn.poll_for_event()? {
                Some(event) => worker.event(event),
                None => break,
            }
        }
        worker.sample_pointer();
        if worker
            .incoming
            .as_ref()
            .is_some_and(|i| Instant::now() > i.deadline)
        {
            worker.incoming_done(false);
        }
        worker
            .chunks
            .retain(|_, chunk| Instant::now() < chunk.deadline);
        let expired: Vec<_> = worker
            .finishes
            .iter()
            .filter(|(_, finish)| Instant::now() > finish.deadline)
            .map(|(id, _)| *id)
            .collect();
        for id in expired {
            worker.complete(id, None);
        }
        worker.conn.flush()?;
        thread::sleep(Duration::from_millis(
            if worker.outgoing.is_some() || worker.incoming.is_some() {
                8
            } else {
                30
            },
        ));
    }
    worker.outgoing_end();
    for id in worker.finishes.keys().copied().collect::<Vec<_>>() {
        worker.complete(id, None);
    }
    if worker
        .property(window, worker.atoms.XdndProxy, AtomEnum::WINDOW.into())
        .first()
        == Some(&proxy)
    {
        let _ = worker.conn.delete_property(window, worker.atoms.XdndProxy);
    }
    let _ = worker.conn.destroy_window(proxy);
    let _ = worker.conn.flush();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_incoming_uses_the_proxy_and_exact_drop_position() {
        for (action, success) in [
            (Action::Copy, true),
            (Action::Move, true),
            (Action::Move, false),
        ] {
            incoming_protocol(action, success);
        }
    }
    fn incoming_protocol(action: Action, success: bool) {
        let Ok((conn, screen)) = x11rb::connect(None) else {
            return;
        };
        let root = conn.setup().roots[screen].root;
        let mut atoms = Atoms::new(&conn).unwrap().reply().unwrap();
        let target = conn.generate_id().unwrap();
        let source = conn.generate_id().unwrap();
        atoms.XdndSelection = conn
            .intern_atom(
                false,
                format!("_LSE_TEST_INCOMING_{}_{}", std::process::id(), source).as_bytes(),
            )
            .unwrap()
            .reply()
            .unwrap()
            .atom;
        for (window, x, y) in [(target, 120, 130), (source, 0, 0)] {
            conn.create_window(
                0,
                window,
                root,
                x,
                y,
                200,
                200,
                0,
                WindowClass::INPUT_ONLY,
                0,
                &CreateWindowAux::new(),
            )
            .unwrap()
            .check()
            .unwrap();
        }
        conn.set_selection_owner(source, atoms.XdndSelection, x11rb::CURRENT_TIME)
            .unwrap();
        conn.flush().unwrap();
        let bridge =
            Bridge::with_selection(target, egui::Context::default(), Some(atoms.XdndSelection));
        bridge.scale(2.0);
        bridge.accept(true);
        let deadline = Instant::now() + Duration::from_secs(3);
        let proxy = loop {
            let reply = conn
                .get_property(false, target, atoms.XdndProxy, AtomEnum::WINDOW, 0, 1)
                .unwrap()
                .reply()
                .unwrap();
            if let Some(proxy) = reply.value32().and_then(|mut values| values.next()) {
                break proxy;
            }
            assert!(Instant::now() < deadline);
            thread::sleep(Duration::from_millis(5));
        };
        let send = |kind, data| {
            conn.send_event(
                false,
                proxy,
                EventMask::NO_EVENT,
                ClientMessageEvent::new(32, target, kind, data),
            )
            .unwrap();
            conn.flush().unwrap();
        };
        send(atoms.XdndEnter, [source, 5 << 24, atoms.URI, 0, 0]);
        send(
            atoms.XdndPosition,
            [
                source,
                0,
                (180 << 16) | 210,
                0,
                if action == Action::Move {
                    atoms.XdndActionMove
                } else {
                    atoms.XdndActionCopy
                },
            ],
        );
        let position = loop {
            if let Some(position) = bridge.poll().into_iter().find_map(|event| {
                if let AppEvent::Hover(position) = event {
                    Some(position)
                } else {
                    None
                }
            }) {
                break position;
            }
            assert!(Instant::now() < deadline);
            thread::sleep(Duration::from_millis(5));
        };
        assert_eq!(position, egui::pos2(30.0, 40.0));
        send(atoms.XdndDrop, [source, 0, 0, 0, 0]);
        let fixture = tempfile::tempdir().unwrap();
        let source_file = fixture.path().join("source with spaces.txt");
        std::fs::write(&source_file, "source content").unwrap();
        let paths = vec![source_file.clone()];
        let token = loop {
            while let Some(event) = conn.poll_for_event().unwrap() {
                assert!(
                    !matches!(&event,Event::ClientMessage(message) if message.type_==atoms.XdndFinished),
                    "completion sent before the filesystem operation"
                );
                if let Event::SelectionRequest(request) = event {
                    conn.change_property8(
                        PropMode::REPLACE,
                        request.requestor,
                        request.property,
                        atoms.URI,
                        &uri_list(&paths).unwrap(),
                    )
                    .unwrap();
                    let notify = SelectionNotifyEvent {
                        response_type: SELECTION_NOTIFY_EVENT,
                        sequence: 0,
                        time: request.time,
                        requestor: request.requestor,
                        selection: request.selection,
                        target: request.target,
                        property: request.property,
                    };
                    conn.send_event(false, request.requestor, EventMask::NO_EVENT, notify)
                        .unwrap();
                    conn.flush().unwrap();
                }
            }
            if let Some((received, point, chosen, completion)) =
                bridge.poll().into_iter().find_map(|event| {
                    if let AppEvent::Dropped {
                        paths,
                        position,
                        internal: false,
                        action,
                        completion,
                    } = event
                    {
                        Some((paths, position, action, completion))
                    } else {
                        None
                    }
                })
            {
                assert_eq!(received, paths);
                assert_eq!(point, position);
                assert_eq!(chosen, action);
                assert!(source_file.exists());
                break match completion.unwrap() {
                    Completion::X11(id) => id,
                    _ => panic!("wrong backend"),
                };
            }
            assert!(Instant::now() < deadline);
            thread::sleep(Duration::from_millis(5));
        };
        // The protocol must not finish while the receiving application is still busy.
        thread::sleep(Duration::from_millis(40));
        while let Some(event) = conn.poll_for_event().unwrap() {
            assert!(
                !matches!(event,Event::ClientMessage(message) if message.type_==atoms.XdndFinished)
            );
        }
        let directory = fixture.path().join("received");
        std::fs::create_dir(&directory).unwrap();
        let destination = directory.join(source_file.file_name().unwrap());
        let report = crate::transfers::run(
            crate::transfers::Request {
                clipboard: crate::transfers::Clipboard {
                    id: 1,
                    mode: if action == Action::Move {
                        crate::transfers::Mode::Cut
                    } else {
                        crate::transfers::Mode::Copy
                    },
                    paths: paths.clone(),
                },
                destination: directory,
            },
            std::sync::Arc::new(std::sync::atomic::AtomicBool::new(!success)),
            |_| crate::transfers::Decision {
                choice: crate::transfers::Choice::Cancel,
                apply_all: false,
            },
            |_| {},
        );
        let completed = !report.cancelled
            && report.errors.is_empty()
            && report.skipped == 0
            && report.completed.len() == paths.len();
        assert_eq!(completed, success);
        bridge.complete(token, completed.then_some(action));
        loop {
            if let Some(Event::ClientMessage(message)) = conn.poll_for_event().unwrap()
                && message.type_ == atoms.XdndFinished
            {
                let data = message.data.as_data32();
                assert_eq!(data[1], u32::from(success));
                assert_eq!(
                    data[2],
                    if success {
                        if action == Action::Move {
                            atoms.XdndActionMove
                        } else {
                            atoms.XdndActionCopy
                        }
                    } else {
                        0
                    }
                );
                break;
            }
            assert!(Instant::now() < deadline);
            thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(source_file.exists(), !success || action == Action::Copy);
        if success {
            assert_eq!(
                std::fs::read_to_string(destination).unwrap(),
                "source content"
            );
        }
        drop(bridge);
        conn.destroy_window(target).unwrap();
        conn.destroy_window(source).unwrap();
        conn.flush().unwrap();
    }

    #[test]
    fn native_outgoing_serves_a_file_uri_selection_to_another_client() {
        let Ok((conn, screen)) = x11rb::connect(None) else {
            return;
        };
        let root = conn.setup().roots[screen].root;
        let mut atoms = Atoms::new(&conn).unwrap().reply().unwrap();
        let proxy = conn.generate_id().unwrap();
        atoms.XdndSelection = conn
            .intern_atom(
                false,
                format!("_LSE_TEST_OUTGOING_{}_{}", std::process::id(), proxy).as_bytes(),
            )
            .unwrap()
            .reply()
            .unwrap()
            .atom;
        conn.create_window(
            0,
            proxy,
            root,
            0,
            0,
            1,
            1,
            0,
            WindowClass::INPUT_ONLY,
            0,
            &CreateWindowAux::new(),
        )
        .unwrap()
        .check()
        .unwrap();
        conn.set_selection_owner(proxy, atoms.XdndSelection, x11rb::CURRENT_TIME)
            .unwrap()
            .check()
            .unwrap();
        let (sender, _) = mpsc::channel();
        let fixture = tempfile::tempdir().unwrap();
        let source_file = fixture.path().join("with spaces #.pdf");
        std::fs::write(&source_file, "original").unwrap();
        let paths = vec![source_file.clone()];
        let mut worker = Worker {
            conn,
            root,
            window: proxy,
            proxy,
            atoms,
            sender,
            ctx: egui::Context::default(),
            scale: 1.0,
            accepted: true,
            incoming: None,
            outgoing: Some(Outgoing {
                bytes: uri_list(&paths).unwrap(),
                started: true,
                target: 0,
                receiver: 0,
                accepted: true,
                dropped: None,
                released: None,
                requested: Action::Copy,
            }),
            chunks: BTreeMap::new(),
            finishes: BTreeMap::new(),
            next_finish: 0,
        };
        let (receiver, screen) = x11rb::connect(None).unwrap();
        let window = receiver.generate_id().unwrap();
        receiver
            .create_window(
                0,
                window,
                receiver.setup().roots[screen].root,
                0,
                0,
                1,
                1,
                0,
                WindowClass::INPUT_ONLY,
                0,
                &CreateWindowAux::new(),
            )
            .unwrap()
            .check()
            .unwrap();
        receiver
            .convert_selection(
                window,
                worker.atoms.XdndSelection,
                worker.atoms.URI,
                worker.atoms.TRANSFER,
                x11rb::CURRENT_TIME,
            )
            .unwrap();
        receiver.flush().unwrap();
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            while let Some(event) = worker.conn.poll_for_event().unwrap() {
                worker.event(event);
            }
            worker.conn.flush().unwrap();
            if let Some(Event::SelectionNotify(notify)) = receiver.poll_for_event().unwrap() {
                assert_ne!(notify.property, 0);
                let reply = receiver
                    .get_property(true, window, notify.property, worker.atoms.URI, 0, 4096)
                    .unwrap()
                    .reply()
                    .unwrap();
                assert_eq!(parse_uris(&reply.value).unwrap(), paths);
                let output = worker.outgoing.as_mut().unwrap();
                output.target = window;
                output.receiver = window;
                output.requested = Action::Move;
                let status = |action| {
                    Event::ClientMessage(ClientMessageEvent::new(
                        32,
                        proxy,
                        worker.atoms.XdndStatus,
                        [window, 1, 0, 0, action],
                    ))
                };
                let copy = status(worker.atoms.XdndActionCopy);
                let moved = status(worker.atoms.XdndActionMove);
                worker.event(copy);
                assert!(
                    !worker.outgoing.as_ref().unwrap().accepted,
                    "Shift must not silently become Copy"
                );
                worker.event(moved);
                assert!(worker.outgoing.as_ref().unwrap().accepted);
                let received = fixture.path().join("moved.pdf");
                std::fs::rename(&source_file, &received).unwrap();
                std::fs::write(&source_file, "replacement").unwrap();
                worker.outgoing.as_mut().unwrap().dropped = Some(Instant::now());
                worker.event(Event::ClientMessage(ClientMessageEvent::new(
                    32,
                    proxy,
                    worker.atoms.XdndFinished,
                    [window, 1, worker.atoms.XdndActionMove, 0, 0],
                )));
                assert!(worker.outgoing.is_none());
                assert_eq!(
                    std::fs::read_to_string(&source_file).unwrap(),
                    "replacement",
                    "the source must not delete again after the receiver moved the file"
                );
                assert_eq!(std::fs::read_to_string(received).unwrap(), "original");
                break;
            }
            assert!(Instant::now() < deadline);
            thread::sleep(Duration::from_millis(5));
        }
    }
}
