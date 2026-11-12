use super::*;

fn request(paths: Vec<PathBuf>, destination: &Path, mode: Mode) -> Request {
    Request {
        clipboard: Clipboard { id: 7, mode, paths },
        destination: destination.to_owned(),
    }
}

fn execute(paths: Vec<PathBuf>, destination: &Path, mode: Mode, choice: Choice) -> Report {
    run(
        request(paths, destination, mode),
        Arc::new(AtomicBool::new(false)),
        |_| Decision {
            choice,
            apply_all: true,
        },
        |_| {},
    )
}

#[test]
fn copies_nested_hidden_files_and_symlinks_without_following_them() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source");
    let destination = root.path().join("destination");
    fs::create_dir_all(source.join("nested")).unwrap();
    fs::create_dir(&destination).unwrap();
    fs::write(source.join("nested/data"), b"binary\0data").unwrap();
    fs::write(source.join(".hidden"), b"hidden").unwrap();
    fs::set_permissions(
        source.join("nested/data"),
        fs::Permissions::from_mode(0o640),
    )
    .unwrap();
    symlink("missing", source.join("broken")).unwrap();
    symlink("..", source.join("parent")).unwrap();
    let original = fs::metadata(source.join("nested/data")).unwrap();
    let report = execute(vec![source.clone()], &destination, Mode::Copy, Choice::Skip);
    assert!(report.errors.is_empty(), "{:?}", report.errors);
    assert_eq!(report.completed, vec![source.clone()]);
    let copy = destination.join("source");
    assert_eq!(fs::read(copy.join("nested/data")).unwrap(), b"binary\0data");
    assert_eq!(fs::read(copy.join(".hidden")).unwrap(), b"hidden");
    assert_eq!(fs::read_link(copy.join("parent")).unwrap(), Path::new(".."));
    assert_eq!(
        fs::read_link(copy.join("broken")).unwrap(),
        Path::new("missing")
    );
    let copied = fs::metadata(copy.join("nested/data")).unwrap();
    assert_eq!(copied.permissions().mode() & 0o777, 0o640);
    assert_eq!(copied.modified().unwrap(), original.modified().unwrap());
    assert!(source.join("nested/data").exists());
    assert_eq!(
        fs::read_dir(&destination).unwrap().count(),
        1,
        "sem pasta temporária residual"
    );
}

#[test]
fn cut_moves_files_and_folders_and_pasting_in_same_folder_is_a_noop() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source");
    let destination = root.path().join("destination");
    fs::create_dir(&source).unwrap();
    fs::create_dir(&destination).unwrap();
    fs::create_dir(source.join("tree")).unwrap();
    fs::write(source.join("tree/file"), "inside").unwrap();
    fs::write(source.join("plain"), "plain").unwrap();
    let paths = vec![source.join("tree"), source.join("plain")];
    let report = execute(paths.clone(), &destination, Mode::Cut, Choice::Skip);
    assert!(report.errors.is_empty());
    assert_eq!(report.completed.len(), 2);
    assert!(paths.iter().all(|p| !p.exists()));
    assert_eq!(
        fs::read_to_string(destination.join("tree/file")).unwrap(),
        "inside"
    );
    let plain = destination.join("plain");
    let report = execute(vec![plain.clone()], &destination, Mode::Cut, Choice::Cancel);
    assert_eq!(report.completed, vec![plain.clone()]);
    assert_eq!(fs::read_to_string(plain).unwrap(), "plain");
}

#[test]
fn conflicts_keep_both_skip_or_cancel_without_overwriting() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("report.txt");
    let destination = root.path().join("destination");
    fs::create_dir(&destination).unwrap();
    fs::write(&source, "new").unwrap();
    fs::write(destination.join("report.txt"), "existing").unwrap();
    for mode in [Mode::Copy, Mode::Cut] {
        let report = execute(vec![source.clone()], &destination, mode, Choice::Skip);
        assert_eq!(report.skipped, 1);
        assert!(report.completed.is_empty() && source.exists());
    }
    let report = execute(
        vec![source.clone()],
        &destination,
        Mode::Copy,
        Choice::Cancel,
    );
    assert!(report.cancelled && source.exists());
    for name in ["report (cópia).txt", "report (cópia 2).txt"] {
        let report = execute(
            vec![source.clone()],
            &destination,
            Mode::Copy,
            Choice::KeepBoth,
        );
        assert!(report.errors.is_empty());
        assert_eq!(fs::read_to_string(destination.join(name)).unwrap(), "new");
    }
    assert_eq!(
        fs::read_to_string(destination.join("report.txt")).unwrap(),
        "existing"
    );
    let report = execute(
        vec![source.clone()],
        &destination,
        Mode::Cut,
        Choice::KeepBoth,
    );
    assert!(report.errors.is_empty() && !source.exists());
    assert_eq!(
        fs::read_to_string(destination.join("report (cópia 3).txt")).unwrap(),
        "new"
    );
}

