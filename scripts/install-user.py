#!/usr/bin/env python3
"""Install for the current user; optionally change the folder handler, with a backup."""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import shutil
import shlex
import subprocess
import tempfile
import time

ROOT = Path(__file__).resolve().parent.parent
SPEC = importlib.util.spec_from_file_location("lse_package", ROOT / "scripts/package.py")
PACKAGE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(PACKAGE)
APP_ID = PACKAGE.APP_ID
DESKTOP_ID = APP_ID + ".desktop"


def run(args, **kwargs):
    return subprocess.run([str(a) for a in args], check=True, **kwargs)


def atomic_copy(source, destination):
    destination.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile(dir=destination.parent, delete=False) as file:
        temporary = Path(file.name)
    try:
        shutil.copyfile(source, temporary)
        temporary.chmod(source.stat().st_mode & 0o777)
        os.replace(temporary, destination)
    finally:
        temporary.unlink(missing_ok=True)


def write_state(path, state):
    path.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile(mode="w", dir=path.parent, delete=False) as file:
        temporary = Path(file.name)
        json.dump(state, file, indent=2)
        file.write("\n")
    os.replace(temporary, path)


def default_handler():
    if shutil.which("gio"):
        result = run(["gio", "mime", "inode/directory"], capture_output=True, text=True,
                     env=os.environ | {"LC_ALL": "C"}).stdout.splitlines()
        if result and result[0].startswith("Default application for "):
            return result[0].rsplit(": ", 1)[-1].strip()
    return run(["xdg-mime", "query", "default", "inode/directory"], capture_output=True, text=True).stdout.strip()


def set_handler(desktop):
    # Older xdg-utils invokes Qt 5's `qtpaths` on KDE 6, where only qtpaths6 exists.
    # GIO writes the standard MIME association without that legacy dependency.
    if shutil.which("gio"):
        run(["gio", "mime", "inode/directory", desktop])
    else:
        run(["xdg-mime", "default", desktop, "inode/directory"])
    data = Path(os.environ.get("XDG_DATA_HOME", Path.home() / ".local/share"))
    refresh(data)
    if default_handler() != desktop:
        raise RuntimeError("The desktop did not confirm the requested folder handler")


def desktop_value(value):
    return value.replace("\\", "\\\\").replace("\n", "\\n").replace("\r", "\\r")


def launcher_text(template, binary):
    # xdg-utils 1.2.x does not parse quoted Exec paths when querying defaults.
    # Simple absolute paths need no quotes; special paths still follow the desktop spec.
    executable = str(binary)
    if not re.fullmatch(r"[A-Za-z0-9_./-]+", executable):
        executable = '"' + "".join("\\" + c if c in '\\"`$' else "%%" if c == "%" else c for c in executable) + '"'
    return "\n".join(
        "Exec=" + desktop_value(executable) + " %f" if line.startswith("Exec=") else
        "TryExec=" + desktop_value(str(binary)) if line.startswith("TryExec=") else line
        for line in template.splitlines()
    ) + "\n"


def refresh(data):
    if shutil.which("update-desktop-database"):
        run(["update-desktop-database", data / "applications"])
    if shutil.which("kbuildsycoca6"):
        run(["kbuildsycoca6", "--noincremental"], stdout=subprocess.DEVNULL)
    if shutil.which("gtk-update-icon-cache") and (data / "icons/hicolor").is_dir():
        run(["gtk-update-icon-cache", "--ignore-theme-index", "--force", data / "icons/hicolor"])
    if shutil.which("dbus-send"):
        # KIconLoader clears its cached images when this KDE notification arrives.
        subprocess.run(["dbus-send", "--session", "/KIconLoader",
                        "org.kde.KIconLoader.iconChanged", "int32:0"], check=False)


SERVICE = "org.freedesktop.FileManager1.service"
MANAGED = "X-LinuxSystemExplorer-Managed=true"
CONTROL = "io.github.janderpt.LinuxSystemExplorer.FileManagerControl"


def reload_bus():
    run(["gdbus", "call", "--session", "--dest", "org.freedesktop.DBus",
         "--object-path", "/org/freedesktop/DBus", "--method", "org.freedesktop.DBus.ReloadConfig"],
        stdout=subprocess.DEVNULL)


def stop_own_bridge():
    # NameHasOwner does not auto-activate Dolphin or another manager.
    owned = run(["gdbus", "call", "--session", "--dest", "org.freedesktop.DBus",
                 "--object-path", "/org/freedesktop/DBus", "--method",
                 "org.freedesktop.DBus.NameHasOwner", "org.freedesktop.FileManager1"],
                capture_output=True, text=True).stdout
    if "true" in owned:
        # An unrelated owner has no private control interface, so it remains untouched.
        subprocess.run(["gdbus", "call", "--session", "--dest", "org.freedesktop.FileManager1",
                        "--object-path", "/org/freedesktop/FileManager1", "--method", CONTROL + ".Quit"],
                       stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, check=False)


