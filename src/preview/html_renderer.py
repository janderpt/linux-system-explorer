"""Isolated static HTML renderer embedded in the Rust executable."""
from pathlib import Path
import os
import sys
from urllib.parse import unquote_to_bytes, urlsplit

try:
    from weasyprint import HTML, default_url_fetcher
except ImportError:
    print("LSE_HTML_RUNTIME_MISSING", file=sys.stderr)
    raise SystemExit(78)

snapshot, original, output = map(Path, sys.argv[1:4])
root = original.resolve().parent


def local_resource(url, *args, **kwargs):
    parsed = urlsplit(url)
    if parsed.scheme == "data":
        return default_url_fetcher(url, timeout=3)
    if parsed.scheme == "file" and parsed.netloc in ("", "localhost"):
        target = Path(os.fsdecode(unquote_to_bytes(parsed.path))).resolve()
        if root in target.parents and target.is_file() and target.stat().st_size <= 16 * 1024 * 1024:
            return default_url_fetcher(url, timeout=3)
    raise ValueError("Preview only loads embedded resources and files within the HTML directory")


# WeasyPrint renders HTML/CSS without a JavaScript engine. The fetcher prevents
# network requests and local references outside this document's directory.
with snapshot.open("rb") as source:
    HTML(file_obj=source, base_url=original.resolve().as_uri(),
         url_fetcher=local_resource, media_type="screen").write_pdf(str(output))
