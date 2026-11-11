use super::*;
use std::os::unix::{ffi::OsStringExt, fs::symlink};

struct Fixture {
    root: tempfile::TempDir,
    serial: usize,
}
impl Fixture {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir_all(root.path().join("Trash/info")).unwrap();
        fs::create_dir(root.path().join("Trash/files")).unwrap();
        fs::create_dir(root.path().join("original")).unwrap();
        Self { root, serial: 0 }
    }
    fn original(&self, name: impl AsRef<Path>) -> PathBuf {
        self.root.path().join("original").join(name)
    }
    fn item(&mut self, original: &Path) -> Entry {
        self.serial += 1;
        let receipt = format!("receipt.{}", self.serial);
        let payload = self.root.path().join("Trash/files").join(&receipt);
        fs::rename(original, payload).unwrap();
        let info = self
            .root
            .path()
            .join("Trash/info")
            .join(format!("{receipt}.trashinfo"));
        fs::write(
            &info,
            format!(
                "[Trash Info]\nPath={}\nDeletionDate=2026-11-10T10:00:00\n",
                original.display()
            ),
        )
        .unwrap();
        read_item(trash::TrashItem {
            id: info.into_os_string(),
            name: original.file_name().unwrap().to_owned(),
            original_parent: original.parent().unwrap().to_owned(),
            time_deleted: 1_700_000_000,
        })
        .unwrap()
    }
    fn file(&mut self, name: impl AsRef<Path>, text: &str) -> Entry {
        let path = self.original(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, text).unwrap();
        self.item(&path)
    }
}
fn decision(choice: transfers::Choice) -> transfers::Decision {
    transfers::Decision {
        choice,
        apply_all: false,
    }
}
fn no_conflict(_: transfers::Conflict) -> transfers::Decision {
    panic!("unexpected collision")
}

#[test]
fn listing_preserves_original_names_extensions_hidden_items_and_metadata() {
    let mut fixture = Fixture::new();
    let item = fixture.file(".report.txt", "content");
    assert_eq!(item.name, ".report.txt");
    assert_eq!(item.extension(), "txt");
    assert_eq!(item.kind(), "Arquivo TXT");
    assert_eq!(crate::filetypes::icon(&item), crate::icons::Icon::FileText);
    assert!(!item.hidden);
    let record = item.trash.as_ref().unwrap();
    assert_eq!(record.original_path, fixture.original(".report.txt"));
    assert!(record.deleted.is_some());
    assert!(
        crate::columns::Column::OriginalPath
            .value(&item)
            .ends_with("/original/.report.txt")
    );
    assert_ne!(crate::columns::Column::Deleted.value(&item), "—");
}

#[test]
fn restores_a_selection_of_files_folders_and_links_and_recreates_missing_parents() {
    let mut fixture = Fixture::new();
    let a = fixture.file("missing/nested.txt", "data");
    fs::remove_dir(fixture.original("missing")).unwrap();
    let tree = fixture.original("folder");
    fs::create_dir(&tree).unwrap();
    fs::write(tree.join(".hidden"), "hidden").unwrap();
    let folder = fixture.item(&tree);
    let preserved = fixture.root.path().join("preserved");
    fs::write(&preserved, "preserved").unwrap();
    let link_path = fixture.original("link");
    symlink(&preserved, &link_path).unwrap();
    let link = fixture.item(&link_path);
    let broken_path = fixture.original("broken");
    symlink("missing-target", &broken_path).unwrap();
    let broken = fixture.item(&broken_path);
    let entries = vec![a, folder, link, broken];
    let paths: Vec<_> = entries
        .iter()
        .map(|e| (e.path.clone(), e.trash.as_ref().unwrap().info_path.clone()))
        .collect();
    let report = restore(
        entries,
        Arc::new(AtomicBool::new(false)),
        no_conflict,
        |_| {},
    );
    assert!(report.errors.is_empty(), "{:?}", report.errors);
    assert_eq!(report.completed.len(), 4);
    assert_eq!(
        fs::read_to_string(fixture.original("missing/nested.txt")).unwrap(),
        "data"
    );
    assert_eq!(fs::read_to_string(tree.join(".hidden")).unwrap(), "hidden");
    assert_eq!(fs::read_link(link_path).unwrap(), preserved);
    assert_eq!(
        fs::read_link(broken_path).unwrap(),
        Path::new("missing-target")
    );
    assert_eq!(fs::read_to_string(preserved).unwrap(), "preserved");
    assert!(
        paths
            .iter()
            .all(|(file, info)| fs::symlink_metadata(file).is_err() && !info.exists())
    );
}

