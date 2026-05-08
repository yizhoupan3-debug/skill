#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage: compile_standalone.sh <figure.tex> [--engine xelatex|pdflatex|lualatex] [--out-dir DIR] [--dpi 300]

Compiles a TikZ standalone figure and, when pdftoppm is available, creates a PNG preview.
USAGE
}

if [[ $# -lt 1 ]]; then
  usage
  exit 2
fi

tex_file=$1
shift
engine=xelatex
out_dir=
dpi=300

while [[ $# -gt 0 ]]; do
  case "$1" in
    --engine)
      engine=${2:?missing engine}
      shift 2
      ;;
    --out-dir)
      out_dir=${2:?missing output directory}
      shift 2
      ;;
    --dpi)
      dpi=${2:?missing dpi}
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown option: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

if [[ ! -f "$tex_file" ]]; then
  echo "Missing TeX file: $tex_file" >&2
  exit 2
fi

case "$engine" in
  xelatex|pdflatex|lualatex) ;;
  *)
    echo "Unsupported engine: $engine" >&2
    exit 2
    ;;
esac

tex_dir=$(cd "$(dirname "$tex_file")" && pwd)
tex_base=$(basename "$tex_file")
stem=${tex_base%.tex}

if [[ -z "$out_dir" ]]; then
  out_dir="$tex_dir/build"
fi
mkdir -p "$out_dir"
out_dir=$(cd "$out_dir" && pwd)

if command -v latexmk >/dev/null 2>&1; then
  latexmk "-$engine" -interaction=nonstopmode -halt-on-error -file-line-error \
    -outdir="$out_dir" "$tex_file"
else
  "$engine" -interaction=nonstopmode -halt-on-error -file-line-error \
    -output-directory="$out_dir" "$tex_file"
  "$engine" -interaction=nonstopmode -halt-on-error -file-line-error \
    -output-directory="$out_dir" "$tex_file"
fi

pdf="$out_dir/$stem.pdf"
log="$out_dir/$stem.log"
png="$out_dir/$stem.png"

if [[ ! -f "$pdf" ]]; then
  echo "Expected PDF was not created: $pdf" >&2
  exit 1
fi

if [[ -f "$log" ]]; then
  if grep -E "Missing character|Overfull \\\\hbox|Overfull \\\\vbox" "$log" >/dev/null; then
    echo "Warnings found in $log:" >&2
    grep -E "Missing character|Overfull \\\\hbox|Overfull \\\\vbox" "$log" >&2 || true
  fi
fi

if command -v pdftoppm >/dev/null 2>&1; then
  pdftoppm -png -r "$dpi" -singlefile "$pdf" "$out_dir/$stem"
fi

cat <<EOF
PDF: $pdf
PNG: $png
LOG: $log
EOF

