import hashlib
import importlib.util
import io
import json
from pathlib import Path
import tarfile
import tempfile
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location('lse_finalize', Path(__file__).with_name('finalize-prerelease.py'))
RELEASE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(RELEASE)


class PrereleaseIntegrityTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.output = self.root / 'dist'
        self.output.mkdir()
        (self.root / 'Cargo.toml').write_text('[package]\nversion="0.1.0-alpha.1"\n')
        (self.root / 'Cargo.lock').write_text('test lock')
        docs = self.root / 'docs/releases'
        docs.mkdir(parents=True)
        (docs / 'v0.1.0-alpha.1.md').write_text('installation')
        (self.root / 'docs/THIRD_PARTY_REVIEW.md').write_text('review')
        self.archive = self.output / 'linux-system-explorer-0.1.0-alpha.1-x86_64-unknown-linux-gnu.tar.gz'
        report = {'packages_without_notice_files': [], 'packages': [{'name': 'mpl-crate', 'license': 'MPL-2.0', 'included_source': 'third-party-sources/mpl-crate-1'}]}
        with tarfile.open(self.archive, 'w:gz') as archive:
            prefix = 'linux-system-explorer-0.1.0-alpha.1/share/licenses/io.github.janderpt.linux-system-explorer/'
            for name, value in [('THIRD_PARTY.json', json.dumps(report)), ('THIRD_PARTY_NOTICES.txt', 'notices'), ('third-party-sources/mpl-crate-1/src/lib.rs', 'source')]:
                data = value.encode()
                info = tarfile.TarInfo(prefix + name)
                info.size = len(data)
                archive.addfile(info, io.BytesIO(data))
        deb = self.output / 'package.deb'
        deb.write_bytes(b'test package')
        self.build = {'version': '0.1.0-alpha.1', 'source_revision': 'abc', 'source_has_uncommitted_changes': False,
                      'target': 'x86_64-unknown-linux-gnu', 'build_libc': ['glibc', '2.39'],
                      'cargo_lock_sha256': hashlib.sha256((self.root / 'Cargo.lock').read_bytes()).hexdigest(),
                      'artifacts': {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in [self.archive, deb]}}
        (self.output / 'RUSTSEC-AUDIT.json').write_text('{"vulnerabilities":{"found":false}}')
        self.root_patch = patch.object(RELEASE, 'ROOT', self.root)
        self.root_patch.start()
        self.addCleanup(self.root_patch.stop)
        self.git_patch = patch.object(RELEASE.subprocess, 'check_output', return_value='abc\n')
        self.git_patch.start()
        self.addCleanup(self.git_patch.stop)

    def finalize(self, tag='v0.1.0-alpha.1'):
        (self.output / 'BUILD-INFO.json').write_text(json.dumps(self.build))
        RELEASE.finalize(self.output, tag)

    def test_checksums_cover_packages_audit_build_info_and_instructions(self):
        self.finalize()
        for line in (self.output / 'SHA256SUMS').read_text().splitlines():
            expected, name = line.split('  ')
            self.assertEqual(expected, hashlib.sha256((self.output / name).read_bytes()).hexdigest())
        self.assertEqual((self.output / 'INSTALL.md').read_text(), 'installation')
        self.assertTrue((self.output / 'THIRD_PARTY.json').is_file())

    def test_tampered_package_prevents_finalization(self):
        (self.output / 'package.deb').write_bytes(b'changed')
        with self.assertRaisesRegex(ValueError, 'checksum'):
            self.finalize()

    def test_dirty_build_or_newer_libc_prevents_publication(self):
        self.build['source_has_uncommitted_changes'] = True
        with self.assertRaisesRegex(ValueError, 'clean'):
            self.finalize()
        self.build['source_has_uncommitted_changes'] = False
        self.build['build_libc'] = ['glibc', '2.43']
        with self.assertRaisesRegex(ValueError, 'baseline'):
            self.finalize()

    def test_stable_tag_or_audit_failure_cannot_publish_an_alpha(self):
        with self.assertRaisesRegex(ValueError, 'Tag'):
            self.finalize('v0.1.0')
        (self.output / 'RUSTSEC-AUDIT.json').write_text('{"vulnerabilities":{"found":true}}')
        with self.assertRaisesRegex(ValueError, 'audit'):
            self.finalize()


if __name__ == '__main__':
    unittest.main()
