#!/usr/bin/env python3
"""Read-only runtime capability probe; never launches a host application."""
import ctypes
import ctypes.util
import json
import shutil
import subprocess


def library(name):
    try:
        ctypes.CDLL(name)
        return True
    except OSError:
        return False


def version(program, arguments):
    executable = shutil.which(program)
    if not executable or not executable.startswith('/app/bin/'):
        return None
    try:
        result = subprocess.run([executable, *arguments], text=True, capture_output=True,
                                timeout=20, check=True)
        return (result.stdout + result.stderr).strip().splitlines()[0]
    except (OSError, subprocess.SubprocessError, IndexError):
        return None


tools = {name: version(name, ['--version'] if name == 'libreoffice' else ['-v'])
         for name in ['pdfinfo', 'pdftoppm', 'pdfunite', 'libreoffice']}
try:
    import weasyprint
    html = weasyprint.__file__.startswith('/app/lib/')
    html_version = weasyprint.__version__ if html else None
except (ImportError, OSError):
    html, html_version = False, None


print(json.dumps({
    'gio': bool(shutil.which('gio')),
    'gstreamer': all(library(name) for name in [
        'libgstreamer-1.0.so.0', 'libgstapp-1.0.so.0', 'libgobject-2.0.so.0', 'libglib-2.0.so.0']),
    'pdf': all(tools[name] for name in ['pdfinfo', 'pdftoppm', 'pdfunite']),
    'office': bool(tools['libreoffice']),
    'html_markdown_presentation': html,
    'converter_versions': tools | {'weasyprint': html_version},
    'archive': library('libarchive.so.13'),
    'shell': bool(shutil.which('sh')),
}, indent=2))
