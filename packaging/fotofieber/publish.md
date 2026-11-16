# Publishing LightCraft Film Scan

We publish LightCraft Film Scan ourselves until Convert Negative is in an official LightCraft
release. After that, this fork keeps only what upstream does not have (Zap!, roll mode), or it stops.

Each release has three parts:

1. Analyse what upstream did with Convert Negative.
2. Compare upstream with our `filmscan` branch.
3. Write a summary and a recommendation, then build and publish.

Put each release's analysis in the log at the end of this file. Keep the newest entry first.

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

1. Merge the new upstream tag, if there is one. Set the version to `<x>+filmscan`; a rebuild on the same tag gets `+filmscan.N`. On a conflict in `Cargo.toml` or `Cargo.lock`, take theirs, set the version, then run `cargo update -w`.
2. Take in the series delta with `git apply --3way` and resolve the conflicts. Keep upstream's code, and our Zap! on top of it.
3. Check that `crates/gpu/src/render.rs` `linear()` still sends `lin_needs_cpu` to `lin_cpu`.
4. Run the tests:
   - `cargo xtask ci`. The one expected failure is `xtask version::tests::the_real_manifest_has_a_valid_version`.
   - `cargo test -p lightcraft-gpu --test it gpu_matches_cpu`.
   - The Zap! check on real scans (`zap_on_real_scans`, ignored by default).
