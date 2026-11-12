#!/usr/bin/env bash
# Write the release notes of LightCraft Film Scan, the FotoFieber build with Convert Negative
# (Markdown, to stdout).
#
#   packaging/fotofieber/release-notes.sh v0.21.1 > dist/fotofieber/notes.md
#
# <upstream-tag> is the storytold/lightcraft release this build is based on. Every commit in
# <upstream-tag>..HEAD is listed by where it comes from: an open upstream pull request, or this fork.
# Screenshots in dist/fotofieber/screenshots/*.png are embedded from the release's assets.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
BASE="${1:?usage: release-notes.sh <upstream-tag>}"
FORK=FotoFieber/lightcraft-filmscan
UPSTREAM=storytold/lightcraft
TAG="$BASE-filmscan"
VERSION="$(awk '/^\[/ { p = ($0 == "[workspace.package]"); next } p && $1 == "version" { gsub(/[" ]/, "", $3); print $3; exit }' "$ROOT/Cargo.toml")"
git -C "$ROOT" rev-parse -q --verify "$BASE^{commit}" >/dev/null || { echo "error: no tag $BASE" >&2; exit 1; }

# Commits of the open upstream pull requests, matched by subject: "<subject prefix>|<PR>".
PRS=(
  "Convert Negative 2/4:|3985"
  "Convert Negative 3/4:|3986"
  "Convert Negative 4/4:|3987"
)

commits() { git -C "$ROOT" log --reverse --no-merges --format='%H%x09%ae%x09%s' "$BASE..HEAD"; }
line() { printf -- '- %s ([`%s`](https://github.com/%s/commit/%s))\n' "$3" "${1:0:8}" "$FORK" "$1"; }
first() { git -C "$ROOT" log --reverse --no-merges --format='%H' --grep="$1" "$BASE..HEAD" | head -1; }
short() { local h; h="$(first "$1")"; [[ -n "$h" ]] && printf '[`%s`](https://github.com/%s/commit/%s)' "${h:0:8}" "$FORK" "$h"; }

pr_of() {
  local s="$1" e
  for e in "${PRS[@]}"; do
    [[ "$s" == "${e%%|*}"* ]] && { echo "${e##*|}"; return 0; }
  done
  return 1
}

# Our commits by kind, from the subject.
kind_of() {
  case "$1" in
    "FotoFieber build:"* | "Release tooling"* | "Local build:"*) echo build ;;
    German:* | *"German labels"*) echo i18n ;;
    *Zap!* | *"Film Scan"* | *roll* | *"tool strip"*) echo zap ;;
    *) echo fix ;;
  esac
}

from_prs="" ours_fix="" ours_zap="" ours_i18n="" ours_build="" n_ours=0
while IFS=$'\t' read -r hash email subject; do
  if pr="$(pr_of "$subject")"; then
    from_prs+="$(line "$hash" "$email" "$subject") — [#$pr](https://github.com/$UPSTREAM/pull/$pr)"$'\n'
  else
    n_ours=$((n_ours + 1))
    l="$(line "$hash" "$email" "$subject")"$'\n'
    case "$(kind_of "$subject")" in
      fix) ours_fix+="$l" ;;
      zap) ours_zap+="$l" ;;
      i18n) ours_i18n+="$l" ;;
      build) ours_build+="$l" ;;
    esac
  fi
done < <(commits)

