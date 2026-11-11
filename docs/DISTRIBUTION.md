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
