#!/usr/bin/env python3
"""Install CI dependencies using direct official Ubuntu HTTPS mirrors."""
import os
from pathlib import Path
import subprocess
import sys


def direct_mirrors(text):
    replacements = {
        "mirror+file:/etc/apt/apt-mirrors.txt": "https://archive.ubuntu.com/ubuntu/",
        "mirror+file:/etc/apt/apt-mirrors-security.txt": "https://security.ubuntu.com/ubuntu/",
        "http://azure.archive.ubuntu.com/ubuntu": "https://archive.ubuntu.com/ubuntu",
        "https://azure.archive.ubuntu.com/ubuntu": "https://archive.ubuntu.com/ubuntu",
        "http://archive.ubuntu.com/ubuntu": "https://archive.ubuntu.com/ubuntu",
        "http://security.ubuntu.com/ubuntu": "https://security.ubuntu.com/ubuntu",
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
            print("Using direct official Ubuntu HTTPS mirrors:", path, flush=True)
    options = ["apt-get", "-o", "Acquire::Retries=3", "-o", "Acquire::http::Timeout=30",
               "-o", "Acquire::https::Timeout=30", "-o", "DPkg::Lock::Timeout=60"]
    environment = os.environ | {"DEBIAN_FRONTEND": "noninteractive"}
    subprocess.run([*options, "-o", "APT::Update::Error-Mode=any", "update"], check=True, timeout=240, env=environment)
    subprocess.run([*options, "install", "-y", "--no-install-recommends", *sys.argv[1:]], check=True, timeout=240, env=environment)


if __name__ == "__main__":
    main()