5. Build: `packaging/fotofieber/build-deb.sh`. For macOS and Windows, push the branch to `fotofieber` (with the
   user's OK) and run `packaging/fotofieber/fetch-builds.sh`. It starts `.github/workflows/fotofieber-build.yml`
   on GitHub's macOS and Windows machines, waits about 30–60 minutes, and downloads the ZIPs to `dist/fotofieber/`.
   The builds are unsigned: an Apple Developer ID or a Windows code-signing certificate costs money and is the
   user's decision. Film Scan builds of these two get their own settings folder, library and app id through
   `LIGHTCRAFT_FILMSCAN=1` (`crates/engine/src/flavor.rs`), because they have no launcher script.
6. Optionally, take screenshots: `screenshots.sh`.
7. Write the notes with `release-notes.sh <tag> > dist/fotofieber/notes.md`. Credit every source by hand: upstream, @smyrgeorge, the review helpers, us, and ideas from #3623. Add a sentence for users.
8. **Get the user's OK**, including on the screenshots.
9. Run `git fetch fotofieber`, then `publish.sh <tag>`. It attaches the macOS and Windows ZIPs when they were built from the same source as the .deb.

### When to stop

- **Parts 2–4 are merged upstream:** rebase our own work on that release. Drop our copy of the series. Publish only what upstream lacks.
- **Zap! and roll mode are upstream too:** retire the fork. Point users to the official release.
- **A maintainer picks #3623 instead:** stop, and decide with the user before doing anything else.

---

## Log

### 2026-11-16: v0.21.1-filmscan.7 published (first Windows and macOS builds)

**Upstream**
- Not checked again for this release. It was a packaging release on the same upstream v0.21.1 as `.6`, built the
  same day.

**New compared with v0.21.1-filmscan.6** (10 commits, all ours):
- Zap! speed: the sample grid is measured in parallel (`5d51127a`), and a frame's crop is cut out once
  (`9eb444c4`).
- Windows x64 and macOS universal builds on GitHub (`4eeb1226`): `.github/workflows/fotofieber-build.yml` and
  `fetch-builds.sh`. `LIGHTCRAFT_FILMSCAN=1` gives them their own settings folder, library and app id
  (`crates/engine/src/flavor.rs`).
- The programs are named like the Linux commands: `lightcraft-filmscan.exe`, `lightcraft-filmscan-cli.exe` and,
  on macOS, `lightcraft-filmscan-cli` (`422e0741`, `e1751ec9`). The macOS app is `LightCraft Film Scan.app`.
- README and release notes: Windows and macOS downloads with install steps, and "use at your own risk"
  (`4d203ee3`, `aabe7e56`, `419ec87e`).
- The macOS job runs on `macos-15-intel` (`e66969a9`).
- Version `0.21.1+filmscan.7` (`de6a89c2`). All packages are built from `419ec87e`.

**Testing**
- `cargo xtask ci --quick` passed, apart from the known version test.
- The user tried the Windows build: it works well.
- Nobody has started the macOS app. On GitHub, only the command-line tool was started, on an Intel Mac. The README
  and the notes say the macOS build is not tested by the maintainer.

**What went wrong on the way**
- The first push of the workflow file was refused: the gh token lacked the `workflow` scope. Fixed with
  `gh auth refresh -h github.com -s workflow`.
- `macos-15` (Apple silicon) got no runner twice: the job was cancelled after 15 minutes with "not acquired by
  Runner". `macos-15-intel` got one at once. Its build takes about 50 minutes, against about 20 for Windows.
- The ZIPs contain `README.md`. A README change after a build means building again: `publish.sh` refuses
  packages whose source differs outside `packaging/fotofieber` and the workflow file.

### 2026-11-16: state before v0.21.1-filmscan.6

**Upstream**
- Still v0.21.1 (2026-11-12). `main` has 133 commits since then, none on Convert Negative or `crates/film`. They are mostly the craft-libs cut-over (TIFF/JPEG/PSD/print).
- #3985 (`2404f941`, mergeable), #3986 (`a529bdb2`) and #3987 (`a9d3c229`) are unchanged since 2026-11-13. #3986 and #3987 are conflicting. There is no review and no comment since then.
- @manueldeprada's `cr-2-pipeline` (`f84a8499`), `cr-3-engine` (`4c800191`) and `cr-4-panel` (`bc02c044`) are unchanged. We took them in with `c0a0d408`, and @smyrgeorge hasn't yet.
- #3623 (`45b866f8`) is open and mergeable, with no review. It only merged `main` (2026-11-13 to 15): no new ideas.
- No maintainer has ruled on the series or on #3623.

**New compared with the published v0.21.1-filmscan.2** (12 commits, all ours or already taken in):
- @manueldeprada's fixes to the series (`c0a0d408`), and Zap! measuring a turned frame straightened.
- Zap! is only Photo ▸ Film Scan ▸ Zap!… (`3920d313`).
- B&W Negative, and "Then Zap!…" in the import review (`b7cda261`).
- The rebate is found where the film fills the scan (`e2c6bf8d`).
- As One Roll, the frames agree on their crop (`6071bd2d`).
- Speed (`31617013`, `58de1617`, `b6898ae2`):
  - Zap! on a selection works on up to 4 photos at once: 24 scans of 61 MP take 17.6 s instead of 9.0 s.
  - A final render of a negative is ~0.6 s faster.
  - Results are byte-identical on the 330-scan eval, single and roll.

**Risks**
- The parallel Zap! is new. Its tests pass and the eval is identical, but nobody has used it in the app yet: check the activity row, Cancel and the single undo step.
- `b6898ae2` changes upstream code (the colour noise model). That is a possible merge conflict later, and a candidate PR upstream.
- The series is still unreviewed upstream, so its hashes may change again.

**Recommendation**
- Release now as `v0.21.1-filmscan.6`. Nothing upstream is waiting to be taken in.
- The .3–.5 builds were never published but were installed locally, so the number goes past them.
- Credit @manueldeprada in the notes for the `cr-*` fixes.

### 2026-11-14: could AI improve Zap!'s crop? (study, no code change)

**Method**
- The 330 truth crops were checked against Zap! filmscan.5 (single mode: 106/141/171 within 1/2/4 %), with overlays of the worst rolls.
- A temporary candidate dump gave an oracle: the best of Zap!'s own edge candidates per side.

**The 159 misses over 4 %**

| Count | Cause | Zap! error? |
|---|---|---|
| ~85 | The user kept the sprocket holes or the film edge (Sommer every frame, PhoenixII and Kentmere partly) | No: a margin choice |
| ~22 | Composition crops inside the frame | No |
| ~23 | Slides | |
| ~9 | Zuerichsee: the user turned the photo, so its truth is in other coordinates | No |
| ~25–30 | A scene edge taken for the frame (dark sky or night, RPX/Kentmere, GS1), or the film fills the scan | Yes |

**Oracle**
- Picking the best existing candidate per side gives 114/149/181, only +8/+8/+10 over Zap!.
- So a learned scorer over the existing candidates (the cheap route) can gain at most ~3 % of photos.

**Existing models**
- SAM 3 is in-tree, but it is gated, 3.4 GB, under a licence outside the allowlist, and runs on the CPU on Linux. It segments objects, not a camera gate against a dark scene.
- Generic edge detectors find the edges Zap! already has.
- None addresses the real failure.

**Own CNN**
- The real errors are ~30 photos, too few to train or to validate an edge model.
- It would need several hundred checked frame crops (frame crops, not composition crops) from varied rolls.

**Recommendation**
- No AI for now.
- The bigger lever is the margin: let Zap! keep the holes when the user wants it (per roll, remembered), and keep the truth clean (a flag for composition crops).
- Revisit a small CNN once ≥ 300 checked frame crops exist.
### 2026-11-14: scheduled workflows off on the fork

- FreeBSD, Release and Windows ARM64 are disabled on `FotoFieber/lightcraft-filmscan` with `gh workflow disable`.
- They ran upstream's nightly checks on `filmscan` every day and failed only because of the `+filmscan` version: the xtask version test, and Release's "Resolve version" in its dry run.
- Releases of this fork are built with `build-deb.sh`.
- `gh workflow enable <name> -R FotoFieber/lightcraft-filmscan` turns one back on.

### 2026-11-14: local build 0.21.1+filmscan.5 (not published)

- `e2c6bf8d`: Zap! finds the rebate on a scan the film fills. The base comes from between the sprocket holes, else from an even, narrow edge strip.
- `6071bd2d`: Zap! "As One Roll" makes the frames agree on their crop. A frame keeps its own edge where the roll's would reach onto blank film. It also fixes a pooled-measurement error on mixed selections.
- Scored against the user's crops with `corpus/zap-1114/tools/` (`truth.py`, `score_eval.py`, ignored test `zap_eval_on_truth`).
- Results, within 1/2/4 %:
  - B&W roll: 6/9/30 → 19/25/34 (19/26/35 as one roll).
  - Holdout: 79/101/117 → unchanged, 83/107/125 as one roll.
  - Tune: unchanged.
- `ci --quick`: the expected version test fails. `layers`, `assets`, `roots` and `parity` pass.

### 2026-11-14: local build 0.21.1+filmscan.4 (not published)

- `3920d313`: Zap! is only Photo ▸ Film Scan ▸ Zap!… (the command keeps no menu of its own).
- `b7cda261`, two changes:
  - Zap!'s Film choice gains B&W Negative. It uses the Monochrome profile, and the base still comes from the rebate, so XP2's coloured base turns black.
  - The import review has "Then Zap!…" in every mode.
- Tests:
  - `ci --quick`: the expected version test fails.
  - `find_similar::stopping_leaves_the_view_alone…` failed under full load and passed on its own. It is known to be flaky.
  - `layers`, `assets`, `roots` and `parity` pass.

### 2026-11-14: local build 0.21.1+filmscan.3 (not published)

- `c0a0d408` takes in @manueldeprada's `cr-2-pipeline`, `cr-3-engine` and `cr-4-panel` fixes against `a9d3c229`.
  - The two conflicts in `crates/engine/src/cmd/negative.rs` were resolved: we kept the "Film Scan" undo label and their section-enable change.
  - One clippy fix in their test: `field_reassign_with_default` under Rust 1.99.
- Zap! now measures a turned frame straightened. Measure Again and Zap! share `negative_zap::measure_in`, so they read the same pixels.
  - The test `zap_measures_a_turned_frame_as_measure_again_does` fails without this change: the old Zap! read the rebate as the picture's black.
- Tests:
  - `cargo xtask ci --quick`: only the expected version test fails.
  - `layers`, `assets`, `roots` and `parity` pass.
  - `gpu_matches_cpu` passes.
  - Zap! on the London scans runs without errors.
- Wait for the user's test before publishing. When the PR author takes the fixes in, check whether their hashes changed and credit @manueldeprada.

### 2026-11-14: state after v0.21.1-filmscan.2

**Upstream**

- The latest release is v0.21.1 (2026-11-12). `main` has 44 commits since then, none on Convert Negative, and the version is still 0.21.1.
- Only part 1/4 (#2490, `crates/film`) is merged.
- #3985, #3986 and #3987 (by @smyrgeorge) are open. They have no maintainer review, and GitHub shows their merge state as unknown.
  - Their heads have not moved since the review round we took in: `2404f941`, `a529bdb2` and `a9d3c229`.
  - The activity on 2026-11-13 was CodeRabbit and @manueldeprada.
- New since our release: @manueldeprada fixed the valid CodeRabbit findings on stacked branches in his fork. He asked @smyrgeorge to take them in, which hasn't happened yet.
  - `cr-2-pipeline` `f84a8499`:
    - preset scaling starts from the merged settings;
    - a missing `balance` means 1;
    - the blank canvas of a warp is left alone.
  - `cr-3-engine` `f80c0dc6` and `4c800191`: the picker clamps each pixel before averaging, and keeps the image-size radius.
  - `cr-4-panel` `5216dce2`, `0d3be5fd` and `bc02c044`:
    - switching on or picking turns the section back on;
    - picks map to the unwarped scan;
    - a picked gray stays gray;
    - the measurement follows a straightened crop;
    - the menu shows a hidden conversion.
  - The whole delta: 13 files, +225 −28, on the same merge-base as the PR heads (`a75497d6`).
- #3623 (@mcalapurge) is open and mergeable. It merges `main` every day and has no review.

**Compared with `filmscan` (`54406b49`)**

- We hold v0.21.1 and the series exactly at the PR heads above. Upstream has nothing on Convert Negative that we lack, apart from @manueldeprada's fix branches.
- The fix delta applies cleanly to all files except `crates/engine/src/cmd/negative.rs`. That file has two conflicts with our Zap! refactor:
  - our `measure_scan` calls `measure_in`, which Zap! shares;
  - their change adds a straightened-crop branch to it.
  - Both are small. Keep their branch inside `measure_scan`, and keep `measure_in` for Zap!.
- Check the behaviour too:
  - Zap! straightens and then measures through `negative_zap::measure_in` on the crop's box. After the fix, a manual Measure Again on a turned crop samples the straightened crop, but Zap! still measures the unstraightened box.
  - Decide whether Zap! should use the same path: `frame_for` + `sample`.
- About 5,950 lines differ from v0.21.1. About 2,200 of them are Zap! alone, which exists only here.

**Recommendation**

1. **Don't publish today just for these fixes.** They are correctness fixes on top of the PRs, not yet accepted by the PR author. Their hashes may change again when @smyrgeorge rebases.
2. **Watch #3985–#3987.** Once @smyrgeorge takes in the `cr-*` fixes, or a maintainer reviews the PRs, take in the delta against `a9d3c229`, as described above, and release it as `v0.21.1-filmscan.3`.
3. **Do the next upstream release first.** If upstream tags v0.21.2 or v0.22.0 before step 2, merge the tag first. If the fixes are still only on @manueldeprada's branches by then, take them in together and credit him. That gives one release instead of two.
4. **Keep Zap! here for now.** Offer it upstream (as a 5/4 PR or an issue) only after 2–4 are merged. Before that it would stack on unmerged work and add review load. Whether to offer it at all is the user's decision.
5. When taking in the fixes, also align Zap!'s measurement on turned crops with the new straightened path, and measure it on the Zap! evaluation set before releasing.
