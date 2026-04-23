#!/usr/bin/env python3
from __future__ import annotations

import argparse
import re
import shutil
import subprocess
import sys
import tempfile
import zipfile
from pathlib import Path

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

from rust_bridge import build_command


NOTES_MASTER_RE = re.compile(r"(?s)<p:notesMasterIdLst>.*?</p:notesMasterIdLst>")
NOTES_SZ_RE = re.compile(r"<p:notesSz\b[^>]*/>")
SLD_SZ_RE = re.compile(r"<p:sldSz\b[^>]*/>")


def sanitize_presentation_xml(xml: str) -> str:
    match = NOTES_MASTER_RE.search(xml)
    if not match:
        return xml
    notes_master = match.group(0)
    without_notes_master = NOTES_MASTER_RE.sub("", xml, count=1)
    insert_after = NOTES_SZ_RE.search(without_notes_master) or SLD_SZ_RE.search(without_notes_master)
    if not insert_after:
        return without_notes_master
    insert_at = insert_after.end()
    return (
        without_notes_master[:insert_at]
        + notes_master
        + without_notes_master[insert_at:]
    )


def sanitize_with_python(input_path: Path, output_path: Path) -> int:
    temp_output = output_path
    replace_in_place = input_path.resolve() == output_path.resolve()
    if replace_in_place:
        fd, temp_name = tempfile.mkstemp(prefix="pptx_sanitize_", suffix=".pptx")
        Path(temp_name).unlink(missing_ok=True)
        temp_output = Path(temp_name)

    with zipfile.ZipFile(input_path, "r") as src, zipfile.ZipFile(temp_output, "w") as dst:
        for info in src.infolist():
            data = src.read(info.filename)
            if info.filename == "ppt/presentation.xml":
                data = sanitize_presentation_xml(data.decode("utf-8")).encode("utf-8")
            new_info = zipfile.ZipInfo(info.filename)
            new_info.date_time = info.date_time
            new_info.compress_type = info.compress_type
            new_info.comment = info.comment
            new_info.extra = info.extra
            new_info.create_system = info.create_system
            new_info.external_attr = info.external_attr
            new_info.internal_attr = info.internal_attr
            new_info.flag_bits = info.flag_bits
            dst.writestr(new_info, data)

    if replace_in_place:
        shutil.move(str(temp_output), str(output_path))
    return 0


def sanitize_with_rust(argv: list[str]) -> int | None:
    try:
        command = build_command("sanitize-pptx", argv)
    except SystemExit:
        return None
    completed = subprocess.run(command)
    if completed.returncode == 0:
        return 0
    return None


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Sanitize generated PPTX OOXML for OfficeCLI/schema compatibility.")
    parser.add_argument("input_path")
    parser.add_argument("-o", "--output")
    return parser


def main(argv: list[str] | None = None) -> int:
    args = list(sys.argv[1:] if argv is None else argv)
    rust_result = sanitize_with_rust(args)
    if rust_result is not None:
        return rust_result

    parser = build_parser()
    parsed = parser.parse_args(args)
    input_path = Path(parsed.input_path).expanduser().resolve()
    output_path = Path(parsed.output).expanduser().resolve() if parsed.output else input_path
    return sanitize_with_python(input_path, output_path)


if __name__ == "__main__":
    raise SystemExit(main())
