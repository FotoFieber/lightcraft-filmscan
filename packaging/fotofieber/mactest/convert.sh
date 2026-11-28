#!/usr/bin/env bash
# Zap! each test scan with a command-line tool and export the positive as PNG.
#
#   convert.sh <lightcraft-cli> <scans> <out>
#
# Writes <out>/<scan>.png and <out>/<scan>.json (the commands' replies). Set LIGHTCRAFT_GPU=0 to
# render on the CPU.
set -euo pipefail
cli="${1:?usage: convert.sh <lightcraft-cli> <scans> <out>}"
scans="${2:?}"
mkdir -p "${3:?}"
out="$(cd "$3" && pwd)"
for scan in "$scans"/*.png; do
  name="$(basename "$scan" .png)"
  "$cli" run --import "$scan" \
    develop.negativeZap format=35mm \
    app.export "path=$out/$name.png" | tee "$out/$name.json"
  [[ -s "$out/$name.png" ]] || { echo "error: no export of $name" >&2; exit 1; }
done
