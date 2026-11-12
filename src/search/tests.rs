use super::*;
use std::{
    fs,
    os::unix::{
        ffi::OsStringExt,
        fs::{PermissionsExt, symlink},
    },
};
fn request(root: &std::path::Path, query: &str, hidden: bool) -> Request {
    let (sender, _) = mpsc::sync_channel(1);
    Request {
        root: root.to_owned(),
        query: query.to_lowercase(),
        hidden,
        cancelled: Arc::new(AtomicBool::new(false)),
        sender,
        locale: crate::i18n::Locale::Portuguese,
    }
}
#[test]
fn scans_descendants_by_name_preserves_raw_paths_and_skips_hidden_directories_and_symlink_cycles() {
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    for dir in ["one/deeper", "two", ".private"] {
        fs::create_dir_all(root.path().join(dir)).unwrap();
    }
    for file in [
        "root-REPORT.txt",
        "one/deeper/report.txt",
        "two/report.txt",
        ".private/report.txt",
        "one/.report.txt",
    ] {
        fs::write(root.path().join(file), "original").unwrap();
    }
    let raw = root.path().join(std::ffi::OsString::from_vec(
        b"two/report-\xff.txt".to_vec(),
    ));
    fs::write(&raw, "raw").unwrap();
    fs::write(outside.path().join("report-outside.txt"), "outside").unwrap();
    symlink(outside.path(), root.path().join("outside")).unwrap();
    symlink(root.path(), root.path().join("one/cycle")).unwrap();
    let mut found = vec![];
    let progress = scan(&request(root.path(), "REPORT", false), |e, _| {
        if let Some(e) = e {
            found.push(e.path);
        }
        true
    })
    .unwrap();
    assert_eq!(found.len(), 4);
    assert_eq!(progress.matched, 4);
    assert!(found.contains(&raw));
    assert!(found.contains(&root.path().join("one/deeper/report.txt")));
    assert!(found.iter().all(|p| p.starts_with(root.path())));
    let mut hidden = vec![];
    scan(&request(root.path(), "report", true), |e, _| {
        if let Some(e) = e {
            hidden.push(e.path);
        }
        true
    })
    .unwrap();
    assert_eq!(hidden.len(), 6);
    assert_eq!(fs::read_to_string(&raw).unwrap(), "raw");
}
#[test]
fn replaced_directory_is_not_followed_and_permission_failures_are_partial() {
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    fs::create_dir(root.path().join("match-dir")).unwrap();
    fs::write(outside.path().join("match-secret"), "outside").unwrap();
    let mut found = vec![];
    let progress = scan(&request(root.path(), "match", false), |e, _| {
        if let Some(e) = e {
            if e.is_dir {
                fs::rename(
                    root.path().join("match-dir"),
                    root.path().join("original-dir"),
                )
                .unwrap();
                symlink(outside.path(), root.path().join("match-dir")).unwrap();
            }
            found.push(e);
        }
        true
    })
    .unwrap();
    assert_eq!(found.len(), 1);
    assert!(progress.skipped > 0);
    let blocked = root.path().join("blocked");
    fs::create_dir(&blocked).unwrap();
    fs::write(blocked.join("match-hidden"), "data").unwrap();
    fs::set_permissions(&blocked, fs::Permissions::from_mode(0o0)).unwrap();
    let progress = scan(&request(root.path(), "absent", false), |_, _| true).unwrap();
    fs::set_permissions(&blocked, fs::Permissions::from_mode(0o700)).unwrap();
    if rustix::process::geteuid().as_raw() != 0 {
        assert!(progress.skipped > 0);
    }
}
#[test]
fn cancelled_and_missing_roots_return_without_scanning_and_worker_accepts_newer_queries() {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir(root.path().join("sub")).unwrap();
    fs::write(root.path().join("sub/needle.txt"), "keep").unwrap();
    let r = request(root.path(), "needle", false);
    r.cancelled.store(true, Ordering::Release);
    assert_eq!(
        scan(&r, |_, _| panic!("cancelled")).unwrap_err().kind(),
        io::ErrorKind::Interrupted
    );
    assert!(
        scan(
            &request(&root.path().join("missing"), "needle", false),
            |_, _| true
        )
        .is_err()
    );
    let ctx = eframe::egui::Context::default();
    let worker = Worker::new(&ctx);
    let old = worker.start(root.path().into(), "obsolete".into(), false);
    drop(old);
    let current = worker.start(root.path().into(), "needle".into(), false);
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut entries = vec![];
    loop {
        assert!(Instant::now() < deadline);
        match current
            .incoming
            .recv_timeout(Duration::from_millis(100))
            .unwrap()
        {
            Update::Batch(batch) => entries.extend(batch),
            Update::Progress(_) => {}
            Update::Finished(p) => {
                assert_eq!(p.unwrap().matched, 1);
                break;
            }
        }
    }
    assert_eq!(entries[0].path, root.path().join("sub/needle.txt"));
}

#[test]
fn result_and_traversal_budgets_report_partial_results_without_reading_contents() {
    let root = tempfile::tempdir().unwrap();
    for name in ["hit-one", "hit-two", "hit-three"] {
        fs::write(root.path().join(name), "unchanged").unwrap();
    }
    let mut found = vec![];
    let p = scan_with_limits(
        &request(root.path(), "hit", false),
        Limits {
            results: 2,
            ..Default::default()
        },
        |e, _| {
            if let Some(e) = e {
                found.push(e);
            }
            true
        },
    )
    .unwrap();
    assert!(p.limited);
    assert_eq!(found.len(), 2);
    let p = scan_with_limits(
        &request(root.path(), "absent", false),
        Limits {
            visited: 1,
            ..Default::default()
        },
        |_, _| true,
    )
    .unwrap();
    assert!(p.limited);
    assert_eq!(p.visited, 1);
    fs::create_dir_all(root.path().join("a/b")).unwrap();
    fs::write(root.path().join("a/b/hit-deep"), "unchanged").unwrap();
    let p = scan_with_limits(
        &request(root.path(), "hit", false),
        Limits {
            depth: 1,
            ..Default::default()
        },
        |_, _| true,
    )
    .unwrap();
    assert!(p.limited && p.skipped > 0);
}
