"""Read archive headers only, in an isolated, bounded worker. Never extract files."""
import ctypes as C
import json
import os
from pathlib import Path
import resource
import sys
import time

MAX_ENTRIES = 5000
MAX_NAMES = 2 * 1024 * 1024
MAX_SECONDS = 10


def inspect(source, output):
    # Native parsers run outside the GUI and cannot grow memory/output indefinitely.
    resource.setrlimit(resource.RLIMIT_AS, (512 * 1024 * 1024,) * 2)
    resource.setrlimit(resource.RLIMIT_CPU, (15, 15))
    resource.setrlimit(resource.RLIMIT_FSIZE, (8 * 1024 * 1024,) * 2)
    try:
        lib = C.CDLL("libarchive.so.13")
    except OSError:
        raise RuntimeError("LSE_ARCHIVE_RUNTIME_MISSING") from None

    def function(name, result, *args):
        fn = getattr(lib, name)
        fn.restype = result
        fn.argtypes = args
        return fn

    pointer = C.c_void_p
    new = function("archive_read_new", pointer)
    free = function("archive_read_free", C.c_int, pointer)
    filters = function("archive_read_support_filter_all", C.c_int, pointer)
    formats = function("archive_read_support_format_all", C.c_int, pointer)
    raw = function("archive_read_support_format_raw", C.c_int, pointer)
    open_file = function("archive_read_open_filename", C.c_int, pointer, C.c_char_p, C.c_size_t)
    next_header = function("archive_read_next_header", C.c_int, pointer, C.POINTER(pointer))
    name = function("archive_entry_pathname_utf8", C.c_char_p, pointer)
    fallback_name = function("archive_entry_pathname", C.c_char_p, pointer)
    size = function("archive_entry_size", C.c_int64, pointer)
    has_size = function("archive_entry_size_is_set", C.c_int, pointer)
    kind = function("archive_entry_filetype", C.c_uint, pointer)
    symlink = function("archive_entry_symlink", C.c_char_p, pointer)
    hardlink = function("archive_entry_hardlink", C.c_char_p, pointer)
    filter_count = function("archive_filter_count", C.c_int, pointer)
    archive_format = function("archive_format", C.c_int, pointer)
    reader = new()
    if not reader:
        raise RuntimeError("LSE_ARCHIVE_READ_FAILED")
    entries = []
    truncated = False
    names_size = 0
    deadline = time.monotonic() + MAX_SECONDS
    try:
        filters(reader)
        formats(reader)
        if Path(source).suffix.lower() in (".gz", ".bz2", ".xz", ".zst", ".lz4", ".lzma", ".lz"):
            raw(reader)
        if open_file(reader, os.fsencode(source), 64 * 1024) < -20:
            raise RuntimeError("LSE_ARCHIVE_READ_FAILED")
        entry = pointer()
        while True:
            if len(entries) >= MAX_ENTRIES or names_size >= MAX_NAMES or time.monotonic() >= deadline:
                truncated = True
                break
            status = next_header(reader, C.byref(entry))
            if status == 1:  # ARCHIVE_EOF
                break
            if status not in (0, -20) or not entry:  # OK or WARN with a usable header
                raise RuntimeError("LSE_ARCHIVE_READ_FAILED")
            path = (name(entry) or fallback_name(entry) or b"?").decode("utf-8", "replace")
            if archive_format(reader) & 0xFF0000 == 0x90000:  # raw compressed stream
                if filter_count(reader) <= 1:
                    raise RuntimeError("LSE_ARCHIVE_READ_FAILED")
                path = Path(source).stem
            path = "".join(char if char.isprintable() else "\ufffd" for char in path)
            if len(path.encode("utf-8")) > 4096:
                path = path[:1024] + "\u2026"
                truncated = True
            names_size += len(path.encode("utf-8"))
            filetype = kind(entry)
            entries.append({
                "path": path,
                "size": max(0, size(entry)) if has_size(entry) else None,
                "directory": filetype == 0o040000,
                "link": bool(symlink(entry) or hardlink(entry)),
            })
        Path(output).write_text(json.dumps({"entries": entries, "truncated": truncated}, ensure_ascii=False), encoding="utf-8")
    finally:
        free(reader)


if __name__ == "__main__":
    try:
        inspect(sys.argv[1], sys.argv[2])
    except Exception as error:
        # No parser tracebacks or archive-controlled strings in notifications.
        code = str(error)
        sys.stderr.write(code if code == "LSE_ARCHIVE_RUNTIME_MISSING" else "LSE_ARCHIVE_READ_FAILED")
        sys.exit(1)
