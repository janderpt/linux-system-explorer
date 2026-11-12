"""Generate this project's legacy Microsoft and OpenDocument fixtures from authored sources."""
from pathlib import Path
import argparse
import shutil
import subprocess
import tempfile

root = Path(__file__).resolve().parent
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--force", action="store_true", help="Replace existing fixtures")
args = parser.parse_args()
program = shutil.which("libreoffice") or shutil.which("soffice")
if not program:
    raise SystemExit("Install LibreOffice Calc and Impress to regenerate these fixtures")
for source, extension in [
    ("two-pages.docx", "doc"), ("two-pages.docx", "odt"),
    ("two-sheets.fods", "xlsx"), ("two-sheets.fods", "xls"), ("two-sheets.fods", "ods"),
    ("two-slides.fodp", "pptx"), ("two-slides.fodp", "ppt"), ("two-slides.fodp", "odp"),
    ("two-drawings.fodg", "odg"), ("formula.mml", "odf"),
]:
    destination = root.joinpath(source).with_suffix("." + extension)
    if destination.exists() and not args.force:
        continue
    with tempfile.TemporaryDirectory(prefix="lse-fixture-") as temporary:
        profile = (Path(temporary) / "profile").as_uri()
        subprocess.run([program, f"-env:UserInstallation={profile}", "--headless", "--norestore",
                        "--convert-to", extension, "--outdir", str(root), str(root / source)],
                       check=True, timeout=30)
    if not destination.is_file():
        raise SystemExit(f"Failed to generate {extension}; check the installed LibreOffice modules")
