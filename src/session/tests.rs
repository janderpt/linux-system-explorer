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

#[test]
fn persisted_closed_context_preserves_raw_paths_and_normalizes_history_layout_and_limit() {
    use crate::{
        navigation::{History, Navigation},
        settings::{FolderLayout, View},
    };
    let root = tempfile::tempdir().unwrap();
    let raw = root
        .path()
        .join(std::ffi::OsString::from_vec(b"closed-\xff".to_vec()));
    let selected = raw.join(std::ffi::OsString::from_vec(b"selected-\xfe".to_vec()));
    let mut history = History::default();
    history.commit(root.path().to_owned(), Navigation::Visit);
    history.commit(raw.clone(), Navigation::Visit);
    let mut state = State::from_paths(vec![root.path().to_owned()], 0);
    state.closed_tabs.push_back(closed::ClosedTab {
        id: 1,
        position: 0,
        left: None,
        right: Some(0),
        path: raw.clone(),
        context: Some(closed::BrowsingContext {
            history,
            layout: FolderLayout {
                view: View::Icons,
                item_scale: 1.75,
                ..Default::default()
            },
            query: "selected".into(),
            selected: BTreeSet::from([selected.clone()]),
            anchor: Some(selected.clone()),
            cursor: Some(selected.clone()),
            reveal_hidden: false,
        }),
    });
    let file = root.path().join("session.json");
    state.save(&file).unwrap();
    assert_eq!(State::load(&file).unwrap().unwrap(), state);
    let context = State::load(&file)
        .unwrap()
        .unwrap()
        .closed_tabs
        .pop_back()
        .unwrap()
        .context
        .unwrap();
    assert_eq!(context.history.back(), Some(root.path()));
    assert!(context.selected.contains(&selected));
    let mut data = serde_json::to_value(&state).unwrap();
    data["closed_tabs"][0]["context"]["history"]["cursor"] = serde_json::json!(usize::MAX);
    data["closed_tabs"][0]["context"]["layout"]["item_scale"] = serde_json::json!(99.0);
    std::fs::write(&file, serde_json::to_vec(&data).unwrap()).unwrap();
    let loaded = State::load(&file).unwrap().unwrap();
    assert_eq!(
        loaded.closed_tabs[0]
            .context
            .as_ref()
            .unwrap()
            .layout
            .item_scale,
        2.0
    );
    assert_eq!(
        loaded.closed_tabs[0]
            .context
            .as_ref()
            .unwrap()
            .history
            .back(),
        Some(root.path())
    );
    let template = state.closed_tabs[0].clone();
    state.closed_tabs = (1..=30)
        .map(|id| closed::ClosedTab {
            id,
            ..template.clone()
        })
        .collect();
    state.save(&file).unwrap();
    let loaded = State::load(&file).unwrap().unwrap();
    assert_eq!(loaded.closed_tabs.len(), 25);
    assert_eq!(loaded.closed_tabs[0].id, 6);
    for invalid in [
        serde_json::json!({"id":1,"position":0,"left":null,"right":null,"path":"relative","context":null}),
        serde_json::json!({"id":0,"position":0,"left":null,"right":null,"path":"/tmp","context":null}),
    ] {
        let data = serde_json::json!({"tabs":["/tmp"],"closed_tabs":[invalid]});
        let text = serde_json::to_string(&data).unwrap();
        std::fs::write(&file, &text).unwrap();
        let mut startup = Startup::at(Some(file.clone()), true);
        assert!(startup.error.is_some());
        startup.store.save(State::default(), true).unwrap();
        assert_eq!(std::fs::read_to_string(&file).unwrap(), text);
    }
}
