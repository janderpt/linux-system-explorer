# Linux System Explorer

[Português](README.md)

A Linux desktop file manager written in Rust with egui/eframe, inspired by familiar
desktop file explorers. **Source published on GitHub; version 0.1.0-alpha.2 remains a development build, without a stable release or store approval.**


**v0.1.0-alpha.2** prerelease: Ubuntu 24.04+ x86_64 packages, checksums and [installation notes](docs/releases/v0.1.0-alpha.2.md). [GitHub downloads](https://github.com/janderpt/linux-system-explorer/releases/tag/v0.1.0-alpha.2).

## File previews with Space

Select a file and press **Space**, or choose **Preview** from its context menu. The explorer's popup uses the active pane's selection. Pressing Space again, Escape, Close or clicking outside stops playback and closes it. Images and videos fit the available area, centered and preserving their aspect ratio. Video popups follow the decoded landscape, portrait or square format within the window limits, with responsive playback controls. Opening the system default application is a separate explicit action. Space still types normally in text fields and the terminal.

The footer stays anchored during loading and scrolling, and viewer controls are centered. **Up/Down** select the previous/next file in the current sorted, filtered folder, skipping directories; **Left/Right** change pages or slides.

Supported previews include PNG/JPEG/WebP/BMP/TIFF/ICO/GIF (first frame), SVG, paginated PDF, bounded plain text/code and internal audio/video playback with pause, seek and volume. Images support fit, zoom and scrolling. CSV is shown as text; external SVG image references are disabled. PDF page navigation, page count and zoom share one toolbar. Office previews cover DOC/DOCX/ODT/RTF, XLS/XLSX/ODS, PPT/PPTX/ODP, ODG drawings and ODF formulas, plus templates and Flat XML variants. They retain document formatting, spreadsheet print layouts and static slides through isolated LibreOffice Writer/Calc/Impress/Draw/Math conversion, cached while the popup remains open. HTML and Markdown offer **Source** and **Presentation**. Markdown supports headings, lists, tables, code blocks, task lists and footnotes. Static presentation uses WeasyPrint and HTML print styles, with local document-directory resources, without scripts or remote requests. Archive previews list names, directories, links and sizes for ZIP, TAR, compressed TAR, 7z, RAR and other libarchive formats without extracting or opening entries. Single compressed streams show their contained item. An isolated, cancellable worker limits memory/time and lists up to 5,000 entries, marking partial results. Encrypted headers or damaged archives can prevent listing. Unsupported files only produce warning toasts, without empty popups; rendering failures use error toasts. HTML/Markdown presentation failures retain the source view and report concise errors instead of library tracebacks. Loading runs in background workers and leaves source files unchanged.

Image decoders, SVG rendering and fallback fonts are compiled into the application. PDF uses Poppler, Office previews use LibreOffice Writer/Calc/Impress/Draw/Math and HTML/Markdown presentation uses WeasyPrint; media uses GStreamer, independent of KDE/GNOME viewers and the X11/Wayland window system. [Runtime dependencies](packaging/preview-runtime.json) are declared in the DEB (Base/Good/Libav plugins, `poppler-utils`, LibreOffice Writer/Calc/Impress/Draw/Math, Liberation fonts, WeasyPrint, Python 3 and libarchive). Missing converters only disable their corresponding preview; HTML/Markdown source and the rest of the explorer remain usable. Office conversion copies the source into private temporary storage, disables macros/active content and preserves the personal LibreOffice profile. Office input is limited to 256 MiB and copied in bounded chunks instead of loading the entire file into memory. Conversion allows two minutes and remains cancellable through closing the preview. Installed fonts may affect pagination. Source/tar installations need these components installed; other package formats must provide equivalent runtimes and undergo compatibility testing. Flatpak and AppImage packages remain in preparation.

## Thumbnails and file sizes

Icon view generates content thumbnails for raster images, SVG, PDFs, videos and Office/LibreOffice documents, preserving aspect ratio without cropping. Only visible entries request work; at most two decoders run concurrently, with a bounded in-memory cache per tab. Listing metadata changes invalidate cached content, F5 retries failed thumbnails, and navigation/scrolling cancels unnecessary pending work. SVG thumbnails limit input to 2 MiB. Videos use a silent, paused GStreamer frame. Office/LibreOffice files use the first page/slide via an isolated LibreOffice conversion and Poppler, including DOC/DOCX/ODT, XLS/XLSX/ODS, PPT/PPTX/ODP and the other supported preview formats. At most one background Office conversion runs at once, with a 30-second conversion deadline and 256 MiB input limit; video frame generation allows 10 seconds. These use the existing declared preview runtimes. Unsupported/corrupt files, missing codecs and missing converters keep their file-type icons without repeated notices. Details and List retain compact icons.

Properties show logical size and **Size on disk**, with exact byte counts. Folder totals and file/link/subfolder counts are calculated in the background, including hidden entries; closing cancels the worker and reopening recalculates. Symbolic links are not followed. Logical folder size sums file/link lengths by directory entry; allocation uses Linux-reported blocks × 512, includes directory blocks and counts each hard-linked inode once. Sparse files may allocate less than their length. Compression/shared-block accounting depends on the filesystem. Unreadable/changed entries mark totals partial. FD-relative no-follow traversal prevents a replaced directory from becoming a followed symbolic link.

## Features

- X11 and Wayland windows, folder tabs, keyboard navigation and a compact toolbar.
- Details, list and icon views, resizable/reorderable columns and folder-specific layouts.
- Zoom, sorting, filtering, multiple selection and configurable sidebar sections.
- Copy, move, rename, Trash, restore and confirmed permanent deletion.
- Native drag and drop, default application opening and an Open With chooser.
- Portuguese/English UI, themes and configurable Font Awesome icons.
- Stacked success/warning/error notifications with separate timing and live previews.

Pending cut items are dimmed (icons, names and metadata) in Details, List and Icons, in both themes and across tabs/panes displaying their source. **Escape** while browsing cancels a pending cut before pasting and restores its appearance without changing files. Copying or cutting another selection replaces that state. Text fields, dialogs and the terminal retain their Escape behavior; transfers already started keep their explicit cancellation controls.

The **Split into two panes** toolbar button (two columns, beside Settings) opens the current folder in two side-by-side panes within the same window. Each pane has its own sidebar and independent navigation, filtering, selection, scrolling and view state. A sidebar shortcut navigates only its own pane. Click a pane to activate it; a subtle top line identifies the pane controlled by the toolbar, terminal and keyboard shortcuts. Drag the center divider to resize. Clipboard transfers and file drag/drop work between panes with the existing transfer modifiers. The split uses window tabs: enabling it creates a new tab for the right pane, and choosing another tab replaces the active pane. **Show one pane** keeps the active folder visible and preserves the other as a tab; closing either visible tab ends the split. With session restoration enabled, the saved session also restores the split, each pane's folder, the active pane and the divider ratio using the existing saved tabs, even after tab reordering. A narrower window adjusts the ratio to respect minimum pane widths. Disabling the split or closing a visible pane saves the single-pane layout. Older sessions, home-folder startup and explicit-folder launches retain their existing behavior.

**Add network share** in the sidebar accepts SMB/UNC, SFTP, NFS, FTP and WebDAV addresses or an existing local mount. Remote connections select **GIO/GVfs in GNOME**, with GTK native authentication in a helper process, or **KIO Fuse in KDE**, using installed protocol modules. For SMB server-share listing and connections, a failed KIO operation or an unreadable mount retries through GIO/GVfs when available, allowing temporary authentication without requiring a saved password. To enable this fallback on KDE, also install `gvfs-backends`, `gvfs-fuse` and GTK 3 or 4. The folder listing is checked before saving the connection or navigating. Work runs in the background, and Cancel stops waiting and terminates the GNOME helper; saved connection addresses/names reconnect when clicked. Passwords in URLs are rejected and never stored in preferences. Connection shortcuts can be removed from their context menu or Settings. Server discovery is not implemented.

Breadcrumbs and **Ctrl+L** display the network address of mounted connections instead of internal GVfs/KIO paths. Tabs show the share name at its root and the current folder name in subfolders; tab tooltips and recently closed tabs also display network addresses. Breadcrumb ancestors remain clickable, and moving up from an SMB share root opens the server share list. Presentation uses saved connections and recognizes GVfs SMB mounts even after removing their sidebar shortcut. File operations and per-folder preferences retain the underlying local mount paths.

The **Local shares** sidebar section lists this computer's Samba user shares by their published name; clicking opens the local folder. The manager updates the sidebar immediately after creating, renaming or stopping a share, with one shared cache across tabs. Other windows and external changes are refreshed in the background every five seconds; F5 requests an earlier refresh. Refresh failures retain the last known list with a visible warning. Visibility is independently configurable in Settings → Sidebar. A small gear icon beside the **Local shares** heading opens the manager, with a **Manage local shares** tooltip. Owners can edit a share's name, description and per-user/group read, write or deny permissions; other owners' shares remain read only. Copy buttons provide SMB and Windows addresses for the saved share, with a choice of local host addresses. **Stop sharing** asks for confirmation and preserves the folder and all files; existing connections may remain until closed. Folder **Share over the network** opens the manager for that folder, selecting an existing share or preparing a new one. New shares grant only the current user read/write access and disable guests. Existing guest settings and Linux filesystem permissions are preserved. Duplicate names and external changes are checked; renaming creates the new share first and rolls it back on failure, reporting cleanup failures. Operations run in the background without terminal password prompts. Samba usershares must be configured for the desktop user (`sambashare` on Ubuntu/Debian). This manager does not list/edit administrative `smb.conf` shares. **System sharing settings** retains access to Nautilus/nautilus-share or KDE native properties for setup. Server installation, passwords, services and firewall remain system-owned. Remote mounting requires `gvfs-backends`, `gvfs-fuse` and GTK 3/4 on GNOME, or `kio-fuse` and suitable `kio-extras` modules on KDE; existing local mounts work in other desktop environments. System clipboard file MIME interoperability is still pending; copy/cut/paste
currently tracks transfers inside this app and exports path text. Drag and drop is a
separate native integration. Flatpak/Snap sandbox compatibility has not been certified.

## Build and run

Use Rust 1.95 or newer and a graphical Linux session with OpenGL. Install Rust through [rustup](https://rustup.rs/). On Ubuntu 24.04 or newer:

```sh
sudo apt update
sudo apt install git python3 desktop-file-utils appstream build-essential pkg-config libxkbcommon-dev libxkbcommon-x11-0 libwayland-dev libx11-dev libxi-dev libxcursor-dev libxrandr-dev libegl1-mesa-dev xdg-utils libglib2.0-bin
sudo apt install poppler-utils libgstreamer1.0-0 libgstreamer-plugins-base1.0-0 gstreamer1.0-plugins-base gstreamer1.0-plugins-good gstreamer1.0-libav libreoffice-writer libreoffice-calc libreoffice-impress libreoffice-draw libreoffice-math fonts-liberation weasyprint libarchive13t64
git clone https://github.com/janderpt/linux-system-explorer.git
cd linux-system-explorer
cargo run --locked
cargo run --locked -- /path/to/folder
cargo build --locked --release
./target/release/linux-system-explorer
```

Run Cargo commands from the cloned directory containing `Cargo.toml`. If Rust was just installed, open a new terminal or run `source "$HOME/.cargo/env"`. Other distributions need equivalent packages; see [preview-runtime.json](packaging/preview-runtime.json), including libarchive 3. Network backends and Samba are optional according to the features used. Install `ffmpeg` to run the complete media test suite.

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
python3 scripts/install-user.py install
```

Install the build, runtime and metadata-validation prerequisites listed above first. To enable the default folder handler and Show in Folder integration, run `python3 scripts/install-user.py install --set-default`.
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

## Update an existing user installation

From the cloned repository:

```sh
git pull --ff-only origin main
cargo build --locked --release
python3 scripts/install-user.py install
```

Install newly listed dependencies first. Close and reopen all explorer windows after updating; existing windows keep the older executable. Per-folder preferences, tabs and session data remain in the user's XDG directories. The current default folder association is preserved.

## Validate

```sh
cargo fmt --all --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
python3 scripts/package.py check
python3 -m unittest discover -s scripts -p 'test_*.py'
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

Enter an SMB server alone (`smb://server/` or `\\server\`) to list its available shares, select one and connect. Listing alone does not create a shortcut; only the selected, successfully mounted share is saved. Ctrl+L also routes network addresses to the connection dialog. KDE lists through KIO Fuse; GNOME enumerates SMB mountables through the GIO/GTK helper without requiring a FUSE path for the server root. Cancellation and errors do not save a connection. Credentials remain in native system authentication. Discovering computers without a specified server is not implemented.
