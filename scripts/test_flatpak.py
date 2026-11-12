import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
from types import SimpleNamespace
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location('lse_flatpak', Path(__file__).with_name('flatpak.py'))
FLATPAK = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(FLATPAK)


class FlatpakStagingTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.install = Path(self.temporary.name) / 'app'
        self.binary = self.install / 'bin' / FLATPAK.PACKAGE.NAME
        self.binary.parent.mkdir(parents=True)
        self.binary.write_bytes(b'synthetic executable')
        self.metadata = {
            'app_id': FLATPAK.PACKAGE.APP_ID, 'version': FLATPAK.PACKAGE.VERSION,
            'stage_kind': 'experimental-flatpak',
            'binary_sha256': FLATPAK.digest(self.binary),
            'staged_files': FLATPAK.tree_hashes(self.install),
        }
        self.write_metadata()

    def write_metadata(self):
        (self.install / 'flatpak-stage.json').write_text(json.dumps(self.metadata))

    def test_wrong_version_and_tampered_files_prevent_bundling(self):
        self.assertEqual(FLATPAK.validate_stage(self.install), self.metadata)
        self.metadata['version'] = 'invalid'
        self.write_metadata()
        with self.assertRaisesRegex(ValueError, 'identity/version'):
            FLATPAK.validate_stage(self.install)
        self.metadata['version'] = FLATPAK.PACKAGE.VERSION
        self.write_metadata()
        self.binary.write_bytes(b'changed')
        with self.assertRaisesRegex(ValueError, 'binary checksum'):
            FLATPAK.validate_stage(self.install)
        self.binary.write_bytes(b'synthetic executable')
        (self.install / 'extra').write_text('unrecorded file')
        with self.assertRaisesRegex(ValueError, 'file checksum'):
            FLATPAK.validate_stage(self.install)

    def test_staging_symlinks_cannot_import_files_from_outside_the_stage(self):
        (self.install / 'link').symlink_to('/etc/passwd')
        with self.assertRaisesRegex(ValueError, 'symbolic links'):
            FLATPAK.validate_stage(self.install)

    def test_existing_outputs_are_never_overwritten(self):
        output = self.install.parent / 'output'
        output.mkdir()
        keep = output / 'keep'
        keep.write_text('preserved')
        with patch.object(FLATPAK.shutil, 'which', return_value='/usr/bin/flatpak'):
            with self.assertRaisesRegex(ValueError, 'must be empty'):
                FLATPAK.build(self.install, self.install.parent / 'work', output)
        self.assertEqual(keep.read_text(), 'preserved')

    def test_educational_manifest_is_explicitly_local_and_matches_tested_permissions(self):
        example = json.loads((FLATPAK.ROOT / 'docs/examples/flatpak-local.json').read_text())
        self.assertEqual(example['finish-args'], FLATPAK.PERMISSIONS)
        self.assertEqual(example['branch'], 'experimental')
        self.assertEqual(example['runtime-version'], FLATPAK.RUNTIME_VERSION)
        self.assertEqual(example['modules'][0]['sources'][0]['type'], 'dir')
        for permission in FLATPAK.PERMISSIONS:
            self.assertNotIn('org.freedesktop.Flatpak', permission)
            self.assertNotIn('socket=session-bus', permission)
            self.assertNotIn('socket=system-bus', permission)
            self.assertNotIn('share=network', permission)

    def test_obsolete_dbus_proxy_is_rejected_before_building_a_broken_bundle(self):
        for version, allowed in [('0.1.5', False), ('0.1.6', False), ('0.1.9', True), ('0.2.0', True)]:
            with patch.object(FLATPAK, 'run', return_value=SimpleNamespace(stdout='xdg-dbus-proxy ' + version)):
                if allowed:
                    self.assertTrue(FLATPAK.proxy_version().endswith(version))
                else:
                    with self.assertRaisesRegex(RuntimeError, 'old proxies'):
                        FLATPAK.proxy_version()


if __name__ == '__main__':
    unittest.main()
