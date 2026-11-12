#!/usr/bin/env python3
"""Build experimental native packages without installing anything on the host."""
import argparse
import datetime
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import shutil
import subprocess
import struct
import tarfile
import tempfile
import tomllib
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parent.parent
PACKAGE = tomllib.loads((ROOT / "Cargo.toml").read_text())["package"]
NAME = PACKAGE["name"]
VERSION = PACKAGE["version"]
APP_ID = (ROOT / "packaging/app-id").read_text().strip()


def run(args, **kwargs):
    return subprocess.run([str(a) for a in args], check=True, **kwargs)


def rust_tool(name):
    return shutil.which(name) or str(Path.home() / ".cargo/bin" / name)


def debian_version(version):
    # Debian sorts ~alpha before the corresponding final version.
    return version.replace("-", "~", 1) + "-1"


def host_target():
    info = run([rust_tool("rustc"), "-vV"], capture_output=True, text=True).stdout
    return next(line.removeprefix("host: ") for line in info.splitlines() if line.startswith("host: "))


def render_metadata(prefix):
    replacements = {
        "@APP_ID@": APP_ID,
        "@VERSION@": VERSION,
        "@RELEASE_DATE@": datetime.datetime.fromtimestamp(
            int(os.environ.get("SOURCE_DATE_EPOCH", "0")) or int((ROOT / "Cargo.toml").stat().st_mtime),
            datetime.timezone.utc,
        ).date().isoformat(),
    }
    for source, destination in [
        ("linux-system-explorer.desktop.in", f"share/applications/{APP_ID}.desktop"),
        ("linux-system-explorer.metainfo.xml.in", f"share/metainfo/{APP_ID}.metainfo.xml"),
    ]:
        text = (ROOT / "packaging" / source).read_text()
        for key, value in replacements.items():
            text = text.replace(key, value)
        if re.search(r"@[A-Z_]+@", text):
            raise ValueError(f"Unresolved metadata placeholder in {source}")
        path = prefix / destination
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text)
    icon = prefix / f"share/icons/hicolor/scalable/apps/{APP_ID}.svg"
    icon.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(ROOT / "assets/icons/linux-system-explorer.svg", icon)


def validate(prefix):
    desktop = prefix / f"share/applications/{APP_ID}.desktop"
    metainfo = prefix / f"share/metainfo/{APP_ID}.metainfo.xml"
    tree = ET.parse(metainfo).getroot()
    assert tree.findtext("id") == APP_ID
    assert tree.findtext("launchable") == desktop.name
    assert tree.findtext("project_license") == PACKAGE["license"]
    assert f"Icon={APP_ID}\n" in desktop.read_text()
    for tool, args in [
        ("desktop-file-validate", [desktop]),
        ("appstreamcli", ["validate", "--no-net", metainfo]),
    ]:
        if not shutil.which(tool):
            raise RuntimeError(f"Install {tool} before validating packages")
        run([tool, *args])


def font_copyright(font):
    """Preserve copyright/license name records from the original embedded SFNT font."""
    data = font.read_bytes()
    for index in range(struct.unpack_from(">H", data, 4)[0]):
        record = 12 + index * 16
        if data[record:record + 4] != b"name":
            continue
        offset = struct.unpack_from(">I", data, record + 8)[0]
        _, count, strings = struct.unpack_from(">HHH", data, offset)
        texts = set()
        for index in range(count):
            platform_id, _, _, name_id, length, position = struct.unpack_from(">6H", data, offset + 6 + 12 * index)
            if name_id in (0, 13, 14):
                raw = data[offset + strings + position:offset + strings + position + length]
                texts.add(raw.decode("utf-16-be" if platform_id in (0, 3) else "mac_roman", errors="replace"))
        return "\n".join(sorted(texts))
    raise ValueError(f"Font name table is missing: {font.name}")


