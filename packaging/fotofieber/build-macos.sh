#!/usr/bin/env bash
# Build LightCraft Film Scan for macOS (Apple silicon and Intel in one universal app) on a Mac. The
# GitHub workflow .github/workflows/fotofieber-build.yml runs it; it also works on any Mac with Rust
# and the Xcode command-line tools.
#
#   packaging/fotofieber/build-macos.sh                         # both architectures, one after the other
#   packaging/fotofieber/build-macos.sh --target <triple>       # step 1: one architecture's binaries only
#   packaging/fotofieber/build-macos.sh --prebuilt <dir>        # step 2: the app from step 1's binaries
#
# The GitHub workflow runs step 1 for aarch64-apple-darwin and x86_64-apple-darwin on two machines at
# once, then step 2: about half the wait of one machine compiling both. --target writes
# dist/fotofieber/macos-bin/<triple>/{lightcraft,lightcraft-cli,BUILT_FROM}; --prebuilt <dir> takes a
# folder holding one such <triple> folder per architecture, all built from HEAD.
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
TARGETS=(aarch64-apple-darwin x86_64-apple-darwin)

MODE=all ARG=""
case "${1:-}" in
  "") ;;
  --target | --prebuilt) MODE="${1#--}" ARG="${2:?usage: build-macos.sh [--target <triple> | --prebuilt <dir>]}" ;;
  *) echo "usage: build-macos.sh [--target <triple> | --prebuilt <dir>]" >&2; exit 2 ;;
esac

# The fonts are embedded when compiling, and their licences go into the packages: both steps need them.
# shellcheck source=../fonts.sh
. "$ROOT/packaging/fonts.sh"
prepare_fonts

if [[ "$MODE" == target ]]; then
  [[ " ${TARGETS[*]} " == *" $ARG "* ]] || { echo "error: --target is one of ${TARGETS[*]}" >&2; exit 2; }
  # The same build as packaging/macos/package.sh's, for one architecture.
  (cd "$ROOT" && cargo build --release --locked -p lightcraft -p lightcraft-cli \
    --features lightcraft/heif,lightcraft-cli/heif --target "$ARG")
  bin="$OUT/macos-bin/$ARG"
  rm -rf "$bin" && mkdir -p "$bin"
  cp "$CARGO_TARGET_DIR/$ARG/release/lightcraft" "$CARGO_TARGET_DIR/$ARG/release/lightcraft-cli" "$bin/"
  echo "$SHA" >"$bin/BUILT_FROM"
  lipo -info "$bin/lightcraft"
  exit 0
fi

rm -rf "$DIST"
mkdir -p "$DIST" "$OUT"
if [[ "$MODE" == prebuilt ]]; then
  # Put each architecture's binaries where package.sh --skip-build looks for them. Every one must be
  # built from this commit, or the app would mix sources.
  for t in "${TARGETS[@]}"; do
    from="$ARG/$t"
    [[ "$(cat "$from/BUILT_FROM" 2>/dev/null)" == "$SHA" ]] ||
      { echo "error: $from was not built from $SHA" >&2; exit 1; }
    mkdir -p "$CARGO_TARGET_DIR/$t/release"
    for b in lightcraft lightcraft-cli; do
      # Workflow artifacts lose the executable bit.
      install -m 755 "$from/$b" "$CARGO_TARGET_DIR/$t/release/$b"
    done
  done
  "$ROOT/packaging/macos/package.sh" --arch universal --skip-build
else
  "$ROOT/packaging/macos/package.sh" --arch universal
fi

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
info="$(lipo -info "$APP/Contents/MacOS/LightCraft")" && echo "$info"
[[ "$info" == *arm64* && "$info" == *x86_64* ]] || { echo "error: the app is not universal" >&2; exit 1; }
"$CLI_DIR/lightcraft-filmscan-cli" --version
echo "$SHA" >"$OUT/BUILT_FROM-macos"
(cd "$OUT" && shasum -a 256 "$(basename "$APP_ZIP")" "$(basename "$CLI_ZIP")")