#[test]
fn collisions_keep_both_or_skip_without_overwriting_existing_files_or_dangling_links() {
    let mut fixture = Fixture::new();
    let a = fixture.file("same.txt", "first");
    let b = fixture.file("same.txt", "second");
    fs::write(fixture.original("same.txt"), "existing").unwrap();
    let mut prompts = 0;
    let report = restore(
        vec![a, b],
        Arc::new(AtomicBool::new(false)),
        |_| {
            prompts += 1;
            transfers::Decision {
                choice: transfers::Choice::KeepBoth,
                apply_all: true,
            }
        },
        |_| {},
    );
    assert!(report.errors.is_empty());
    assert_eq!(report.completed.len(), 2);
    assert_eq!(prompts, 1);
    assert_eq!(
        fs::read_to_string(fixture.original("same.txt")).unwrap(),
        "existing"
    );
    assert_eq!(
        fs::read_to_string(fixture.original("same (restaurado).txt")).unwrap(),
        "first"
    );
    assert_eq!(
        fs::read_to_string(fixture.original("same (restaurado 2).txt")).unwrap(),
        "second"
    );
    let item = fixture.file("dangling", "trash");
    symlink("missing", fixture.original("dangling")).unwrap();
    let report = restore(
        vec![item.clone()],
        Arc::new(AtomicBool::new(false)),
        |_| decision(transfers::Choice::Skip),
        |_| {},
    );
    assert!(report.completed.is_empty());
    assert_eq!(report.skipped, 1);
    assert!(item.path.exists() && item.trash.unwrap().info_path.exists());
    assert_eq!(
        fs::read_link(fixture.original("dangling")).unwrap(),
        Path::new("missing")
    );
}

#[test]
fn cancellation_preserves_remaining_items_and_individual_errors_do_not_stop_the_selection() {
    let mut fixture = Fixture::new();
    let a = fixture.file("a", "a");
    let b = fixture.file("b", "b");
    fs::write(fixture.original("a"), "existing").unwrap();
    let report = restore(
        vec![a.clone(), b.clone()],
        Arc::new(AtomicBool::new(false)),
        |_| decision(transfers::Choice::Cancel),
        |_| {},
    );
    assert!(report.cancelled && report.completed.is_empty());
    assert!(a.path.exists() && b.path.exists());
    fs::write(&a.trash.as_ref().unwrap().info_path, "modified receipt").unwrap();
    let report = restore(
        vec![a.clone(), b],
        Arc::new(AtomicBool::new(false)),
        no_conflict,
        |_| {},
    );
    assert_eq!(report.errors.len(), 1);
    assert_eq!(report.completed.len(), 1);
    assert!(a.path.exists());
    assert_eq!(
        fs::read_to_string(fixture.original("a")).unwrap(),
        "existing"
    );
    assert_eq!(fs::read_to_string(fixture.original("b")).unwrap(), "b");
}

#[test]
fn stale_source_and_non_utf8_names_are_handled_without_losing_data() {
    let mut fixture = Fixture::new();
    let stale = fixture.file("stale", "old");
    fs::rename(&stale.path, fixture.root.path().join("preserved-old-inode")).unwrap();
    fs::write(&stale.path, "replacement").unwrap();
    let name = OsString::from_vec(vec![b'a', 0xff, b'.', b't', b'x', b't']);
    let raw = fixture.file(Path::new(&name), "raw");
    let report = restore(
        vec![stale.clone(), raw],
        Arc::new(AtomicBool::new(false)),
        no_conflict,
        |_| {},
    );
    assert_eq!(report.errors.len(), 1);
    assert_eq!(report.completed.len(), 1);
    assert_eq!(fs::read_to_string(&stale.path).unwrap(), "replacement");
    assert_eq!(
        fs::read_to_string(fixture.original(Path::new(&name))).unwrap(),
        "raw"
    );
}

