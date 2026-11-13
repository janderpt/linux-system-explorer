# Third-party distribution record — 0.1.0-alpha.5

Technical review date: 2026-11-12. This records the packaging work for the Linux x86_64 alpha, not a legal certification or a claim of independent human review. The package inventory is generated from the exact Cargo.lock and active normal/build dependency graph; its lock hash is recorded in THIRD_PARTY.json and BUILD-INFO.json. It deliberately retains `review_required: true` for ongoing maintainer review.

## Notices and licensing choices

Original project code/assets remain MIT. Rust dependencies remain under their own licenses. For alternative (`OR`) licenses we use MIT when available, otherwise Apache-2.0, otherwise the supplied BSD license. Combined (`AND`) requirements remain cumulative. All available upstream notices are distributed, including Apache NOTICE/copyright material, BSD, ISC, Unicode, Zlib and font-specific notices. No upstream attribution or license is removed.

The active native dependency inventory has no missing notice files. Workspace-root notices omitted by published crates are included from exact upstream revisions, verified against packaging/third-party/sources.json. Re-run this check for every dependency change; this record does not approve arbitrary future dependencies.

## MPL source availability

`freedesktop-desktop-entry` 0.8.4 and `option-ext` 0.2.0 are unmodified MPL-2.0 dependencies. Each binary package includes their complete published crate source, original license, and metadata under `share/licenses/io.github.janderpt.linux-system-explorer/third-party-sources/` (under `/usr` in the DEB). THIRD_PARTY.json identifies these paths; THIRD_PARTY_NOTICES.txt tells recipients where to find them. This preserves access to the covered source without depending on a future download or a written offer. The project's own tagged source is available from the same GitHub release.

Reference: https://www.mozilla.org/en-US/MPL/2.0/FAQ/ and https://www.mozilla.org/MPL/2.0/ (executable distribution/source availability).

## Embedded fonts and icons

- Font Awesome Free 7.1.0: the original, unmodified Solid OTF is embedded; its supplied OFL/CC BY/MIT license and original attribution README are included. The font's reserved name is preserved. Icon/catalog metadata retain the supplied attribution. The font is bundled with the app, never sold separately.
- `epaint_default_fonts` 0.33.3: Ubuntu Light (Ubuntu Font Licence), Noto Emoji (OFL), Hack (MIT/Bitstream Vera/public-domain portions), and emoji-icon-font (MIT) retain original bytes and names. UFL.txt, OFL.txt, Hack-Regular.txt, emoji-icon-font-mit-license.txt and the crate's root licenses are included. Fonts are neither renamed nor separately sold. FONT-COPYRIGHTS.txt also preserves the copyright/license name records from the original embedded SFNT files.
- Compiled crates and bundled fonts do not become MIT-only merely because the application is MIT.

## External runtime packages

In native DEB/tar packages, GStreamer/codecs, Poppler, LibreOffice, WeasyPrint, libarchive, GIO, UDisks2/Polkit and desktop portal services are system dependencies, not copied into those archives. Their packages supply their own notices/source mechanisms. The DEB declares them for installation through the distribution package manager; tar users install the equivalent runtime packages.

## Experimental Flatpak preview payload

The new experimental payload bundles Poppler 26.11.0, poppler-data 0.4.12, upstream LibreOffice 26.8.3.1 conversion resources, MIT Kerberos 1.22.2 and WeasyPrint 70.0 with its pinned Python dependencies. The original converter-less alpha.4 Flatpak remains unchanged; the converter-equipped variant uses distinct public asset names. Runtime-provided libraries remain part of the Freedesktop Platform; no host program/library is copied. The application's original MIT license does not relicense the bundled components.

