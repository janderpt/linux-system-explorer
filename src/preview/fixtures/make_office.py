"""Generate this project's XLSX/PPTX fixtures from the adjacent authored Flat ODF sources."""
from pathlib import Path
import shutil
import subprocess
import tempfile

root = Path(__file__).resolve().parent
program = shutil.which("libreoffice") or shutil.which("soffice")
if not program:
    raise SystemExit("Install LibreOffice Calc and Impress to regenerate these fixtures")
for source, extension in [("two-sheets.fods", "xlsx"), ("two-slides.fodp", "pptx")]:
    with tempfile.TemporaryDirectory(prefix="lse-fixture-") as temporary:
        profile = (Path(temporary) / "profile").as_uri()
        subprocess.run([program, f"-env:UserInstallation={profile}", "--headless", "--norestore",
                        "--convert-to", extension, "--outdir", str(root), str(root / source)],
                       check=True, timeout=30)
    if not root.joinpath(source).with_suffix("." + extension).is_file():
        raise SystemExit(f"Failed to generate {extension}; check the installed LibreOffice modules")
