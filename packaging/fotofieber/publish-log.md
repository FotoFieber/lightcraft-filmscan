# LightCraft Film Scan: release log

One entry per release, check or study, newest first. The procedure is in [publish.md](publish.md).

## 2026-11-28: study, crops and angle on 202508BW (no code change)

**Crops** (the maintainer's, now in the truth file: 355 photos, set `bw0825`)
- `.15` against them: 18 of 25 within 1 %, all within 2 %. They are about 0.7 % wider at the top and 0.5 % at the
  bottom than Zap!'s, in the path that goes by the sprocket holes.

**Angle**
- Zap!'s angle (the holder's edge) is a median 0.28 degrees off on this roll. Auto Straighten, run after Zap!, gives
  the maintainer's angle on 22 of 25 within 0.02 degrees; they used that tool, so this roll alone proves little.
- The other 326 zapped truth photos: on the 50 whose angle the maintainer changed, Auto Straighten is closer than
  Zap! (median 0.24 against 0.42 degrees, within 0.25: 26 against 13). On the 276 where they kept Zap!'s angle, it
  differs by a median 0.24 degrees, by more than 1 degree on 23.

**Recommendation**
- Offer straightening by the scene as a choice in Zap!, falling back to the frame's angle where the two differ by more
  than 1 degree; keep the frame's angle as the default. Narrow the gap to the holes in the thin-film path.

## 2026-11-28: local build 0.21.1+filmscan.15 (not published)

**Problem**
- On the roll 202508BW (25 B&W 35 mm camera scans, copied without sidecars to `corpus/zap-202508BW/`), Zap! in `.14`
  found no frame on 14 scans and cropped the other 11 to the lower part of the picture.
- Cause: film counts as film above 8 % of the light through the holes. These scans are thin and fogged along one edge
  (a light leak): 0.03 to 0.12 on the fogged rebate, 0.17 to 0.56 on the other, a median of 0.05 to 0.10 in the
  picture, 0.0003 on the holder. So the denser film passed for holder.

**What**
- `c7a7ce9f`: where no frame, or only one row of holes, is found, `detect` looks again for film down to 2 % and takes
  that when it shows a larger strip with both rows. There the holes are read against their own light, the base is the
  thinner rebate of the two rows, the long sides go just inside the holes and light past the film's end is trimmed.
  New synthetic test. Version `.15` (`001788a2`), installed as `~/.local/bin/lightcraft-cn`. No .deb, nothing pushed.

**Testing**
- 202508BW: a frame on 25 of 25, checked on contact sheets. No crop truth for this roll yet.
- 330 truth crops, single and roll: within 1/2/4 % unchanged. Only the GS1 135 sprocket folder changed (4 photos that
  were not zapped now are; 5 better and 1 worse by more than 0.5 %, the worse one in roll mode, beyond 4 % both times).
- London scans: the same output.
- As one roll, the 25 scans convert darker and richer than each on its own.
- `cargo xtask ci`: only the known version test failed; parity, layers, assets, roots, wasm and `gpu_matches_cpu`
  passed.

**Open**
- The maintainer's crops for 202508BW, then add them to the truth file.

## 2026-11-27: local build 0.21.1+filmscan.14 (not published)

**What**
- The maintainer tested `.13` and found it good: `filmscan` was fast-forwarded to `trial/upstream-20261126`.
- Merged upstream `main` at `3249d213` (CI green; `f57b8b4e`), version `.14` (`b62bde4d`). 169 commits (90 without
  merges) after `f778f0e0`: export subfolder variables, the HSL Color Mixer, one mask overlay rule, Upright fixes, a
  Settings filter, import of photos browsed in Local, window placement, looping smart albums, library paths that differ
  in letter case.
