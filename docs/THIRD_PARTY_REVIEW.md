# Third-party distribution record — 0.1.0-alpha.3

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

GStreamer/codecs, Poppler, LibreOffice, WeasyPrint, libarchive, GIO, UDisks2/Polkit and desktop portal services are system dependencies, not copied into these archives. Their packages supply their own notices/source mechanisms. The DEB declares them for installation through the distribution package manager; tar users install the equivalent runtime packages. Other packaging formats that bundle these components require a new review.

## Security review

cargo-audit 0.23.0 against the RustSec database on 2026-11-12 reported no known vulnerable locked crates. It reports the informational unmaintained notice RUSTSEC-2026-0192 for `ttf-parser` 0.25.1, pulled by egui → epaint → ab_glyph → owned_ttf_parser. This alpha records that upstream maintenance issue; it does not suppress advisories. The release workflow audits again and attaches the current report; any vulnerability failure prevents publication. This is not a full application or native-runtime security audit.

Reference: https://rustsec.org/advisories/RUSTSEC-2026-0192.html
