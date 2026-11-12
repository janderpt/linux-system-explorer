# Linux System Explorer

[Português](README.md)

A Linux desktop file manager written in Rust with egui/eframe, inspired by familiar
desktop file explorers. **Source published on GitHub; version 0.1.0-alpha.3 remains a development build, without a stable release or store approval.**


**v0.1.0-alpha.3** prerelease: Ubuntu 24.04+ x86_64 packages, checksums and [installation notes](docs/releases/v0.1.0-alpha.3.md). [GitHub downloads](https://github.com/janderpt/linux-system-explorer/releases/tag/v0.1.0-alpha.3).

A local **experimental Flatpak** and the remaining Flathub work are documented in [Flatpak](docs/FLATPAK.md). It does not yet provide all native-package integrations/renderers.

## Image operations and printing

Image context menus offer **Rotate…**, **Resize…** and **Convert…**, including image-only multi-selections. Every operation creates a new sibling file, preserves originals and never replaces an existing path. Collisions receive numbered names; temporary partial output is discarded. Completed copies are selected after refresh. Processing is built into the executable and runs in the background with progress, cancellation and result toasts.

Rotation supports clockwise/counterclockwise 90° and 180°. Resize preserves aspect ratio by default, fitting each image inside the requested bounds; turning this off produces exact dimensions. Output formats are PNG, JPEG, lossless WebP, BMP and TIFF. Rotate/resize retain the source format when supported; static GIF, ICO and SVG default to PNG, with SVG rasterized. JPEG quality defaults to 90 and transparent pixels are composited onto white. PNG/WebP retain transparency. EXIF orientation is applied before transformation; generated copies do not promise to retain all original metadata. Animated GIF/WebP/APNG and multipage TIFF are rejected instead of silently flattening content. Input limits are 64 MiB (SVG: 2 MiB), 32 million pixels and 16384 pixels per side.

**Print…** is available for supported static images, PDF, Office/LibreOffice, HTML, Markdown and text/code. Fully printable selections offer **Print selection…**, combining documents in display order. Each job first opens the system printer/page/copy dialog (`PreparePrint`); only after confirmation does `Print` use that same job’s returned token. Previous jobs never authorize a new submission. Preparation uses temporary PDFs and read-only originals. Dialog cancellation is normal, Escape can cancel preparation, and success means submission to the printing system rather than confirmation of physical output.

Printing requires `xdg-desktop-portal` with a Print-capable desktop backend. Single PDFs go directly to the portal. Images/text need WeasyPrint pagination, HTML/Markdown use the existing static renderer, Office uses LibreOffice, and multiple documents need Poppler's `pdfunite`. Images fit an A4 printable area without changing the source. Limits are 100 files, 256 MiB per source and 512 MiB of prepared PDFs; UTF-8 text is limited to 1 MiB. Operation/print icons are customizable.

## File type actions

Context menus separate common commands from type-specific actions. Recognized archives have an **Archive** section with **View contents**, **Extract here** and **Extract to…**. Preview stays inside the explorer. Extraction is delegated to Ark, File Roller or Engrampa according to the desktop and installed tools, with Ark preferred on KDE and File Roller on GNOME. XFCE and other desktops use an available compatible tool. The manager owns destination selection, password/conflict dialogs and format support; no force-overwrite options are passed. Extract-here folder organization follows that tool’s behavior, including Ark’s automatic subfolder when needed.

Multi-selection gets extraction actions only if every selected item is an archive file. Mixed selections and directories named with archive suffixes do not receive partial extraction. Work runs in the background, and the folder refreshes when the invoked process finishes; **F5** refreshes after work completed in another window if needed. Errors and missing tools use toasts, without claiming successful extraction merely because it was delegated. Extraction icons are customizable. Host archive-manager extraction is unavailable in the experimental Flatpak; internal preview remains available.

## Recursive search

The folder search field (**Ctrl+F**) matches file and folder names throughout the current directory and its descendants, ignoring case. Results arrive progressively in Details, List and Icons, with relative paths to distinguish duplicate names. File operations and preview retain the actual paths. Opening a matching folder navigates into it and ends the previous search. Result context menus also offer **Open file location**, which opens the containing folder in the same pane, clears the search and selects/scrolls to the clicked item. Folder results reveal the folder in its parent. Destination view preferences are preserved, symbolic links reveal their own location, and the action icon is configurable.

Search starts after a 250 ms typing pause using one worker per tab and bounded result batches. New text, hidden-item changes and navigation cancel obsolete work. Clearing the field, **Escape** while the field is focused, or **Clear search** restores the direct listing; **F5** refreshes and reruns the search. Tabs and panes search independently. Applications and Trash keep their existing filters.

Hidden directories are excluded unless hidden items are enabled. Descendant directory symlinks are not followed, preventing cycles; an explicitly opened root may itself be a symlink. Search reads names and metadata. Unreadable/changed entries yield partial results with a toast. Limits are 20,000 matches, one million examined entries and 128 levels; reaching a limit reports partial results and asks for a narrower query.

## Drives

The sidebar's **Drives** section lists mountable SSD/HDD/removable volumes recognized by **UDisks2**, with capacity, user-available free space and a usage bar. Unmounted volumes show capacity and their state; free space is read after mounting. Toggle the section in **Settings → Sidebar → Show Drives section**. Existing preferences enable it by default.

Click to open, rechecking current system state and mounting first when needed. Context menus offer **Mount and open**, **Unmount** and **Safely remove** as applicable. Removal unmounts related volumes before drive power-off or media ejection. Essential system volumes cannot be unmounted and their drives cannot be removed. Operations never force an unmount; busy/permission failures use toasts, and dismissed authorization is handled without a success notice. Other tabs viewing a removed volume return home.

Discovery, space queries and operations run in background workers, with a shared inventory across tabs/panes, five-second refreshes, F5 and refresh after operations. Authorization uses the desktop's Polkit agent without explorer password fields or terminals. Native installations need UDisks2 and a desktop authorization agent; the Debian package recommends `udisks2` and `polkitd`. This host integration is explicitly unavailable in the experimental Flatpak. Ignored volumes, Snap loops and swap are excluded. Already unlocked encrypted filesystems can appear; unlocking encrypted storage is outside this feature. `/mnt` and `/media` remain independent directory shortcuts.

## System applications

**Applications** is the first sidebar item. It shows menu launchers in a flat collection without folders or categories, using the toolbar’s **Details**, **List** and **Icons** modes. Details has sortable Name, Description and Application ID columns; Icons displays a grid with installed application icons. View mode and zoom are saved for this location independently of folders. Filter by name using the search field or **Ctrl+F**. A single click selects; double-click or **Enter** launches. **F5** refreshes the list after installing or removing programs. Catalog loading and icon decoding run in the background, with a generic icon when necessary.

Native installations read XDG `.desktop` entries, respect hidden/desktop visibility rules and user overrides, check available executables, and launch through GIO with the launcher's arguments. This location works with tabs and navigation history and excludes file operations. Hide its shortcut in **Settings → Sidebar**, or customize its icon in **Appearance and icons**. Host application discovery and launching are not yet available in the experimental Flatpak.

Application context menus offer **Open in software manager**. KDE prefers Discover; GNOME prefers GNOME Software. XFCE and other desktops use whichever of those compatible managers is installed, with GNOME Software preferred when both exist. Linked AppStream metadata opens the application's details page; otherwise a name search opens and a toast explains the fallback. Updates, removal and authorization remain in the manager's UI and depend on its plugins, configured sources and the installation's origin. Manually installed applications may not be manageable there. Missing managers and startup failures are reported through toasts. Metadata lookup and startup run in the background.

## File previews with Space

Select a file and press **Space**, or choose **Preview** from its context menu. The explorer's popup uses the active pane's selection. Pressing Space again, Escape, Close or clicking outside stops playback and closes it. Images and videos fit the available area, centered and preserving their aspect ratio. Video popups follow the decoded landscape, portrait or square format within the window limits, with responsive playback controls. Opening the system default application is a separate explicit action. Space still types normally in text fields and the terminal.

The footer stays anchored during loading and scrolling, and viewer controls are centered. **Up/Down** select the previous/next file in the current sorted, filtered folder, skipping directories; **Left/Right** change pages or slides.

Supported previews include PNG/JPEG/WebP/BMP/TIFF/ICO/GIF (first frame), SVG, paginated PDF, bounded plain text/code and internal audio/video playback with pause, seek and volume. Images support fit, zoom and scrolling. CSV is shown as text; external SVG image references are disabled. PDF page navigation, page count and zoom share one toolbar. Office previews cover DOC/DOCX/ODT/RTF, XLS/XLSX/ODS, PPT/PPTX/ODP, ODG drawings and ODF formulas, plus templates and Flat XML variants. They retain document formatting, spreadsheet print layouts and static slides through isolated LibreOffice Writer/Calc/Impress/Draw/Math conversion, cached while the popup remains open. HTML and Markdown offer **Source** and **Presentation**. Markdown supports headings, lists, tables, code blocks, task lists and footnotes. Static presentation uses WeasyPrint and HTML print styles, with local document-directory resources, without scripts or remote requests. Archive previews list names, directories, links and sizes for ZIP, TAR, compressed TAR, 7z, RAR and other libarchive formats without extracting or opening entries. Single compressed streams show their contained item. An isolated, cancellable worker limits memory/time and lists up to 5,000 entries, marking partial results. Encrypted headers or damaged archives can prevent listing. Unsupported files only produce warning toasts, without empty popups; rendering failures use error toasts. HTML/Markdown presentation failures retain the source view and report concise errors instead of library tracebacks. Loading runs in background workers and leaves source files unchanged.

Image decoders, SVG rendering and fallback fonts are compiled into the application. PDF uses Poppler, Office previews use LibreOffice Writer/Calc/Impress/Draw/Math and HTML/Markdown presentation uses WeasyPrint; media uses GStreamer, independent of KDE/GNOME viewers and the X11/Wayland window system. [Runtime dependencies](packaging/preview-runtime.json) are declared in the DEB (Base/Good/Libav plugins, `poppler-utils`, LibreOffice Writer/Calc/Impress/Draw/Math, Liberation fonts, WeasyPrint, Python 3 and libarchive). Missing converters only disable their corresponding preview; HTML/Markdown source and the rest of the explorer remain usable. Office conversion copies the source into private temporary storage, disables macros/active content and preserves the personal LibreOffice profile. Office input is limited to 256 MiB and copied in bounded chunks instead of loading the entire file into memory. Conversion allows two minutes and remains cancellable through closing the preview. Installed fonts may affect pagination. Source/tar installations need these components installed; other package formats must provide equivalent runtimes and undergo compatibility testing. Flatpak and AppImage packages remain in preparation.

## Thumbnails and file sizes

Icon view generates content thumbnails for raster images, SVG, PDFs, videos and Office/LibreOffice documents, preserving aspect ratio without cropping. Only visible entries request work; at most two decoders run concurrently, with a bounded in-memory cache per tab. Listing metadata changes invalidate cached content, F5 retries failed thumbnails, and navigation/scrolling cancels unnecessary pending work. SVG thumbnails limit input to 2 MiB. Videos use a silent, paused GStreamer frame. Office/LibreOffice files use the first page/slide via an isolated LibreOffice conversion and Poppler, including DOC/DOCX/ODT, XLS/XLSX/ODS, PPT/PPTX/ODP and the other supported preview formats. At most one background Office conversion runs at once, with a 30-second conversion deadline and 256 MiB input limit; video frame generation allows 10 seconds. These use the existing declared preview runtimes. Unsupported/corrupt files, missing codecs and missing converters keep their file-type icons without repeated notices. Details and List retain compact icons.

Folder **Properties → Appearance** lets you choose a color and a Font Awesome icon. **Customize folder…** opens this tab directly. **Apply** saves while keeping the window open; **Save and close** saves and closes; **Cancel** discards unapplied changes. **Restore default** stages removal of this folder's appearance override; Apply or Save and close confirms it, preserving other folders and their column/view/zoom settings.

Custom colors and icons appear in all file views, tabs and sidebar shortcuts, overriding light/dark themes and colored/monochrome icon modes. They are saved by absolute path in the explorer's preferences without modifying folder contents or the system theme. Renaming a folder through the explorer updates its appearance entries and those of its descendants.

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

**Add network share** in the sidebar accepts SMB/UNC, SFTP, NFS, FTP and WebDAV addresses or an existing local mount. Remote connections select **GIO/GVfs in GNOME**, with GTK native authentication in a helper process, or **KIO Fuse in KDE**, using installed protocol modules. For SMB server-share listing and connections, a failed KIO operation or an unreadable mount retries through GIO/GVfs when available, allowing temporary authentication without requiring a saved password. For FTP, the explorer prefers GIO/GVfs when available, including on KDE, to present native authentication; KIO remains available when GVfs is absent. For FTP authentication and the SMB fallback on KDE, also install `gvfs-backends`, `gvfs-fuse` and GTK 3 or 4. Enter the username in the system dialog or address (`ftp://user@server/`), without a password. The folder listing is checked before saving the connection or navigating. Work runs in the background, and Cancel stops waiting and terminates the GNOME helper; saved connection addresses/names reconnect when clicked. Passwords in URLs are rejected and never stored in preferences. Connection shortcuts can be removed from their context menu or Settings. Server discovery is not implemented.

Breadcrumbs and **Ctrl+L** display the network address of mounted connections instead of internal GVfs/KIO paths. Tabs show the share name at its root and the current folder name in subfolders; tab tooltips and recently closed tabs also display network addresses. Breadcrumb ancestors remain clickable, and moving up from an SMB share root opens the server share list. Presentation uses saved connections and recognizes GVfs SMB mounts even after removing their sidebar shortcut. File operations and per-folder preferences retain the underlying local mount paths.

Pasting into a mounted FTP folder uploads files and folders through the regular transfer workflow. If the destination cannot preserve timestamps or Unix permissions, the copy retains server-provided attributes. Write, sync and access errors remain failures; conflicts never overwrite existing files. Cut removes the source only after completing the copy and verifying that the source has not changed. Context menus label file previews **Preview (space)**, with the same shortcut shown for archive contents.

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
