#!/usr/bin/env python3
"""Test real filtered D-Bus transport, without replacing the host proxy.

Supply the compiled Rust test executable and an xdg-dbus-proxy binary. A private
session bus hosts mock portals; only application clients use the filtered socket.
No real desktop portal, application launcher or printer is contacted.
"""
import argparse
import json
import os
from pathlib import Path
import select
import subprocess
import sys
import tempfile

APP = 'io.github.janderpt.linux-system-explorer'
CASES = [
    ('sandbox::tests::portal_opens_an_fd_supports_chooser_cancellation_and_failure_on_an_isolated_bus',
     'LSE_TEST_PORTAL_BUS'),
    ('printing::tests::print_portal_requires_confirmation_passes_read_only_pdf_and_handles_cancellation_without_printing',
     'LSE_PRINT_BUS_TEST'),
    ('file_manager::window::tests::window_reuse_preserves_selection_and_proxy_restricts_ungranted_names',
     'LSE_WINDOW_BUS_TEST'),
]


def session(binary, proxy):
    version = subprocess.check_output([proxy, '--version'], text=True).strip()
    with tempfile.TemporaryDirectory(prefix='lse-dbus-proxy-') as temporary:
        socket = Path(temporary) / 'bus'
        read, write = os.pipe()
        try:
            child = subprocess.Popen([
                proxy, os.environ['DBUS_SESSION_BUS_ADDRESS'], str(socket),
                '--filter', '--talk=org.freedesktop.portal.Desktop',
                '--own=io.github.janderpt.LinuxSystemExplorer.Window1.*',
                '--fd=' + str(write),
            ], pass_fds=(write,))
        finally:
            os.close(write)
        try:
            if not select.select([read], [], [], 10)[0] or not os.read(read, 1):
                raise RuntimeError('Filtered proxy did not become ready')
            env = os.environ | {
                'LSE_TEST_DBUS_SERVICE_ADDRESS': os.environ['DBUS_SESSION_BUS_ADDRESS'],
                'DBUS_SESSION_BUS_ADDRESS': 'unix:path=' + str(socket),
                'FLATPAK_ID': APP,
                'LSE_TEST_FILTERED_DBUS': '1',
            }
            for selection, marker in CASES:
                result = subprocess.run([
                    binary, '--exact', selection, '--nocapture',
                ], env=env | {marker: '1'}, text=True, capture_output=True, timeout=90)
                print(result.stdout, result.stderr, end='', flush=True)
                if result.returncode or '1 passed; 0 failed' not in result.stdout:
                    raise RuntimeError('Filtered proxy regression failed: ' + selection)
            print(json.dumps({'xdg_dbus_proxy': version, 'passed': len(CASES),
                              'filtered': True, 'real_desktop_portal': False}))
        finally:
            os.close(read)
            child.terminate()
            child.wait(timeout=10)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('binary', type=Path)
    parser.add_argument('--proxy', default=os.environ.get('FLATPAK_DBUSPROXY', 'xdg-dbus-proxy'))
    parser.add_argument('--private-session', action='store_true', help=argparse.SUPPRESS)
    args = parser.parse_args()
    if args.private_session:
        session(str(args.binary.resolve()), args.proxy)
    else:
        subprocess.run(['dbus-run-session', '--', sys.executable, str(Path(__file__).resolve()),
                        str(args.binary.resolve()), '--proxy', args.proxy, '--private-session'], check=True)


if __name__ == '__main__':
    main()