def activate_integration(data, binary, backup, old_state):
    path = data / "dbus-1/services" / SERVICE
    old = old_state.get("file_manager_activation")
    if old and path.is_file() and path.read_text() == old["installed_text"]:
        previous = old["previous_file"]
    else:
        previous = None
        if path.exists():
            original = backup / SERVICE
            shutil.copy2(path, original)
            previous = str(original)
    template = (ROOT / "packaging/file-manager.service.in").read_text()
    # D-Bus uses shell-style argument parsing, without invoking a shell.
    text = template.replace("@EXECUTABLE@", shlex.quote(str(binary)))
    path.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile(mode="w", dir=path.parent, delete=False) as file:
        file.write(text)
        temporary = Path(file.name)
    temporary.chmod(0o644)
    os.replace(temporary, path)
    return {"path": str(path), "previous_file": previous, "installed_text": text}


def restore_integration(state):
    activation = state.get("file_manager_activation")
    if not activation:
        return
    path = Path(activation["path"])
    if not path.is_file() or path.read_text() != activation["installed_text"]:
        raise RuntimeError("The FileManager1 service has changed; leaving that newer choice untouched")
    stop_own_bridge()
    if activation["previous_file"]:
        atomic_copy(Path(activation["previous_file"]), path)
    else:
        path.unlink()
    reload_bus()


def install(binary, make_default, data, config, state_path):
    binary = binary.resolve()
    bindir = Path.home() / ".local/bin"
    installed_binary = bindir / PACKAGE.NAME
    old_state = json.loads(state_path.read_text()) if state_path.exists() else {}
    previous = default_handler()
    if previous == DESKTOP_ID:
        previous = old_state.get("previous_folder_handler", "")
    if make_default and not previous:
        raise RuntimeError("No previous folder handler was found; choose one before automatic replacement")
    backup = state_path.parent / "backups" / str(time.time_ns())
    backup.mkdir(parents=True, mode=0o700)
    # Preserve the full association file for inspection, but restore only inode/directory later.
    for name in ["mimeapps.list", "kde-mimeapps.list"]:
        path = config / name
        if path.is_file():
            shutil.copy2(path, backup / name)
    with tempfile.TemporaryDirectory(prefix="lse-user-install-") as directory:
        stage = Path(directory)
        prefix, _ = PACKAGE.stage(stage, "/usr", binary)
        launcher = prefix / "share/applications" / DESKTOP_ID
        launcher.write_text(launcher_text(launcher.read_text(), installed_binary))
        run(["desktop-file-validate", launcher])
        files = []
        for source in sorted(prefix.rglob("*")):
            if not source.is_file():
                continue
            relative = source.relative_to(prefix)
            destination = bindir / relative.name if relative.parts[0] == "bin" else data / Path(*relative.parts[1:])
            if destination.exists():
                original = backup / relative
                original.parent.mkdir(parents=True, exist_ok=True)
                shutil.copy2(destination, original)
            atomic_copy(source, destination)
            files.append({"path": str(destination), "sha256": hashlib.sha256(destination.read_bytes()).hexdigest()})
    revision = run(["git", "rev-parse", "HEAD"], cwd=ROOT, capture_output=True, text=True).stdout.strip()
    state = {
        "application_id": APP_ID, "version": PACKAGE.VERSION, "source_revision": revision,
        "source_repository": PACKAGE.PACKAGE["repository"],
        "previous_folder_handler": previous, "backup": str(backup), "files": files,
    }
    if "file_manager_activation" in old_state:
        state["file_manager_activation"] = old_state["file_manager_activation"]
    write_state(state_path, state)
    refresh(data)
    run([installed_binary, "--version"])
    if make_default:
        try:
            state["file_manager_activation"] = activate_integration(data, installed_binary, backup, old_state)
            write_state(state_path, state)
            reload_bus()
            set_handler(DESKTOP_ID)
            stop_own_bridge()
        except (RuntimeError, subprocess.CalledProcessError):
            restore_integration(state)
            state.pop("file_manager_activation", None)
            write_state(state_path, state)
            set_handler(previous)
            raise
    print("Installed:", installed_binary)
    print("Folder handler:", default_handler())
    print("Installation state and rollback information:", state_path)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=["install", "restore-default", "status"])
    parser.add_argument("--binary", type=Path, default=ROOT / "target/release" / PACKAGE.NAME)
    parser.add_argument("--set-default", action="store_true")
    args = parser.parse_args()
    data = Path(os.environ.get("XDG_DATA_HOME", Path.home() / ".local/share"))
    config = Path(os.environ.get("XDG_CONFIG_HOME", Path.home() / ".config"))
    state_path = Path(os.environ.get("XDG_STATE_HOME", Path.home() / ".local/state")) / PACKAGE.NAME / "install-state.json"
    if args.action == "status":
        print("Folder handler:", default_handler())
        print(state_path.read_text() if state_path.exists() else "No managed user installation")
    elif args.action == "restore-default":
        state = json.loads(state_path.read_text())
        previous = state.get("previous_folder_handler")
        if not previous:
            raise RuntimeError("No previous folder handler is recorded")
        if default_handler() != DESKTOP_ID:
            raise RuntimeError("The current folder handler has changed; leaving that newer choice untouched")
        restore_integration(state)
        state.pop("file_manager_activation", None)
        write_state(state_path, state)
        set_handler(previous)
        refresh(data)
        print("Restored folder handler:", previous)
    else:
        if os.geteuid() == 0:
            raise RuntimeError("Run the per-user installer without sudo")
        install(args.binary, args.set_default, data, config, state_path)


if __name__ == "__main__":
    try:
        main()
    except (ValueError, RuntimeError, subprocess.CalledProcessError) as error:
        raise SystemExit(str(error)) from error
