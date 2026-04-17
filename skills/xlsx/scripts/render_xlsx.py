#!/usr/bin/env python3
"""Render an XLSX workbook to PDF and optionally PNGs via LibreOffice."""

from __future__ import annotations

import argparse
import glob
import shutil
import subprocess
import tempfile
from pathlib import Path

SOFFICE_CANDIDATES = [
    "/Applications/LibreOffice.app/Contents/MacOS/soffice",
    "soffice",
    "/usr/bin/soffice",
    "/snap/bin/libreoffice",
]
SOFFICE_GLOBS = [
    "/opt/homebrew/Caskroom/libreoffice/*/LibreOffice.app/Contents/MacOS/soffice",
]


def command_runs(candidate: str) -> bool:
    try:
        proc = subprocess.run(
            [candidate, "--version"],
            text=True,
            capture_output=True,
            timeout=10,
        )
    except Exception:
        return False
    return proc.returncode == 0


def iter_soffice_candidates() -> list[str]:
    values: list[str] = []
    seen: set[str] = set()
    for candidate in SOFFICE_CANDIDATES:
        resolved = shutil.which(candidate) if "/" not in candidate else candidate
        actual = resolved or candidate
        if actual not in seen:
            seen.add(actual)
            values.append(actual)
    for pattern in SOFFICE_GLOBS:
        for match in sorted(glob.glob(pattern), reverse=True):
            if match not in seen:
                seen.add(match)
                values.append(match)
    return values


def find_soffice() -> str:
    for candidate in iter_soffice_candidates():
        if command_runs(candidate):
            return candidate
    raise SystemExit(
        "LibreOffice soffice not found or not runnable. Install/fix libreoffice first."
    )


def require_binary(name: str) -> str:
    resolved = shutil.which(name)
    if not resolved:
        raise SystemExit(f"Required binary not found: {name}")
    return resolved


def render_pdf(workbook: Path, outdir: Path) -> Path:
    outdir.mkdir(parents=True, exist_ok=True)
    soffice = find_soffice()
    with tempfile.TemporaryDirectory(prefix="xlsx-render-") as tmp:
        tmpdir = Path(tmp)
        tmp_input = tmpdir / workbook.name
        shutil.copy2(workbook, tmp_input)
        proc = subprocess.run(
            [
                soffice,
                "--headless",
                "--convert-to",
                "pdf",
                "--outdir",
                str(outdir),
                str(tmp_input),
            ],
            text=True,
            capture_output=True,
        )
        if proc.returncode != 0:
            raise SystemExit(proc.stderr.strip() or proc.stdout.strip() or "LibreOffice conversion failed")
    pdf = outdir / f"{workbook.stem}.pdf"
    if not pdf.exists():
        matches = sorted(outdir.glob("*.pdf"))
        if len(matches) == 1:
            return matches[0]
        raise SystemExit("PDF output not found after conversion")
    return pdf


def render_pngs(pdf: Path, outdir: Path, dpi: int) -> None:
    pdftoppm = require_binary("pdftoppm")
    prefix = outdir / pdf.stem
    subprocess.run(
        [pdftoppm, "-png", "-r", str(dpi), str(pdf), str(prefix)],
        check=True,
        text=True,
        capture_output=True,
    )


def main() -> int:
    parser = argparse.ArgumentParser(description="Render XLSX workbook to PDF/PNGs")
    parser.add_argument("workbook", type=Path, help="Path to workbook")
    parser.add_argument("--outdir", type=Path, default=Path("rendered"), help="Output directory")
    parser.add_argument("--png", action="store_true", help="Also export PNG pages from rendered PDF")
    parser.add_argument("--dpi", type=int, default=144, help="PNG render DPI when --png is set")
    args = parser.parse_args()

    workbook = args.workbook.resolve()
    if not workbook.is_file():
        raise SystemExit(f"Workbook not found: {workbook}")

    outdir = args.outdir.resolve()
    pdf = render_pdf(workbook, outdir)
    print(f"PDF: {pdf}")

    if args.png:
        render_pngs(pdf, outdir, args.dpi)
        print(f"PNG prefix: {outdir / pdf.stem}-*.png")

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
