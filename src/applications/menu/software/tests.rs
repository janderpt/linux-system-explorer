use super::*;
fn app() -> Application {
    Application {
        id: "editor.desktop".into(),
        path: "/usr/share/applications/editor.desktop".into(),
        name: "Editor $(not-a-shell)".into(),
        description: String::new(),
        recommended: false,
        is_default: false,
    }
}
#[test]
fn manager_selection_supports_kde_gnome_and_xfce_with_installed_fallbacks() {
    assert_eq!(choose("KDE", |_| true), Some(Manager::Discover));
    assert_eq!(choose("ubuntu:GNOME", |_| true), Some(Manager::Gnome));
    assert_eq!(choose("XFCE", |_| true), Some(Manager::Gnome));
    assert_eq!(
        choose("XFCE", |p| p == "plasma-discover"),
        Some(Manager::Discover)
    );
    assert_eq!(
        choose("KDE", |p| p == "gnome-software"),
        Some(Manager::Gnome)
    );
    assert_eq!(choose("XFCE", |_| false), None);
}
#[test]
fn managers_open_exact_appstream_ids_or_a_literal_name_search_without_package_operations() {
    for manager in [Manager::Discover, Manager::Gnome] {
        let command = manager.command(&app(), Some("org.example.Editor"));
        let args: Vec<_> = command.get_args().map(|s| s.to_str().unwrap()).collect();
        assert_eq!(
            args,
            if manager == Manager::Discover {
                vec!["--application", "appstream://org.example.Editor"]
            } else {
                vec!["--details=org.example.Editor"]
            }
        );
        let search = manager.command(&app(), None);
        assert_eq!(
            search.get_args().collect::<Vec<_>>(),
            vec!["--search=Editor $(not-a-shell)"]
        );
    }
}
#[test]
fn appstream_mapping_uses_desktop_launchable_instead_of_assuming_ids_match() {
    let xml = "<component type='desktop-application'><id>org.example.Editor</id><launchable type='desktop-id'>editor.desktop</launchable></component>";
    assert_eq!(
        metadata_id(xml, "editor.desktop").as_deref(),
        Some("org.example.Editor")
    );
    assert_eq!(metadata_id(xml, "different.desktop"), None);
    assert_eq!(
        metadata_id(
            "<component><id>editor.desktop</id></component>",
            "editor.desktop"
        )
        .as_deref(),
        Some("editor.desktop")
    );
    assert_eq!(
        metadata_id(
            "<component><id>bad/id</id><desktop_file>editor.desktop</desktop_file></component>",
            "editor.desktop"
        ),
        None
    );
    assert_eq!(metadata_id("<component>", "editor.desktop"), None);
    let root = tempfile::tempdir().unwrap();
    fs::create_dir(root.path().join("metainfo")).unwrap();
    fs::write(root.path().join("metainfo/unrelated-filename.xml"), xml).unwrap();
    assert_eq!(
        lookup(&app(), &[root.path().to_owned()]).as_deref(),
        Some("org.example.Editor")
    );
    fs::write(
        root.path().join("metainfo/unrelated-filename.xml"),
        "invalid",
    )
    .unwrap();
    assert_eq!(lookup(&app(), &[root.path().to_owned()]), None);
}
