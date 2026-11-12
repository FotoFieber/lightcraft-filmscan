#!/usr/bin/env bash
# Publish LightCraft Film Scan (the FotoFieber build with Convert Negative) as a GitHub release of
# FotoFieber/lightcraft-filmscan.
#
#   packaging/fotofieber/build-deb.sh
#   packaging/fotofieber/screenshots.sh            # optional; check the images before publishing
#   packaging/fotofieber/release-notes.sh v0.21.1 > dist/fotofieber/notes.md   # read it
#   packaging/fotofieber/publish.sh v0.21.1
#
# Tags HEAD as <upstream-tag>-filmscan, pushes this branch and the tag to the remote `fotofieber`,
# and creates the release with the .deb, SHA256SUMS.txt, one diff against the upstream tag, the
# patch series and the screenshots. GitHub adds the source archives of the tag itself.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
BASE="${1:?usage: publish.sh <upstream-tag>}"
FORK=FotoFieber/lightcraft-filmscan
REMOTE=fotofieber
TAG="$BASE-filmscan"
OUT="$ROOT/dist/fotofieber"
VERSION="$(awk '/^\[/ { p = ($0 == "[workspace.package]"); next } p && $1 == "version" { gsub(/[" ]/, "", $3); print $3; exit }' "$ROOT/Cargo.toml")"
DEB="$OUT/lightcraft-filmscan-$VERSION-linux-x86_64.deb"
NOTES="$OUT/notes.md"
BRANCH="$(git -C "$ROOT" symbolic-ref --short HEAD)"

[[ -f "$DEB" ]] || { echo "error: $DEB missing; run build-deb.sh" >&2; exit 1; }
[[ -s "$NOTES" ]] || { echo "error: $NOTES missing; run release-notes.sh $BASE > $NOTES and read it" >&2; exit 1; }
[[ -z "$(git -C "$ROOT" status --porcelain --untracked-files=no)" ]] || { echo "error: uncommitted changes" >&2; exit 1; }
(cd "$OUT" && sha256sum -c SHA256SUMS.txt)
# The .deb must be built from the source being tagged: only this release tooling may differ.
built="$(cat "$OUT/BUILT_FROM" 2>/dev/null || true)"
if [[ -z "$built" ]] || ! git -C "$ROOT" diff --quiet "$built" HEAD -- . ':(exclude)packaging/fotofieber'; then
  echo "error: $DEB was not built from this source (built from '${built:-?}'); rebuild" >&2
  exit 1
fi
# Public commits carry only the address chosen for this build.
if git -C "$ROOT" log --format='%ae%n%ce' "$BASE..HEAD" | grep -q '@fotofieber.ch' &&
   git -C "$ROOT" log --format='%ae%n%ce' "$BASE..HEAD" | grep '@fotofieber.ch' | grep -qv '^lightcraft@fotofieber.ch$'; then
  echo "error: a commit in $BASE..HEAD has a fotofieber.ch address other than lightcraft@" >&2
  exit 1
fi

PATCHES="$OUT/lightcraft-filmscan-patches-$BASE.tar.gz"
# One unified diff of the whole source against the upstream release.
DIFF="$OUT/lightcraft-filmscan-$VERSION-vs-$BASE.diff"
git -C "$ROOT" diff --binary "$BASE" HEAD >"$DIFF"
rm -rf "$OUT/patches" && mkdir -p "$OUT/patches"
git -C "$ROOT" format-patch -q --no-merges -o "$OUT/patches" "$BASE..HEAD"
tar -C "$OUT" -czf "$PATCHES" patches

git -C "$ROOT" tag -a "$TAG" -m "LightCraft Film Scan $VERSION, based on LightCraft $BASE"
# gh is the credential helper for this push only; the user's git config is left alone.
git -C "$ROOT" -c credential.helper= -c credential.helper='!gh auth git-credential' push "$REMOTE" "$BRANCH" "refs/tags/$TAG"
# The repository's front page shows this branch (and its README), not the copy of upstream's main.
gh repo edit "$FORK" --default-branch "$BRANCH" --enable-issues

assets=("$DEB" "$OUT/SHA256SUMS.txt" "$DIFF" "$PATCHES")
if compgen -G "$OUT/screenshots/*.png" >/dev/null; then assets+=("$OUT"/screenshots/*.png); fi
gh release create "$TAG" --repo "$FORK" --verify-tag --latest \
  --title "LightCraft Film Scan $VERSION (LightCraft $BASE + Convert Negative)" --notes-file "$NOTES" "${assets[@]}"
echo "==> https://github.com/$FORK/releases/tag/$TAG"
