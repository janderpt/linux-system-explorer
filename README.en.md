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

To install a build from this repository for the current user:

```sh
git clone https://github.com/janderpt/linux-system-explorer.git
cd linux-system-explorer
cargo build --locked --release
python3 scripts/install-user.py install --set-default
```

Install the build and metadata-validation prerequisites listed below first.
The installer uses `~/.local/bin` and the user's XDG data directories, without sudo.
`--set-default` explicitly changes the `inode/directory` handler. Omit it to install
without changing defaults. It backs up previous files and records the previous folder
handler. To return to that handler: `python3 scripts/install-user.py restore-default`.
It leaves any newer folder-handler choice untouched. Other MIME associations and user
preferences are preserved. This flag also registers the per-user FileManager1 D-Bus
service for "Show in Folder", opening parent folders with the requested items selected,
including hidden files. Files in different folders open in separate windows. Folder
and properties requests are also supported. Only local file URIs are accepted; network
shares must be mounted locally. The bridge activates even when the explorer is closed.
An already running file-manager service keeps its bus name until it exits or the next
login. Rollback restores the previous service registration as well as the folder handler.
Native package installation alone does not change these defaults.

Show in Folder reuses a responsive window already displaying (or loading) the
requested directory. It refreshes the listing, clears the filter, selects/reveals
the items and requests foreground activation without changing other windows' folders.
It opens a new window when no matching window is available. Each window verifies its
own current destination and unregisters when closed. Wayland activation forwards the
caller's token through xdg-activation; compositor focus policies still apply. Reopen
windows from earlier versions after updating to enable reuse.

Opening a directory through the system's desktop launcher (`xdg-open`, GIO) or the
executable also reuses a matching window. The launching process forwards its activation
token and exits before initializing another GUI, preserving the window's selection,
filter and layout. Starting without a path applies this to the home directory. A new
window opens normally when there is no responsive match.

Window, launcher and taskbar icons share one project SVG and its build-generated PNG.
The application ID, desktop filename and icon name match on Wayland and X11. The user
installer refreshes KDE's application and icon caches; reopen older windows after updating.

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
