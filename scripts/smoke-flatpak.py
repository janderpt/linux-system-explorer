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
import re
import signal
import subprocess
import sys
import tempfile
import time

APP = 'io.github.janderpt.linux-system-explorer'
REF = APP + '//experimental'


def run(*args, **kwargs):
    try:
        return subprocess.run([str(arg) for arg in args], check=True, text=True,
                              capture_output=True, **kwargs).stdout.strip()
    except subprocess.CalledProcessError as error:
        print(error.stdout, error.stderr, file=sys.stderr)
        raise


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


def sandbox_tests(binary, fixtures, converters=False):
    base = ['flatpak', 'run', '--user', '--branch=experimental',
            '--filesystem=' + str(binary.parent) + ':ro',
            '--filesystem=' + str(fixtures) + ':ro',
            '--env=LSE_FLATPAK_FIXTURES=' + str(fixtures),
            '--command=' + str(binary), APP]
    filters = [
        'transfers::tests::', 'terminal::tests::', 'app::toast::tests::',
        'app::search::tests::filter_',
        'app::search::tests::modified_arrows_',
        'app::workspace::split::tests::filter_arrows_',
        'app::workspace::split::tests::enter_opens_',
        'app::type_ahead::tests::enter_opens_the_folder_',
        'app::sidebar::tests::navigation_reveals_',
    ]
    results = []
    for selection in filters:
        output = run(*base, selection, '--nocapture')
        matches = re.findall(r'test result: ok\. (\d+) passed;', output)
        if not matches or int(matches[-1]) == 0:
            raise RuntimeError('No sandbox tests ran for ' + selection)
        results.append({'selection': selection, 'passed': int(matches[-1])})
    selection = 'sandbox::tests::flatpak_runtime_exercises_renderers_file_operations_and_private_preferences'
    output = run(*base, '--exact', selection, '--ignored', '--nocapture')
    assert '1 passed; 0 failed' in output
    results.append({'selection': selection, 'passed': 1})
    if converters:
        # CI makes missing tools fail these tests rather than being skipped.
        # The test process gets only /app and the Platform, never host programs.
        base.insert(base.index('--command=' + str(binary)), '--env=CI=1')
        selections = [
            'preview::tests::pdf_renderer_handles_real_pages_',
            'preview::tests::office_families_render_',
            'preview::tests::docx_conversion_preserves_',
            'preview::tests::html_presentation_loads_',
            'preview::tests::paginated_html_uses_',
            'preview::tests::markdown_presentation_formats_',
            'preview::thumbnail::tests::image_svg_and_pdf_',
            'preview::thumbnail::tests::office_thumbnails_export_',
            'printing::tests::printable_types_prepare_and_merge_',
        ]
        for selection in selections:
            output = run(*base, selection, '--nocapture', '--test-threads=1')
            if '1 passed; 0 failed' not in output:
                raise RuntimeError('Preview converter test did not run: ' + selection)
            results.append({'selection': selection, 'passed': 1})
    return {'passed': sum(r['passed'] for r in results), 'groups': results}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('bundle', type=Path)
    parser.add_argument('--previous', required=True, type=Path)
    parser.add_argument('--report', required=True, type=Path)
    parser.add_argument('--test-binary', type=Path)
    parser.add_argument('--fixtures', type=Path)
    args = parser.parse_args()
    if not Path('/.dockerenv').exists() or not os.environ.get('DISPLAY'):
        parser.error('Use a disposable container with an isolated D-Bus session and Xvfb')
    if not os.environ.get('XDG_RUNTIME_DIR') or not Path('/run/dbus/system_bus_socket').exists():
        parser.error('Provide a private XDG_RUNTIME_DIR and start the container system D-Bus first')
    if bool(args.test_binary) != bool(args.fixtures):
        parser.error('--test-binary and --fixtures must be supplied together')
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
    # /tmp is private inside Flatpak; put the disposable folder in the exposed
    # test user's home so GUI startup actually opens an existing directory.
    with tempfile.TemporaryDirectory(prefix='lse-flatpak-smoke-', dir=Path.home()) as temporary:
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
            old_origin = run('flatpak', 'info', '--user', '--show-origin', REF)
            assert old_commit != new_commit, 'upgrade must install a different build'
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
            assert run('flatpak', 'info', '--user', '--show-origin', REF) == old_origin
            after = app_python('''import os
from pathlib import Path
print((Path(os.environ['XDG_CONFIG_HOME'])/'linux-system-explorer/preferences.json').read_text())''')
            assert before == after, 'bundle update must preserve user preferences byte for byte'
            upgraded = gui(work, 'upgraded-install')
            functional = sandbox_tests(args.test_binary, args.fixtures,
                                       bool(metadata.get('preview_converters'))) if args.test_binary else None
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
                'origin': old_origin, 'origin_preserved': True,
                'preferences_preserved': True, 'native_preferences_and_defaults_unchanged': True,
                'capabilities': metadata['capabilities'], 'uninstall': True,
                'duration_seconds': round(time.monotonic() - started, 1),
                'real_desktop_session_tested': False, 'flathub_submission_ready': False,
                'sandbox_functional_tests': functional,
            }
        finally:
            subprocess.run(['flatpak', 'uninstall', '--user', '-y', '--noninteractive', REF],
                           capture_output=True, check=False)
    assert subprocess.run(['flatpak', 'info', '--user', REF], capture_output=True).returncode != 0
    args.report.write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps(report, indent=2))


if __name__ == '__main__':
    main()
