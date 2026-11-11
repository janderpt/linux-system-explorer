# Contributing

Use Rust 1.95 or newer and a Linux desktop. See README.md for system dependencies.

Before proposing a change, run:

```sh
cargo fmt --all --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
python3 scripts/package.py check
```

The metadata check needs Python 3.11+, `desktop-file-utils` and `appstream`.
Use temporary directories and isolated XDG directories when testing deletion,
restoration, associations or clipboard integration. Never exercise destructive tests
against personal files or the user's real Trash.

Describe the problem, resulting behavior and validation. Add regression coverage for
changes to file operations, persistence or native integration. Avoid unrelated edits.
User-facing strings belong in `locales/messages.json`, with both English and Portuguese
translations. Human review of translations is required before store submission.

Contributions are under the project's MIT license. Preserve the original licenses and
notices of third-party material. Disclose AI-assisted code, documentation and assets in
contribution descriptions so maintainers can meet distribution-channel requirements.

`packaging/app-id` is the application identity shared by the Rust build and packaging.
Do not rename it casually: desktop integration and future store identity depend on it.
