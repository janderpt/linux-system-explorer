use super::*;
#[test]
fn menu_is_flat_localized_and_respects_overrides_visibility_and_desktop_rules() {
    let root = tempfile::tempdir().unwrap();
    let user = root.path().join("user");
    let system = root.path().join("system");
    std::fs::create_dir_all(user.join("nested")).unwrap();
    std::fs::create_dir(&system).unwrap();
    let write = |directory: &Path, name: &str, extra: &str| {
        std::fs::write(directory.join(name),format!("[Desktop Entry]\nType=Application\nName={name}\nName[pt]=Nome {name}\nExec=/bin/true\n{extra}")).unwrap()
    };
    write(&system, "masked.desktop", "");
    write(&user, "masked.desktop", "Hidden=true\n");
    write(&user, "nested/tool.desktop", "Categories=Utility;\n");
    write(&system, "ordinary.desktop", "");
    write(&system, "hidden.desktop", "NoDisplay=true\n");
    write(&system, "only.desktop", "OnlyShowIn=KDE;\n");
    write(&system, "excluded.desktop", "NotShowIn=KDE;\n");
    write(
        &system,
        "missing.desktop",
        "TryExec=/definitely-not-an-installed-program\n",
    );
    std::fs::write(
        system.join("dbus.desktop"),
        "[Desktop Entry]\nType=Application\nName=D-Bus app\nDBusActivatable=true\n",
    )
    .unwrap();
    let found = collect([user.clone(), system], &["pt"], "KDE");
    let ids: Vec<_> = found.iter().map(|(a, _)| a.id.as_str()).collect();
    assert_eq!(
        ids,
        vec![
            "dbus.desktop",
            "nested-tool.desktop",
            "only.desktop",
            "ordinary.desktop"
        ]
    );
    assert!(
        found
            .iter()
            .find(|(a, _)| a.id == "ordinary.desktop")
            .unwrap()
            .0
            .name
            .starts_with("Nome")
    );
    assert!(found.iter().all(|(a, _)| !a.id.contains('/')));
}
