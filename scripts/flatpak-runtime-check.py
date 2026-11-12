#!/usr/bin/env python3
"""Read-only runtime capability probe; never launches a host application."""
import ctypes
import ctypes.util
import importlib.util
import json
import shutil


def library(name):
    try:
        ctypes.CDLL(name)
        return True
    except OSError:
        return False


print(json.dumps({
    'gio': bool(shutil.which('gio')),
    'gstreamer': all(library(name) for name in [
        'libgstreamer-1.0.so.0', 'libgstapp-1.0.so.0', 'libgobject-2.0.so.0', 'libglib-2.0.so.0']),
    'pdf': all(shutil.which(name) for name in ['pdfinfo', 'pdftoppm']),
    'office': any(shutil.which(name) for name in ['libreoffice', 'soffice']),
    'html_markdown_presentation': importlib.util.find_spec('weasyprint') is not None,
    'archive': library('libarchive.so.13'),
    'shell': bool(shutil.which('sh')),
}, indent=2))
