#!/usr/bin/env python3
"""Validate an exact clean alpha build and prepare its public release assets."""
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys
import tarfile
import tomllib

ROOT = Path(__file__).resolve().parent.parent


def finalize(output, tag):
    package = tomllib.loads((ROOT / 'Cargo.toml').read_text())['package']
    version = package['version']
    if not re.fullmatch(r'v\d+\.\d+\.\d+-alpha\.\d+', tag) or tag != 'v' + version:
        raise ValueError('Tag must exactly match the Cargo alpha version')
    revision = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip()
    build = json.loads((output / 'BUILD-INFO.json').read_text())
    if build['version'] != version or build['source_revision'] != revision or build['source_has_uncommitted_changes']:
        raise ValueError('Packages must come from this exact, clean source revision')
    if build['target'] != 'x86_64-unknown-linux-gnu' or build['build_libc'][0] != 'glibc' or tuple(map(int, build['build_libc'][1].split('.'))) > (2, 39):
        raise ValueError('This prerelease requires the Ubuntu 24.04 x86_64 build baseline')
    if build['cargo_lock_sha256'] != hashlib.sha256((ROOT / 'Cargo.lock').read_bytes()).hexdigest():
        raise ValueError('Lockfile does not match the package build')
    for name, checksum in build['artifacts'].items():
        if Path(name).name != name or hashlib.sha256((output / name).read_bytes()).hexdigest() != checksum:
            raise ValueError('Package checksum mismatch')
    audit = json.loads((output / 'RUSTSEC-AUDIT.json').read_text())
    if audit.get('vulnerabilities', {}).get('found') is not False:
        raise ValueError('A successful RustSec audit is required')
    archive = output / f'linux-system-explorer-{version}-x86_64-unknown-linux-gnu.tar.gz'
    with tarfile.open(archive) as source:
        prefix = f'linux-system-explorer-{version}/share/licenses/io.github.janderpt.linux-system-explorer/'
        for name in ['THIRD_PARTY.json', 'THIRD_PARTY_NOTICES.txt']:
            (output / name).write_bytes(source.extractfile(prefix + name).read())
        report = json.loads((output / 'THIRD_PARTY.json').read_text())
        if report['packages_without_notice_files'] or any(not p['license'] for p in report['packages']):
            raise ValueError('Dependency notices or license declarations are missing')
        for crate in report['packages']:
            if 'MPL-2.0' in crate['license']:
                if not crate['included_source']:
                    raise ValueError('Missing MPL source bundle')
                source.getmember(prefix + crate['included_source'] + '/src/lib.rs')
    (output / 'INSTALL.md').write_bytes((ROOT / f'docs/releases/{tag}.md').read_bytes())
    (output / 'DISTRIBUTION-REVIEW.md').write_bytes((ROOT / 'docs/THIRD_PARTY_REVIEW.md').read_bytes())
    names = [*build['artifacts'], 'BUILD-INFO.json', 'RUSTSEC-AUDIT.json', 'THIRD_PARTY.json', 'THIRD_PARTY_NOTICES.txt', 'INSTALL.md', 'DISTRIBUTION-REVIEW.md']
    with (output / 'SHA256SUMS').open('w') as checksums:
        for name in sorted(names):
            checksums.write(f'{hashlib.sha256((output / name).read_bytes()).hexdigest()}  {name}\n')
    print(f'Validated alpha assets for {tag} at {revision}')


if __name__ == '__main__':
    if len(sys.argv) != 3:
        raise SystemExit('Usage: finalize-prerelease.py DIST TAG')
    finalize(Path(sys.argv[1]).resolve(), sys.argv[2])
