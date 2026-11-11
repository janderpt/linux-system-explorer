# Distribution roadmap

Status: preparation, not store approval. The package is experimental.

Repository: https://github.com/janderpt/linux-system-explorer
Application ID: `io.github.janderpt.linux-system-explorer`.
The remote URL and metadata identify the planned repository; creating and publishing
that repository remain separate steps.

## Channels

| Channel | Current preparation | Remaining gates |
| --- | --- | --- |
| GitHub source | Local Git history, SSH remote, MIT license, CI and contribution docs | Create repository, authenticate, review and push |
| GitHub binary downloads | Native tar.gz and DEB builders, checksums and build information | License/security review, clean-system tests and reviewed release |
| Debian/Ubuntu packages | Experimental DEB with binary-derived libc dependencies and desktop metadata | Install/remove/upgrade tests, maintainer review, repository signing and native distro policy |
| AppImage | Relocatable archive and shared desktop assets | AppDir, library bundling, older supported build baseline and clean-host tests; not yet an AppImage |
| Flatpak / Flathub | Shared application identity, desktop launcher, AppStream and icon | Human-authored packaging, sandbox adaptations, upstream support, history, stable releases and review |
| Snap Store | Shared application assets and native build | Snapcraft packaging, interface/confinement design and review; classic approval cannot be assumed |
| Fedora / RPM / COPR | Shared source and desktop metadata | RPM spec, license sources, dependency mapping, clean Fedora build and repository maintenance |
| openSUSE / OBS | Shared source and desktop metadata | Distribution-specific package review and clean builds |
| Arch / AUR | Shared source and desktop metadata | Reviewed PKGBUILD, checksums, dependency mapping and maintenance |
| Nixpkgs | Shared source and desktop metadata | Nix derivation, native-library wrapping and upstream review |

AppImage is a package format, not one central store. Native package formats do not
automatically give admission to distribution repositories or graphical software centers.
Each channel needs maintenance, updates and its own approval; support for every channel
cannot be guaranteed in advance.

## Native build and staging

Python 3.11+, a Rust toolchain, `desktop-file-utils`, `appstream` and, for DEB packages,
`dpkg-dev` are required. First build the optimized binary:

```sh
cargo build --locked --release
python3 scripts/package.py check
python3 scripts/package.py build
```

Outputs go to `dist/`: tar.gz, DEB, `BUILD-INFO.json` and `SHA256SUMS`.
`--formats tar` produces only an archive. These are native builds for the build host,
not universal Linux executables. The CI build uses Ubuntu 24.04; older hosts need a
separate build and compatibility validation. DEB libc requirements are calculated from
the actual executable. OpenGL, X11/Wayland and GIO runtime components are required.

Nothing is installed by the builder. To inspect installation contents without root:

```sh
python3 scripts/package.py stage --destdir /tmp/lse-stage --prefix /usr
/tmp/lse-stage/usr/bin/linux-system-explorer --version
```

The destination must be empty. Standard layout: binary in `bin`, launcher in
`share/applications`, AppStream in `share/metainfo`, SVG and PNG in the hicolor icon theme,
documentation in `share/doc`, and notices in `share/licenses`.

The archive can be extracted and its `bin/linux-system-explorer` run directly, provided
its runtime libraries are installed. No automatic installation or uninstaller modifies
personal preferences. Native package installation is managed by the OS package manager.
The installer does not change the user's default file manager or MIME associations.

For an explicitly requested per-user installation, `scripts/install-user.py install`
installs under `~/.local/bin` and XDG data directories. Its optional `--set-default`
flag changes the default handler of `inode/directory` and registers a per-user
`org.freedesktop.FileManager1` activation file, with backups and recorded
rollback information under the XDG state directory. `restore-default` restores that
previous handler and service registration only while these remain managed by this app.
An already running service is not forcibly replaced; close it or log in again to switch.
The bridge supports ShowFolders, ShowItems and ShowItemProperties for local file URIs,
grouping selections by parent folder. Its private Quit interface stops only the bridge,
leaving GUI windows intact. This is independent
of package-manager installation, which does not change defaults automatically.

GUI windows own versioned private D-Bus names while running. The bridge asks them to
accept requests for their current/loading folder before creating another process.
Selection is refreshed asynchronously and activation uses xdg-activation-v1 on the
native Wayland surface, or the standard viewport focus request on X11. The supplied
activation token is forwarded without changing global environment variables. Closing
a window drops its registration; earlier GUI versions must be reopened after updating.

The desktop Exec entry uses the same per-window protocol directly: normal directory
launches check for an existing matching window before initializing a GUI. Launchers
forward XDG_ACTIVATION_TOKEN, falling back to DESKTOP_STARTUP_ID, without changing the
environment of another process. Relative paths use the same absolute directory identity
as the GUI, and path bytes are preserved. Failure to reach a matching GUI falls back to
opening a new window; this path does not require the FileManager1 bridge to be running.