#[test]
fn copying_in_place_keeps_both_and_self_descendants_including_symlinks_are_rejected() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("tree");
    fs::create_dir_all(source.join("nested")).unwrap();
    let file = source.join("a");
    fs::write(&file, "original").unwrap();
    let report = execute(vec![file.clone()], &source, Mode::Copy, Choice::KeepBoth);
    assert!(report.errors.is_empty());
    assert_eq!(
        fs::read_to_string(source.join("a (cópia)")).unwrap(),
        "original"
    );
    let alias = root.path().join("alias");
    symlink(source.join("nested"), &alias).unwrap();
    for target in [&source, &source.join("nested"), &alias] {
        for mode in [Mode::Copy, Mode::Cut] {
            let report = execute(vec![source.clone()], target, mode, Choice::KeepBoth);
            assert_eq!(report.errors.len(), 1);
            assert!(report.completed.is_empty());
            assert_eq!(fs::read_to_string(&file).unwrap(), "original");
        }
    }
}

#[test]
fn cancellation_leaves_source_intact_and_cleans_staging_directory() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("large");
    let destination = root.path().join("destination");
    fs::create_dir(&destination).unwrap();
    let data = vec![42; 1024 * 1024];
    fs::write(&source, &data).unwrap();
    let cancel = Arc::new(AtomicBool::new(false));
    let trigger = cancel.clone();
    let report = run(
        request(vec![source.clone()], &destination, Mode::Copy),
        cancel,
        |_| panic!("sem conflito"),
        move |progress| {
            if progress.bytes > 0 {
                trigger.store(true, Ordering::Relaxed);
            }
        },
    );
    assert!(report.cancelled && report.completed.is_empty());
    assert_eq!(fs::read(&source).unwrap(), data);
    assert_eq!(fs::read_dir(&destination).unwrap().count(), 0);
}

#[test]
fn failures_are_reported_per_item_and_sockets_do_not_block_copy() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("good");
    let destination = root.path().join("destination");
    fs::create_dir(&destination).unwrap();
    fs::write(&source, "good").unwrap();
    let socket = root.path().join("socket");
    let _listener = std::os::unix::net::UnixListener::bind(&socket).unwrap();
    let report = execute(
        vec![source.clone(), root.path().join("missing"), socket],
        &destination,
        Mode::Copy,
        Choice::Skip,
    );
    assert_eq!(report.completed, vec![source.clone()]);
    assert_eq!(report.errors.len(), 2);
    assert!(source.exists());
    assert_eq!(
        fs::read_to_string(destination.join("good")).unwrap(),
        "good"
    );
    assert_eq!(fs::read_dir(&destination).unwrap().count(), 1);
}

#[test]
fn fallback_move_copies_completely_before_deleting_source() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source");
    let target = root.path().join("target");
    fs::create_dir_all(source.join("nested")).unwrap();
    fs::write(source.join("nested/file"), "data").unwrap();
    symlink("nested/file", source.join("link")).unwrap();
    let cancel = AtomicBool::new(false);
    let mut notify = |_| {};
    let mut work = Work {
        cancel: &cancel,
        progress: Progress::default(),
        notify: &mut notify,
    };
    copy_then_publish(&source, &target, Mode::Cut, &mut work).unwrap();
    assert!(!source.exists());
    assert_eq!(
        fs::read_to_string(target.join("nested/file")).unwrap(),
        "data"
    );
    assert_eq!(
        fs::read_link(target.join("link")).unwrap(),
        Path::new("nested/file")
    );
}

#[test]
fn changed_source_aborts_fallback_move_without_deleting_any_source() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source");
    let target = root.path().join("target");
    fs::write(&source, "old").unwrap();
    let cancel = AtomicBool::new(false);
    let mut changed = false;
    let mut notify = |progress: Progress| {
        if progress.bytes > 0 && !changed {
            fs::write(&source, "new-data").unwrap();
            changed = true;
        }
    };
    let mut work = Work {
        cancel: &cancel,
        progress: Progress::default(),
        notify: &mut notify,
    };
    assert!(copy_then_publish(&source, &target, Mode::Cut, &mut work).is_err());
    assert_eq!(fs::read_to_string(&source).unwrap(), "new-data");
    assert!(!target.exists());
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
}

