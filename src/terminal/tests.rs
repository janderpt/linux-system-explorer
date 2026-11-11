use super::*;
use std::time::Instant;
fn wait(session: &mut Session, expected: &str) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        session.poll();
        let contents = session.parser.lock().unwrap().screen().contents();
        if contents.contains(expected) {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "missing {expected:?} in {contents:?}"
        );
        thread::sleep(Duration::from_millis(10));
    }
}
#[test]
fn real_pty_runs_interactive_shell_in_exact_directory_resizes_and_interrupts() {
    let root = tempfile::tempdir().unwrap();
    let folder = root.path().join("pasta á ' $literal");
    std::fs::create_dir(&folder).unwrap();
    let mut command = CommandBuilder::new("/bin/sh");
    command.arg("-i");
    let mut session = Session::spawn(&folder, command, &eframe::egui::Context::default()).unwrap();
    session.resize(18, 120).unwrap();
    session.write(b"stty -echo; printf '\\033[2J\\033[H'; [ -t 0 ] && [ -t 1 ] && printf 'PTY_READY\\n'; pwd\r".to_vec()).unwrap();
    wait(&mut session, "PTY_READY");
    wait(&mut session, &folder.display().to_string());
    assert_eq!(session.cwd(), folder);
    session
        .write(
            "printf '\\033[31mUNICODE: olá\\033[0m\\n'; stty size\r"
                .as_bytes()
                .to_vec(),
        )
        .unwrap();
    wait(&mut session, "UNICODE: olá");
    wait(&mut session, "18 120");
    let colored = session
        .parser
        .lock()
        .unwrap()
        .screen()
        .cell(2, 0)
        .map(|c| c.fgcolor());
    assert!(colored.is_some());
    session.write(b"sleep 30\r".to_vec()).unwrap();
    thread::sleep(Duration::from_millis(100));
    session.write(vec![3]).unwrap();
    session
        .write(b"printf 'INTERRUPT_OK\\n'\r".to_vec())
        .unwrap();
    wait(&mut session, "INTERRUPT_OK");
    session.write(b"exit\r".to_vec()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    while session.exit_code.is_none() && Instant::now() < deadline {
        session.poll();
        thread::sleep(Duration::from_millis(10));
    }
    assert!(session.exit_code.is_some());
    assert!(!session.alive());
}
#[test]
fn closing_session_reaps_a_shell_that_ignores_hangup_without_blocking() {
    let folder = tempfile::tempdir().unwrap();
    let mut command = CommandBuilder::new("/bin/sh");
    command.args(["-c", "trap '' HUP; printf READY; while :; do sleep 1; done"]);
    let mut session =
        Session::spawn(folder.path(), command, &eframe::egui::Context::default()).unwrap();
    wait(&mut session, "READY");
    let pid = session.child.process_id().unwrap();
    let start = Instant::now();
    drop(session);
    assert!(start.elapsed() < Duration::from_secs(3));
    assert!(!Path::new(&format!("/proc/{pid}")).exists());
}
#[test]
fn invalid_directory_does_not_launch_a_shell() {
    let root = tempfile::tempdir().unwrap();
    assert!(
        Session::start(
            &root.path().join("missing"),
            &eframe::egui::Context::default()
        )
        .is_err()
    );
}