Packaged installations and native windows use the same project SVG, rasterized to PNG
at build time. The user installer refreshes desktop and icon caches, including KDE's
icon-change notification. Uninstalled development builds register the same icon and
application identity. The app ID does not change the
existing preferences directory. A launcher from older development builds using
`io.github.linux-system-explorer.desktop` can remain in the user's menu; remove that old
development shortcut manually if it is no longer needed.

## License review before binary redistribution

MIT covers this project's original source, packaging, SVG and AppStream metadata.
Embedded Font Awesome and Rust dependencies retain their own licenses.
The builder includes Font Awesome's supplied license and inventories active normal/build
dependencies for the native target, excluding inactive optional dependencies. It copies
available notices from cached crate sources, including font-specific UFL/OFL text files.
Workspace notices omitted from published crates are bundled under `packaging/third-party`,
with exact upstream revisions, source URLs and hashes verified at package time.

`THIRD_PARTY.json` records any remaining missing notice files. Embedded font licenses
and source-availability obligations still need individual verification. The inventory
is an aid, not an assertion of license compliance. `review_required` deliberately stays
true because file coverage alone does not establish that a maintainer has completed the
review. Do not publicly redistribute these draft artifacts until those obligations have
been checked. A reviewed license tool such as cargo-about can be used as an additional aid.

## Flatpak-specific engineering

The current implementation assumes host access for GIO application discovery/launch,
default MIME association writes, Trash, mounted shares, system colors and external drag
and drop. These must be adapted or explicitly validated inside a real sandbox. Merely
granting filesystem access does not make the host integration correct. Prefer portals
where they cover the use case and document justified permissions.

File copy/cut/paste between this app and other file managers still needs native clipboard
MIME integration. Current clipboard transfer data is internal, with path text exported
to the system clipboard. Native drag and drop is a separate implementation.

The current Flathub policy requires stable releases, meaningful project history and
upstream support for file managers. It also requires disclosure of AI-generated material,
prohibits AI-generated or AI-assisted Flatpak manifests, and prohibits agents from opening
or automating submission PRs or generating their submission/review messages. Accordingly,
this repository does not include an agent-generated Flatpak manifest or submission text.
A human maintainer must handle those parts and verify the current policy when submitting.

This project has received substantial AI assistance in source, tests, UI, translations,
documentation, original SVG and native packaging. A human maintainer must review the
actual extent and disclose it accurately; no independent human review is claimed here.

## References

- https://docs.flathub.org/docs/for-app-authors/requirements
- https://docs.flatpak.org/en/latest/sandbox-permissions.html
- https://docs.flatpak.org/en/latest/conventions.html
- https://docs.appimage.org/packaging-guide/manual.html
- https://snapcraft.io/docs/explanation/security/snap-confinement/
- https://www.debian.org/doc/debian-policy/

Policies were checked on 2026-11-11 and must be checked again before submission.

## Tabs and terminal lifecycle

Split view pairs two existing Explorer tab states by stable ID within one Workspace and native viewport. Enabling it creates a second tab for the same directory; independent folder bodies render under unique UI IDs while the focused pane owns the shared toolbar/sidebar, native activation/DND controller, dialogs and keyboard shortcuts. Pointer presses select a pane before shortcut processing; drag hover keeps the source controller, merges clipped drop targets from both bodies, and clears mirrored feedback after rendering. Disabling the split preserves tabs and in-flight operations. Reordering preserves pane IDs; closing a displayed tab invalidates the split safely. Only the active pane persists preferences, avoiding stale inactive-layout writes. The layout/ratio are window-local, while ordinary tab session persistence is unchanged. Headless UI tests exercise the toolbar toggle, initial equality, independent filters/selections/navigation, divider resizing, all three views in both themes, tab reorder/close, keyboard clipboard transfers and real cross-pane file moves in temporary directories.

One versioned private D-Bus endpoint belongs to each native window. It routes reveal requests to the matching active or inactive folder tab and activates that tab before returning success. Tabs share the native drag/drop and activation controllers while maintaining independent directory readers and UI state. Explicit `--new-window` launches bypass reuse. Folder paths are passed as arguments without a shell.

Integrated terminals use portable-pty and a bounded vt100 screen with 5,000 scrollback rows. Each session starts in an explicit working directory, inherits the user's shell environment and runs the configured shell interactively. The PTY is nonblocking; a worker handles output and queued input, while resizing updates the PTY and emulator together. Hiding a dock leaves its sessions running. Closing the owning tab/window stops the workers, closes their PTYs and reaps the shells. Terminal shell sessions are not persisted across restarts. The ordered list of open folder tabs, the active index and up to 25 closed-tab snapshots are atomically saved in the user XDG state directory, independently of preferences. The last changed or gracefully closed window supplies the next session. Startup without an explicit directory restores it by default; the General startup preference can instead choose a single home-folder tab. Older preferences retain restoration. This global choice does not affect per-folder layouts or explicit destinations; explicit folder/reveal/new-window launches retain their requested destination. Unavailable folders remain recorded and can be retried, while malformed or unsupported session data is reported and left untouched. Closed snapshots retain folder, relative neighbors, back/forward navigation, layout, filter and selection. Open and closed identifiers are normalized in the saved snapshot and remapped on restore; consumed closures are autosaved. Closed history is inherited even when open-tab restoration is disabled or a specific folder is requested. Older session files default to an empty closed history. Paths, including history and selected items, use the same byte-preserving encoding as per-folder preferences. General preferences and per-directory layouts retain their existing persistence.

