import importlib.util
import json
from pathlib import Path
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location("lse_package_test", Path(__file__).with_name("package.py"))
PACKAGE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(PACKAGE)


class PreviewRuntimeTests(unittest.TestCase):
    def test_dynamic_preview_backends_are_declared_in_debian_dependencies(self):
        with tempfile.TemporaryDirectory() as directory:
            with patch.object(PACKAGE, "run", return_value=SimpleNamespace(stdout="shlibs:Depends=libc6 (>= 2.39)\n")):
                depends = PACKAGE.debian_dependencies("test-binary", Path(directory))
        self.assertIn("libc6 (>= 2.39)", depends)
        runtime = json.loads((PACKAGE.ROOT / "packaging/preview-runtime.json").read_text())
        for section in ["media", "pdf", "office", "html", "archive"]:
            for dependency in runtime[section]["debian"]:
                self.assertIn(dependency, depends.split(", "))
        self.assertIn("gstreamer1.0-libav", depends)
        self.assertIn("poppler-utils", depends)
        self.assertIn("libreoffice-writer", depends)
        self.assertIn("libreoffice-calc", depends)
        self.assertIn("libreoffice-impress", depends)
        self.assertIn("libreoffice-draw", depends)
        self.assertIn("libreoffice-math", depends)
        self.assertIn("weasyprint", depends)


if __name__ == "__main__":
    unittest.main()
