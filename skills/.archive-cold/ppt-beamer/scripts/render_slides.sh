#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
Usage: render_slides.sh <pdf-file> [--prefix slides] [--dpi 180]
EOF
}

if [[ $# -lt 1 ]]; then
  usage
  exit 1
fi

pdf_file=""
prefix="slides"
dpi="180"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --prefix)
      prefix="${2:-}"
      shift 2
      ;;
    --dpi)
      dpi="${2:-}"
      shift 2
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
      if [[ -n "$pdf_file" ]]; then
        echo "Only one PDF may be provided." >&2
        usage
        exit 1
      fi
      pdf_file="$1"
      shift
      ;;
  esac
done

if [[ -z "$pdf_file" ]]; then
  echo "Missing PDF file." >&2
  usage
  exit 1
fi

if [[ ! -f "$pdf_file" ]]; then
  echo "PDF file not found: $pdf_file" >&2
  exit 1
fi

output_dir="$(dirname "$pdf_file")/${prefix}-pages"
mkdir -p "$output_dir"
find "$output_dir" -type f -name '*.png' -delete

if command -v gs >/dev/null 2>&1; then
  if gs \
    -dSAFER \
    -dBATCH \
    -dNOPAUSE \
    -sDEVICE=pngalpha \
    -r"$dpi" \
    -sOutputFile="$output_dir/$prefix-%02d.png" \
    "$pdf_file" >/dev/null 2>&1; then
    :
  fi
fi

if ! find "$output_dir" -type f -name '*.png' -size +0c | grep -q .; then
  if command -v pdftocairo >/dev/null 2>&1; then
    if pdftocairo -png -r "$dpi" "$pdf_file" "$output_dir/$prefix" >/dev/null 2>&1; then
      :
    fi
  fi
fi

if ! find "$output_dir" -type f -name '*.png' -size +0c | grep -q .; then
  if command -v pdftoppm >/dev/null 2>&1; then
    if pdftoppm -png -r "$dpi" "$pdf_file" "$output_dir/$prefix" >/dev/null 2>&1; then
      :
    fi
  fi
fi

if ! find "$output_dir" -type f -name '*.png' -size +0c | grep -q .; then
  echo "No rendered PNG files were produced." >&2
  exit 1
fi

echo "Rendered slides to $output_dir"
