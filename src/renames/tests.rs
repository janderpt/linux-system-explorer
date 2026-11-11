use super::*;
use std::{
    ffi::OsString,
    fs,
    os::unix::{ffi::OsStringExt, fs::symlink},
};

#[test]
fn renames_files_directories_and_links_without_overwriting_or_following_links() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("old.txt");
    fs::write(&source, "original").unwrap();
    let entry = filesystem::read_entry(&source).unwrap();
    let destination = rename(&entry, "new ' $name.txt").unwrap();
    assert!(!source.exists());
    assert_eq!(fs::read_to_string(&destination).unwrap(), "original");
    let folder = root.path().join("folder");
    fs::create_dir(&folder).unwrap();
    fs::write(folder.join("child"), "child").unwrap();
    let moved = rename(&filesystem::read_entry(&folder).unwrap(), "renamed").unwrap();
    assert_eq!(fs::read_to_string(moved.join("child")).unwrap(), "child");
    let link = root.path().join("link");
    symlink(&destination, &link).unwrap();
    let new_link = rename(&filesystem::read_entry(&link).unwrap(), "new-link").unwrap();
    assert_eq!(fs::read_link(new_link).unwrap(), destination);
    let dangling = root.path().join("occupied");
    symlink(root.path().join("missing"), &dangling).unwrap();
    let entry = filesystem::read_entry(&destination).unwrap();
    assert_eq!(
        rename(&entry, "occupied").unwrap_err().kind(),
        io::ErrorKind::AlreadyExists
    );
    assert!(dangling.is_symlink());
    assert_eq!(fs::read_to_string(&destination).unwrap(), "original");
    for name in [
        "",
        "  ",
        ".",
        "..",
        "../escape",
        "a/b",
        "/absolute",
        "nul\0name",
    ] {
        assert!(rename(&entry, name).is_err());
    }
}

#[test]
fn stale_sources_and_non_utf8_names_are_handled_without_changing_other_items() {
    let root = tempfile::tempdir().unwrap();
    let source = root
        .path()
        .join(OsString::from_vec(b"raw-\xff.txt".to_vec()));
    fs::write(&source, "original").unwrap();
    let entry = filesystem::read_entry(&source).unwrap();
    assert_eq!(rename(&entry, &entry.name).unwrap(), source);
    let changed = rename(&entry, "readable.txt").unwrap();
    assert!(!source.exists());
    let stale = filesystem::read_entry(&changed).unwrap();
    fs::rename(&changed, root.path().join("preserved")).unwrap();
    fs::write(&changed, "replacement").unwrap();
    assert!(rename(&stale, "should-not-exist").is_err());
    assert_eq!(fs::read_to_string(changed).unwrap(), "replacement");
    assert!(!root.path().join("should-not-exist").exists());
}
