#!/usr/bin/env bash
# Publish a FotoFieber Convert Negative build as a GitHub release of FotoFieber/lightcraft.
#
#   packaging/fotofieber/build-deb.sh
#   packaging/fotofieber/screenshots.sh            # optional; check the images before publishing
#   packaging/fotofieber/release-notes.sh v0.21.1 > dist/fotofieber/notes.md   # read it
#   packaging/fotofieber/publish.sh v0.21.1
#
# Tags HEAD as <upstream-tag>-convert-negative, pushes this branch and the tag to the remote
# `fotofieber`, and creates the release with the .deb, SHA256SUMS.txt, the patch series and the
# screenshots. GitHub adds the source archives of the tag itself.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
BASE="${1:?usage: publish.sh <upstream-tag>}"
FORK=FotoFieber/lightcraft
REMOTE=fotofieber
TAG="$BASE-convert-negative"
OUT="$ROOT/dist/fotofieber"
VERSION="$(awk '/^\[/ { p = ($0 == "[workspace.package]"); next } p && $1 == "version" { gsub(/[" ]/, "", $3); print $3; exit }' "$ROOT/Cargo.toml")"
DEB="$OUT/lightcraft-$VERSION-linux-x86_64.deb"
NOTES="$OUT/notes.md"
BRANCH="$(git -C "$ROOT" symbolic-ref --short HEAD)"

[[ -f "$DEB" ]] || { echo "error: $DEB missing; run build-deb.sh" >&2; exit 1; }
[[ -s "$NOTES" ]] || { echo "error: $NOTES missing; run release-notes.sh $BASE > $NOTES and read it" >&2; exit 1; }
[[ -z "$(git -C "$ROOT" status --porcelain --untracked-files=no)" ]] || { echo "error: uncommitted changes" >&2; exit 1; }
(cd "$OUT" && sha256sum -c SHA256SUMS.txt)
# The .deb must be built from HEAD: its control data holds no commit, so compare the binary's.
built="$(dpkg-deb --fsys-tarfile "$DEB" | tar -xO ./usr/bin/lightcraft-cli | strings | grep -m1 -o "$(git -C "$ROOT" rev-parse HEAD)" || true)"
[[ -n "$built" ]] || { echo "error: $DEB was not built from HEAD; rebuild" >&2; exit 1; }
# Public commits carry only the address chosen for this build.
if git -C "$ROOT" log --format='%ae%n%ce' "$BASE..HEAD" | grep -q '@fotofieber.ch' &&
   git -C "$ROOT" log --format='%ae%n%ce' "$BASE..HEAD" | grep '@fotofieber.ch' | grep -qv '^lightcraft@fotofieber.ch$'; then
  echo "error: a commit in $BASE..HEAD has a fotofieber.ch address other than lightcraft@" >&2
  exit 1
fi

PATCHES="$OUT/convert-negative-patches-$BASE.tar.gz"
rm -rf "$OUT/patches" && mkdir -p "$OUT/patches"
git -C "$ROOT" format-patch -q --no-merges -o "$OUT/patches" "$BASE..HEAD"
tar -C "$OUT" -czf "$PATCHES" patches

git -C "$ROOT" tag -a "$TAG" -m "LightCraft $BASE with Convert Negative ($VERSION)"
git -C "$ROOT" push "$REMOTE" "$BRANCH" "refs/tags/$TAG"

assets=("$DEB" "$OUT/SHA256SUMS.txt" "$PATCHES")
if compgen -G "$OUT/screenshots/*.png" >/dev/null; then assets+=("$OUT"/screenshots/*.png); fi
gh release create "$TAG" --repo "$FORK" --verify-tag --latest \
  --title "LightCraft $BASE + Convert Negative" --notes-file "$NOTES" "${assets[@]}"
echo "==> https://github.com/$FORK/releases/tag/$TAG"
