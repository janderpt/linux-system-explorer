#!/usr/bin/env python3
"""Build the experimental preview payload inside the Freedesktop SDK.

Not a store manifest. Downloads are pinned; compilation and installation are
offline. Nothing is installed on, or copied from, the host system.
"""
import argparse
from concurrent.futures import ThreadPoolExecutor
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tarfile
import tempfile
import urllib.request

ROOT = Path(__file__).resolve().parent.parent
LOCK_PATH = ROOT / 'packaging/flatpak-converters.lock.json'
LO_SOURCES = ROOT / 'packaging/flatpak-libreoffice-sources.lock.json'
LOCK = json.loads(LOCK_PATH.read_text())
APP_ID = (ROOT / 'packaging/app-id').read_text().strip()


def run(args, **kwargs):
    return subprocess.run([str(a) for a in args], check=True, **kwargs)


def digest(path):
    with path.open('rb') as source:
        return hashlib.file_digest(source, 'sha256').hexdigest()


def download(item, cache):
    path = cache / item['filename']
    if path.exists():
        if digest(path) != item['sha256']:
            raise ValueError(f'Cached checksum mismatch: {path.name}')
        return path
    cache.mkdir(parents=True, exist_ok=True)
    # A failed download cannot become a valid cached input.
    with tempfile.NamedTemporaryFile(dir=cache, delete=False) as temporary:
        partial = Path(temporary.name)
        try:
            with urllib.request.urlopen(item['url'], timeout=120) as response:
                shutil.copyfileobj(response, temporary)
            temporary.close()
            if digest(partial) != item['sha256']:
                raise ValueError('Download checksum mismatch: ' + path.name)
            partial.replace(path)
        finally:
            partial.unlink(missing_ok=True)
    return path


def sources():
    return ([component['source'] for component in LOCK['components'].values()]
            + [package['source'] for package in LOCK['python_runtime']]
            + json.loads(LO_SOURCES.read_text()))


def fetch(cache):
    items = sources() + [LOCK['components']['libreoffice']['binary']]
    items += [p['wheel'] for p in LOCK['python_runtime'] + LOCK['python_build'] if 'wheel' in p]
    with ThreadPoolExecutor(max_workers=4) as workers:
        # Consume every result: failures must abort before any build.
        list(workers.map(lambda item: download(item, cache), items))
    print(f'Verified {len(items)} pinned inputs in {cache}', flush=True)


def extract(archive, destination):
    destination.mkdir(parents=True, exist_ok=True)
    with tarfile.open(archive) as source:
        source.extractall(destination, filter='data')


def sdk(app, arguments, **kwargs):
    return run(['flatpak', 'build', app, *arguments], **kwargs)


def write_source_bundle(cache, destination, items):
    source_bundle = destination / 'FLATPAK-PREVIEW-SOURCES.tar'
    with tarfile.open(source_bundle, 'w') as archive:
        archive.add(LOCK_PATH, arcname='packaging/' + LOCK_PATH.name)
        archive.add(LO_SOURCES, arcname='packaging/' + LO_SOURCES.name)
        archive.add(ROOT / 'packaging/app-id', arcname='packaging/app-id')
        archive.add(ROOT / 'LICENSE', arcname='LICENSE')
        archive.add(ROOT / 'scripts/flatpak-converters.py', arcname='scripts/flatpak-converters.py')
        archive.add(ROOT / 'docs/FLATPAK-PREVIEW.md', arcname='docs/FLATPAK-PREVIEW.md')
        archive.add(destination / 'PREVIEW-CONVERTERS.json', arcname='PREVIEW-CONVERTERS.json')
        for item in items:
            archive.add(cache / item['filename'], arcname='sources/' + item['filename'])
    (destination / 'SHA256SUMS').write_text(''.join(
        f'{digest(destination / name)}  {name}\n'
        for name in ['PREVIEW-CONVERTERS.json', source_bundle.name]))


