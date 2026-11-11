fn main() {
    println!("cargo:rerun-if-changed=packaging/app-id");
    let id = std::fs::read_to_string("packaging/app-id").expect("missing application ID");
    let id = id.trim();
    let parts: Vec<_> = id.split('.').collect();
    assert!(
        parts.len() >= 3 && id.len() <= 255,
        "invalid application ID"
    );
    for (index, part) in parts.iter().enumerate() {
        assert!(
            !part.is_empty()
                && !part.starts_with(|c: char| c.is_ascii_digit())
                && part.bytes().all(|c| c.is_ascii_alphanumeric()
                    || c == b'_'
                    || (c == b'-' && index + 1 == parts.len())),
            "invalid application ID component"
        );
    }
    println!("cargo:rustc-env=LSE_APP_ID={id}");
    println!("cargo:rerun-if-changed=assets/icons/linux-system-explorer.svg");
    let svg =
        std::fs::read("assets/icons/linux-system-explorer.svg").expect("missing application icon");
    let tree = resvg::usvg::Tree::from_data(&svg, &resvg::usvg::Options::default())
        .expect("invalid application SVG");
    let mut pixels = resvg::tiny_skia::Pixmap::new(256, 256).expect("icon allocation failed");
    let size = tree.size();
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_scale(256.0 / size.width(), 256.0 / size.height()),
        &mut pixels.as_mut(),
    );
    let output = std::path::PathBuf::from(std::env::var_os("OUT_DIR").expect("OUT_DIR missing"));
    pixels
        .save_png(output.join("application-icon.png"))
        .expect("could not encode application icon");
}
