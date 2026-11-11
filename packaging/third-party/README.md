# Upstream license notices

Some crates omit license files stored at their repository's workspace root.
These files were retrieved without modification from the exact Git commit recorded
in each cached crate's `.cargo_vcs_info.json`.

`sources.json` lists covered package names/versions, upstream revisions, source URLs
and SHA-256 hashes. The packager verifies both the crate revision and the notice hashes
before including these files. Upgrading a covered dependency requires updating and
reviewing its mapping. Do not substitute a repository's current default branch.

Font-specific UFL/OFL and other text notices are copied from the fonts directory of
the actual `epaint_default_fonts` crate. Font Awesome's supplied license is included
separately. These files preserve their upstream licenses, rather than the project's MIT.

Collecting texts does not replace a human review of attribution, redistribution,
embedded-font requirements and source-availability obligations.
