#!/usr/bin/env python3
"""Build a local experimental bundle from reviewed native staging, without a manifest.

This is NOT Flathub submission material. Its manifest and submission interactions
must be authored independently by a human under Flathub's current AI policy.
"""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parent.parent
SPEC = importlib.util.spec_from_file_location('lse_native_package', ROOT / 'scripts/package.py')
PACKAGE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(PACKAGE)
RUNTIME = 'org.freedesktop.Platform'
RUNTIME_VERSION = '26.08'
BRANCH = 'experimental'
REMOTE = 'https://dl.flathub.org/repo/flathub.flatpakrepo'
# Experimental file-manager access; no host execution, unrestricted D-Bus, or
# system bus. These permissions still require independent human store review.
PERMISSIONS = [
    '--share=ipc', '--socket=wayland', '--socket=fallback-x11', '--device=dri',
    '--socket=pulseaudio', '--filesystem=host', '--filesystem=xdg-run/gvfs',
    '--own-name=io.github.janderpt.LinuxSystemExplorer.Window1.*',
]


def run(arguments, **kwargs):
    return subprocess.run([str(a) for a in arguments], check=True, **kwargs)


def digest(path):
    with path.open('rb') as source:
        return hashlib.file_digest(source, 'sha256').hexdigest()


def tree_hashes(root):
    paths = sorted(root.rglob('*'))
    if any(path.is_symlink() for path in paths):
        raise ValueError('Staging must not contain symbolic links')
    return {path.relative_to(root).as_posix(): digest(path) for path in paths
            if path.is_file() and path != root / 'flatpak-stage.json'}


def stage(destination, binary, converters=None):
    install, report = PACKAGE.stage(destination, '/app', binary)
    if report['packages_without_notice_files']:
        raise ValueError('Dependency notices must be complete before bundling')
    desktop = install / f'share/applications/{PACKAGE.APP_ID}.desktop'
    # A host TryExec cannot locate a binary existing only inside /app.
    desktop.write_text('\n'.join(line for line in desktop.read_text().splitlines()
                                 if not line.startswith('TryExec=')) + '\n')
    service = install / f'share/{PACKAGE.NAME}/org.freedesktop.FileManager1.service.in'
    service.unlink()  # Do not advertise native host registration from a sandbox.
    documentation = install / f'share/doc/{PACKAGE.NAME}'
    shutil.copyfile(ROOT / 'docs/FLATPAK.md', documentation / 'INSTALL.md')
    shutil.copyfile(ROOT / 'docs/FLATPAK.md', documentation / 'FLATPAK.md')
    shutil.copyfile(ROOT / 'docs/FLATPAK-PREVIEW.md', documentation / 'FLATPAK-PREVIEW.md')
    preview = add_converters(install, converters) if converters is not None else None
    PACKAGE.validate(install)
    metadata = {
        'app_id': PACKAGE.APP_ID, 'version': PACKAGE.VERSION,
        'source_revision': PACKAGE.run(['git', 'rev-parse', 'HEAD'], cwd=ROOT,
                                      capture_output=True, text=True).stdout.strip(),
        'source_has_uncommitted_changes': bool(PACKAGE.run(
            ['git', 'status', '--porcelain'], cwd=ROOT, capture_output=True, text=True).stdout),
        'cargo_lock_sha256': digest(ROOT / 'Cargo.lock'),
        'binary_sha256': digest(install / 'bin' / PACKAGE.NAME),
        'native_target': PACKAGE.host_target(),
        'stage_kind': 'experimental-flatpak',
        'preview_converters': preview,
        'staged_files': tree_hashes(install),
    }
    (install / 'flatpak-stage.json').write_text(json.dumps(metadata, indent=2) + '\n')
    print(f'Flatpak staging: {install}')


def add_converters(install, payload):
    report_path = payload / 'PREVIEW-CONVERTERS.json'
    report = json.loads(report_path.read_text())
    lock = ROOT / 'packaging/flatpak-converters.lock.json'
    if (report['lock_sha256'] != digest(lock) or report['runtime_branch'] != RUNTIME_VERSION
            or report['architecture'] != PACKAGE.host_target().split('-')[0]):
        raise ValueError('Converter payload does not match the locked inputs/runtime/architecture')
    if tree_hashes(payload / 'app') != report['files']:
        raise ValueError('Converter payload checksum mismatch')
    source = payload / 'FLATPAK-PREVIEW-SOURCES.tar'
    expected = dict(line.split('  ', 1)[::-1]
                    for line in (payload / 'SHA256SUMS').read_text().splitlines())
    for path in [report_path, source]:
        if expected.get(path.name) != digest(path):
            raise ValueError('Converter sources/report checksum mismatch')
    # Reject collisions rather than silently replacing native files.
    for relative in report['files']:
        target = install / relative
        if target.exists():
            raise ValueError('Converter payload collides with staging: ' + relative)
    for relative in report['files']:
        target = install / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(payload / 'app' / relative, target)
    notices = install / 'share/licenses/lse-preview'
    notices.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(report_path, notices / 'PREVIEW-CONVERTERS.json')
    shutil.copyfile(source, install.parent / source.name)
    return report | {'source_artifact': source.name, 'source_sha256': digest(source)}


def validate_stage(install):
    metadata = json.loads((install / 'flatpak-stage.json').read_text())
    if (metadata['app_id'] != PACKAGE.APP_ID or metadata['version'] != PACKAGE.VERSION
            or metadata['stage_kind'] != 'experimental-flatpak'):
        raise ValueError('Staging identity/version does not match this source')
    if digest(install / 'bin' / PACKAGE.NAME) != metadata['binary_sha256']:
        raise ValueError('Staged binary checksum mismatch')
    if metadata['staged_files'] != tree_hashes(install):
        raise ValueError('Staged file checksum mismatch')
    if preview := metadata.get('preview_converters'):
        if digest(install.parent / preview['source_artifact']) != preview['source_sha256']:
            raise ValueError('Corresponding preview sources checksum mismatch')
    return metadata