def third_party(destination, target):
    """Inventory metadata and available notices; this is not a legal audit."""
    result = run([rust_tool("cargo"), "metadata", "--locked", "--offline", "--format-version", "1",
                  "--filter-platform", target], cwd=ROOT, capture_output=True, text=True)
    metadata = json.loads(result.stdout)
    tree = run([rust_tool("cargo"), "tree", "--locked", "--offline", "--target", target,
                "-e", "normal,build", "--prefix", "none", "--format", "{p}"],
               cwd=ROOT, capture_output=True, text=True).stdout
    active = {tuple(line.split()[:2]) for line in tree.splitlines()}
    bundled = json.loads((ROOT / "packaging/third-party/sources.json").read_text())
    inventory = []
    font_notices = ["Copyright/license metadata preserved from the unmodified embedded fonts.", ""]
    fontawesome = ROOT / "assets/fontawesome/fa-solid-900.otf"
    font_notices += [fontawesome.name, font_copyright(fontawesome), ""]
    for package in sorted(metadata["packages"], key=lambda p: (p["name"], p["version"])):
        if (package["name"], "v" + package["version"]) not in active or package["name"] == NAME:
            continue
        source = Path(package["manifest_path"]).parent
        if package["name"] == "epaint_default_fonts":
            for font in sorted((source / "fonts").glob("*.ttf")):
                font_notices += [font.name, font_copyright(font), ""]
        texts = []
        for path in source.rglob("*"):
            is_notice = any(word in path.name.lower() for word in ("license", "licence", "copying", "copyright", "notice"))
            is_font_notice = "fonts" in path.relative_to(source).parts and path.suffix.lower() == ".txt"
            if path.is_file() and not path.is_symlink() and (is_notice or is_font_notice):
                relative = path.relative_to(source)
                output = destination / "third-party" / f"{package['name']}-{package['version']}" / relative
                output.parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(path, output)
                texts.append(str(output.relative_to(destination)))
        upstream = []
        for entry in bundled:
            if f"{package['name']}@{package['version']}" not in entry["packages"]:
                continue
            revision = json.loads((source / ".cargo_vcs_info.json").read_text())["git"]["sha1"]
            if revision != entry["revision"]:
                raise ValueError(f"Notice revision mismatch for {package['name']}")
            for notice in entry["files"]:
                path = ROOT / "packaging/third-party" / notice["path"]
                if hashlib.sha256(path.read_bytes()).hexdigest() != notice["sha256"]:
                    raise ValueError(f"Notice checksum mismatch: {path}")
                output = destination / "third-party" / f"{package['name']}-{package['version']}" / "upstream" / path.name
                output.parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(path, output)
                texts.append(str(output.relative_to(destination)))
                upstream.append(notice)
        source_bundle = None
        if "MPL-2.0" in (package.get("license") or ""):
            output = destination / "third-party-sources" / f"{package['name']}-{package['version']}"
            shutil.copytree(source, output)
            source_bundle = str(output.relative_to(destination))
        inventory.append({
            "name": package["name"], "version": package["version"],
            "license": package.get("license"), "repository": package.get("repository"),
            "source": package.get("source"), "notices": sorted(texts),
            "upstream_notices": upstream, "included_source": source_bundle,
        })
    report = {
        "target": target, "review_required": True,
        "cargo_lock_sha256": hashlib.sha256((ROOT / "Cargo.lock").read_bytes()).hexdigest(),
        "note": "Native normal/build dependency inventory, including available and revision-verified upstream notices. Review license obligations before public binary distribution.",
        "packages": inventory,
        "packages_without_notice_files": [p["name"] + "-" + p["version"] for p in inventory if not p["notices"]],
    }
    destination.mkdir(parents=True, exist_ok=True)
    (destination / "FONT-COPYRIGHTS.txt").write_text("\n".join(font_notices) + "\n")
    (destination / "THIRD_PARTY.json").write_text(json.dumps(report, indent=2, ensure_ascii=False) + "\n")
    lines = [report["note"], "", f"Target: {target}", ""]
    lines.extend(f"{p['name']} {p['version']}: {p['license'] or 'MISSING LICENSE DECLARATION'}" for p in inventory)
    lines += ["", "Unmodified MPL-covered source is included alongside these notices:"]
    lines.extend(f"{p['name']} {p['version']}: {p['included_source']}" for p in inventory if p['included_source'])
    (destination / "THIRD_PARTY_NOTICES.txt").write_text("\n".join(lines) + "\n")
    return report


