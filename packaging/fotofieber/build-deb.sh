#!/usr/bin/env bash
# Build the .deb of LightCraft Film Scan, the FotoFieber build with Convert Negative (x86_64), inside an
# Ubuntu 22.04 container, so it runs on every distribution with glibc >= 2.35 (Ubuntu 22.04+, Debian 12+,
# Mint 21+, ...). The package is called lightcraft-filmscan and installs beside the official lightcraft
# package (packaging/fotofieber/nfpm.yaml).
#
#   packaging/fotofieber/build-deb.sh
#
# Output: dist/fotofieber/lightcraft-filmscan-<version>-linux-x86_64.deb and SHA256SUMS.txt.
# Needs podman (or CONTAINER=docker). Caches (rustup, cargo, target, craft-fonts) live in
# target/fotofieber/, so a second run only rebuilds what changed.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
CONTAINER="${CONTAINER:-podman}"
IMAGE=docker.io/library/ubuntu:22.04
CACHE="$ROOT/target/fotofieber"
OUT="$ROOT/dist/fotofieber"
MAINTAINER="FotoFieber <lightcraft@fotofieber.ch>"
RUST_TOOLCHAIN="${RUST_TOOLCHAIN:-1.99.1}"
# nfpm (MIT) builds the .deb; pinned, checked against the release's checksums.txt.
NFPM_VERSION=2.48.0
NFPM_SHA256=6d7f9a28b2f397934403950fcbba83f598b08d02edbe79a0815b3b4897341750

mkdir -p "$CACHE" "$OUT"

# craft-fonts at the revision packaging/fonts.sh (and upstream's release) pins.
FONTS_REV="$(sed -n 's/^ *local revision=\([0-9a-f]\{40\}\)$/\1/p' "$ROOT/packaging/fonts.sh")"
[[ -n "$FONTS_REV" ]] || { echo "error: no craft-fonts revision in packaging/fonts.sh" >&2; exit 1; }
if [[ ! -d "$CACHE/craft-fonts/.git" ]]; then
  git clone --no-checkout https://github.com/storytold/craft-fonts "$CACHE/craft-fonts"
fi
git -C "$CACHE/craft-fonts" fetch -q origin "$FONTS_REV" 2>/dev/null || true
git -C "$CACHE/craft-fonts" checkout -q --detach "$FONTS_REV"

SHA="$(git -C "$ROOT" rev-parse HEAD)"
# A dirty tree would publish a binary whose source is not in the tag.
if [[ -n "$(git -C "$ROOT" status --porcelain --untracked-files=no)" ]]; then
  echo "error: uncommitted changes; commit first so the binary matches the tagged source" >&2
  exit 1
fi

"$CONTAINER" run --rm -i \
  -v "$ROOT:/src:Z" \
  -e LIGHTCRAFT_BUILD_SHA="$SHA" \
  -e LIGHTCRAFT_MAINTAINER="$MAINTAINER" \
  -e RUST_TOOLCHAIN="$RUST_TOOLCHAIN" \
  -e NFPM_VERSION="$NFPM_VERSION" -e NFPM_SHA256="$NFPM_SHA256" \
  "$IMAGE" bash -s <<'IN_CONTAINER'
set -euo pipefail
export DEBIAN_FRONTEND=noninteractive
apt-get update -qq
apt-get install -y -qq --no-install-recommends ca-certificates curl git build-essential pkg-config \
  desktop-file-utils >/dev/null
C=/src/target/fotofieber
export RUSTUP_HOME=$C/rustup CARGO_HOME=$C/cargo CARGO_TARGET_DIR=$C/target-ubuntu2204
export PATH="$CARGO_HOME/bin:$C/bin:$PATH"
if ! command -v rustup >/dev/null; then
  curl --proto '=https' -sSf https://sh.rustup.rs | sh -s -- -y -q --no-modify-path --profile minimal \
    --default-toolchain "$RUST_TOOLCHAIN"
