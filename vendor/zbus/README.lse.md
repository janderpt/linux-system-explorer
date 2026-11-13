# zbus 6.2.1: distribution proxy compatibility backport

This is the complete upstream crates.io source of zbus 6.2.1, MIT licensed, with
one local production change in `src/connection/handshake/client.rs`. The original
LICENSE, package manifest, notices, and source are preserved. This is a local
compatibility patch, not a claim that upstream 6.2.1 supports the older proxy.

On Linux Flatpak connections only (`FLATPAK_ID` or `/.flatpak-info`), AUTH and
NEGOTIATE_UNIX_FD each wait for their response before the next command. BEGIN and
Hello then use the upstream path. Authentication rejection, server GUID validation,
FD capability negotiation, serialization, filtering, and all other transport code
remain upstream. Native connections retain upstream pipelining. This restores the
ordering used by zbus 5 without depending on its retired standalone crates.

Provenance and integrity records: `packaging/third-party/zbus-source.json`.
Reproducible diff: `packaging/patches/zbus-flatpak-auth.patch` (apply with `patch -p1`
to the official 6.2.1 crate, then add this documentation). Run
`python3 scripts/check-vendored-zbus.py` before packaging. The package includes
this exact modified source, its MIT license, provenance and patch with its notices.

To update, obtain the new official crate, verify its published checksum, compare
upstream authentication changes, port or remove the patch, and refresh the recorded
source/patch hashes. Keep the proxy 0.1.5/0.1.9, hostile authentication peer, portal
FD/cancellation, window reuse and installed-Flatpak regressions passing. Remove the
local override when maintained upstream provides equivalent compatibility. Do not
silence RustSec advisories or relax sandbox permissions to make an update pass.
