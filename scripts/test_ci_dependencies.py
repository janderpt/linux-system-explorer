import importlib.util
from pathlib import Path
import unittest
import tempfile

SPEC = importlib.util.spec_from_file_location("ci_dependencies", Path(__file__).with_name("ci-dependencies.py"))
CI = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CI)


class AptMirrorsTests(unittest.TestCase):
    def test_private_snapshot_preserves_keys_and_remains_stable_if_runner_sources_change(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            apt = root / "apt"
            (apt / "sources.list.d").mkdir(parents=True)
            source = apt / "sources.list.d/ubuntu.sources"
            source.write_text("Types: deb\nURIs: http://archive.ubuntu.com/ubuntu\nSuites: noble\nComponents: main\nSigned-By: /usr/share/keyrings/ubuntu-archive-keyring.gpg\n")
            original = source.read_text()
            private = root / "private"
            private.mkdir()
            options = CI.snapshot_sources(apt, private)
            self.assertEqual(source.read_text(), original)
            source.write_text("changed by runner maintenance")
            snapshot = (private / "sources.list.d/ubuntu.sources").read_text()
            self.assertIn(CI.MIRROR, snapshot)
            self.assertIn("Signed-By: /usr/share/keyrings/ubuntu-archive-keyring.gpg", snapshot)
            self.assertNotIn("changed", snapshot)
            self.assertIn(f"Dir::Etc::sourceparts={private / 'sources.list.d'}", options)

    def test_replaces_mirror_lists_without_losing_suites_or_signatures(self):
        source = "Types: deb\nURIs: mirror+file:/etc/apt/apt-mirrors.txt\nSuites: noble noble-updates\nComponents: main universe\nSigned-By: /usr/share/keyrings/ubuntu-archive-keyring.gpg\n\nURIs: mirror+file:/etc/apt/apt-mirrors-security.txt\nSuites: noble-security\n"
        result = CI.direct_mirrors(source)
        self.assertNotIn("mirror+file:", result)
        self.assertEqual(result.count("URIs: https://mirrors.edge.kernel.org/ubuntu/"), 2)
        self.assertIn("Suites: noble noble-updates", result)
        self.assertIn("Signed-By: /usr/share/keyrings/ubuntu-archive-keyring.gpg", result)
        self.assertEqual(result, CI.direct_mirrors(result))

    def test_replaces_azure_but_preserves_other_repositories(self):
        source = "deb http://azure.archive.ubuntu.com/ubuntu noble main\ndeb https://packages.microsoft.com/repos/code stable main\n"
        result = CI.direct_mirrors(source)
        self.assertNotIn("azure.archive", result)
        self.assertIn("https://mirrors.edge.kernel.org/ubuntu noble main", result)
        self.assertIn("https://packages.microsoft.com/repos/code stable main", result)