def compile_payload(app, cache, jobs):
    files = app / 'files'
    build = files / 'build'
    build.mkdir()
    components = LOCK['components']
    for name in ['poppler', 'poppler-data', 'krb5']:
        extract(cache / components[name]['source']['filename'], build)
    extract(cache / components['libreoffice']['binary']['filename'], build / 'lo')
    # Extract only upstream's conversion engines, fonts and language resources;
    # never run Debian maintainer scripts or add another desktop application.
    selected = {'libobasis26.8-' + suffix for suffix in [
        'core', 'writer', 'calc', 'impress', 'draw', 'math', 'en-us', 'images',
        'ooofonts', 'ooolinguistic', 'graphicfilter', 'xsltfilter',
    ]} | {'libreoffice26.8', 'libreoffice26.8-ure', 'libreoffice26.8-en-us'}
    packages = [p for p in (build / 'lo').rglob('*.deb') if p.name.split('_')[0] in selected]
    if len(packages) != len(selected):
        raise ValueError('Upstream LibreOffice package layout changed')
    for package in packages:
        run(['dpkg-deb', '--extract', package, build / 'lo-root'])
    shutil.copytree(build / 'lo-root/opt/libreoffice26.8', files / 'lib/libreoffice')
    for package in LOCK['python_runtime'] + LOCK['python_build']:
        item = package.get('wheel', package.get('source'))
        directory = 'build-wheels' if package in LOCK['python_build'] else 'python-inputs'
        (build / directory).mkdir(exist_ok=True)
        shutil.copyfile(cache / item['filename'], build / directory / item['filename'])
    # Kerberos 1.22 predates the SDK's const-preserving libc string overloads.
    # Its old configure promotes those compatibility warnings to errors.
    commands = [
        ['sh', '-eu', '-c', 'cd /app/build/krb5-1.22.2/src; '
         'CFLAGS="-O2 -std=gnu17" WARN_CFLAGS="-Wall -Wextra" '
         './configure --prefix=/app --libdir=/app/lib '
         '--disable-static --disable-rpath --without-ldap; '
         f'make -j{jobs}; make install'],
        ['cmake', '-S', '/app/build/poppler-26.11.0', '-B', '/app/build/poppler-build',
         '-G', 'Ninja', '-DCMAKE_BUILD_TYPE=Release', '-DCMAKE_INSTALL_PREFIX=/app',
         '-DCMAKE_INSTALL_LIBDIR=lib', '-DCMAKE_INSTALL_RPATH=/app/lib',
         '-DENABLE_CPP=OFF', '-DENABLE_GLIB=OFF', '-DENABLE_QT5=OFF', '-DENABLE_QT6=OFF',
         '-DENABLE_NSS3=OFF', '-DENABLE_GPGME=OFF', '-DENABLE_LIBCURL=OFF',
         '-DENABLE_BOOST=OFF',
         '-DBUILD_MANUAL_TESTS=OFF', '-DBUILD_CPP_TESTS=OFF'],
        ['cmake', '--build', '/app/build/poppler-build', '--parallel', str(jobs)],
        ['cmake', '--install', '/app/build/poppler-build'],
        ['make', '-C', '/app/build/poppler-data-0.4.12', 'install', 'prefix=/app'],
        ['python3', '-m', 'pip', 'install', '--no-index', '--no-deps', '--target=/app/build/python-tools',
         *['/app/build/build-wheels/' + p['wheel']['filename'] for p in LOCK['python_build']]],
        ['env', 'PYTHONPATH=/app/build/python-tools', 'python3', '-m', 'pip', 'install',
         '--no-index', '--no-deps', '--no-build-isolation', '--no-compile', '--prefix=/app',
         *['/app/build/python-inputs/' + p.get('wheel', p['source'])['filename']
           for p in LOCK['python_runtime']]],
    ]
    for index, command in enumerate(commands):
        log = build / f'step-{index}.log'
        print(f'Offline SDK step {index + 1}/{len(commands)}: {command[0]} ({log})', flush=True)
        with log.open('w') as output:
            try:
                sdk(app, command, stdout=output, stderr=subprocess.STDOUT)
            except subprocess.CalledProcessError:
                print(log.read_text()[-12000:])
                raise
    # Only the libraries of Kerberos are needed; don't ship its admin commands,
    # development headers or services. Poppler CLI programs are kept for print
    # preparation and text verification as well as the raster preview.
    keep = {'pdfinfo', 'pdftoppm', 'pdftotext', 'pdfunite', 'pdftocairo', 'pdfimages', 'pdfseparate'}
    for path in (files / 'bin').iterdir():
        if path.name not in keep:
            path.unlink()
    for directory in ['include', 'sbin', 'var', 'com', 'etc', 'share/man', 'share/doc/krb5',
                      'lib/pkgconfig', 'lib/cmake']:
        shutil.rmtree(files / directory, ignore_errors=True)
    launcher = files / 'bin/libreoffice'
    launcher.write_text('#!/bin/sh\nexport SAL_USE_VCLPLUGIN=svp\n'
                        'exec /app/lib/libreoffice/program/soffice "$@"\n')
    launcher.chmod(0o755)
    # The runtime interpreter already searches /app/lib/python3.14/site-packages.
    # Do not override python3, LD_LIBRARY_PATH, or the application's environment.
    sdk(app, ['/app/bin/pdfinfo', '-v'])
    sdk(app, ['/app/bin/libreoffice', '--version'])
    sdk(app, ['python3', '-s', '-c', 'import weasyprint; print(weasyprint.__version__)'])
    shutil.rmtree(files / 'cache', ignore_errors=True)
    notices = files / 'share/licenses/lse-preview'
    notices.mkdir(parents=True)
    for name, directory, license_names in [
        ('poppler', 'poppler-26.11.0', ['COPYING']),
        ('poppler-data', 'poppler-data-0.4.12', ['COPYING', 'COPYING.adobe', 'COPYING.gpl2']),
        ('krb5', 'krb5-1.22.2', ['NOTICE']),
    ]:
        destination = notices / name
        destination.mkdir()
        for filename in license_names:
            shutil.copyfile(build / directory / filename, destination / filename)
    shutil.copyfile(LOCK_PATH, notices / LOCK_PATH.name)
    shutil.copyfile(LO_SOURCES, notices / LO_SOURCES.name)
    shutil.rmtree(build)