- 5 files conflicted, all kept both sides: a test in `tests_geometry.rs`, `de.json`, `de-formats.json`,
  `slider_named` in `widgets.rs` (upstream's spoken name, else our translation context for the negative sliders) and
  the parity summary (rewritten with `cargo xtask parity --write`).
- Catalog format stays 14. `Cargo.lock` keeps `rustls-rustcrypto 0.1.0-pre.0` and `rustls-webpki 0.103.16`.
- Built with `--features heif` and the pinned craft-fonts, installed as `~/.local/bin/lightcraft-cn`. No .deb, nothing
  pushed.

**Testing**
- `cargo xtask ci`: fmt, clippy and heif passed; `test` failed only at the known version test. parity, layers, assets,
  roots, wasm and the 3 `gpu_matches_cpu` tests passed. 170 tests skipped their checks.
- Zap! on the 330 truth crops, single and roll: byte-identical to `.13`. `zap_on_real_scans` on the London scans: the
  same output.
- London DNGs, Zap! and a 900 px export, `.13` against `.14`: all 6 identical.
- Headless snapshot: Photo > Film Scan, the Film Scan panel and the two flip entries of the filmstrip's context menu
  are there.

**Upstream**
- Still v0.21.1. #3985 (`2404f941`), #3986 (`a529bdb2`), #3987 (`a9d3c229`) and #3623 (`b0797a9b`): heads unchanged,
  no ruling. @manueldeprada may push to the series' branches since 2026-11-25 and has not yet. Our #4504 is open.
- Draft #4750 ports the whole UI to the shared Craft framework. If it is merged, our panel, menu rows, Zap! dialog and
  tool strip button have to be rebuilt.
- New `.github/workflows/notify-services.yml` runs on release events, in the fork too, where its secret is missing.
  Disable it there before the next release (needs the maintainer's OK).

**Recommendation**
- No release now. Try `.14`, then publish it as the successor of `.10`.

## 2026-11-26: local build 0.21.1+filmscan.13 (not published)

**What**
- On `trial/upstream-20261126`: Flip Horizontal and Flip Vertical in the photo context menu (filmstrip, grid, loupe,
  compare), below the two rotate entries (`a5ae8df7`). Version `.13` (`3cbef880`).
- Built with `--features heif` and the pinned craft-fonts, installed as `~/.local/bin/lightcraft-cn`.

**Testing**
- `cargo xtask ci --quick`: fmt, clippy and heif passed; `test` failed only at the known version test. The steps
  after `test` were not run.
- Headless snapshot: a right-click on the filmstrip shows both entries; Flip Horizontal mirrors the photo. Flip
  Vertical was not clicked.

## 2026-11-26: trial build 0.21.1+filmscan.12 (not published)

**What**
- On `trial/upstream-20261126`: upstream `main` at `f778f0e0` (the newest commit with a green CI run at the time)
  merged (`a05f4193`), version `.12` (`49471700`; the merge's message names it too).
- 13 files conflicted. Decisions: the app id keeps `LIGHTCRAFT_APP_ID` and the flavor's id, with upstream's channel id
  for Nightly; presets use upstream's `blend_base`, which includes `to_json_for_merge`; `crop_dims` keeps
  `negative_zap::oriented_dims` (same result as upstream's new body); the captured import list and "Then Zap!…" are
  both kept; the Film Scan menu rows sit beside upstream's move of Capture Time and Tracklog to Photo > Metadata.
- Built with `--features heif` and the pinned craft-fonts, installed as `~/.local/bin/lightcraft-cn`. No .deb, nothing
  pushed.

**Testing**
- `cargo xtask ci`: fmt, clippy and heif passed; `test` failed only at the known version test (run just before the
  version bump). parity, layers, assets, roots, wasm and the 3 `gpu_matches_cpu` tests passed. 170 tests skipped
  their checks (corpus and models absent).
- Zap! on the 330 truth crops, single and roll: the same crops as `.11`. `zap_on_real_scans` on the London scans:
  the same output.
- London DNGs, Zap! and a 900 px export, `.11` against `.12`: not identical (mean difference below 0.9 levels, up to
  96 on single pixels, 2 to 20 % of pixels off by more than 2, mostly at edges). They look the same side by side.
- `lightcraft-cli render` of a DNG with a `.11` sidecar works (mean difference 0.9 levels).
- Photo > Film Scan has its three commands; the panel view matches `.11`.
- A copy of a 91,026-photo library opens: upgraded to catalog format 14, the old one kept beside it as
  `catalog.snap.v13-<seq>`.

## 2026-11-26: state after local build 0.21.1+filmscan.11 (no release)

**Upstream**
- Still v0.21.1 (2026-11-12). `main` moved fast on 2026-11-26: 730 commits (merges included) between `ddc0eef3`, the
  state merged for `.11`, and `02060784`. The craft-libs pin is unchanged (`b2d2a1e9`).
- Relevant for us: raw decoding keeps negative channels (#2278) and pushed shadows render differently (#2092); the
  menus have new `LAYOUT`/`SUBMENUS`/`HIDDEN` tables (#2194); catalog format 13 → 14. Nothing new in `crates/film`
  or the negative files.
- #3985 (`2404f941`), #3986 (`a529bdb2`), #3987 (`a9d3c229`): heads unchanged. On 2026-11-25 @smyrgeorge allowed
  @manueldeprada to push to the branches, so expect movement. #3985 conflicts with `main`.
- @manueldeprada's `cr-2-pipeline` (`f84a8499`), `cr-3-engine` (`4c800191`), `cr-4-panel` (`bc02c044`): unchanged,
  already in `filmscan`.
- #3623 (`b0797a9b`): open, conflicting, no ruling. Our #4504 (Pillow) is still open.

**Compared with `filmscan`**
- `filmscan` is at `f5924da7`; `.11` is not pushed. The published release is `.10`.
- Changed line ranges overlap in about 16 places in 13 files, plus `Cargo.lock`.

**Recommendation**
- No release. Do a trial merge now on its own branch, test it, and install it as a local build.

## 2026-11-24: local build 0.21.1+filmscan.11 (not published)

**What**
- Merged upstream `main` at `ddc0eef3` (`d022a152`) without conflicts; version `0.21.1+filmscan.11` (`f5924da7`).
  `Cargo.lock` keeps `rustls-rustcrypto 0.1.0-pre.0` and `rustls-webpki 0.103.16`.
- Built with `--features heif` and the pinned craft-fonts. No .deb, nothing pushed.

**Testing**
- `cargo xtask ci`: fmt, clippy and heif passed. `test` failed; a rerun of the workspace tests showed only the known
  version test. The corpus was mostly absent, so 160 tests skipped their checks.
- parity, layers, assets, roots and wasm passed one by one (wasm with warnings in `ui-egui`). The GPU equivalence
  tests (`gpu_matches_cpu` filter, 3 tests) passed.
- `zap_on_real_scans` found the frame on all 6 London scans. `LC_ZAP_DIR` must be an absolute path: with a relative
  one the test passes without looking at any scan.
- London DNGs, `develop.negativeZap` and a 900 px PNG export with the `.10` and `.11` CLI: all 6 byte-identical.
- Heal on a converted negative (one London scan): GPU and CPU exports differ by at most 1 level. A spot in plain sky
  looks seamless in `.10` and `.11`. A large spot across hard edges smears in both.

## 2026-11-24: state after v0.21.1-filmscan.10 (no release)

**Upstream**
- Still v0.21.1 (2026-11-12); `main` is at `ddc0eef3` and still says `0.21.1`. It has 31 commits (without merges)
  after `94219de0`, the state merged for `.10`: the pinned Heal (process V14, `spots::pinned`, `docs/heal-pinned.md`),
  Remove/Crop mode dropdowns (`labeled_dropdown`), balanced demosaic on the white-balanced mosaic (#1328, craft-libs
  `b2d2a1e94`) and app-drop teardown fixes. Nothing in `crates/film`, the negative or Zap! files.
- Only `crates/gpu/src/render.rs` changes (+8 lines): `spots()` reads the image back and runs the heal on the CPU for
  V14 photos with spots. Our `lin_needs_cpu_stage` routing sits elsewhere in the file and is not in upstream.
- #3985 (`2404f941`), #3986 (`a529bdb2`), #3987 (`a9d3c229`): unchanged, open, `CONFLICTING`, no comment or review
  since the last entry. #3623 (`b0797a9b`): unchanged, `CONFLICTING`, no ruling.
- @manueldeprada's `cr-2-pipeline` (`f84a8499`), `cr-3-engine` (`4c800191`), `cr-4-panel` (`bc02c044`): unchanged,
  already in `filmscan`.

**Compared with `filmscan`**
- `filmscan` is at `64b27887` and equals `fotofieber/filmscan`. The tag `v0.21.1-filmscan.10` is on an earlier
  commit (the build commit); only log entries came after it. Nothing unpublished apart from this log.
- A trial merge of `origin/main` (`git merge-tree`) is clean: no conflicts.
- Series layer, Zap!, roll mode and packaging: no overlap with the upstream changes.

**Risks**
- Process V14 changes how Heal and Remove spots render for new edits. Our negative step runs before spots, so the
  conversion is unaffected, but a negative with spots goes through the GPU readback; check one such frame.
- The demosaic change (#1328) alters raw renders slightly. Compare the London DNGs before and after, as on 2026-11-18,
  because Convert Negative amplifies small differences.
- The craft-libs pin moves again, so the build needs the network for the new rev.

**Recommendation**
- Wait. No new tag, the series is unchanged, and we have nothing new for users since `.10`.
- Merge `main` when upstream tags the next release (or when we need a fix from it). Expect no conflict now; check
  `Cargo.lock` for `rustls-rustcrypto 0.1.0-pre.0` afterwards.

## 2026-11-23: v0.21.1-filmscan.10

- Contents: `.9` plus upstream main `94219de0` (merged without conflicts) and one own change: while the Film Base or
  Gray eyedropper is armed the Develop view shows the whole scan (the crop tool's geometry), so the rebate outside
  the crop can be picked. Test: `the_selector_picks_outside_the_crop`; it fails without the change.
- Upstream, the PRs #3985-#3987 and #3623: unchanged since the entry below.
- Checks: `cargo xtask ci` green apart from the known version test; layers, assets, parity, roots and the GPU
  equivalence test pass. The Zap! check on real scans was not rerun (no Zap! code changed).
- Built: .deb locally; Windows and macOS on GitHub from `2e860474`. The maintainer tested the Windows build; macOS is untested.

## 2026-11-23: state after v0.21.1-filmscan.9 (no release)

**Upstream**
- Still v0.21.1 (2026-11-12); `main` is at `94219de0` and still says `0.21.1`. It has 36 commits (without merges) after
  `c17badc6`, the state merged for `.9`. They are #523 (Process V13: global Highlights and Shadows ease off at large
  amounts, Texture, grading wheel), craft-libs pins (`33be356`, then `2be967e`), SQLite/WAL record boundaries,
  extracting the #587 modules into craft-develop (#4656), and CI changes (nightly prereleases, then reverted in #4699).
  None touches `crates/film`, the negative or Zap! files, or `crates/gpu/src/render.rs`.
- #3985 (`2404f941`), #3986 (`a529bdb2`) and #3987 (`a9d3c229`) are unchanged since 2026-11-13: open, merge state
  unknown, no maintainer ruling. #3985's `updatedAt` moved on 2026-11-22 without a new comment.
- @manueldeprada's `cr-2-pipeline` (`f84a8499`), `cr-3-engine` (`4c800191`), `cr-4-panel` (`bc02c044`) are unchanged
  and already in `filmscan`.
- #3623 (`b0797a9b`) is unchanged since 2026-11-16 and now conflicts with `main`. No ruling.

**Compared with `filmscan`**
- `filmscan` is at `1867fe19` (UnZap!) and equals the tag `v0.21.1-filmscan.9` and `fotofieber/filmscan`: nothing
  unpublished.
- A trial merge of `origin/main` (`git merge-tree`) conflicts in one file, `crates/pipeline/src/local.rs`. Upstream
  moved about 1,300 lines of it into craft-develop. Our 24 added lines sit in the region that moved: the call
  `negative::apply_in` in `frame_airlight` and the test `the_airlight_proxy_of_a_negative_is_the_positive`.
  `frame_airlight` is still in `local.rs` upstream (line 260), so the resolution is to take upstream's file and
  re-add those two hunks. `Cargo.lock` and `Cargo.toml` merged by themselves, but upstream's lock still has
  `rustls-rustcrypto 0.0.2-alpha` and `rustls-webpki 0.102.8`: check for them after the merge.
- Also touched by the merge: the craft-libs pin moves from `52d2560c` to `2be967ec`, so the build needs the network
  for the new rev, and `crates/pipeline` now depends on craft-develop.

**Risks**
- The V13 process work changes how Highlights and Shadows render for new edits. The merge does not touch our
  negative step, but the London renders should be compared before and after, as on 2026-11-18.
- The airlight proxy of a negative depends on the moved code: the test above must pass after the merge.

**Recommendation**
- Wait. No new tag, the series is unchanged, and we have nothing new for users since `.9`.
- When upstream tags the next release, merge it then. Expect the one `local.rs` conflict described above.

## 2026-11-20: local build 0.21.1+filmscan.9 (not published)

**What**
- Merged upstream `main` at `c17badc6` (`8ea02895`): 100 commits after `27b755a9`. The one conflict was
  `assets/ATTRIBUTION.md`; we kept upstream's craft-fonts row (now with Noto Sans Hebrew in the web build) and left
  out the `docs/brand/` row, whose logos this fork removes. `Cargo.lock` merged by itself and keeps
  `rustls-rustcrypto 0.1.0-pre.0`. Upstream replaced `AGENTS.md` with the shared Craft rules.
- The Convert Negative series and the review fix branches are unchanged, so there was nothing else to take in.
- `crates/gpu/src/render.rs` `linear()` now asks `lin_needs_cpu_stage`, which still includes `negative::enabled`.
- Version `0.21.1+filmscan.9` (`e067e7ad`). Built with `--features heif` and the pinned craft-fonts.

**Testing**
- `cargo xtask ci`: fmt, clippy and heif passed. `test` failed only at the known version test. The steps after it
  (parity, layers, assets, roots, wasm) were run one by one and passed.
- `gpu_matches_cpu` passed. `zap_on_real_scans` found the frame on all 6 London scans (2–4 sides each). It was not
  compared with the `.8` results.
- `lightcraft-cli render` of a London DNG worked.

## 2026-11-20: state after v0.21.1-filmscan.8 (no release)

**Upstream**
- Still v0.21.1 (2026-11-12). The version in `main` is still `0.21.1`.
- `main` is at `c17badc6`: 100 commits (without merges) after `27b755a9`, the state we merged for `.8`. They are
  mostly keywording, craft-libs cut-overs (AVIF, codecs, raw), CR3/RAW fixtures and LR-BEHAV-LOCALIZE. None touches
  `crates/film`, the negative or Zap! files, or `crates/gpu/src/render.rs`.
- #3985 (`2404f941`), #3986 (`a529bdb2`) and #3987 (`a9d3c229`) are unchanged since 2026-11-13, with no review and
  no comment since 2026-11-18. GitHub shows their merge state as unknown.
- @manueldeprada's `cr-2-pipeline` (`f84a8499`), `cr-3-engine` (`4c800191`) and `cr-4-panel` (`bc02c044`) are
  unchanged and already in `filmscan` (`c0a0d408`).
- #3623 (`b0797a9b`) is unchanged since 2026-11-16: mergeable, no review.
- No maintainer has ruled on the series or on #3623. Upstream's rustls issue #2568 is still open.

**Compared with `filmscan`**
- Ours since `.8`: the split macOS build (`0ecc6068`), the publish.md update (`c6a92ab2`) and Pillow 13.0.0 for
  the dev-only Sony distortion check (`1d1f2eac`). None of them is user-facing.
- `1d1f2eac` changes `tools/`, which `publish.sh` does not leave out, so the `.8` packages no longer match HEAD.
  That only matters for a rebuild, and a new release needs one anyway.
- A trial merge of `origin/main` (`git merge-tree`) conflicts only in `assets/ATTRIBUTION.md`: our removal of the
  ArtCraft logos against upstream's new entries (#4312, #1915). `Cargo.lock` merges by itself and keeps our
  `rustls-rustcrypto 0.1.0-pre.0` / `rustls-webpki 0.103.16`.

**Risks**
- Upstream's `Cargo.lock` still has `rustls-rustcrypto 0.0.2-alpha`. If a later merge conflicts in `Cargo.lock`,
  the usual "take theirs, then `cargo update -w`" undoes `001f4778`. Afterwards check the lock for
  `rustls-webpki 0.102.8` and run `cargo update -p rustls-rustcrypto --precise 0.1.0-pre.0` again if needed.

**Recommendation**
- Wait. There is no new tag, the series is unchanged, and we have nothing new for users.
- Merge upstream `main` with the next release, or when upstream tags one.
- Dependabot: no open alerts on the fork. The 43 Pillow alerts were fixed by `1d1f2eac` (pushed 2026-11-18), and
  the 5 zip alerts were dismissed as "not used" on 2026-11-18, with the reason given in each dismissal.
- Also: the log moved from the end of `publish.md` into this file.

## 2026-11-18: macOS builds on two machines at once

- `0ecc6068`: `fotofieber-build.yml` compiles `aarch64-apple-darwin` and `x86_64-apple-darwin` in two parallel jobs
  (`build-macos.sh --target`), and a third job puts the universal app and the CLI ZIP together without compiling
  (`--prebuilt`, through `package.sh --skip-build`). It refuses binaries of another commit and an app that is not
  universal.
- `fetch-builds.sh` downloads only `filmscan-macos` and `filmscan-windows`. The intermediate `macos-bin-*` artifacts
  (kept 1 day) hold a `BUILT_FROM` that would overwrite the .deb's.
- Test run 44275374063 on `ci/macos-split`: macOS took 39 minutes instead of ~90 (aarch64 25 min, x86_64 36 min,
  assembly 3 min); Windows took 28. The app has arm64 and x86_64, the package README, the Film Scan library name
  and the CJK fonts, and is the same size as the `.8` release's to within 2 KB.

## 2026-11-18: v0.21.1-filmscan.8 published (upstream main + rustls fix)

**What**
- Base: upstream v0.21.1 + `main` at `27b755a9` (146 commits, untagged), see the merge entry below.
- Ours since `.7`: the rustls-rustcrypto bump (`001f4778`), package README (`62a29f90`), release notes that summarise
  upstream `main` in one line (`6a6315d2`).
- All four packages were built from `6a6315d2`: the .deb locally, Windows and macOS on GitHub (run 44254737667).
  macOS took ~90 minutes on the Intel runner (`.7`: 53); Windows ~28.
- Notes: a hand-written sentence for users under "What's new", and the `.6` screenshots reused.

**Testing**
- The user tried the Windows ZIP (the same file that was published, sha256 `d4352203…`): it works.
- The .deb's CLI renders the demo library. macOS is not tested by the maintainer, as the notes say.
- `publish.sh --check` passed. The published `SHA256SUMS.txt` matches the local one.

**Notes for next time**
- The release title from `publish.sh` still says "LightCraft v0.21.1 + Convert Negative". The notes say "+ main".
- The Windows test copy of the ZIP was removed after the release.

## 2026-11-18: merged upstream main, rustls-rustcrypto bump (no release)

**What**
- Merged `origin/main` at `27b755a9` (239 commits past v0.21.1, untagged). Conflicts: `Cargo.lock` (theirs, then
  `cargo update -w`) and `crates/engine/src/tests.rs` (kept `mod negative;` and upstream's `mod opened_folder;`).
- `001f4778`: `rustls-rustcrypto` 0.0.2-alpha → 0.1.0-pre.0 and `webpki-roots` 1.0.12. This drops `rustls-webpki
  0.102.8` and `crypto-bigint 0.5` (the 4 rustls-webpki Dependabot alerts; RUSTSEC-2026-0387, upstream #2568). It
  needs no code change. Taken from upstream's Dependabot PR #4381, whose gate fails on `harfrust` 0.16, not on this.
- Not taken: candle 0.11.1. It would clear the zip alerts, but upstream pins `=0.9.2` on purpose (later releases pull
  Oniguruma, a C library). zip doesn't reach us.
- `webpki-roots` is CDLA-Permissive-2.0, which is not in the house allowlist. That's unchanged since 1.0.10;
  it's upstream's to decide.
- Version `0.21.1+filmscan.8`.

**Testing**
- `cargo xtask ci`: everything passed except the known version test. The steps after `test` (GPU, parity, layers,
  assets, roots, wasm) were run one by one and passed.
- `gpu_matches_cpu` passed.
- `cargo test -p lightcraft-fetch`: 29 tests passed.
- `zap_on_real_scans` on `negatives-london`: the frames and Zap! measurements are identical to before. The converted
  pictures differ (up to 55 levels on 170429, ~3 levels darker on average).
  - The cause is upstream: plain renders of the same DNGs differ the same way between upstream v0.21.1 and `main`.
    Ours are byte-identical to upstream's in both states.
  - Probably the TIFF/DNG decode moving to craft-libs (#4244 and following).

**Next**
- Release when upstream tags; the release notes then need the upstream commits summarised. Nothing is pushed yet.

## 2026-11-18: README changes no longer force a rebuild

- Until `.7`, every package contained the repository's `README.md`, and `publish.sh` refused packages whose source
  differed from HEAD outside `packaging/fotofieber`. So the README note that Windows was tested (`419ec87e`), written
  after the builds, meant building all three platforms again (macOS about 50 minutes).
- Now the packages carry `packaging/fotofieber/README-package.md` as their README (`build-deb.sh`,
  `build-windows.ps1`, `build-macos.sh`), and `publish.sh` leaves `README.md` out of its check (`same_source`).
- `README.md` no longer names versions or test results. Those are in each release's notes.
- New: `publish.sh <tag> --check`, and the "Docs only (no release)" section above.
- The new package README reaches users with the next build. The Windows and macOS scripts were not run here: the
  next `fetch-builds.sh` run tests them.

## 2026-11-18: state after v0.21.1-filmscan.7 (no release)

**Upstream**
- Still v0.21.1 (2026-11-12). `main` has 239 commits since then (last 2026-11-17), none on Convert Negative,
  `crates/film` or the negative files. They are mostly the craft-libs cut-over (codecs, gain map, print, PSD, MRW).
- #3985 (`2404f941`, mergeable), #3986 (`a529bdb2`) and #3987 (`a9d3c229`) are unchanged since 2026-11-13. #3986
  and #3987 are conflicting. No review, comment or label since then.
- @manueldeprada's `cr-2-pipeline` (`f84a8499`), `cr-3-engine` (`4c800191`) and `cr-4-panel` (`bc02c044`) are
  unchanged and already in `filmscan`.
- #3623 moved to `b0797a9b`, but only by merging `main` (0 own commits since `45b866f8`): no new ideas. No review.
- No maintainer has ruled on the series or on #3623.

**Compared with `filmscan`**
- `filmscan` has nothing since `419ec87e` apart from the `.7` log entry.
- A trial merge of `origin/main` (`git merge-tree`) conflicts only in `Cargo.lock` and `crates/engine/src/tests.rs`.
  16 files are touched by both sides; the next upstream tag should merge cleanly apart from those two.

**Recommendation**
- Wait. There is nothing to take in and nothing new of ours to ship.
- Release next when upstream tags a release after v0.21.1 (as `v<x>-filmscan`), when the series changes, or when we
  have new work of our own.

## 2026-11-16: v0.21.1-filmscan.7 published (first Windows and macOS builds)

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

## 2026-11-16: state before v0.21.1-filmscan.6

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

## 2026-11-14: could AI improve Zap!'s crop? (study, no code change)

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
## 2026-11-14: scheduled workflows off on the fork

- FreeBSD, Release and Windows ARM64 are disabled on `FotoFieber/lightcraft-filmscan` with `gh workflow disable`.
- They ran upstream's nightly checks on `filmscan` every day and failed only because of the `+filmscan` version: the xtask version test, and Release's "Resolve version" in its dry run.
- Releases of this fork are built with `build-deb.sh`.
- `gh workflow enable <name> -R FotoFieber/lightcraft-filmscan` turns one back on.

## 2026-11-14: local build 0.21.1+filmscan.5 (not published)

- `e2c6bf8d`: Zap! finds the rebate on a scan the film fills. The base comes from between the sprocket holes, else from an even, narrow edge strip.
- `6071bd2d`: Zap! "As One Roll" makes the frames agree on their crop. A frame keeps its own edge where the roll's would reach onto blank film. It also fixes a pooled-measurement error on mixed selections.
- Scored against the user's crops with `corpus/zap-1114/tools/` (`truth.py`, `score_eval.py`, ignored test `zap_eval_on_truth`).
- Results, within 1/2/4 %:
  - B&W roll: 6/9/30 → 19/25/34 (19/26/35 as one roll).
  - Holdout: 79/101/117 → unchanged, 83/107/125 as one roll.
  - Tune: unchanged.
- `ci --quick`: the expected version test fails. `layers`, `assets`, `roots` and `parity` pass.

## 2026-11-14: local build 0.21.1+filmscan.4 (not published)

- `3920d313`: Zap! is only Photo ▸ Film Scan ▸ Zap!… (the command keeps no menu of its own).
- `b7cda261`, two changes:
  - Zap!'s Film choice gains B&W Negative. It uses the Monochrome profile, and the base still comes from the rebate, so XP2's coloured base turns black.
  - The import review has "Then Zap!…" in every mode.
- Tests:
  - `ci --quick`: the expected version test fails.
  - `find_similar::stopping_leaves_the_view_alone…` failed under full load and passed on its own. It is known to be flaky.
  - `layers`, `assets`, `roots` and `parity` pass.

## 2026-11-14: local build 0.21.1+filmscan.3 (not published)

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

## 2026-11-14: state after v0.21.1-filmscan.2

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
