# Linux System Explorer

[Português](README.md)

A Linux desktop file manager written in Rust with egui/eframe, inspired by familiar
desktop file explorers. **Currently experimental; no stable release or store approval.**

## Features

- X11 and Wayland windows, keyboard navigation and a compact toolbar.
- Details, list and icon views, resizable/reorderable columns and folder-specific layouts.
- Zoom, sorting, filtering, multiple selection and configurable sidebar sections.
- Copy, move, rename, Trash, restore and confirmed permanent deletion.
- Native drag and drop, default application opening and an Open With chooser.
- Portuguese/English UI, themes and configurable Font Awesome icons.
- Stacked success/warning/error notifications with separate timing and live previews.

Network shares in the sidebar are existing local mounts; the app does not mount remote
servers. System clipboard file MIME interoperability is still pending; copy/cut/paste
currently tracks transfers inside this app and exports path text. Drag and drop is a
separate native integration. Flatpak/Snap sandbox compatibility has not been certified.

## Build and run

Use Rust 1.95 or newer and a graphical Linux session with OpenGL. On Ubuntu/Debian:

```sh
sudo apt install build-essential pkg-config libxkbcommon-dev libwayland-dev libx11-dev libxi-dev libxcursor-dev libxrandr-dev libegl1-mesa-dev xdg-utils libglib2.0-bin
cargo run
cargo run -- /path/to/folder
cargo build --locked --release
./target/release/linux-system-explorer
```

`--help` and `--version` work without opening a window. Use `-- DIRECTORY` for a path
beginning with a dash. File opening relies on GIO, with xdg-open as the default-opening
fallback. Actual functionality follows the user's permissions and installed applications.

## Packaging

On a Debian/Ubuntu build host, install Python 3.11+, `dpkg-dev`, `desktop-file-utils`
and `appstream`, then:

```sh
python3 scripts/package.py check
cargo build --locked --release
python3 scripts/package.py build
cd dist
sha256sum --check SHA256SUMS
```

Draft native tar.gz and DEB packages, build information and checksums are written to
`dist/`. These target the build host's ABI. The workflow baseline is Ubuntu 24.04.
They require license/security review and clean-system testing before public distribution.
No store upload or installation is performed automatically.

See [distribution status](docs/DISTRIBUTION.md), [release checklist](docs/RELEASING.md),
[contributing](CONTRIBUTING.md), [security](SECURITY.md) and [changelog](CHANGELOG.md).

## Validate

```sh
cargo fmt --all --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
python3 scripts/package.py check
```

File-operation tests use disposable files and isolated Trash data. Native integration
still needs real desktop testing on supported distributions and package formats.

## License

Original source, native packaging, AppStream and the SVG application icon: [MIT](LICENSE).
Embedded Font Awesome Free retains its [third-party license](assets/fontawesome/LICENSE.txt).
Dependencies and embedded default fonts retain their individual licenses. The generated
license inventory is not a declaration that a maintainer has completed license review.

This project has received substantial AI assistance. Human review and channel-specific
disclosure are required before store submission; see docs/DISTRIBUTION.md.
