# Publishing LightCraft Film Scan

We publish LightCraft Film Scan ourselves until Convert Negative is in an official LightCraft
release. After that, this fork keeps only what upstream does not have (Zap!, roll mode), or it stops.

Each release has three parts:

1. Analyse what upstream did with Convert Negative.
2. Compare upstream with our `filmscan` branch.
3. Write a summary and a recommendation, then build and publish.

Put each release's analysis in [publish-log.md](publish-log.md). Keep the newest entry first.

---

## 1. Analyse upstream

```sh
git fetch origin --tags; git fetch smyrgeorge; git fetch fotofieber
git remote add manueldeprada https://github.com/manueldeprada/lightcraft.git 2>/dev/null
git fetch manueldeprada
```

| Question | How to check |
|---|---|
| Is there a new upstream release? | `gh release list -R storytold/lightcraft --limit 3` |
| Is any part of the series in `main`? | `git log origin/main --oneline --grep 'Convert Negative'` and `git log v<last>..origin/main -- crates/film crates/pipeline/src/negative.rs` |
| Where do the PRs stand? | `gh pr view <n> -R storytold/lightcraft --json state,mergeable,headRefOid,reviewDecision,updatedAt` for #3985, #3986, #3987 (series 2/4–4/4) and #3623 (competing) |
| What do the reviews say? | Read the PR comments since the last release, especially those by maintainers. Bots and contributors only matter once their fixes reach a branch. |
| Are there fix branches outside the PRs? | Helpers sometimes push fixes to their own forks first. Today that is `manueldeprada/cr-2-pipeline`, `cr-3-engine` and `cr-4-panel`, each stacked on the one before. |
| Has a maintainer ruled on the series or on #3623? | Read the PR comments and their labels. |

Write down the head SHA of every PR and fix branch. The next comparison starts from them.

## 2. Compare with `filmscan`

Our branch holds four layers. Check each one on its own:

1. **Upstream release**: the last tag merged into `filmscan`. Check with `git describe --tags --abbrev=0 origin/main` against our version in `Cargo.toml`.
2. **Series 2/4–4/4**: our copy of #3985–#3987. Compare the new heads with the ones we last took in:
   ```sh
   git diff <old 4/4 head> <new 4/4 head or fix branch> --stat
   ```
   Do this only when both have the same merge-base with `origin/main`: `git merge-base <head> origin/main`. If the merge-bases differ, apply each one to a worktree of the same upstream tag first, as for `dc5c092a`. See the release memory for the steps.
3. **Our own work**: Zap! (`crates/pipeline/src/negative_frame.rs`, `crates/engine/src/negative_zap.rs`, `crates/ui-egui/src/negative_zap.rs`), roll mode, the Photo ▸ Film Scan menu, the tool strip button, German strings, the panel layout fix and the GPU routing fix.
   For each upstream change, ask whether it makes one of ours unnecessary, conflicts with it, or changes what Zap! relies on: the measurement, the crop and straightening, or the `Negative` fields.
4. **Fork identity and packaging**: the brand removal, the README, `LIGHTCRAFT_APP_ID`, `packaging/fotofieber`. These only change when upstream touches the same files.

Before taking in a delta, do a dry run:

```sh
git diff <old head> <new head> > $SCRATCH/delta.diff
git apply --check -v $SCRATCH/delta.diff      # which files conflict
git worktree add --detach $SCRATCH/wt HEAD && (cd $SCRATCH/wt && git apply --3way $SCRATCH/delta.diff)
```

Look at #3623 for ideas only. Never merge or copy its code: it conflicts with the series. We rebuild its ideas clean-room, and only when asked.

## 3. Summary, recommendation, release

Write a log entry with:
- the upstream state;
- what is new compared with `filmscan`;
- the risks;
- a recommendation: release now, take something in first, or wait.

Then follow the steps below.

