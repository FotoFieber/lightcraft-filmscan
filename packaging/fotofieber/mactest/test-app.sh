#!/usr/bin/env bash
# Test the packaged macOS app and command-line tool on a throwaway Mac (a GitHub runner).
#
#   test-app.sh <folder-with-the-two-ZIPs> <out>
#
# Checks the bundle, converts synthetic negatives with the command-line tool (CPU and GPU), then
# starts the app, drives it over its control port and takes screenshots. The app runs with its
# default settings folder and library, to check that they are Film Scan's own: it therefore refuses
# to run where either of them already exists.
set -euo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
zips="$(cd "${1:?usage: test-app.sh <zips> <out>}" && pwd)"
mkdir -p "${2:?}"
out="$(cd "$2" && pwd)"
work="$(mktemp -d)"
summary="$out/summary.md"
port=7993
failed=0

say() { echo "$*" | tee -a "$summary"; }
check() { # check <label> <command…>
  local label="$1"
  shift
  if "$@" >>"$out/checks.log" 2>&1; then say "- ok: $label"; else say "- **fail**: $label" && failed=1; fi
}

settings="$HOME/Library/Application Support/LightCraft Film Scan"
library="$HOME/Pictures/LightCraft Film Scan Library"
official_settings="$HOME/Library/Application Support/LightCraft"
official_library="$HOME/Pictures/LightCraft Library"
for d in "$settings" "$library" "$official_settings" "$official_library"; do
  [[ ! -e "$d" ]] || { echo "error: $d exists; this test is for a throwaway Mac" >&2; exit 1; }
done

shopt -s nullglob
app_zips=("$zips"/lightcraft-filmscan-[0-9]*-macos-universal.zip)
cli_zips=("$zips"/lightcraft-filmscan-cli-*-macos-universal.zip)
[[ ${#app_zips[@]} == 1 && ${#cli_zips[@]} == 1 ]] || { echo "error: $zips needs one app ZIP and one CLI ZIP" >&2; exit 1; }
ditto -x -k "${app_zips[0]}" "$work/app"
ditto -x -k "${cli_zips[0]}" "$work/cli"
app="$work/app/LightCraft Film Scan.app"
bin="$app/Contents/MacOS/LightCraft"
clis=("$work"/cli/*/lightcraft-filmscan-cli)
cli="${clis[0]}"

say "## macOS $(sw_vers -productVersion) on $(uname -m)"
say ""
say "Package: $(basename "${app_zips[0]}")"
say ""
check "app signature is intact" codesign --verify --strict --deep "$app"
check "app has Apple silicon and Intel code" lipo "$bin" -verify_arch arm64 x86_64
check "bundle id is Film Scan's" test "$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$app/Contents/Info.plist")" = ch.fotofieber.LightCraftFilmScan
check "command-line tool starts" "$cli" --version
# The app is not notarized, so Gatekeeper is expected to reject it: recorded, not a failure.
say "- Gatekeeper: $(spctl --assess --type execute --verbose "$app" 2>&1 | tr '\n' ' ' | sed "s|$work/app/||")"

python3 -I "$here/make-negative.py" "$out/scans" >/dev/null
check "Zap! and export on the CPU" env LIGHTCRAFT_CONFIG_DIR="$work/cli-config" LIGHTCRAFT_GPU=0 "$here/convert.sh" "$cli" "$out/scans" "$out/cli-cpu"
check "Zap! and export on the GPU" env LIGHTCRAFT_CONFIG_DIR="$work/cli-config" "$here/convert.sh" "$cli" "$out/scans" "$out/cli-gpu"

"$bin" --no-demo --control "$port" "$out"/scans/*.png >"$out/app.log" 2>&1 &
pid=$!
check "app answers on its control port and converts the scans" python3 -I "$here/drive-app.py" "$port" "$out/app"
screencapture -x "$out/app/screen.png" >>"$out/checks.log" 2>&1 || say "- note: no picture of the whole screen"
for _ in $(seq 30); do kill -0 "$pid" 2>/dev/null || break; sleep 1; done
if kill -0 "$pid" 2>/dev/null; then
  say "- **fail**: the app did not quit" && failed=1
  kill -9 "$pid"
fi
check "app ended without an error" wait "$pid"
check "settings are in Film Scan's own folder" test -d "$settings"
check "library is Film Scan's own" test -d "$library"
check "official LightCraft folders untouched" test ! -e "$official_settings" -a ! -e "$official_library"
if grep -q "rendering headlessly" "$out/app.log"; then
  say "- note: some screenshots were drawn on the CPU, not captured from the window"
fi
if [[ -f "$out/app/inspect.json" ]]; then
  say "- graphics: $(python3 -I -c 'import json,sys; p=json.load(open(sys.argv[1])).get("perf",{}); print({k:p.get(k) for k in ("gpu","gpuReason","gpuFallback","mainOnGpu")})' "$out/app/inspect.json")"
fi
grep -E ' (ERROR|WARN) |panicked' "$out/app.log" | head -40 >"$out/app-warnings.log" || true
reports=("$HOME"/Library/Logs/DiagnosticReports/LightCraft*)
if [[ ${#reports[@]} -gt 0 ]]; then
  mkdir -p "$out/crash-reports" && cp "${reports[@]}" "$out/crash-reports/"
  say "- **fail**: macOS wrote a crash report" && failed=1
fi
exit "$failed"
