# Contributing

Use Rust 1.95 or newer and a Linux desktop. See README.md for system dependencies.

Before proposing a change, run:

```sh
cargo fmt --all --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
python3 scripts/package.py check
python3 -m unittest discover -s scripts -p 'test_*.py'
```

The metadata check needs Python 3.11+, `desktop-file-utils` and `appstream`.
Use temporary directories and isolated XDG directories when testing deletion,
restoration, associations or clipboard integration. Never exercise destructive tests
against personal files or the user's real Trash.

An opt-in FTP integration test uploads, downloads and moves temporary files/folders,
checks conflicts and cancels a transfer. Run it only against a writable disposable
FTP mount (for example, a loopback FTP server mounted through GVfs). Its files stay
inside a uniquely named temporary directory, which is removed after the test:

```sh
LSE_TEST_FTP_MOUNT=/path/to/disposable/ftp/mount cargo test --locked \
  transfers::tests::mounted_ftp_uploads_downloads_moves_and_conflicts_preserve_contents_and_sources \
  -- --ignored --nocapture
```

For the unsupported-attribute regression, configure the FTP fixture without
`SITE CHMOD` and `MFMT`. This exercises real FTP/GVfs errors without touching a
production server or depending on stored credentials.

Describe the problem, resulting behavior and validation. Add regression coverage for
changes to file operations, persistence or native integration. Avoid unrelated edits.
User-facing strings belong in `locales/messages.json`, with both English and Portuguese
translations. Human review of translations is required before store submission.

Contributions are under the project's MIT license. Preserve the original licenses and
notices of third-party material. Disclose AI-assisted code, documentation and assets in
contribution descriptions so maintainers can meet distribution-channel requirements.

`packaging/app-id` is the application identity shared by the Rust build and packaging.
Do not rename it casually: desktop integration and future store identity depend on it.
