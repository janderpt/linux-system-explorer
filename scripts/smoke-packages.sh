#!/bin/sh
# Run only inside a disposable Ubuntu 24.04 container with /packages mounted read-only.
set -eu
export DEBIAN_FRONTEND=noninteractive
[ -f /.dockerenv ] || { echo 'Requires a disposable Docker container' >&2; exit 1; }
cd /packages
sha256sum --check SHA256SUMS
apt-get update -qq
apt-get install -y --no-install-recommends ./*.deb xvfb xauth dbus-x11 libgl1-mesa-dri
version=$(python3 -c 'import json; print(json.load(open("BUILD-INFO.json"))["version"])')
/usr/bin/linux-system-explorer --version | grep -F "$version"
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
archive=$(find /packages -maxdepth 1 -name '*-x86_64-unknown-linux-gnu.tar.gz' -print)
tar -xzf "$archive" -C "$work"
binary="$work/linux-system-explorer-$version/bin/linux-system-explorer"
"$binary" --help >/dev/null
cmp "$binary" /usr/bin/linux-system-explorer
licenses="/usr/share/licenses/io.github.janderpt.linux-system-explorer"
test -f "$licenses/third-party-sources/freedesktop-desktop-entry-0.8.4/src/lib.rs"
test -f "$licenses/third-party-sources/option-ext-0.2.0/src/lib.rs"
# Reinstallation must preserve normal package ownership and executable contents.
dpkg -i ./*.deb
cmp "$binary" /usr/bin/linux-system-explorer
mkdir "$work/folder"
set +e
HOME="$work/home" LIBGL_ALWAYS_SOFTWARE=1 dbus-run-session -- xvfb-run -a \
    timeout --signal=TERM 8s /usr/bin/linux-system-explorer --new-window -- "$work/folder" >"$work/gui.log" 2>&1
result=$?
set -e
cat "$work/gui.log"
[ "$result" -eq 124 ] || { echo "X11 startup failed: $result" >&2; exit 1; }
apt-get remove -y linux-system-explorer
test ! -e /usr/bin/linux-system-explorer
# Extracted tar remains independently runnable after removing the installed DEB.
"$binary" --version | grep -F "$version"
echo 'PASS: checksums, DEB install/reinstall/removal, tar execution, MPL source and X11 startup'