Network integration discovers desktop services and selects GVfs in GNOME or KIO Fuse in KDE, with fallback to an available backend. The GIO/GVfs path runs a dedicated `--mount-network` process, dynamically loading the public GLib/GIO and GTK 3/4 APIs to isolate the GTK main loop and authentication from eframe. Shared libraries stay loaded for that process lifetime because GObject type registrations retain function pointers. Async GIO callbacks keep their state and objects alive until completion. The parent enforces a timeout, kills/reaps a cancelled helper, validates the returned local directory and never receives authentication passwords. The KDE path uses `org.kde.KIOFuse.VFS.mountUrl`; cancelling drops the pending D-Bus request. SMB server-root listings and connections retry through the available GVfs backend if KIO fails or returns an unreadable mount; cancellation prevents a second authentication attempt. Both paths must successfully start reading the directory before a connection can be saved or activated. Real-session testing on KDE with GVfs confirmed that a temporary authenticated SMB mount remains readable after the GTK helper exits, without selecting password storage; full GNOME-session testing remains separate. Protocol modules are supplied by KIO/KIO extras. URI passwords are rejected before storage or connection; saved addresses and mount paths are normalized without persisting credentials. Existing local mounts remain supported. Local user-share management invokes `net usershare` without a shell, passwords or elevation: list all usershares, identify editable shares from the current user's own list, preserve guest settings, edit names/comments/ordered ACLs, copy saved SMB/UNC addresses, and stop sharing after confirmation. Renaming creates the destination first and removes the original only after a fresh state check; removal failures trigger a checked rollback and report cleanup failures. Existing-share snapshots guard against stale edits, and case-insensitive collision checks protect other usershares. Commands have bounded execution time and captured output; failures remain visible for refresh/recovery. The UI does not edit static `smb.conf` shares or change Unix filesystem permissions. New shares grant the current user write access with guests disabled. The native Nautilus or KDE sharing setup remains available through System sharing settings; Samba server installation, account passwords, firewall/service configuration and administrative authorization remain system-owned. This behavior is not yet certified inside a sandbox. Tests exercise KIO and Nautilus APIs on an isolated D-Bus session, GIO ABI path round-trips when runtime libraries are available, desktop backend selection, native-tool argument passing, helper cancellation, dialog cancellation and sidebar persistence without contacting a real server or publishing a share.

Local-share tests use an isolated in-memory command runner for ownership, collision, stale-edit, permission, rollback and cancellation behavior, plus headless UI checks for copying saved addresses and removal confirmation. An explicitly ignored host integration test exercises a uniquely named temporary usershare against an already-configured Samba server and cleans up its own definitions without touching existing shares.

The Local shares sidebar uses a per-workspace shared cache of Samba usershares. Manager results publish immediately across tabs; other processes/external tools are discovered through nonblocking five-second refreshes (30 seconds after failure). F5 invalidates the cache. A revision guard discards queries that started before a newer manager snapshot. Refresh failure retains the last known list with an inline warning. Local visibility is persisted independently of remote connections, and discovering entries never changes shares or credentials.

Network presentation resolves saved connection mount prefixes (most specific first) into remote breadcrumb targets, tab titles and address strings without network I/O. GVfs SMB mount specifications provide a fallback after sidebar shortcut removal, including escaped names, users, domains and ports. Local operations, layout keys and session paths remain unchanged. Remote ancestor clicks and Up at an SMB share root enter server browsing; submitting an unchanged remote address stays in the same local folder without creating a shortcut. Pure resolver and headless UI tests cover path boundaries, encoded/non-UTF-8 names, visible labels, ancestor navigation and tab titles.

SMB server-root addresses now open a share-selection stage before mounting a selected share. KIO Fuse supplies server directory entries; the isolated GNOME helper mounts the SMB browse backend and enumerates GFileInfo directory/mountable entries directly, without needing GVfs FUSE for the browse root. Returned names are validated and encoded as a single path component under the requested server. The shared helper runner drains stdout/stderr through temporary files, caps output, and retains timeout/cancellation handling. Listing does not persist shortcuts; only a selected successful mount is saved. Ctrl+L routes recognized network addresses through the same connection UI. Isolated D-Bus, GIO ABI, output-pressure and headless UI tests cover listing, selection and cancellation; full native GNOME session authentication still requires real-session validation.