fi
rustup toolchain install --profile minimal "$RUST_TOOLCHAIN"
rustup default "$RUST_TOOLCHAIN"
if [[ ! -x "$C/bin/nfpm-$NFPM_VERSION" ]]; then
  mkdir -p "$C/bin"
  tgz="$C/nfpm_${NFPM_VERSION}_Linux_x86_64.tar.gz"
  curl --proto '=https' -fsSL -o "$tgz" \
    "https://github.com/goreleaser/nfpm/releases/download/v$NFPM_VERSION/nfpm_${NFPM_VERSION}_Linux_x86_64.tar.gz"
  echo "$NFPM_SHA256  $tgz" | sha256sum -c -
  tar -xzf "$tgz" -C "$C/bin" nfpm
  mv "$C/bin/nfpm" "$C/bin/nfpm-$NFPM_VERSION"
fi
ln -sf "nfpm-$NFPM_VERSION" "$C/bin/nfpm"
git config --global --add safe.directory '*'
cd /src
# package.sh builds and stages the official file tree (its tarball is not shipped); the .deb is
# made from it under LightCraft Film Scan's own names, so it installs beside an official LightCraft.
CRAFT_FONTS_DIR=$C/craft-fonts DIST=$C/package-dist packaging/linux/package.sh --formats tar
STAGE=$CARGO_TARGET_DIR/linux-package/root
FS=$C/filmscan-root
ID=ch.fotofieber.LightCraftFilmScan
rm -rf "$FS"
install -Dm755 "$STAGE/usr/bin/lightcraft" "$FS/usr/lib/lightcraft-filmscan/lightcraft"
install -Dm755 "$STAGE/usr/bin/lightcraft-cli" "$FS/usr/lib/lightcraft-filmscan/lightcraft-cli"
install -Dm755 packaging/fotofieber/lightcraft-filmscan.sh "$FS/usr/bin/lightcraft-filmscan"
mkdir -p "$FS/usr/share/applications"
sed -e 's|^Name=.*|Name=LightCraft Film Scan|' \
  -e 's|^GenericName=.*|GenericName=Photo Library, Raw Developer and Film Scans|' \
  -e 's|^Exec=lightcraft |Exec=lightcraft-filmscan |' \
  -e "s|^Icon=.*|Icon=$ID|" \
  -e "s|^StartupWMClass=.*|StartupWMClass=$ID|" \
  -e 's|^Keywords=\(.*\)|Keywords=\1film;negative;scan;|' \
  packaging/linux/ai.storyteller.lightcraft.desktop >"$FS/usr/share/applications/$ID.desktop"
grep -q "^Exec=lightcraft-filmscan " "$FS/usr/share/applications/$ID.desktop"
desktop-file-validate "$FS/usr/share/applications/$ID.desktop"
for f in "$STAGE"/usr/share/icons/hicolor/*/apps/ai.storyteller.lightcraft.*; do
  rel="${f#"$STAGE"/}"
  install -Dm644 "$f" "$FS/$(dirname "$rel")/$ID.${f##*.}"
done
mkdir -p "$FS/usr/share/doc/lightcraft-filmscan"
cp "$STAGE"/usr/share/doc/lightcraft/* "$FS/usr/share/doc/lightcraft-filmscan/"
VERSION="$(awk '/^\[/ { p = ($0 == "[workspace.package]"); next } p && $1 == "version" { gsub(/[" ]/, "", $3); print $3; exit }' Cargo.toml)"
sed "s|\${FS}|$FS|g" packaging/fotofieber/nfpm.yaml >"$C/nfpm-filmscan.yaml"
VERSION="$VERSION" NFPM_ARCH=amd64 LIGHTCRAFT_MAINTAINER="$LIGHTCRAFT_MAINTAINER" \
  nfpm package -f "$C/nfpm-filmscan.yaml" -p deb -t "/src/dist/fotofieber/lightcraft-filmscan-$VERSION-linux-x86_64.deb"
IN_CONTAINER

VERSION="$(awk '/^\[/ { p = ($0 == "[workspace.package]"); next } p && $1 == "version" { gsub(/[" ]/, "", $3); print $3; exit }' "$ROOT/Cargo.toml")"
DEB="lightcraft-filmscan-$VERSION-linux-x86_64.deb"
[[ -f "$OUT/$DEB" ]] || { echo "error: $OUT/$DEB was not written" >&2; exit 1; }
(cd "$OUT" && sha256sum "$DEB" >SHA256SUMS.txt)
# The commit the .deb was built from; publish.sh refuses to publish it with other source.
echo "$SHA" >"$OUT/BUILT_FROM"
echo "==> $OUT/$DEB"
cat "$OUT/SHA256SUMS.txt"