**What a change after the build costs.** `publish.sh` only publishes packages built from the source being tagged,
apart from three paths that reach no package: `packaging/fotofieber/` (this tooling and the release notes, but not
`README-package.md`),
`.github/workflows/fotofieber-build.yml` and `README.md`. The packages carry `packaging/fotofieber/README-package.md`
as their README, a short install guide without version numbers or test results. So:
- `README.md`, the notes and the tooling can change after the build without a rebuild;
- `README-package.md` is in the packages: when you change it, build again (the check covers it);
- everything else, `docs/` too (`docs/whats-new.md` is compiled into the app), means building again.

Release facts (which builds were tested, from which version a platform exists) go in the release notes
(`release-notes.sh`), not in `README.md`. `publish.sh <tag> --check` runs every check and stops before it writes,
tags or pushes anything.

1. Merge the new upstream tag, if there is one. Set the version to `<x>+filmscan`; a rebuild on the same tag gets `+filmscan.N`. On a conflict in `Cargo.toml` or `Cargo.lock`, take theirs, set the version, then run `cargo update -w`.
2. Take in the series delta with `git apply --3way` and resolve the conflicts. Keep upstream's code, and our Zap! on top of it.
3. Check that `crates/gpu/src/render.rs` `linear()` still sends `lin_needs_cpu` to `lin_cpu`.
4. Finish the changes that go into the packages: the code, `docs/` and `README-package.md`. `README.md` can wait.
5. Run the tests:
   - `cargo xtask ci`. The one expected failure is `xtask version::tests::the_real_manifest_has_a_valid_version`.
   - `cargo test -p lightcraft-gpu --test it gpu_matches_cpu`.
   - The Zap! check on real scans (`zap_on_real_scans`, ignored by default).
6. Build: `packaging/fotofieber/build-deb.sh`. For macOS and Windows, push the branch to `fotofieber` (with the
   user's OK) and run `packaging/fotofieber/fetch-builds.sh`. It starts `.github/workflows/fotofieber-build.yml`
   on GitHub's macOS and Windows machines, waits about 40 minutes, and downloads the ZIPs to `dist/fotofieber/`.
   The two macOS architectures compile on two machines at once (`build-macos.sh --target`), then one job
   puts the universal app together (`--prebuilt`).
   The builds are unsigned: an Apple Developer ID or a Windows code-signing certificate costs money and is the
   user's decision. Film Scan builds of these two get their own settings folder, library and app id through
   `LIGHTCRAFT_FILMSCAN=1` (`crates/engine/src/flavor.rs`), because they have no launcher script.
7. Optionally, take screenshots: `screenshots.sh`.
8. Write the notes with `release-notes.sh <tag> > dist/fotofieber/notes.md`. Credit every source by hand: upstream, @smyrgeorge, the review helpers, us, and ideas from #3623. Add a sentence for users. Update what the maintainer has tried (`tested=` in `release-notes.sh`).
9. **Get the user's OK**, including on the screenshots. Changes to `README.md` and the notes from this review need no rebuild.
10. Run `git fetch fotofieber`, then `publish.sh <tag> --check`, then `publish.sh <tag>`. It attaches the macOS and Windows ZIPs when they were built from the same source as the .deb.

### Docs only (no release)

The repository's front page shows the `filmscan` branch, so a change to `README.md` needs no release and no build:

```sh
git commit …            # README.md only
git fetch fotofieber
git -c credential.helper= -c credential.helper='!gh auth git-credential' push fotofieber filmscan
```

To fix the text of a published release: `release-notes.sh <upstream-tag> <tag> > dist/fotofieber/notes.md`, edit,
then `gh release edit <tag> -R FotoFieber/lightcraft-filmscan --notes-file dist/fotofieber/notes.md`.

### When to stop

- **Parts 2–4 are merged upstream:** rebase our own work on that release. Drop our copy of the series. Publish only what upstream lacks.
- **Zap! and roll mode are upstream too:** retire the fork. Point users to the official release.
- **A maintainer picks #3623 instead:** stop, and decide with the user before doing anything else.