def build_payload(cache, destination, work, jobs):
    if destination.exists() and any(destination.iterdir()):
        raise ValueError('Converter output directory must be empty')
    # Offline: every required input, including corresponding sources, is checked.
    items = sources() + [LOCK['components']['libreoffice']['binary']]
    items += [p['wheel'] for p in LOCK['python_runtime'] + LOCK['python_build'] if 'wheel' in p]
    for item in items:
        if digest(cache / item['filename']) != item['sha256']:
            raise ValueError('Input checksum mismatch: ' + item['filename'])
    work.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='lse-preview-', dir=work) as temporary:
        app = Path(temporary) / 'sdk-app'
        run(['flatpak', 'build-init', app, APP_ID, 'org.freedesktop.Sdk',
             'org.freedesktop.Platform', LOCK['runtime_branch']])
        compile_payload(app, cache, jobs)
        destination.mkdir(parents=True, exist_ok=True)
        # Materialize SDK-created soname links within this controlled tree. The
        # ordinary staging verifier can then hash every installed byte.
        shutil.copytree(app / 'files', destination / 'app')
        metadata = {
            'schema': 1, 'lock_sha256': digest(LOCK_PATH),
            'architecture': LOCK['architecture'], 'runtime_branch': LOCK['runtime_branch'],
            'sdk_commit': run(['flatpak', 'info', '--show-commit',
                               'org.freedesktop.Sdk//' + LOCK['runtime_branch']],
                              capture_output=True, text=True).stdout.strip(),
            'components': {k: v['version'] for k, v in LOCK['components'].items()},
            'python_runtime': {p['name']: p['version'] for p in LOCK['python_runtime']},
            'files': {p.relative_to(destination / 'app').as_posix(): digest(p)
                      for p in sorted((destination / 'app').rglob('*')) if p.is_file()},
        }
        (destination / 'PREVIEW-CONVERTERS.json').write_text(json.dumps(metadata, indent=2) + '\n')
        write_source_bundle(cache, destination, items)
        print(f'Preview payload and corresponding sources: {destination}', flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('action', choices=['fetch', 'build'])
    parser.add_argument('--cache', type=Path, default=ROOT / 'build/flatpak-preview-downloads')
    parser.add_argument('--output', type=Path, default=ROOT / 'build/flatpak-preview-converters')
    parser.add_argument('--work-dir', type=Path, default=ROOT / 'build/flatpak-preview-work')
    parser.add_argument('--jobs', type=int, default=8)
    args = parser.parse_args()
    if not 1 <= args.jobs <= 32:
        parser.error('--jobs must be between 1 and 32')
    if args.action == 'fetch':
        fetch(args.cache.resolve())
    else:
        build_payload(args.cache.resolve(), args.output.resolve(), args.work_dir.resolve(), args.jobs)


if __name__ == '__main__':
    main()
