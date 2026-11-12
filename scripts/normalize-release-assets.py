#!/usr/bin/env python3
"""Reconcile GitHub-normalized asset filenames without changing package bytes."""
import hashlib
import json
from pathlib import Path
import sys


def normalize(directory, notes):
    checksums = {}
    for line in (directory / 'SHA256SUMS').read_text().splitlines():
        digest, old_name = line.split('  ')
        name = old_name.replace('~', '.')
        if Path(name).name != name:
            raise ValueError('Invalid asset name')
        if hashlib.sha256((directory / name).read_bytes()).hexdigest() != digest:
            raise ValueError('The uploaded asset bytes differ from the original checksum')
        checksums[name] = digest
    path = directory / 'BUILD-INFO.json'
    build = json.loads(path.read_text())
    artifacts = build['artifacts']
    renames = {name: name.replace('~', '.') for name in artifacts if '~' in name}
    if renames:
        build['original_artifacts'] = artifacts
        build['public_asset_name_changes'] = renames
        build['artifacts'] = {name.replace('~', '.'): digest for name, digest in artifacts.items()}
    # Source revision and build-clean status describe the original, unchanged binary build.
    path.write_text(json.dumps(build, indent=2) + '\n')
    (directory / 'INSTALL.md').write_bytes(notes.read_bytes())
    with (directory / 'SHA256SUMS').open('w') as output:
        for name in sorted(checksums):
            digest = hashlib.sha256((directory / name).read_bytes()).hexdigest()
            output.write(f'{digest}  {name}\n')
    print('Asset names reconciled; original package bytes and source provenance preserved')


if __name__ == '__main__':
    normalize(Path(sys.argv[1]), Path(sys.argv[2]))
