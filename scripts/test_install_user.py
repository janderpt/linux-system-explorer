import importlib.util
import json
import os
from pathlib import Path
import subprocess
import shutil
import tempfile
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location("user_installer", Path(__file__).with_name("install-user.py"))
INSTALLER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(INSTALLER)


class UserInstallationTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="lse-install-test-")
        self.addCleanup(self.temporary.cleanup)
        self.home = Path(self.temporary.name)
        self.data = self.home / "data"
        self.config = self.home / "config"
        self.state = self.home / "state" / INSTALLER.PACKAGE.NAME / "install-state.json"
        self.config.mkdir()
        self.mime = self.config / "mimeapps.list"
        self.mime.write_text("[Default Applications]\ntext/plain=editor.desktop\n")
        self.binary = self.home / "source-binary"
        self.binary.write_bytes(b"new executable")
        self.binary.chmod(0o755)
        self.handler = "org.kde.dolphin.desktop"
        self.enterContext(patch.dict(os.environ, {
            "HOME": str(self.home), "XDG_DATA_HOME": str(self.data),
            "XDG_CONFIG_HOME": str(self.config), "XDG_STATE_HOME": str(self.state.parent.parent),
        }))
        self.enterContext(patch.object(INSTALLER, "default_handler", side_effect=lambda: self.handler))
        self.change = self.enterContext(patch.object(INSTALLER, "set_handler", side_effect=self.set_handler))
        self.enterContext(patch.object(INSTALLER, "refresh"))
        self.enterContext(patch.object(INSTALLER, "run", return_value=subprocess.CompletedProcess([], 0, stdout="test-revision\n")))
        self.enterContext(patch.object(INSTALLER.PACKAGE, "stage", side_effect=self.stage))

    def set_handler(self, handler):
        self.handler = handler

    def stage(self, root, prefix, binary):
        target = root / prefix.lstrip("/")
        (target / "bin").mkdir(parents=True)
        INSTALLER.atomic_copy(binary, target / "bin" / INSTALLER.PACKAGE.NAME)
        INSTALLER.PACKAGE.render_metadata(target)
        return target, {}

    def install(self, make_default=True):
        INSTALLER.install(self.binary, make_default, self.data, self.config, self.state)

    def test_install_backs_up_existing_binary_and_preserves_previous_folder_choice(self):
        existing = self.home / ".local/bin" / INSTALLER.PACKAGE.NAME
        existing.parent.mkdir(parents=True)
        existing.write_bytes(b"old executable")
        self.install()
        state = json.loads(self.state.read_text())
        self.assertEqual(self.handler, INSTALLER.DESKTOP_ID)
        self.assertEqual(state["previous_folder_handler"], "org.kde.dolphin.desktop")
        self.assertEqual(existing.read_bytes(), b"new executable")
        self.assertTrue(existing.stat().st_mode & 0o100)
        self.assertEqual((Path(state["backup"]) / "bin" / INSTALLER.PACKAGE.NAME).read_bytes(), b"old executable")
        self.assertEqual(self.mime.read_text(), "[Default Applications]\ntext/plain=editor.desktop\n")

    def test_reinstall_retains_the_original_default_for_rollback(self):
        self.install()
        self.install()
        self.assertEqual(json.loads(self.state.read_text())["previous_folder_handler"], "org.kde.dolphin.desktop")

    def test_failed_default_change_restores_the_previous_choice(self):
        def fail_new(handler):
            if handler == INSTALLER.DESKTOP_ID:
                raise RuntimeError("desktop rejected the new handler")
            self.handler = handler
        self.change.side_effect = fail_new
        with self.assertRaises(RuntimeError):
            self.install()
        self.assertEqual(self.handler, "org.kde.dolphin.desktop")
        self.assertEqual(self.change.call_args_list[-1].args, ("org.kde.dolphin.desktop",))

    def test_restore_default_leaves_a_newer_user_choice_untouched(self):
        self.install()
        self.handler = "other-file-manager.desktop"
        self.change.reset_mock()
        with patch("sys.argv", ["install-user.py", "restore-default"]):
            with self.assertRaises(RuntimeError):
                INSTALLER.main()
        self.assertEqual(self.handler, "other-file-manager.desktop")
        self.change.assert_not_called()

    @unittest.skipUnless(shutil.which("xdg-mime"), "xdg-utils not installed")
    def test_installed_launcher_is_recognized_by_real_xdg_mime_query(self):
        binary = self.home / ".local/bin" / INSTALLER.PACKAGE.NAME
        INSTALLER.atomic_copy(self.binary, binary)
        template = (INSTALLER.ROOT / "packaging/linux-system-explorer.desktop.in").read_text().replace("@APP_ID@", INSTALLER.APP_ID)
        launcher = self.data / "applications" / INSTALLER.DESKTOP_ID
        launcher.parent.mkdir(parents=True)
        launcher.write_text(INSTALLER.launcher_text(template, binary))
        self.mime.write_text("[Default Applications]\ninode/directory=" + INSTALLER.DESKTOP_ID + "\n")
        env = os.environ | {"XDG_CURRENT_DESKTOP": "Generic", "XDG_CONFIG_DIRS": str(self.home / "no-config"), "XDG_DATA_DIRS": str(self.home / "no-data")}
        result = subprocess.run(["xdg-mime", "query", "default", "inode/directory"], env=env, check=True, capture_output=True, text=True)
        self.assertEqual(result.stdout.strip(), INSTALLER.DESKTOP_ID)


if __name__ == "__main__":
    unittest.main()
