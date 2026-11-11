# Linux System Explorer

[Português](README.md)

A Linux desktop file manager written in Rust with egui/eframe, inspired by familiar
desktop file explorers. **Currently experimental; no stable release or store approval.**

## Features

- X11 and Wayland windows, folder tabs, keyboard navigation and a compact toolbar.
- Details, list and icon views, resizable/reorderable columns and folder-specific layouts.
- Zoom, sorting, filtering, multiple selection and configurable sidebar sections.
- Copy, move, rename, Trash, restore and confirmed permanent deletion.
- Native drag and drop, default application opening and an Open With chooser.
- Portuguese/English UI, themes and configurable Font Awesome icons.
- Stacked success/warning/error notifications with separate timing and live previews.

**Add network share** in the sidebar accepts SMB/UNC, SFTP, NFS, FTP and WebDAV addresses or an existing local mount. Remote connections select **GIO/GVfs in GNOME**, with GTK native authentication in a helper process, or **KIO Fuse in KDE**, using installed protocol modules. Work runs in the background, and Cancel stops waiting and terminates the GNOME helper; saved connection addresses/names reconnect when clicked. Passwords in URLs are rejected and never stored in preferences. Connection shortcuts can be removed from their context menu or Settings. Server discovery is not implemented.

**Share over the network** in folder context menus opens native folder properties through **Nautilus/nautilus-share in GNOME** or **kioclient/kdenetwork-filesharing in KDE**; use the sharing section for Samba permissions and setup. A configured Samba server is required. The explorer does not automatically publish folders, install a server, change firewall rules or alter permissions. Remote mounting requires `gvfs-backends`, `gvfs-fuse` and GTK 3/4 on GNOME, or `kio-fuse` and suitable `kio-extras` modules on KDE; existing local mounts work in other desktop environments. System clipboard file MIME interoperability is still pending; copy/cut/paste
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

## Tabs, new windows and integrated terminal

Folder context menus, sidebar shortcuts and the current-folder background offer **Open in new tab**, **Open in new window** and **Open terminal here**, with configurable icons. The compact tab strip uses touching tabs, an opaque theme-matching background and a tab-shaped **+** control with a configurable icon. Tabs have distinct outlines, folder icons, integrated close buttons and a subtle active-tab background based on the desktop accent, with neutral borders shared by all tabs. The accent is detected from KDE’s palette or the desktop appearance portal, falling back to the application palette if unavailable. Drag a tab to reorder it; an insertion marker shows the destination and the strip scrolls at its edges when tabs overflow. Escape or dropping outside the strip cancels the drag. The new order and active tab are saved for the next session when session restoration is enabled. Right-click a tab for **Duplicate tab**, **Close other tabs**, **Close tabs to the right** and **Reopen closed tab**. A duplicate opens immediately to the right and becomes active, with independent folder/history/filter/selection/view state; operations and terminal sessions stay in the original. Bulk closing uses the clicked tab as its reference and preserves tabs with file operations in progress. Unavailable actions are disabled, changes are saved in the session, and action icons are customizable. **Reopen closed tab** or **Ctrl+Shift+T** restores and activates the most recently closed tab, with its folder, neighbor position, history, filter, selection and view. The **Recently closed tabs** submenu on tabs and **+** lists up to 25 entries newest first, with folder names and paths in a scrollable menu. Selecting an entry reopens only that tab and removes only that entry from the saved history, including when folders repeat. Right-clicking **+** also offers the latest-tab action. Each window remembers up to 25 closed tabs, including bulk closures, and persists this history with the session across restarts, preserving folder, neighbor position, navigation history, filter, selection and layout. The history is also loaded when starting at home or opening an explicit folder. With multiple windows, the last changed/closed window supplies both the open session and the closed-tab history. Terminated terminals and file operations are not restarted, and restored selections are checked against the refreshed listing. Tabs keep independent folders, history, selection, filters, scrolling and layouts; clipboard transfers and notifications are shared within a window. Use **Ctrl+T**, **Ctrl+W**, **Ctrl+Tab** and **Ctrl+Shift+Tab** to manage tabs. New windows bypass reuse explicitly (`--new-window -- DIRECTORY`); normal system opening and Show in Folder also locate and activate matching inactive tabs. File operations continue in background tabs and prevent closing their tab until finished. Folder layouts remain persistent. Tab directories, their order and the active tab are saved automatically and restored when starting without a directory. In **Settings → General → On startup**, choose **Restore tabs from the last session** (default) or **Always start in the home folder**. This application-wide setting takes effect on the next launch.

The session lives separately from preferences in `$XDG_STATE_HOME/linux-system-explorer/session.json` (normally `~/.local/state/linux-system-explorer/session.json`). Duplicate tabs and non-UTF-8 directory names are preserved. With multiple windows, the most recently used/closed window is resumed. Explicit folder launches, Show in Folder and forced new windows keep their requested destination instead of restoring another session. Temporarily unavailable directories remain as tabs and can be retried with F5. Terminal shell processes are not restored.

The terminal is a resizable bottom dock within the application, backed by a real PTY and VT100 screen. It starts the user's shell in the folder chosen from the menu; shell directory changes and explorer navigation are independent. ANSI colors, interactive keyboard input, output scrollback, text selection, clipboard input and Ctrl+C are supported. Explorer keyboard shortcuts are suppressed while the terminal has focus. **F4** or the panel's close button hides it without terminating commands. Starting a session in another folder preserves the previous session in the selector; closing its explorer tab or window closes its terminals. Shell sessions do not auto-restore, and external terminal profiles (e.g. Konsole) are not embedded.

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
