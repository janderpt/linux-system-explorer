use super::*;
use std::os::unix::ffi::OsStringExt;
#[test]
fn session_round_trip_keeps_order_duplicates_active_tab_raw_paths_and_trash() {
    let root = tempfile::tempdir().unwrap();
    let a = root.path().join("pasta á # spaces");
    let raw = root
        .path()
        .join(std::ffi::OsString::from_vec(b"raw-\xff".to_vec()));
    let paths = vec![a.clone(), raw.clone(), a, crate::trashbin::location()];
    let state = State::from_paths(paths.clone(), 2);
    let path = root.path().join("state/session.json");
    state.save(&path).unwrap();
    let loaded = State::load(&path).unwrap().unwrap();
    assert_eq!(loaded, state);
    assert_eq!(loaded.paths().unwrap(), paths);
    assert_eq!(loaded.active_folder(), Some(paths[2].clone()));
    assert!(
        std::fs::read_to_string(&path)
            .unwrap()
            .contains("unix-bytes:")
    );
}
#[test]
fn missing_state_and_old_minimal_data_fall_back_and_invalid_active_is_clamped() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("session.json");
    assert!(State::load(&path).unwrap().is_none());
    std::fs::write(&path, r#"{"tabs":["/tmp/a","/tmp/b"],"active":99}"#).unwrap();
    let state = State::load(&path).unwrap().unwrap();
    assert_eq!(state.active, 1);
    assert_eq!(state.version, 1);
    std::fs::write(&path, b"{}").unwrap();
    assert!(
        State::load(&path)
            .unwrap()
            .unwrap()
            .paths()
            .unwrap()
            .is_empty()
    );
}
#[test]
fn invalid_or_future_sessions_are_reported_and_never_overwritten_on_startup() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("session.json");
    for data in [
        "not json",
        r#"{"version":2,"tabs":["/tmp"]}"#,
        r#"{"tabs":["relative"]}"#,
        r#"{"tabs":["unix-bytes:ffzz"]}"#,
        r#"{"tabs":["unix-bytes:2f746d702f00"]}"#,
    ] {
        std::fs::write(&path, data).unwrap();
        let mut startup = Startup::at(Some(path.clone()), true);
        assert!(startup.error.is_some(), "{data}");
        assert!(startup.saved.is_none());
        startup
            .store
            .save(State::from_paths(vec!["/tmp/new".into()], 0), true)
            .unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), data);
    }
}
#[test]
fn autosave_skips_identical_snapshots_but_forced_close_can_record_last_window() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("session.json");
    let one = State::from_paths(vec!["/tmp/one".into()], 0);
    let two = State::from_paths(vec!["/tmp/two".into()], 0);
    let mut first = Store::at(Some(path.clone()));
    let mut second = Store::at(Some(path.clone()));
    first.save(one.clone(), false).unwrap();
    second.save(two.clone(), false).unwrap();
    first.save(one.clone(), false).unwrap();
    assert_eq!(State::load(&path).unwrap(), Some(two));
    first.save(one.clone(), true).unwrap();
    assert_eq!(State::load(&path).unwrap(), Some(one));
}
#[test]
fn explicit_directory_launch_does_not_restore_saved_tabs() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("session.json");
    let saved = State::from_paths(vec!["/tmp/a".into(), "/tmp/b".into()], 1);
    saved.save(&path).unwrap();
    assert!(Startup::at(Some(path.clone()), false).saved.is_none());
    assert_eq!(Startup::at(Some(path), true).saved, Some(saved));
}