#[test]
fn cut_between_distinct_linux_filesystems_uses_copy_fallback() {
    let source_root = tempfile::tempdir().unwrap();
    let Ok(destination_root) = tempfile::tempdir_in("/dev/shm") else {
        return;
    };
    if fs::metadata(source_root.path()).unwrap().dev()
        == fs::metadata(destination_root.path()).unwrap().dev()
    {
        return;
    }
    let source = source_root.path().join("tree");
    fs::create_dir(&source).unwrap();
    fs::write(source.join("file"), "cross-device").unwrap();
    symlink("file", source.join("link")).unwrap();
    let report = execute(
        vec![source.clone()],
        destination_root.path(),
        Mode::Cut,
        Choice::Skip,
    );
    assert!(report.errors.is_empty(), "{:?}", report.errors);
    assert_eq!(report.completed, vec![source.clone()]);
    assert!(!source.exists());
    assert_eq!(
        fs::read_to_string(destination_root.path().join("tree/file")).unwrap(),
        "cross-device"
    );
    assert_eq!(
        fs::read_link(destination_root.path().join("tree/link")).unwrap(),
        Path::new("file")
    );
}

#[test]
fn unsupported_attributes_are_optional_but_permission_and_storage_errors_are_preserved() {
    assert!(optional_metadata(Ok(())).is_ok());
    assert!(optional_metadata(Err(io::Error::from_raw_os_error(95))).is_ok());
    for errno in [1, 5, 13, 28, 30] {
        let error = optional_metadata(Err(io::Error::from_raw_os_error(errno))).unwrap_err();
        assert_eq!(error.raw_os_error(), Some(errno));
    }
}

#[test]
#[ignore = "Explicit FTP transfer integration; requires LSE_TEST_FTP_MOUNT (a writable test mount)"]
fn mounted_ftp_uploads_downloads_moves_and_conflicts_preserve_contents_and_sources() {
    let mount = PathBuf::from(
        std::env::var_os("LSE_TEST_FTP_MOUNT")
            .expect("Set LSE_TEST_FTP_MOUNT to a writable FTP mount"),
    );
    let remote = tempfile::Builder::new()
        .prefix("lse-transfer-test-")
        .tempdir_in(mount)
        .unwrap();
    let local = tempfile::tempdir().unwrap();
    let source = local.path().join("upload.bin");
    let data: Vec<u8> = (0..600_000).map(|i| (i % 251) as u8).collect();
    fs::write(&source, &data).unwrap();
    let tree = local.path().join("folder");
    fs::create_dir_all(tree.join("nested")).unwrap();
    fs::write(tree.join("nested/file.txt"), "nested FTP upload").unwrap();
    let report = execute(
        vec![source.clone(), tree.clone()],
        remote.path(),
        Mode::Copy,
        Choice::Cancel,
    );
    assert!(report.errors.is_empty(), "{:?}", report.errors);
    assert_eq!(report.completed, [source.clone(), tree.clone()]);
    assert_eq!(fs::read(remote.path().join("upload.bin")).unwrap(), data);
    assert_eq!(fs::read(&source).unwrap(), data);
    assert_eq!(
        fs::read_to_string(remote.path().join("folder/nested/file.txt")).unwrap(),
        "nested FTP upload"
    );
    let report = execute(
        vec![source.clone()],
        remote.path(),
        Mode::Copy,
        Choice::Skip,
    );
    assert_eq!(report.skipped, 1);
    let report = execute(
        vec![source.clone()],
        remote.path(),
        Mode::Copy,
        Choice::KeepBoth,
    );
    assert!(report.errors.is_empty(), "{:?}", report.errors);
    assert_eq!(
        fs::read(remote.path().join("upload (cópia).bin")).unwrap(),
        data
    );
    let download = local.path().join("download");
    fs::create_dir(&download).unwrap();
    let report = execute(
        vec![remote.path().join("upload.bin")],
        &download,
        Mode::Copy,
        Choice::Cancel,
    );
    assert!(report.errors.is_empty(), "{:?}", report.errors);
    assert_eq!(fs::read(download.join("upload.bin")).unwrap(), data);
    assert!(remote.path().join("upload.bin").exists());
    let moved = local.path().join("move.bin");
    fs::write(&moved, &data).unwrap();
    let report = execute(
        vec![moved.clone()],
        remote.path(),
        Mode::Cut,
        Choice::Cancel,
    );
    assert!(report.errors.is_empty(), "{:?}", report.errors);
    assert!(!moved.exists());
    assert_eq!(fs::read(remote.path().join("move.bin")).unwrap(), data);
    let cancelled_source = local.path().join("cancel.bin");
    fs::write(&cancelled_source, &data).unwrap();
    let cancel = Arc::new(AtomicBool::new(false));
    let trigger = cancel.clone();
    let report = run(
        request(vec![cancelled_source.clone()], remote.path(), Mode::Cut),
        cancel,
        |_| panic!("no conflict"),
        |progress| {
            if progress.bytes > 0 {
                trigger.store(true, Ordering::Relaxed);
            }
        },
    );
    assert!(report.cancelled && report.completed.is_empty());
    assert_eq!(fs::read(&cancelled_source).unwrap(), data);
    assert!(!remote.path().join("cancel.bin").exists());
    assert_eq!(fs::read_dir(remote.path()).unwrap().count(), 4);
}