shots=""
if compgen -G "$ROOT/dist/fotofieber/screenshots/*.png" >/dev/null; then
  for f in "$ROOT"/dist/fotofieber/screenshots/*.png; do
    name="$(basename "$f")"
    case "$name" in
      1-the-negative-scan.png) caption="A camera scan of a 35 mm colour negative in its holder, as imported" ;;
      2-photo-film-scan-zap.png) caption="Photo ▸ Film Scan ▸ Zap!… on a roll: film, format, sprocket holes, edge, and each photo on its own or as one roll" ;;
      3-zapped-with-the-film-scan-panel.png) caption="After Zap!: the frame found, straightened, cropped and converted, with the Film Scan panel" ;;
      4-the-roll-converted.png) caption="The whole roll, each photo measured on its own" ;;
      5-measured-as-one-roll.png) caption="The same frame measured as one roll: one colour measurement for all six photos" ;;
      *) caption="${name%.png}"; caption="${caption#[0-9]*-}"; caption="${caption//-/ }"; caption="${caption^}" ;;
    esac
    shots+="**${caption}**"$'\n\n'"![${caption}](https://github.com/$FORK/releases/download/$TAG/$name)"$'\n\n'
  done
fi

cat <<EOF
**LightCraft Film Scan** is LightCraft **$BASE** with **Convert Negative**: film negative scans to positives, Zap! (find the frame,
straighten, crop and measure the film base in one step) and a roll mode. Version \`$VERSION\`.

This is **not an official LightCraft release**. It is based on
[LightCraft $BASE](https://github.com/$UPSTREAM/releases/tag/$BASE) by the ArtCraft team and the LightCraft
contributors (MIT OR Apache-2.0), plus the changes listed below. Report problems with this build
[here](https://github.com/$FORK/issues), not upstream.

## Download

| File | For |
|---|---|
| \`lightcraft-filmscan-$VERSION-linux-x86_64.deb\` | Debian, Ubuntu, Linux Mint, Zorin OS, Pop!_OS, ... on x86_64 with glibc ≥ 2.35 (Ubuntu 22.04 / Debian 12 or newer) |
| \`SHA256SUMS.txt\` | checksum of the .deb |
| \`lightcraft-filmscan-$VERSION-vs-$BASE.diff\` | the whole difference to LightCraft $BASE as one unified diff (\`git apply\` on a $BASE checkout) |
| \`lightcraft-filmscan-patches-$BASE.tar.gz\` | the same changes commit by commit, as \`git format-patch\` files |
| Source code (zip / tar.gz) | the full source of this build, tag \`$TAG\` |

Install: \`sudo apt install ./lightcraft-filmscan-$VERSION-linux-x86_64.deb\`. The package is called
\`lightcraft-filmscan\` and replaces an installed official \`lightcraft\` package (both install \`/usr/bin/lightcraft\`);
to go back, install the official package again. The app itself is still called LightCraft.
Built on Ubuntu 22.04 with Rust, HEIC/HEIF support and the craft-fonts CJK faces, like the official packages.

${shots:+## Screenshots

$shots}## Where the differences come from

All changes: [$UPSTREAM $BASE...$FORK $TAG](https://github.com/$UPSTREAM/compare/$BASE...FotoFieber:lightcraft-filmscan:$TAG).

### 1. Already in upstream LightCraft

- **Convert Negative 1/4** — \`lightcraft-film\`, the negative conversion maths
  ([#2490](https://github.com/$UPSTREAM/pull/2490), by [@smyrgeorge](https://github.com/smyrgeorge)), ported from
  his [Film Lab](https://github.com/smyrgeorge/film-lab) (MIT). Part of $BASE, but without a menu or panel.

### 2. From open upstream pull requests, by Yorgos S. ([@smyrgeorge](https://github.com/smyrgeorge))

Parts 2–4 of the series, not yet merged upstream: the settings and pipeline stage, the engine commands, and the
Photo menu entry with its panel.

$from_prs
### 3. Our own changes ($n_ours commits)

**Keeping parts 2–4 working on the current upstream**

$ours_fix
**Zap!, the Film Scan menu and roll mode**

$ours_zap
**German translations**

$ours_i18n
**This build** (version, branding, release tooling)

$ours_build
### 4. Ideas taken from other work (reimplemented, no code copied)

- [#3623](https://github.com/$UPSTREAM/pull/3623) "LR-EDIT-NEGATIVE" by [@mcalapurge](https://github.com/mcalapurge)
  (open upstream):
  - its **Roll sync** idea (pool the measurement of several frames, and treat a frame whose film base differs by
    more than 15 % as scanned apart) → our **As One Roll** option of Zap!… ($(short 'as one roll'));
  - measuring the **film base on the rebate or holder anywhere around an uncropped camera scan** → how Zap!
    picks the film base ($(short 'Zap! finds the frame')).
- [Film Lab](https://github.com/smyrgeorge/film-lab) by @smyrgeorge (MIT): the conversion maths, through #2490 (above).

## Not changed

Everything else is LightCraft $BASE as released: see its
[release notes](https://github.com/$UPSTREAM/releases/tag/$BASE). The in-app update check still reports official
releases.
EOF