Poppler retains GPL notices; character data retain Adobe/GPL notices. Kerberos retains its complete NOTICE. LibreOffice is unmodified upstream binary content with LICENSE, LICENSE.html, NOTICE, credits, fonts and third-party notices preserved. Python distributions retain their installed license metadata; native Python extensions are built from the pinned published sources instead of repackaging wheels with hidden native dependencies. The precise versions, input URLs/hashes and SDK commit are recorded in the converter locks/report. This is a binary repackaging of LibreOffice, not a full source build or independent legal/security certification.

The build produces **FLATPAK-PREVIEW-SOURCES.tar**, containing the exact LibreOffice core source plus its revision's external source archives, all bundled Python sources, Poppler/data, Kerberos and build instructions. Distribute that source archive alongside every bundle using this payload and retain its checksum. It is separate from the installed application to avoid installing a large source collection. See [FLATPAK-PREVIEW.md](FLATPAK-PREVIEW.md) for paths, reproduction, included components and validation. Future converter updates require a fresh notice/source check, sandbox tests and maintenance of the corresponding sources; the existing Rust-only audit does not audit these native engines.

## Security review

cargo-audit 0.23.0 against the RustSec database on 2026-11-12 reported no known vulnerable locked crates. It reports the informational unmaintained notice RUSTSEC-2026-0192 for `ttf-parser` 0.25.1, pulled by egui → epaint → ab_glyph → owned_ttf_parser. This alpha records that upstream maintenance issue; it does not suppress advisories. The release workflow audits again and attaches the current report; any vulnerability failure prevents publication. This is not a full application or native-runtime security audit.

Reference: https://rustsec.org/advisories/RUSTSEC-2026-0192.html


## D-Bus compatibility variant: informational maintenance advisories

The alpha.4 `flatpak-previews-dbus` variant uses zbus 5.19.0 to retain sequential Flatpak authentication with distribution xdg-dbus-proxy 0.1.5. RustSec reports no known vulnerable locked crates, but additionally reports informational unmaintained notices for the standalone crates `zvariant` 5.15.1 (RUSTSEC-2026-0370), `zbus_names` 4.3.5 (RUSTSEC-2026-0371) and `zvariant_derive` 5.15.1 (RUSTSEC-2026-0372), which upstream merged into zbus 6. These are in addition to the existing `ttf-parser` notice. The full, unsuppressed report accompanies this experimental variant as `FLATPAK-DBUS-RUSTSEC-AUDIT.json`.

This alpha explicitly retains the older dependency family for tested host-proxy compatibility. A maintained long-term D-Bus dependency/compatibility strategy needs review before a stable/store release; the proxy transport regression tests must continue to pass during that migration. A clean RustSec vulnerability result does not mean every dependency is maintained or that native preview engines have been audited.


## zbus 6.2.1 compatibility publication

The flatpak-previews-dbus6 lock uses zbus 6.2.1 and no longer contains zbus_names, zvariant or zvariant_derive. A fresh unsuppressed RustSec audit reports none of their three maintenance advisories; the unrelated ttf-parser notice remains. Already published alpha.4 artifacts still describe their original dependency graph and are not retroactively relabeled.

The full official MIT-licensed zbus crate is retained in vendor/zbus. Its crates.io archive SHA-256, upstream revision and per-file hashes are in packaging/third-party/zbus-source.json. Only connection/handshake/client.rs changes: Linux Flatpak authentication waits for AUTH and UNIX_FD negotiation responses, preserving upstream validation and native pipelining. The exact patch is in packaging/patches/zbus-flatpak-auth.patch. CI and packaging verify the source/patch hashes and reject unreviewed changes. Native/Flatpak staging includes the modified crate, original license, README.lse.md, LSE-PROVENANCE.json and LSE-COMPATIBILITY.patch with dependency sources/notices.

This is a locally maintained backport on current upstream source, not an upstream claim of old-proxy support. Track upstream security and maintenance updates, rebase and retest the patch when updating, and remove the override when equivalent upstream support exists. See DBUS-COMPATIBILITY.md for the test and update procedure. Other store readiness requirements remain separate.
