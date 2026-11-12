import hashlib
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

SPEC = importlib.util.spec_from_file_location('lse_asset_names', Path(__file__).with_name('normalize-release-assets.py'))
ASSETS = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(ASSETS)


class PublicAssetNamesTests(unittest.TestCase):
    def test_github_name_changes_preserve_package_bytes_and_source_provenance(self):
        with tempfile.TemporaryDirectory() as work:
            root = Path(work)
            old = 'package_0.1.0~alpha.1.deb'
            public = old.replace('~', '.')
            package = root / public
            package.write_bytes(b'original binary')
            digest = hashlib.sha256(package.read_bytes()).hexdigest()
            build = root / 'BUILD-INFO.json'
            build.write_text(json.dumps({'source_revision': 'original-commit', 'source_has_uncommitted_changes': False, 'artifacts': {old: digest}}))
            install = root / 'INSTALL.md'
            install.write_text('old instructions')
            (root / 'SHA256SUMS').write_text(''.join(f'{hashlib.sha256(path.read_bytes()).hexdigest()}  {name}\n' for path, name in [(package, old), (build, build.name), (install, install.name)]))
            notes = root / 'new-notes.md'
            notes.write_text('new public instructions')
            ASSETS.normalize(root, notes)
            ASSETS.normalize(root, notes)  # Safe to retry without changing provenance.
            metadata = json.loads(build.read_text())
            self.assertEqual(metadata['source_revision'], 'original-commit')
            self.assertFalse(metadata['source_has_uncommitted_changes'])
            self.assertEqual(metadata['original_artifacts'], {old: digest})
            self.assertEqual(metadata['artifacts'], {public: digest})
            self.assertEqual(package.read_bytes(), b'original binary')
            for line in (root / 'SHA256SUMS').read_text().splitlines():
                expected, name = line.split('  ')
                self.assertEqual(expected, hashlib.sha256((root / name).read_bytes()).hexdigest())

    def test_changed_package_is_rejected_before_instructions_are_updated(self):
        with tempfile.TemporaryDirectory() as work:
            root = Path(work)
            (root / 'package.deb').write_bytes(b'changed')
            (root / 'SHA256SUMS').write_text('0' * 64 + '  package.deb\n')
            with self.assertRaisesRegex(ValueError, 'bytes differ'):
                ASSETS.normalize(root, root / 'unused.md')


if __name__ == '__main__':
    unittest.main()
