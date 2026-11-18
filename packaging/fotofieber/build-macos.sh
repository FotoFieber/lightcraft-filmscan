#!/usr/bin/env bash
# Build LightCraft Film Scan for macOS (Apple silicon and Intel in one universal app) on a Mac. The
# GitHub workflow .github/workflows/fotofieber-build.yml runs it; it also works on any Mac with Rust
# and the Xcode command-line tools.
#
#   packaging/fotofieber/build-macos.sh
#
# Output in dist/fotofieber/:
#   lightcraft-filmscan-<version>-macos-universal.zip       "LightCraft Film Scan.app"
#   lightcraft-filmscan-cli-<version>-macos-universal.zip   the command-line tool, lightcraft-filmscan-cli
#
# It runs upstream's packaging/macos/package.sh unchanged, with ad-hoc signing (we have no Apple
# Developer ID), then renames the app: its own bundle id, name and version, so it installs beside an
# official LightCraft. LIGHTCRAFT_FILMSCAN=1 gives the binaries their own settings folder, library
# and app id (crates/engine/src/flavor.rs). The app is not notarized: macOS asks before opening it
# the first time.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
OUT="$ROOT/dist/fotofieber"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT/target}"
export DIST="$CARGO_TARGET_DIR/fotofieber-macos-dist"
export LIGHTCRAFT_FILMSCAN=1
unset MACOS_SIGN_IDENTITY APPLE_ID APPLE_PASSWORD APPLE_TEAM_ID

VERSION="$(awk '/^\[/ { p = ($0 == "[workspace.package]"); next } p && $1 == "version" { gsub(/[" ]/, "", $3); print $3; exit }' "$ROOT/Cargo.toml")"
# Finder and the App Store tools want a plain number: 0.21.1+filmscan.7 -> 0.21.1.
PLAIN="${VERSION%%[+-]*}"
NAME="LightCraft Film Scan"
BUNDLE_ID="ch.fotofieber.LightCraftFilmScan"
SHA="$(git -C "$ROOT" rev-parse HEAD)"

rm -rf "$DIST"
mkdir -p "$DIST" "$OUT"
"$ROOT/packaging/macos/package.sh" --arch universal

WORK="$CARGO_TARGET_DIR/macos-package"
STAGE="$CARGO_TARGET_DIR/fotofieber-macos"
APP="$STAGE/$NAME.app"
rm -rf "$STAGE"
mkdir -p "$STAGE"
ditto "$WORK/LightCraft.app" "$APP"
plist="$APP/Contents/Info.plist"
pb() { /usr/libexec/PlistBuddy -c "$1" "$plist"; }
pb "Set :CFBundleIdentifier $BUNDLE_ID"
pb "Set :CFBundleName $NAME"
pb "Set :CFBundleDisplayName $NAME"
pb "Set :CFBundleShortVersionString $PLAIN"
pb "Set :CFBundleVersion $PLAIN"
plutil -lint "$plist"
# Our own package README, not the repository's: README.md can then change after a build without making
# the package stale (publish.sh leaves README.md out of its source check). Before signing: it is in the bundle.
README_PKG="$ROOT/packaging/fotofieber/README-package.md"
cp "$README_PKG" "$APP/Contents/Resources/Licenses/README.md"
# The changed Info.plist breaks the ad-hoc signature package.sh made: sign again, the same way.
xattr -cr "$APP"
codesign --force --sign - --timestamp=none --options runtime --entitlements "$ROOT/packaging/macos/entitlements.plist" "$APP/Contents/MacOS/LightCraft"
codesign --force --sign - --timestamp=none --options runtime --entitlements "$ROOT/packaging/macos/entitlements.plist" "$APP"
codesign --verify --strict --deep --verbose=2 "$APP"
[[ "$(pb 'Print :CFBundleIdentifier')" == "$BUNDLE_ID" ]] || { echo "error: bundle id not set" >&2; exit 1; }

APP_ZIP="$OUT/lightcraft-filmscan-$VERSION-macos-universal.zip"
CLI_ZIP="$OUT/lightcraft-filmscan-cli-$VERSION-macos-universal.zip"
rm -f "$APP_ZIP" "$CLI_ZIP"
ditto -c -k --keepParent "$APP" "$APP_ZIP"
# The command-line tool from package.sh's folder, named like the Linux command so it is not mistaken
# for an official lightcraft-cli. Renaming keeps the ad-hoc signature valid.
CLI_NAME="lightcraft-filmscan-cli-$VERSION-macos-universal"
CLI_DIR="$STAGE/$CLI_NAME"
ditto "$WORK/lightcraft-cli-$VERSION-macos-universal" "$CLI_DIR"
mv "$CLI_DIR/lightcraft-cli" "$CLI_DIR/lightcraft-filmscan-cli"
cp "$README_PKG" "$CLI_DIR/README.md"
codesign --verify --strict --verbose=2 "$CLI_DIR/lightcraft-filmscan-cli"
ditto -c -k --keepParent "$CLI_DIR" "$CLI_ZIP"

# Smoke test: both architectures are in the app, and the command-line tool runs.
lipo -info "$APP/Contents/MacOS/LightCraft"
"$CLI_DIR/lightcraft-filmscan-cli" --version
echo "$SHA" >"$OUT/BUILT_FROM-macos"
(cd "$OUT" && shasum -a 256 "$(basename "$APP_ZIP")" "$(basename "$CLI_ZIP")")
