#!/usr/bin/env bash
# Publish LightCraft Film Scan (the FotoFieber build with Convert Negative) as a GitHub release of
# FotoFieber/lightcraft-filmscan.
#
#   packaging/fotofieber/build-deb.sh
#   packaging/fotofieber/fetch-builds.sh           # optional: macOS and Windows, built on GitHub
#   packaging/fotofieber/screenshots.sh            # optional; check the images before publishing
#   packaging/fotofieber/release-notes.sh v0.21.1 > dist/fotofieber/notes.md   # read it
#   packaging/fotofieber/publish.sh v0.21.1
#   packaging/fotofieber/publish.sh v0.21.1 --check    # only the checks; tags, writes and pushes nothing
#
# Tags HEAD as <upstream-tag>-filmscan, pushes this branch and the tag to the remote `fotofieber`,
# and creates the release with the .deb, the macOS and Windows ZIPs when fetch-builds.sh got them,
# SHA256SUMS.txt, one diff against the upstream tag, the patch series and the screenshots. GitHub
# adds the source archives of the tag itself.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
BASE="${1:?usage: publish.sh <upstream-tag> [--check]}"
CHECK=0
[[ "${2:-}" == --check ]] && CHECK=1
FORK=FotoFieber/lightcraft-filmscan
REMOTE=fotofieber
OUT="$ROOT/dist/fotofieber"
VERSION="$(awk '/^\[/ { p = ($0 == "[workspace.package]"); next } p && $1 == "version" { gsub(/[" ]/, "", $3); print $3; exit }' "$ROOT/Cargo.toml")"
# The tag follows the version: 0.21.1+filmscan -> v0.21.1-filmscan, 0.21.1+filmscan.2 -> v0.21.1-filmscan.2.
TAG="$BASE-${VERSION#*+}"
DEB="$OUT/lightcraft-filmscan-$VERSION-linux-x86_64.deb"
NOTES="$OUT/notes.md"
BRANCH="$(git -C "$ROOT" symbolic-ref --short HEAD)"

[[ -f "$DEB" ]] || { echo "error: $DEB missing; run build-deb.sh" >&2; exit 1; }
[[ $CHECK == 1 || -s "$NOTES" ]] || { echo "error: $NOTES missing; run release-notes.sh $BASE > $NOTES and read it" >&2; exit 1; }
[[ -z "$(git -C "$ROOT" status --porcelain --untracked-files=no)" ]] || { echo "error: uncommitted changes" >&2; exit 1; }
(cd "$OUT" && sha256sum -c SHA256SUMS.txt)
# Every package must be built from the source being tagged. Only paths that reach no package may differ:
# - packaging/fotofieber: this release tooling, except README-package.md, which goes into every package;
# - the workflow that runs the macOS and Windows builds;
# - README.md: the packages carry README-package.md instead, so the repository README can change after a build.
# docs/ is not left out: docs/whats-new.md is compiled into the app (crates/ui-egui/src/dialogs.rs).
same_source() {
  [[ -n "$1" ]] && git -C "$ROOT" diff --quiet "$1" HEAD -- . ':(exclude)packaging/fotofieber' \
    ':(exclude).github/workflows/fotofieber-build.yml' ':(exclude)README.md' &&
    git -C "$ROOT" diff --quiet "$1" HEAD -- packaging/fotofieber/README-package.md
}
built="$(cat "$OUT/BUILT_FROM" 2>/dev/null || true)"
if ! same_source "$built"; then
  echo "error: $DEB was not built from this source (built from '${built:-?}'); rebuild" >&2
  exit 1
fi
# Public commits carry only the address chosen for this build.
if git -C "$ROOT" log --format='%ae%n%ce' "$BASE..HEAD" | grep -q '@fotofieber.ch' &&
   git -C "$ROOT" log --format='%ae%n%ce' "$BASE..HEAD" | grep '@fotofieber.ch' | grep -qv '^lightcraft@fotofieber.ch$'; then
  echo "error: a commit in $BASE..HEAD has a fotofieber.ch address other than lightcraft@" >&2
  exit 1
fi

# macOS and Windows builds from GitHub (fetch-builds.sh): only this version's, and only when built
# from the same source as the .deb. They join SHA256SUMS.txt.
others=()
for os in macos windows; do
  files=("$OUT"/lightcraft-filmscan-*"$VERSION"-"$os"-*.zip)
  [[ -e "${files[0]}" ]] || continue
  from="$(cat "$OUT/BUILT_FROM-$os" 2>/dev/null || true)"
  if ! same_source "$from"; then
    echo "error: the $os build was not built from this source (built from '${from:-?}'); run fetch-builds.sh" >&2
    exit 1
  fi
  others+=("${files[@]}")
done
if [[ $CHECK == 1 ]]; then
  echo "==> ok: $TAG can be published from HEAD with $(basename "$DEB")${others[*]:+ and ${#others[@]} more}"
  exit 0
fi
if [[ ${#others[@]} -gt 0 ]]; then
  (cd "$OUT" && sha256sum "$(basename "$DEB")" "${others[@]##*/}" >SHA256SUMS.txt)
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

assets=("$DEB" ${others[@]+"${others[@]}"} "$OUT/SHA256SUMS.txt" "$DIFF" "$PATCHES")
if compgen -G "$OUT/screenshots/*.png" >/dev/null; then assets+=("$OUT"/screenshots/*.png); fi
gh release create "$TAG" --repo "$FORK" --verify-tag --latest \
  --title "LightCraft Film Scan $VERSION (LightCraft $BASE + Convert Negative)" --notes-file "$NOTES" "${assets[@]}"
echo "==> https://github.com/$FORK/releases/tag/$TAG"