def proxy_version():
    executable = os.environ.get('FLATPAK_DBUSPROXY') or 'xdg-dbus-proxy'
    version = run([executable, '--version'], capture_output=True, text=True).stdout.strip()
    number = re.search(r'\b(\d+)\.(\d+)\.(\d+)\b', version)
    if not number or tuple(map(int, number.groups())) < (0, 1, 9):
        raise RuntimeError('Use maintained xdg-dbus-proxy 0.1.9+; old proxies cannot handle zbus 6')
    return version


def build(install, work, output):
    install, work, output = install.resolve(), work.resolve(), output.resolve()
    metadata = validate_stage(install)
    if not shutil.which('flatpak'):
        raise RuntimeError('Install flatpak before building the experimental bundle')
    if output.exists() and any(output.iterdir()):
        raise ValueError('Output directory must be empty')
    proxy = proxy_version()
    work.mkdir(parents=True, exist_ok=True)
    output.mkdir(parents=True, exist_ok=True)
    # Private empty directories avoid deleting or overwriting another build tree.
    with tempfile.TemporaryDirectory(prefix='lse-flatpak-', dir=work) as temporary:
        temporary = Path(temporary)
        app = temporary / 'app'
        repo = temporary / 'repo'
        # This packages a prebuilt executable, not an SDK/source build. Running
        # it under the actual runtime below is mandatory before export.
        run(['flatpak', 'build-init', app, PACKAGE.APP_ID, RUNTIME, RUNTIME, RUNTIME_VERSION])
        shutil.copytree(install, app / 'files', dirs_exist_ok=True)
        result = run(['flatpak', 'build', app, 'ldd', '/app/bin/' + PACKAGE.NAME],
                     capture_output=True, text=True)
        if 'not found' in result.stdout + result.stderr:
            raise RuntimeError('Native executable is not compatible with the runtime: ' + result.stdout)
        version = run(['flatpak', 'build', app, '/app/bin/' + PACKAGE.NAME, '--version'],
                      capture_output=True, text=True).stdout.strip()
        if version != f'{PACKAGE.NAME} {PACKAGE.VERSION}':
            raise ValueError('Runtime executable version differs from staging')
        # Record which features are really present instead of claiming native parity.
        probe = run(['flatpak', 'build', '--filesystem=' + str(ROOT) + ':ro', app,
                     'python3', str(ROOT / 'scripts/flatpak-runtime-check.py')],
                    capture_output=True, text=True)
        capabilities = json.loads(probe.stdout)
        run(['flatpak', 'build-finish', '--command=' + PACKAGE.NAME, *PERMISSIONS, app])
        run(['flatpak', 'build-export', repo, app, BRANCH])
        name = f'{PACKAGE.NAME}-{PACKAGE.VERSION}-flatpak-experimental-{metadata["native_target"].split("-")[0]}.flatpak'
        artifact = output / name
        if artifact.exists():
            raise ValueError('Output bundle already exists; choose an empty output directory')
        run(['flatpak', 'build-bundle', '--runtime-repo=' + REMOTE,
             repo, artifact, PACKAGE.APP_ID, BRANCH])
        runtime = run(['flatpak', 'info', '--show-commit', RUNTIME + '//' + RUNTIME_VERSION],
                      capture_output=True, text=True).stdout.strip()
        metadata.update(runtime=RUNTIME, runtime_branch=RUNTIME_VERSION,
                        runtime_commit=runtime, app_branch=BRANCH,
                        permissions=PERMISSIONS, capabilities=capabilities,
                        host_tools={'xdg_dbus_proxy': proxy},
                        minimum_host_xdg_dbus_proxy='0.1.9',
                        submission_ready=False, flathub_manifest_included=False,
                        artifacts={name: digest(artifact)})
        artifacts = [artifact]
        if preview := metadata.get('preview_converters'):
            if not all(capabilities.get(name) for name in ['pdf', 'office', 'html_markdown_presentation']):
                raise RuntimeError('Bundled preview converters failed the runtime probe')
            sources = output / preview['source_artifact']
            shutil.copyfile(install.parent / sources.name, sources)
            metadata['artifacts'][sources.name] = digest(sources)
            artifacts.append(sources)
        report = output / 'FLATPAK-BUILD-INFO.json'
        report.write_text(json.dumps(metadata, indent=2) + '\n')
        (output / 'SHA256SUMS').write_text(''.join(
            f'{digest(path)}  {path.name}\n' for path in [*artifacts, report]))
        print(f'Experimental bundle: {artifact}')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('action', choices=['stage', 'build'])
    parser.add_argument('--binary', type=Path, default=ROOT / 'target/release' / PACKAGE.NAME)
    parser.add_argument('--destdir', type=Path, default=ROOT / 'build/flatpak-stage')
    parser.add_argument('--converters', type=Path, default=ROOT / 'build/flatpak-preview-converters',
                        help='Verified payload produced by flatpak-converters.py')
    parser.add_argument('--stage', type=Path, default=ROOT / 'build/flatpak-stage/app')
    parser.add_argument('--work-dir', type=Path, default=ROOT / 'build/flatpak-build')
    parser.add_argument('--output', type=Path, default=ROOT / 'dist/flatpak-experimental')
    args = parser.parse_args()
    if args.action == 'stage':
        stage(args.destdir, args.binary, args.converters)
    else:
        build(args.stage, args.work_dir, args.output)


if __name__ == '__main__':
    main()