def stage(destination, prefix, binary):
    if not binary.is_file():
        raise ValueError(f"Build the binary first: cargo build --locked --release ({binary})")
    if destination.exists() and any(destination.iterdir()):
        raise ValueError(f"Staging destination must be empty: {destination}")
    if not prefix.startswith("/") or ".." in Path(prefix).parts:
        raise ValueError("Prefix must be an absolute installation path without '..'")
    install = destination / prefix.lstrip("/")
    bindir = install / "bin"
    bindir.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(binary, bindir / NAME)
    (bindir / NAME).chmod(0o755)
    render_metadata(install)
    icon = install / f"share/icons/hicolor/256x256/apps/{APP_ID}.png"
    icon.parent.mkdir(parents=True, exist_ok=True)
    run([binary.resolve(), "--export-icon", icon.resolve()])
    service = install / "share" / NAME / "org.freedesktop.FileManager1.service.in"
    service.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(ROOT / "packaging/file-manager.service.in", service)
    doc = install / "share/doc" / NAME
    doc.mkdir(parents=True, exist_ok=True)
    for source in ["LICENSE", "README.md", "CHANGELOG.md", "docs/RELEASING.md", "docs/DISTRIBUTION.md", "README.en.md", "docs/THIRD_PARTY_REVIEW.md"]:
        shutil.copyfile(ROOT / source, doc / Path(source).name)
    release_notes = ROOT / "docs/releases" / f"v{VERSION}.md"
    if release_notes.is_file():
        shutil.copyfile(release_notes, doc / "INSTALL.md")
    shutil.copyfile(ROOT / "packaging/preview-runtime.json", doc / "preview-runtime.json")
    notices = install / "share/licenses" / APP_ID
    notices.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(ROOT / "LICENSE", notices / "LICENSE")
    shutil.copyfile(ROOT / "assets/fontawesome/LICENSE.txt", notices / "FontAwesome-Free-LICENSE.txt")
    shutil.copyfile(ROOT / "assets/fontawesome/README.md", notices / "FontAwesome-ATTRIBUTION.md")
    shutil.copyfile(ROOT / "docs/THIRD_PARTY_REVIEW.md", notices / "DISTRIBUTION-REVIEW.md")
    report = third_party(notices, host_target())
    validate(install)
    run([bindir / NAME, "--version"])
    return install, report


def debian_dependencies(binary, work):
    # Derive glibc/libgcc requirements from this actual binary, rather than guessing a baseline.
    control = work / "debian/control"
    control.parent.mkdir(parents=True)
    control.write_text(f"Source: {NAME}\nSection: utils\nPriority: optional\nMaintainer: {PACKAGE['metadata']['distribution']['maintainer']}\n\nPackage: {NAME}\nArchitecture: any\nDescription: Linux file manager\n")
    result = run(["dpkg-shlibdeps", "-O", "-e", binary], cwd=work, capture_output=True, text=True)
    depends = next(line.removeprefix("shlibs:Depends=") for line in result.stdout.splitlines() if line.startswith("shlibs:Depends="))
    # These libraries are opened dynamically; GIO and xdg-utils provide desktop integration.
    preview = json.loads((ROOT / "packaging/preview-runtime.json").read_text())
    engines = [package for engine in preview.values() if isinstance(engine, dict)
               for package in engine.get("debian", [])]
    return depends + ", libx11-6, libxi6, libxcursor1, libxrandr2, libxkbcommon0, libxkbcommon-x11-0, libwayland-client0, libegl1, libgl1, libglib2.0-bin, xdg-utils, shared-mime-info, hicolor-icon-theme, " + ", ".join(engines)


