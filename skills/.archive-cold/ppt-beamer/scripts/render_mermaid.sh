#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
Usage: render_mermaid.sh <input.md|input.mmd> [--outdir assets/diagrams] [--name basename] [--format svg|pdf|both]
EOF
}

if [[ $# -lt 1 ]]; then
  usage
  exit 1
fi

input_file=""
outdir="assets/diagrams"
name=""
format="both"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --outdir)
      outdir="${2:-}"
      shift 2
      ;;
    --name)
      name="${2:-}"
      shift 2
      ;;
    --format)
      format="${2:-}"
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
      if [[ -n "$input_file" ]]; then
        echo "Only one input file may be provided." >&2
        usage
        exit 1
      fi
      input_file="$1"
      shift
      ;;
  esac
done

if [[ -z "$input_file" || ! -f "$input_file" ]]; then
  echo "Input file not found: $input_file" >&2
  exit 1
fi

case "$format" in
  svg|pdf|both)
    ;;
  *)
    echo "Unsupported format: $format" >&2
    exit 1
    ;;
esac

if ! command -v npx >/dev/null 2>&1; then
  echo "npx is required but not found in PATH." >&2
  exit 1
fi

mkdir -p "$outdir"

input_basename="$(basename "$input_file")"
input_stem="${input_basename%.*}"
if [[ -z "$name" ]]; then
  name="$input_stem"
fi

tmpdir="$(mktemp -d)"
trap 'rm -rf "$tmpdir"' EXIT

render_source="$input_file"
if [[ "$input_file" == *.md ]]; then
  awk '
    BEGIN { in_block=0; found=0 }
    /^```mermaid[[:space:]]*$/ { in_block=1; found=1; next }
    /^```[[:space:]]*$/ && in_block { in_block=0; exit }
    in_block { print }
    END { if (!found) exit 2 }
  ' "$input_file" > "$tmpdir/$name.mmd" || {
    echo "No fenced mermaid block found in Markdown input." >&2
    exit 1
  }
  render_source="$tmpdir/$name.mmd"
fi

svg_output="$outdir/$name.svg"
pdf_output="$outdir/$name.pdf"

npx -y @mermaid-js/mermaid-cli \
  -i "$render_source" \
  -o "$svg_output" \
  -b transparent \
  -t default \
  -q

if [[ ! -s "$svg_output" ]]; then
  echo "SVG output was not created: $svg_output" >&2
  exit 1
fi

if [[ "$format" == "pdf" || "$format" == "both" ]]; then
  if command -v rsvg-convert >/dev/null 2>&1; then
    rsvg-convert -f pdf -o "$pdf_output" "$svg_output"
  else
    echo "rsvg-convert not found; skipping PDF conversion." >&2
  fi
fi

if [[ "$format" == "pdf" ]]; then
  rm -f "$svg_output"
fi

echo "Rendered Mermaid asset(s) to $outdir"
