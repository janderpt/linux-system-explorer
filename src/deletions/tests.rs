use super::*;
use std::os::unix::fs::symlink;

fn target(path: &Path) -> Target {
    let metadata = fs::symlink_metadata(path).unwrap();
    Target {
        path: path.to_owned(),
        device: metadata.dev(),
        inode: metadata.ino(),
    }
}

#[test]
fn permanent_deletion_removes_trees_and_links_without_following_them() {
    let root = tempfile::tempdir().unwrap();
    let outside = root.path().join("preserved");
    fs::create_dir(&outside).unwrap();
    fs::write(outside.join("data"), "keep").unwrap();
    let tree = root.path().join("tree");
    fs::create_dir(&tree).unwrap();
    fs::write(tree.join(".hidden"), "remove").unwrap();
    symlink(&outside, tree.join("link")).unwrap();
    let link = root.path().join("selected-link");
    symlink(&outside, &link).unwrap();
    let broken = root.path().join("broken");
    symlink(root.path().join("missing"), &broken).unwrap();
    let report = run(
        vec![target(&tree), target(&link), target(&broken)],
        Mode::Permanent,
        &AtomicBool::new(false),
        |_| {},
    );
    assert_eq!(report.completed.len(), 3);
    assert!(report.errors.is_empty());
    assert!(
        !tree.exists()
            && fs::symlink_metadata(&link).is_err()
            && fs::symlink_metadata(&broken).is_err()
    );
    assert_eq!(fs::read_to_string(outside.join("data")).unwrap(), "keep");
}

#[test]
fn changed_or_missing_items_fail_individually_and_other_items_continue() {
    let root = tempfile::tempdir().unwrap();
    let changed = root.path().join("changed");
    let missing = root.path().join("missing");
    let valid = root.path().join("valid");
    for path in [&changed, &missing, &valid] {
        fs::write(path, "original").unwrap();
    }
    let targets = vec![target(&changed), target(&missing), target(&valid)];
    fs::rename(&changed, root.path().join("old-inode")).unwrap();
    fs::write(&changed, "replacement").unwrap();
    fs::remove_file(&missing).unwrap();
    let report = run(targets, Mode::Permanent, &AtomicBool::new(false), |_| {});
    assert_eq!(report.completed, vec![valid]);
    assert_eq!(report.errors.len(), 2);
    assert_eq!(fs::read_to_string(changed).unwrap(), "replacement");
}

#[test]
fn trash_failure_never_falls_back_to_permanent_deletion() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("preserve");
    fs::write(&path, "keep").unwrap();
    let report = run_with(
        vec![target(&path)],
        Mode::Trash,
        &AtomicBool::new(false),
        &mut |_| {},
        |_| Err(io::Error::other("Lixeira indisponível")),
    );
    assert!(report.completed.is_empty());
    assert_eq!(report.errors.len(), 1);
    assert_eq!(fs::read_to_string(path).unwrap(), "keep");
}

#[test]
fn cancellation_stops_between_items_and_root_is_rejected() {
    let root = tempfile::tempdir().unwrap();
    let a = root.path().join("a");
    let b = root.path().join("b");
    fs::write(&a, "a").unwrap();
    fs::write(&b, "b").unwrap();
    let cancel = AtomicBool::new(false);
    let report = run(
        vec![target(&a), target(&b)],
        Mode::Permanent,
        &cancel,
        |progress| {
            if progress.done == 1 {
                cancel.store(true, Ordering::Relaxed);
            }
        },
    );
    assert_eq!(report.completed, vec![a]);
    assert!(report.cancelled && b.exists());
    let report = run(
        vec![target(Path::new("/"))],
        Mode::Permanent,
        &AtomicBool::new(false),
        |_| {},
    );
    assert_eq!(report.errors.len(), 1);
    assert!(report.completed.is_empty());
}

#[test]
fn real_trash_uses_an_isolated_data_home() {
    // A child process isolates XDG/HOME without changing this multithreaded test process.
    let root = tempfile::tempdir().unwrap();
    let data_home = root.path().join("data");
    fs::create_dir(&data_home).unwrap();
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "deletions::tests::isolated_trash_child",
            "--nocapture",
        ])
        .env("EXPLORER_TRASH_TEST_ROOT", root.path())
        .env("XDG_DATA_HOME", &data_home)
        .env("HOME", root.path())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(data_home.join("Trash/info/file.trashinfo").exists());
    assert!(data_home.join("Trash/info/folder.trashinfo").exists());
    assert!(data_home.join("Trash/files/folder/.hidden").exists());
    assert!(
        fs::symlink_metadata(data_home.join("Trash/files/link"))
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert!(root.path().join("preserved/data").exists());
}

#[test]
fn isolated_trash_child() {
    let Some(root) = std::env::var_os("EXPLORER_TRASH_TEST_ROOT") else {
        return;
    };
    let root = PathBuf::from(root);
    assert_eq!(
        std::env::var_os("XDG_DATA_HOME").unwrap(),
        root.join("data")
    );
    let file = root.join("file");
    let folder = root.join("folder");
    let link = root.join("link");
    fs::write(&file, "file").unwrap();
    fs::create_dir(&folder).unwrap();
    fs::write(folder.join(".hidden"), "hidden").unwrap();
    fs::create_dir(root.join("preserved")).unwrap();
    fs::write(root.join("preserved/data"), "keep").unwrap();
    symlink(root.join("preserved"), &link).unwrap();
    let report = run(
        vec![target(&file), target(&folder), target(&link)],
        Mode::Trash,
        &AtomicBool::new(false),
        |_| {},
    );
    assert!(report.errors.is_empty(), "{:?}", report.errors);
    assert_eq!(report.completed.len(), 3);
    assert!(!file.exists() && !folder.exists() && fs::symlink_metadata(link).is_err());
}