def build(output, binary, formats):
    output.mkdir(parents=True, exist_ok=True)
    target = host_target()
    if "linux" not in target:
        raise ValueError("Only native Linux packages are supported")
    artifacts = []
    report = None
    with tempfile.TemporaryDirectory(prefix="lse-package-") as temporary:
        temporary = Path(temporary)
        if "tar" in formats:
            root = temporary / "archive"
            _, report = stage(root, "/", binary)
            artifact = output / f"{NAME}-{VERSION}-{target}.tar.gz"
            with tarfile.open(artifact, "w:gz") as archive:
                archive.add(root, arcname=f"{NAME}-{VERSION}")
            artifacts.append(artifact)
        if "deb" in formats:
            root = temporary / "deb"
            _, report = stage(root, "/usr", binary)
            architecture = run(["dpkg", "--print-architecture"], capture_output=True, text=True).stdout.strip()
            depends = debian_dependencies(binary, temporary / "shlibs")
            debian = root / "DEBIAN"
            debian.mkdir()
            size = sum(p.stat().st_size for p in root.rglob("*") if p.is_file()) // 1024 + 1
            (debian / "control").write_text(
                f"Package: {NAME}\nVersion: {debian_version(VERSION)}\nArchitecture: {architecture}\n"
                f"Section: utils\nPriority: optional\nInstalled-Size: {size}\n"
                f"Maintainer: {PACKAGE['metadata']['distribution']['maintainer']}\n"
                f"Homepage: {PACKAGE['homepage']}\nDepends: {depends}\n"
                "Description: Experimental file manager for Linux\n"
                " Configurable views, per-folder layouts, file operations and Trash support.\n"
                " Experimental build; not yet approved for store distribution.\n"
            )
            artifact = output / f"{NAME}_{debian_version(VERSION).replace('~', '.')}_{architecture}.deb"
            run(["dpkg-deb", "--root-owner-group", "--build", root, artifact])
            artifacts.append(artifact)
    revision = subprocess.run(["git", "rev-parse", "HEAD"], cwd=ROOT, capture_output=True, text=True)
    status = subprocess.run(["git", "status", "--porcelain"], cwd=ROOT, capture_output=True, text=True)
    manifest = {
        "name": NAME, "version": VERSION, "application_id": APP_ID, "target": target,
        "channel": PACKAGE["metadata"]["distribution"]["channel"],
        "build_libc": platform.libc_ver(),
        "rustc": run([rust_tool("rustc"), "--version"], capture_output=True, text=True).stdout.strip(),
        "cargo_lock_sha256": hashlib.sha256((ROOT / "Cargo.lock").read_bytes()).hexdigest(),
        "license_review_required": report["review_required"],
        "source_revision": revision.stdout.strip() if revision.returncode == 0 else None,
        "source_has_uncommitted_changes": bool(status.stdout) if status.returncode == 0 else None,
        "packages_without_notice_files": report["packages_without_notice_files"],
        "artifacts": {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in artifacts},
    }
    (output / "BUILD-INFO.json").write_text(json.dumps(manifest, indent=2) + "\n")
    with (output / "SHA256SUMS").open("w") as checksums:
        for path in [*artifacts, output / "BUILD-INFO.json"]:
            checksums.write(f"{hashlib.sha256(path.read_bytes()).hexdigest()}  {path.name}\n")
    print(f"Experimental packages: {output}; third-party license review remains required.")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=["check", "stage", "build"])
    parser.add_argument("--binary", type=Path, default=ROOT / "target/release" / NAME)
    parser.add_argument("--destdir", type=Path)
    parser.add_argument("--prefix", default="/usr")
    parser.add_argument("--output", type=Path, default=ROOT / "dist")
    parser.add_argument("--formats", nargs="+", choices=["tar", "deb"], default=["tar", "deb"])
    args = parser.parse_args()
    if args.action == "check":
        with tempfile.TemporaryDirectory(prefix="lse-metadata-") as temporary:
            root = Path(temporary)
            render_metadata(root)
            validate(root)
    elif args.action == "stage":
        if args.destdir is None:
            parser.error("stage requires --destdir pointing to an empty directory")
        stage(args.destdir.resolve(), args.prefix, args.binary.resolve())
    else:
        build(args.output.resolve(), args.binary.resolve(), args.formats)


if __name__ == "__main__":
    try:
        main()
    except (ValueError, RuntimeError, subprocess.CalledProcessError) as error:
        raise SystemExit(str(error)) from error