#[test]
fn restoration_between_filesystems_uses_safe_copy_and_move_fallback() {
    let Ok(destination) = tempfile::tempdir_in("/dev/shm") else {
        return;
    };
    let mut fixture = Fixture::new();
    let mut item = fixture.file("cross-device.txt", "content");
    let original = destination.path().join("restored.txt");
    item.trash.as_mut().unwrap().original_path = original.clone();
    let info = item.trash.as_ref().unwrap().info_path.clone();
    let source = item.path.clone();
    let report = restore(
        vec![item],
        Arc::new(AtomicBool::new(false)),
        no_conflict,
        |_| {},
    );
    assert!(report.errors.is_empty(), "{:?}", report.errors);
    assert_eq!(report.completed.len(), 1);
    assert_eq!(fs::read_to_string(original).unwrap(), "content");
    assert!(!source.exists() && !info.exists());
}

#[test]
fn emptying_trash_removes_confirmed_items_and_receipts_but_preserves_link_targets_and_later_items()
{
    let mut fixture = Fixture::new();
    let a = fixture.file("a", "a");
    let tree = fixture.original("tree");
    fs::create_dir(&tree).unwrap();
    fs::write(tree.join(".hidden"), "hidden").unwrap();
    let outside = fixture.root.path().join("preserved");
    fs::create_dir(&outside).unwrap();
    fs::write(outside.join("keep"), "keep").unwrap();
    symlink(&outside, tree.join("inside-link")).unwrap();
    let folder = fixture.item(&tree);
    let link = fixture.original("link");
    symlink(&outside, &link).unwrap();
    let link = fixture.item(&link);
    let confirmed = vec![a, folder, link];
    let receipts: Vec<_> = confirmed
        .iter()
        .map(|entry| entry.trash.as_ref().unwrap().info_path.clone())
        .collect();
    let later = fixture.file("later", "later");
    let report = purge(confirmed, Arc::new(AtomicBool::new(false)), |_| {});
    assert!(report.errors.is_empty(), "{:?}", report.errors);
    assert_eq!(report.completed.len(), 3);
    assert!(receipts.iter().all(|path| !path.exists()));
    assert!(
        report
            .completed
            .iter()
            .all(|path| fs::symlink_metadata(path).is_err())
    );
    assert_eq!(fs::read_to_string(outside.join("keep")).unwrap(), "keep");
    assert!(later.path.exists() && later.trash.unwrap().info_path.exists());
}

#[test]
fn purge_rejects_changed_items_and_continues_after_individual_errors() {
    let mut fixture = Fixture::new();
    let changed_receipt = fixture.file("changed-receipt", "preserve");
    fs::write(
        &changed_receipt.trash.as_ref().unwrap().info_path,
        "changed",
    )
    .unwrap();
    let changed_payload = fixture.file("changed-payload", "preserve");
    fs::rename(&changed_payload.path, fixture.root.path().join("old-inode")).unwrap();
    fs::write(&changed_payload.path, "replacement").unwrap();
    let valid = fixture.file("valid", "remove");
    let report = purge(
        vec![
            changed_receipt.clone(),
            changed_payload.clone(),
            valid.clone(),
        ],
        Arc::new(AtomicBool::new(false)),
        |_| {},
    );
    assert_eq!(report.completed, vec![valid.path]);
    assert_eq!(report.errors.len(), 2);
    assert!(changed_receipt.path.exists() && changed_payload.path.exists());
    assert_eq!(
        fs::read_to_string(changed_payload.path).unwrap(),
        "replacement"
    );
}

#[test]
fn cancelling_purge_stops_between_items() {
    let mut fixture = Fixture::new();
    let a = fixture.file("a", "a");
    let b = fixture.file("b", "b");
    let cancel = Arc::new(AtomicBool::new(false));
    let report = purge(vec![a.clone(), b.clone()], cancel.clone(), |progress| {
        if progress.done == 1 {
            cancel.store(true, Ordering::Relaxed);
        }
    });
    assert_eq!(report.completed, vec![a.path]);
    assert!(report.cancelled);
    assert!(b.path.exists() && b.trash.unwrap().info_path.exists());
}
