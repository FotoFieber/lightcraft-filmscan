#!/usr/bin/env bash
# Build LightCraft Film Scan for macOS and Windows on GitHub and download the result.
#
#   packaging/fotofieber/fetch-builds.sh            # starts a run for HEAD, waits, downloads
#   packaging/fotofieber/fetch-builds.sh <run-id>   # downloads a finished run
#
# The workflow .github/workflows/fotofieber-build.yml runs on GitHub's macOS and Windows machines
# (free for this public repository) and publishes nothing. It builds the branch as pushed to the remote
# `fotofieber`, so HEAD must be pushed first; this script stops if it is not. The files land in
# dist/fotofieber/ beside the .deb, where publish.sh picks them up.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
FORK=FotoFieber/lightcraft-filmscan
REMOTE=fotofieber
WORKFLOW=fotofieber-build.yml
OUT="$ROOT/dist/fotofieber"
BRANCH="$(git -C "$ROOT" symbolic-ref --short HEAD)"
HEAD="$(git -C "$ROOT" rev-parse HEAD)"
mkdir -p "$OUT"

if [[ $# -ge 1 ]]; then
  RUN="$1"
else
  git -C "$ROOT" fetch -q "$REMOTE" "$BRANCH"
  pushed="$(git -C "$ROOT" rev-parse "$REMOTE/$BRANCH")"
  [[ "$pushed" == "$HEAD" ]] || {
    echo "error: $REMOTE/$BRANCH is $pushed, HEAD is $HEAD; push the branch first" >&2
    exit 1
  }
  before="$(gh run list -R "$FORK" -w "$WORKFLOW" -L 1 --json databaseId -q '.[0].databaseId // 0')"
  gh workflow run "$WORKFLOW" -R "$FORK" --ref "$BRANCH"
  RUN=""
  for _ in $(seq 30); do
    RUN="$(gh run list -R "$FORK" -w "$WORKFLOW" -L 1 --json databaseId,headSha \
      -q ".[0] | select(.databaseId != $before and .headSha == \"$HEAD\") | .databaseId")"
    [[ -n "$RUN" ]] && break
    sleep 5
  done
  [[ -n "$RUN" ]] || { echo "error: the workflow run did not show up" >&2; exit 1; }
  echo "==> https://github.com/$FORK/actions/runs/$RUN"
  gh run watch "$RUN" -R "$FORK" --exit-status --interval 60
fi

rm -f "$OUT"/*-macos-universal.zip "$OUT"/*-windows-x64.zip "$OUT"/BUILT_FROM-macos "$OUT"/BUILT_FROM-windows
# Only the packages: the per-architecture macOS binaries (macos-bin-*) are the workflow's own
# intermediate step, and their BUILT_FROM would overwrite the .deb's.
gh run download "$RUN" -R "$FORK" -n filmscan-macos -n filmscan-windows -D "$OUT/.download"
find "$OUT/.download" -type f -exec mv -f {} "$OUT/" \;
rm -rf "$OUT/.download"
for os in macos windows; do
  built="$(cat "$OUT/BUILT_FROM-$os")"
  [[ "$built" == "$HEAD" ]] ||
    echo "warning: the $os build is from $built, HEAD is $HEAD; publish.sh <upstream-tag> --check says whether that matters" >&2
done
ls -l "$OUT"/*-macos-universal.zip "$OUT"/*-windows-x64.zip
