#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
Usage: build_beamer.sh <tex-file> [--engine xelatex|lualatex|pdflatex] [--outdir build] [--clean]
EOF
}

if [[ $# -lt 1 ]]; then
  usage
  exit 1
fi

tex_file=""
engine="xelatex"
outdir="build"
clean=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --engine)
      engine="${2:-}"
      shift 2
      ;;
    --outdir)
      outdir="${2:-}"
      shift 2
      ;;
    --clean)
      clean=1
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    -*)
      echo "Unknown option: $1" >&2
      usage
      exit 1
      ;;
    *)
      if [[ -n "$tex_file" ]]; then
        echo "Only one tex file may be provided." >&2
        usage
        exit 1
      fi
      tex_file="$1"
      shift
      ;;
  esac
done

if [[ -z "$tex_file" ]]; then
  echo "Missing tex file." >&2
  usage
  exit 1
fi

if [[ ! -f "$tex_file" ]]; then
  echo "TeX file not found: $tex_file" >&2
  exit 1
fi

case "$engine" in
  xelatex|lualatex|pdflatex)
    ;;
  *)
    echo "Unsupported engine: $engine" >&2
    exit 1
    ;;
esac

if ! command -v latexmk >/dev/null 2>&1; then
  echo "latexmk is required but not found in PATH." >&2
  exit 1
fi

mkdir -p "$outdir"

if [[ $clean -eq 1 ]]; then
  latexmk -C -outdir="$outdir" "$tex_file"
fi

engine_flag="-$engine"
latexmk \
  "$engine_flag" \
  -interaction=nonstopmode \
  -halt-on-error \
  -file-line-error \
  -synctex=1 \
  -outdir="$outdir" \
  "$tex_file"
