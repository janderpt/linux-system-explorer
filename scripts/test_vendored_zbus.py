import importlib.util
from pathlib import Path
import shutil
import tempfile
import unittest

SPEC = importlib.util.spec_from_file_location('lse_zbus_source', Path(__file__).with_name('check-vendored-zbus.py'))
SOURCE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(SOURCE)


class VendoredSourceTests(unittest.TestCase):
    def test_packaging_rejects_unreviewed_source_patch_and_symlink_changes(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            shutil.copyfile(SOURCE.ROOT / 'Cargo.toml', root / 'Cargo.toml')
            for name in ['vendor/zbus', 'packaging/patches', 'packaging/third-party']:
                shutil.copytree(SOURCE.ROOT / name, root / name)
            SOURCE.verify(root)
            for name in ['vendor/zbus/src/lib.rs', 'vendor/zbus/src/connection/handshake/client.rs', 'packaging/patches/zbus-flatpak-auth.patch']:
                path = root / name
                original = path.read_bytes()
                path.write_bytes(original + b'\n// unreviewed change\n')
                with self.assertRaises(ValueError):
                    SOURCE.verify(root)
                path.write_bytes(original)
            path = root / 'vendor/zbus/unreviewed.rs'
            path.write_text('extra source')
            with self.assertRaises(ValueError):
                SOURCE.verify(root)
            path.unlink()
            path.symlink_to('/etc/passwd')
            with self.assertRaisesRegex(ValueError, 'Symlinks'):
                SOURCE.verify(root)


if __name__ == '__main__':
    unittest.main()
