#!/usr/bin/env bash
# Take the release-note screenshots headlessly (no window, CPU-rasterized UI).
#
#   packaging/fotofieber/screenshots.sh <folder-of-scans> [tour.jsonl]
#
# Imports the scans into a throwaway session, runs the tour (default: screenshots.jsonl next to
# this script) and writes dist/fotofieber/screenshots/*.png. Name the scans neutrally first: file
# names show in the filmstrip. Look at every image before publishing; never commit them.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SCANS="${1:?usage: screenshots.sh <folder-of-scans> [tour.jsonl]}"
TOUR="${2:-$ROOT/packaging/fotofieber/screenshots.jsonl}"
OUT="$ROOT/dist/fotofieber/screenshots"
CLI="${LIGHTCRAFT_CLI:-$ROOT/target/release/lightcraft-cli}"
[[ -x "$CLI" ]] || { echo "error: build it first: cargo build --release -p lightcraft-cli --features heif" >&2; exit 1; }
rm -rf "$OUT" && mkdir -p "$OUT"
script="$(mktemp)"
trap 'rm -f "$script"' EXIT
sed "s|@OUT@|$OUT|g" "$TOUR" >"$script"
"$CLI" snapshot "$SCANS" --script "$script" -o "$OUT/.last.png" --size 1600x1000 >/dev/null
rm -f "$OUT/.last.png"
ls -1 "$OUT"
