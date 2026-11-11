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
}
