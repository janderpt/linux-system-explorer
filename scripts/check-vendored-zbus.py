#!/usr/bin/env python3
"""Verify the reviewed upstream zbus source and the sole compatibility patch."""
import hashlib
import json
from pathlib import Path
import tomllib

ROOT = Path(__file__).resolve().parent.parent


def verify(root=ROOT):
    record = json.loads((root / 'packaging/third-party/zbus-source.json').read_text())
    source = root / record['vendor_path']
    project = tomllib.loads((root / 'Cargo.toml').read_text())
    if (project['dependencies']['zbus'] != '=' + record['version']
            or project['patch']['crates-io']['zbus']['path'] != record['vendor_path']):
        raise ValueError('The reviewed zbus override/version is not selected by the project')
    manifest = tomllib.loads((source / 'Cargo.toml').read_text())['package']
    if (manifest['name'], manifest['version'], manifest['license']) != ('zbus', record['version'], 'MIT'):
        raise ValueError('Vendored zbus package identity/license mismatch')
    expected = record['upstream_files'] | record['modified_files'] | record['added_files']
    if set(record['modified_files']) != {'src/connection/handshake/client.rs'}:
        raise ValueError('Unreviewed upstream modification scope')
    if set(record['added_files']) != {'README.lse.md'}:
        raise ValueError('Unreviewed vendored file additions')
    paths = sorted(source.rglob('*'))
    if any(p.is_symlink() for p in paths):
        raise ValueError('Symlinks are not allowed in the verified zbus source')
    actual = {p.relative_to(source).as_posix(): hashlib.sha256(p.read_bytes()).hexdigest()
              for p in paths if p.is_file()}
    if expected != actual:
        changed = sorted(p for p in expected.keys() | actual.keys() if expected.get(p) != actual.get(p))
        raise ValueError('Unreviewed zbus source changes: ' + ', '.join(changed))
    patch = root / record['patch_path']
    if hashlib.sha256(patch.read_bytes()).hexdigest() != record['patch_sha256']:
        raise ValueError('zbus compatibility patch checksum mismatch')
    return record


if __name__ == '__main__':
    record = verify()
    print('Verified zbus ' + record['version'] + ': upstream source, MIT license and compatibility patch')
