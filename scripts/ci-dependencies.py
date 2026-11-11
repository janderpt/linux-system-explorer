#!/usr/bin/env python3
"""Install CI dependencies through the Linux Kernel Archives' Ubuntu mirror."""
import os
from pathlib import Path
import subprocess
import sys
import tempfile

MIRROR = "https://mirrors.edge.kernel.org/ubuntu"


def direct_mirrors(text):
    replacements = {
        "mirror+file:/etc/apt/apt-mirrors.txt": MIRROR + "/",
        "mirror+file:/etc/apt/apt-mirrors-security.txt": MIRROR + "/",
        "http://azure.archive.ubuntu.com/ubuntu": MIRROR,
        "https://azure.archive.ubuntu.com/ubuntu": MIRROR,
        "http://archive.ubuntu.com/ubuntu": MIRROR,
        "https://archive.ubuntu.com/ubuntu": MIRROR,
        "http://security.ubuntu.com/ubuntu": MIRROR,
        "https://security.ubuntu.com/ubuntu": MIRROR,
    }
    for old, new in replacements.items():
        text = text.replace(old, new)
    return text


def snapshot_sources(apt_directory, destination):
    """Use identical source URIs for update/install without modifying runner files."""
    parts = destination / "sources.list.d"
    parts.mkdir()
    main = apt_directory / "sources.list"
    (destination / "sources.list").write_text(direct_mirrors(main.read_text()) if main.is_file() else "")
    for path in sorted((apt_directory / "sources.list.d").glob("*")):
        if path.is_file() and path.suffix in (".list", ".sources"):
            (parts / path.name).write_text(direct_mirrors(path.read_text()))
    return ["-o", f"Dir::Etc::sourcelist={destination / 'sources.list'}",
            "-o", f"Dir::Etc::sourceparts={parts}"]


def main():
    if os.environ.get("GITHUB_ACTIONS") != "true" or os.geteuid() != 0:
        raise SystemExit("This helper is restricted to root inside GitHub Actions runners")
    options = ["apt-get", "-o", "Acquire::Retries=2", "-o", "Acquire::http::Timeout=20",
               "-o", "Acquire::https::Timeout=20", "-o", "DPkg::Lock::Timeout=60", "-o", "Acquire::ForceIPv4=true"]
    environment = os.environ | {"DEBIAN_FRONTEND": "noninteractive"}
    # Keep update/install on the same private index set, independent of runner cleanup
    # hooks or background APT maintenance. Package signatures remain mandatory.
    with tempfile.TemporaryDirectory(prefix="lse-ci-apt-") as temporary:
        Path(temporary).chmod(0o755)
        lists = Path(temporary) / "lists"
        lists.mkdir()
        config = snapshot_sources(Path("/etc/apt"), Path(temporary))
        config += ["-o", f"Dir::State::lists={lists}", "-o", f"Dir::Cache::pkgcache={temporary}/pkgcache.bin",
                   "-o", f"Dir::Cache::srcpkgcache={temporary}/srcpkgcache.bin"]
        options += config
        print("Using a private source/index snapshot and the Ubuntu HTTPS mirror", flush=True)
        subprocess.run([*options, "-o", "APT::Update::Error-Mode=any", "update"], check=True, timeout=240, env=environment)
        try:
            subprocess.run([*options, "install", "-y", "--no-install-recommends", *sys.argv[1:]], check=True, timeout=240, env=environment)
        except subprocess.CalledProcessError:
            subprocess.run(["dpkg", "--print-architecture"], check=False)
            subprocess.run(["apt-cache", *config, "policy", *sys.argv[1:]], check=False)
            subprocess.run([*options, "indextargets", "--format", "$(FILENAME)"], check=False)
            print("Downloaded APT indexes:", [(p.name, p.stat().st_size) for p in lists.iterdir() if p.is_file()], flush=True)
            raise


if __name__ == "__main__":
    main()
