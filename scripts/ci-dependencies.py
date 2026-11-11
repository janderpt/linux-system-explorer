#!/usr/bin/env python3
"""Install CI dependencies through the Linux Kernel Archives' Ubuntu mirror."""
import os
from pathlib import Path
import subprocess
import sys

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


def main():
    if os.environ.get("GITHUB_ACTIONS") != "true" or os.geteuid() != 0:
        raise SystemExit("This helper is restricted to root inside GitHub Actions runners")
    sources = Path("/etc/apt/sources.list.d")
    paths = [Path("/etc/apt/sources.list"), *sources.glob("*.list"), *sources.glob("*.sources")]
    for path in paths:
        if not path.is_file():
            continue
        text = path.read_text()
        updated = direct_mirrors(text)
        if updated != text:
            path.write_text(updated)
            print("Using the Linux Kernel Archives Ubuntu HTTPS mirror:", path, flush=True)
    options = ["apt-get", "-o", "Acquire::Retries=2", "-o", "Acquire::http::Timeout=20",
               "-o", "Acquire::https::Timeout=20", "-o", "DPkg::Lock::Timeout=60", "-o", "Acquire::ForceIPv4=true"]
    environment = os.environ | {"DEBIAN_FRONTEND": "noninteractive"}
    subprocess.run([*options, "-o", "APT::Update::Error-Mode=any", "update"], check=True, timeout=240, env=environment)
    subprocess.run([*options, "install", "-y", "--no-install-recommends", *sys.argv[1:]], check=True, timeout=240, env=environment)


if __name__ == "__main__":
    main()
