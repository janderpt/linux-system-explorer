#!/usr/bin/env python3
"""Exercise a local bundle in a disposable container, never the user's installation.

Run under dbus-run-session and Xvfb, with the required runtime already installed.
The application must not be installed in this private Flatpak user installation.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import signal
import subprocess
import tempfile
import time

APP = 'io.github.janderpt.linux-system-explorer'
REF = APP + '//experimental'


def run(*args, **kwargs):
    return subprocess.run([str(arg) for arg in args], check=True, text=True,
                          capture_output=True, **kwargs).stdout.strip()


def app_python(code):
    return run('flatpak', 'run', '--user', '--branch=experimental',
               '--command=python3', APP, '-c', code)


def install(bundle):
    run('flatpak', 'install', '--user', '-y', '--noninteractive', '--no-related', bundle)


def uninstall():
    run('flatpak', 'uninstall', '--user', '-y', '--noninteractive', REF)


def version():
    return run('flatpak', 'run', '--user', '--branch=experimental', APP, '--version')


def gui(work, label):
    log = work / (label + '.log')
    with log.open('w') as output:
        child = subprocess.Popen([
            'flatpak', 'run', '--user', '--branch=experimental', APP,
            '--new-window', '--', str(work / 'folder'),
        ], stdout=output, stderr=subprocess.STDOUT, start_new_session=True,
           env=os.environ | {'LIBGL_ALWAYS_SOFTWARE': '1'})
        try:
            child.wait(timeout=8)
            raise RuntimeError(f'{label}: GUI exited early ({child.returncode}): {log.read_text()}')
        except subprocess.TimeoutExpired:
            pass
        finally:
            subprocess.run(['flatpak', 'kill', APP], capture_output=True, check=False)
            try:
                child.wait(timeout=5)
            except subprocess.TimeoutExpired:
                os.killpg(child.pid, signal.SIGKILL)
                child.wait()
    text = log.read_text()
    for error in ['failed to register window', 'Address family not supported', 'panicked at']:
        if error.lower() in text.lower():
            raise RuntimeError(f'{label}: {text}')
    return {'seconds_alive': 8, 'renderer': 'Xvfb/software OpenGL'}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('bundle', type=Path)
    parser.add_argument('--previous', required=True, type=Path)
    parser.add_argument('--report', required=True, type=Path)
    args = parser.parse_args()
    if not Path('/.dockerenv').exists() or not os.environ.get('DISPLAY'):
        parser.error('Use a disposable container with an isolated D-Bus session and Xvfb')
    installed = subprocess.run(['flatpak', 'info', '--user', REF], capture_output=True)
    if installed.returncode == 0:
        parser.error('This private test installation must not contain the app')
    metadata = json.loads((args.bundle.parent / 'FLATPAK-BUILD-INFO.json').read_text())
    expected = 'linux-system-explorer ' + metadata['version']
    assert metadata['artifacts'][args.bundle.name] == hashlib.sha256(args.bundle.read_bytes()).hexdigest()
    native_config = Path('/root/.config/linux-system-explorer/preferences.json')
    mime = Path('/root/.config/mimeapps.list')
    native_config.parent.mkdir(parents=True, exist_ok=True)
    native_config.write_text('{"test":"native preferences stay private"}\n')
    mime.write_text('[Default Applications]\ninode/directory=unrelated.desktop\n')
    untouched = {p: p.read_bytes() for p in [native_config, mime]}
    started = time.monotonic()
    with tempfile.TemporaryDirectory(prefix='lse-flatpak-smoke-') as temporary:
        work = Path(temporary)
        (work / 'folder').mkdir()
        install(args.bundle)
        try:
            assert version() == expected
            new_commit = run('flatpak', 'info', '--user', '--show-commit', REF)
            fresh = gui(work, 'fresh-install')
            uninstall()
            install(args.previous)
            old_version = version().removeprefix('linux-system-explorer ')
            old_commit = run('flatpak', 'info', '--user', '--show-commit', REF)
            assert old_version != metadata['version'] and old_commit != new_commit
            preferences = json.dumps({
                'language': 'english', 'dark': True, 'restore_tabs': False,
                'show_hidden': True, 'item_scale': 1.5,
                'toast': {'success': {'auto_dismiss': False, 'duration_seconds': 17}},
            })
            app_python('''import os,json
from pathlib import Path
p=Path(os.environ['XDG_CONFIG_HOME'])/'linux-system-explorer/preferences.json'
p.parent.mkdir(parents=True,exist_ok=True)
p.write_text(''' + repr(preferences) + ''')
''')
            before = app_python('''import os
from pathlib import Path
print((Path(os.environ['XDG_CONFIG_HOME'])/'linux-system-explorer/preferences.json').read_text())''')
            install(args.bundle)  # Same origin/ref, in-place bundle update; no uninstall.
            assert version() == expected
            assert run('flatpak', 'info', '--user', '--show-commit', REF) == new_commit
            after = app_python('''import os
from pathlib import Path
print((Path(os.environ['XDG_CONFIG_HOME'])/'linux-system-explorer/preferences.json').read_text())''')
            assert before == after, 'bundle update must preserve user preferences byte for byte'
            upgraded = gui(work, 'upgraded-install')
            for path, data in untouched.items():
                assert path.read_bytes() == data, 'native settings/MIME defaults must remain untouched'
            permissions = run('flatpak', 'info', '--user', '--show-permissions', REF)
            assert 'org.freedesktop.Flatpak=' not in permissions
            assert 'network;' not in permissions and 'network\n' not in permissions
            report = {
                'source_revision': metadata['source_revision'], 'version': metadata['version'],
                'bundle_sha256': metadata['artifacts'][args.bundle.name],
                'flatpak': run('flatpak', '--version'),
                'xdg_dbus_proxy': run(os.environ.get('FLATPAK_DBUSPROXY', 'xdg-dbus-proxy'), '--version'),
                'runtime': metadata['runtime'] + '/' + metadata['runtime_branch'],
                'fresh_install': fresh, 'upgrade': upgraded,
                'previous_version': old_version, 'previous_commit': old_commit, 'commit': new_commit,
                'preferences_preserved': True, 'native_preferences_and_defaults_unchanged': True,
                'capabilities': metadata['capabilities'], 'uninstall': True,
                'duration_seconds': round(time.monotonic() - started, 1),
                'real_desktop_session_tested': False, 'flathub_submission_ready': False,
            }
        finally:
            subprocess.run(['flatpak', 'uninstall', '--user', '-y', '--noninteractive', REF],
                           capture_output=True, check=False)
    assert subprocess.run(['flatpak', 'info', '--user', REF], capture_output=True).returncode != 0
    args.report.write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps(report, indent=2))


if __name__ == '__main__':
    main()
