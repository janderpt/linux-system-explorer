//! A real pseudoterminal and bounded VT100 screen, independent of the file grid.
use portable_pty::{Child, CommandBuilder, MasterPty, PtySize};
use std::{
    collections::VecDeque,
    io::{self, Read, Write},
    num::NonZeroU16,
    os::fd::BorrowedFd,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::Duration,
};

pub struct Replies {
    bytes: Vec<u8>,
}
impl vt100::Callbacks for Replies {
    fn unhandled_csi(
        &mut self,
        screen: &mut vt100::Screen,
        i: Option<u8>,
        _: Option<u8>,
        params: &[&[u16]],
        c: char,
    ) {
        let parameter = params.first().and_then(|p| p.first()).copied().unwrap_or(0);
        let reply = match (i, c, parameter) {
            (None, 'n', 5) => "\x1b[0n".to_owned(),
            (None, 'n', 6) => {
                let (r, c) = screen.cursor_position();
                format!("\x1b[{};{}R", r + 1, c + 1)
            }
            (None, 'c', _) => "\x1b[?1;2c".to_owned(),
            (Some(b'>'), 'c', _) => "\x1b[>0;1;0c".to_owned(),
            _ => String::new(),
        };
        self.bytes.extend_from_slice(reply.as_bytes());
    }
}
pub struct Session {
    pub parser: Arc<Mutex<vt100::Parser<Replies>>>,
    master: Box<dyn MasterPty + Send>,
    child: Box<dyn Child + Send + Sync>,
    input: mpsc::SyncSender<Vec<u8>>,
    stop: Arc<AtomicBool>,
    ended: Arc<AtomicBool>,
    worker: Option<thread::JoinHandle<()>>,
    size: (u16, u16),
    pub initial_folder: PathBuf,
    pub exit_code: Option<u32>,
}
pub fn default_shell() -> PathBuf {
    use uzers::os::unix::UserExt;
    std::env::var_os("SHELL")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute() && p.is_file())
        .or_else(|| {
            uzers::get_user_by_uid(uzers::get_current_uid())
                .map(|u| u.shell().to_owned())
                .filter(|p| p.is_absolute() && p.is_file())
        })
        .unwrap_or_else(|| "/bin/sh".into())
}
impl Session {
    pub fn start(folder: &Path, context: &eframe::egui::Context) -> io::Result<Self> {
        let mut command = CommandBuilder::new(default_shell());
        command.arg("-i");
        Self::spawn(folder, command, context)
    }
    fn spawn(
        folder: &Path,
        mut command: CommandBuilder,
        context: &eframe::egui::Context,
    ) -> io::Result<Self> {
        let folder = crate::filesystem::absolute_path(folder)?;
        if !folder.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::NotADirectory,
                folder.display().to_string(),
            ));
        }
        let pair = portable_pty::native_pty_system()
            .openpty(PtySize {
                rows: 24,
                cols: 80,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(io::Error::other)?;
        command.cwd(&folder);
        command.env("TERM", "xterm-256color");
        command.env("COLORTERM", "truecolor");
        command.env_remove("COLUMNS");
        command.env_remove("LINES");
        let mut reader = pair.master.try_clone_reader().map_err(io::Error::other)?;
        let mut writer = pair.master.take_writer().map_err(io::Error::other)?;
        if let Some(fd) = pair.master.as_raw_fd() {
            // SAFETY: master owns this descriptor throughout the call.
            let fd = unsafe { BorrowedFd::borrow_raw(fd) };
            let flags = rustix::fs::fcntl_getfl(fd)?;
            rustix::fs::fcntl_setfl(fd, flags | rustix::fs::OFlags::NONBLOCK)?;
        }
        let child = pair
            .slave
            .spawn_command(command)
            .map_err(io::Error::other)?;
        drop(pair.slave);
        let parser = Arc::new(Mutex::new(vt100::Parser::new_with_callbacks(
            NonZeroU16::new(24).unwrap(),
            NonZeroU16::new(80).unwrap(),
            5000,
            Replies { bytes: vec![] },
        )));
        let stop = Arc::new(AtomicBool::new(false));
        let ended = Arc::new(AtomicBool::new(false));
        let (input, commands) = mpsc::sync_channel::<Vec<u8>>(64);
        let screen = parser.clone();
        let stopping = stop.clone();
        let finished = ended.clone();
        let ctx = context.clone();
        let worker = thread::spawn(move || {
            let mut pending = VecDeque::new();
            let mut buffer = [0; 8192];
            while !stopping.load(Ordering::Relaxed) {
                while pending.len() < 1_048_576 {
                    let Ok(bytes) = commands.try_recv() else {
                        break;
                    };
                    pending.extend(bytes);
                }
                if !pending.is_empty() {
                    match writer.write(pending.make_contiguous()) {
                        Ok(n) => {
                            pending.drain(..n);
                        }
                        Err(e)
                            if e.kind() == io::ErrorKind::WouldBlock
                                || e.kind() == io::ErrorKind::Interrupted => {}
                        _ => break,
                    }
                }
                let mut changed = false;
                for _ in 0..32 {
                    match reader.read(&mut buffer) {
                        Ok(0) => {
                            finished.store(true, Ordering::Relaxed);
                            break;
                        }
                        Ok(n) => {
                            if let Ok(mut parser) = screen.lock() {
                                parser.process(&buffer[..n]);
                                let (_, replies) = parser.screen_and_callbacks_mut();
                                pending.extend(std::mem::take(&mut replies.bytes));
                            }
                            changed = true;
                        }
                        Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
                        Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                        Err(_) => {
                            finished.store(true, Ordering::Relaxed);
                            break;
                        }
                    }
                }
                if changed {
                    ctx.request_repaint();
                }
                if finished.load(Ordering::Relaxed) {
                    break;
                }
                thread::sleep(Duration::from_millis(8));
            }
            finished.store(true, Ordering::Relaxed);
            ctx.request_repaint();
        });
        Ok(Self {
            parser,
            master: pair.master,
            child,
            input,
            stop,
            ended,
            worker: Some(worker),
            size: (24, 80),
            initial_folder: folder,
            exit_code: None,
        })
    }
    pub fn write(&mut self, bytes: Vec<u8>) -> io::Result<()> {
        if bytes.len() > 1_048_576 {
            return Err(io::Error::other("Terminal input exceeds 1 MiB"));
        }
        self.input.try_send(bytes).map_err(io::Error::other)
    }
    pub fn resize(&mut self, rows: u16, cols: u16) -> io::Result<()> {
        let size = (rows.clamp(1, 200), cols.clamp(1, 500));
        if self.size != size {
            self.master
                .resize(PtySize {
                    rows: size.0,
                    cols: size.1,
                    pixel_width: 0,
                    pixel_height: 0,
                })
                .map_err(io::Error::other)?;
            self.parser.lock().unwrap().set_size(
                NonZeroU16::new(size.0).unwrap(),
                NonZeroU16::new(size.1).unwrap(),
            );
            self.size = size;
        }
        Ok(())
    }
    pub fn poll(&mut self) {
        if self.exit_code.is_none()
            && let Ok(Some(status)) = self.child.try_wait()
        {
            self.exit_code = Some(status.exit_code());
        }
    }
    pub fn alive(&self) -> bool {
        self.exit_code.is_none() && !self.ended.load(Ordering::Relaxed)
    }
    pub fn cwd(&self) -> PathBuf {
        if self.exit_code.is_some() {
            return self.initial_folder.clone();
        }
        self.child
            .process_id()
            .and_then(|pid| std::fs::read_link(format!("/proc/{pid}/cwd")).ok())
            .unwrap_or_else(|| self.initial_folder.clone())
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        use rustix::process::{Pid, Signal};
        if self.child.try_wait().ok().flatten().is_none() {
            if let Some(group) = self.master.process_group_leader().and_then(Pid::from_raw) {
                let _ = rustix::process::kill_process_group(group, Signal::HUP);
            }
            let _ = self.child.kill();
        }
        self.stop.store(true, Ordering::Relaxed);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
        if self.child.try_wait().ok().flatten().is_none()
            && let Some(pid) = self
                .child
                .process_id()
                .and_then(|pid| Pid::from_raw(pid as i32))
        {
            let _ = rustix::process::kill_process(pid, Signal::KILL);
        }
        let _ = self.child.wait();
    }
}
#[cfg(test)]
mod tests;
