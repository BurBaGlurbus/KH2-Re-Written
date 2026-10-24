# T228 — Plain builds leave out the Skate 3 crates; one executable; the image viewer row works

Owner Claude, 2026-10-24, requested by burba in the project chat ("building for skate 3 should
include things that need skate 3 so longer compile time, but without it should in theory be
faster please fix this", then "get rid of all the other exes that show up when compiling the
only exe that should be there after compilation is the kh2rw launcher", then "fix it" about the
image viewer row). Gate: none (build tooling; no gameplay gate is advanced).
Prerequisites: T101, T103, T205. Build identity: branch `build/skate3-optional-deps` cut from
`origin/main` 33ce82e; package 0.1.2, Rust 1.99.0 Windows MSVC, 8 logical CPUs. Source: not read;
no gameplay session was run apart from the smoke run.

Evidence (never in Git; local timing logs and Cargo JSON output): `%USERPROFILE%/kh2t/b/`
(before), `%USERPROFILE%/kh2t/a/` (after), `%USERPROFILE%/kh2t/checks/`, `%USERPROFILE%/kh2t/scripts2/`, `%USERPROFILE%/kh2t/run2/` and `%USERPROFILE%/kh2t/viewer/`
(part 2). Each clean measurement
used its own empty `CARGO_TARGET_DIR`; runs were one at a time.

## What was wrong

- `main` already had `skate-disc` and `skate-sim` as optional dependencies behind `skate3`, and
  the build scripts already pass `-p main`, so `build-release.bat` and `build-debug.bat` never
  compiled the Skate 3 crates (`cargo tree -p main`: 281 packages; with `skate3`: 314).
- But the workspace had no `default-members`, so plain `cargo build`, `cargo test` and
  `cargo clippy`, and the documented `--workspace` checks, built both Skate 3 crates and their 33
  extra packages (rapier3d, parry3d, nalgebra, zstd, ...).
- Worse, those packages turn on extra features in crates Bevy shares (`profiling/procmacros`,
  `downcast-rs/sync`, `approx/std`, `getrandom/std`, `cc/parallel`, ...). Cargo unifies features
  across the packages of one build, so a workspace build compiled a Skate 3-flavoured Bevy, wgpu
  and naga, and every crate of ours on top. The next `-p main` build (the smoke run, the build
  scripts, `cargo run -p main`) then compiled all of them a second time.

## Steps

1. Root `Cargo.toml`: `default-members` lists every member except `skate-disc` and `skate-sim`.
   `cargo tree` confirms the default members resolve exactly the features of `-p main`, and
   `--features main/skate3` (with or without `--workspace`) exactly those of
   `-p main --features skate3`.
2. AGENTS.md, CONTRIBUTING.md, README.md, the PR template and `ci.yml`: the checks drop
   `--workspace`; Skate 3 changes add clippy and test with `--workspace --features main/skate3`.
3. The build scripts already select `-p main` and add `--features skate3` only for the
   `-skate3` scripts; the output is still renamed to `KH2RW Launcher.exe`.

## Part 2: only `KH2RW Launcher.exe` after a build

- The model and image viewers were separate binaries (`src/bin/model_inspector.rs`,
  `src/bin/image_inspector.rs`) that the launcher started as sibling `.exe` files. They are now
  modules of the `KH2RW` binary, run like the tests: the launcher restarts itself as
  `"KH2RW Launcher.exe" model_inspector|image_inspector <install> <manifest> <selection> <log>`
  (their `--tour`, `--capture-dir`, `--frames` and `--direct-draw` options are unchanged). The
  model viewer's unit test now runs with the launcher's tests.
- `tools/Build-Launcher.ps1` builds only `--bin KH2RW` (`-LauncherOnly` is gone) and then removes
  every other top-level `.exe` (with its `.pdb`/`.d`) in the profile folder it built:
  `smoke.exe`, the probes and `validation.exe` that plain `cargo build` runs leave there. Cargo
  writes such an executable again from its cache on the next `cargo build`/`cargo run` that asks
  for it (checked: a deleted `smoke.exe` came back on `cargo build -p main --bin smoke` in 2.6 s
  with nothing recompiled). A file that cannot be removed (for example a running program) only
  warns.
- `tools/Invoke-NativeMemoryReader.ps1` ran `target/debug/validation.exe` directly; it now builds
  it first, because the debug scripts remove it.
- README, CONTRIBUTING, ARCHITECTURE, the folder guide, `source/main/README.md`,
  `tools/README.md`, the two research notes and the `.bat` comments describe the single
  executable.

Part 2 results:

- fmt, clippy (no warnings), `cargo test --locked` (991 passed, 1 ignored), smoke (exit 0),
  clippy and test with `--workspace --features main/skate3` (no warnings; 1013 passed, 1
  ignored): PASS.
- Scripts, in turn in the worktree (`run2/*.ls` list every top-level `.exe` after each):
  before them `target/release` and `target/debug` each held `KH2RW Launcher.exe`,
  `model_inspector.exe` and `image_inspector.exe`. After `build-release.bat`:
  `target/release` holds only `KH2RW Launcher.exe` (66,954,752 bytes); after
  `build-release-skate3.bat` only `KH2RW Launcher.exe` (74,690,048). A plain `cargo build` then
  put 31 executables in `target/debug`; after `build-debug.bat` only `KH2RW Launcher.exe`
  (113,546,752) remained, and after `build-debug-skate3.bat` only `KH2RW Launcher.exe`
  (124,364,800). All exit 0.
- Viewers through the release launcher with the launcher's own argument order (program name,
  install, bound manifest, selection, new log; plus `--frames 180`): the model viewer on the
  Sora selection did a fresh full source scan, opened its window, ran 180 frames and exited 0
  (`complete: true`). The image viewer was reached and rejected that selection with its own
  schema check (exit 1, `missing field inspect_hd_images`); no image selection exists on this
  machine, so no image was shown. `"KH2RW Launcher.exe" nope` prints the usage with both names.
- The viewers were not clicked in the launcher window; the call above is the one
  `start_viewer` makes.

## Part 3: the launcher's viewer rows

- The launcher listed an image viewer only for a selection with `inspect_images`, a field no
  selection reader has; the image viewer requires `inspect_hd_images`. It also listed only
  hand-made `<who>-selection-<n>.json` files, so the selections it makes itself
  (`auto-<who>-selection.json`) never became rows: a new data folder had no viewers at all.
- Now `install::viewers` takes `inspect_hd_images` for the image viewer and lists both name
  forms; a hand-made selection wins over a made one with the same label.
- `TEST_CONFIGS` gains `title-selection`: the title screen's `menu/us/title.2ld` with
  `inspect_hd_images`, made as `auto-title-selection.json` and listed as "Image viewer: Title
  screen". On this install it holds 12 HD images (checked with a throwaway reader before the
  change; the same check found 1 for `zz0command.2dd`, 3 for `fontimage.bar`, 6 for Sora's
  model).
- When a folder is bound (`checked`), the launcher makes the configs and viewer selections if
  this build's are missing (`install::make_configs`) and refreshes the viewer list, so the rows
  are there before any test has run.
- Tests: `viewers_come_from_model_and_image_selections` (both name forms, the old field ignored,
  the hand-made one wins) and `made_selections_are_viewers_the_viewers_accept` (every made
  selection is a row, and the title selection has the fields `read_selected_images` requires).

Part 3 results:

- fmt, clippy (no warnings), `cargo test --locked` (992 passed, 1 ignored), smoke, and clippy and
  test with `--workspace --features main/skate3` (no warnings; 1014 passed, 1 ignored): PASS.
  `build-release.bat`: exit 0, `target/release` holds only `KH2RW Launcher.exe`.
- The release launcher, started without arguments on the `%LOCALAPPDATA%` data folder, made
  `auto-title-selection.json` (with the four model selections) within a second of binding the
  saved folder. Its window was not clicked (screen control was declined).
- The image viewer run with that selection as `start_viewer` runs it, plus `--tour`, a private
  capture folder and `--frames 6000`: fresh full scan, 12 images admitted, 72 of 72 captures,
  exit 0 (`run3/`, `viewer/title-*.jsonl`, `viewer/title-captures/`). The first capture shows the
  title atlas (cursor hand, panels, swirl) over the checker background. Captures stay private.

## Measurements (seconds, clean unless stated)

| Build | Before | After |
| --- | --- | --- |
| Release launcher + viewers (`build-release.bat`'s command) | 579 (313 units) | unchanged command |
| Release launcher + viewers, `skate3` | 619 (353 units) | unchanged command |
| Dev `cargo build --workspace` / plain `cargo build` | 299 (375 units, Skate 3 built) | 286 (334 units, no Skate 3) |
| Then `cargo build -p main` in the same cache | 277 (70 units rebuilt: all of Bevy, wgpu, naga, our crates) | 1 (0 rebuilt) |
| Then `cargo build --features main/skate3` | — | 328 (128 units: the Skate 3 crates and the Bevy stack with their features) |
| Then `cargo build -p main` again | — | 18 (10 binaries relinked; both dependency sets stay cached) |

An earlier before-run of plain `cargo build` took 773 s for the same 375 units; the machine was
shared, so single numbers carry noise of that order.

## Checks and results

- `cargo fmt --all --check`, `cargo clippy --all-targets --locked -- -D warnings` (no
  warnings; the only Skate crate checked is `skate_source`), `cargo test --locked` (991 passed,
  1 ignored as on `main`), `cargo build --locked`, smoke `cargo run -p main --locked -- --frames
  180` (exit 0): PASS.
- `cargo clippy --workspace --features main/skate3 --all-targets --locked -- -D warnings` (no
  warnings; all 7 Skate 3 crates checked) and `cargo test --workspace --features main/skate3
  --locked` (1013 passed, 1 ignored): PASS.
- The four build scripts, run in turn in the worktree's own `target/` (inside OneDrive): all exit
  0. `build-release` 756 s clean, then `build-release-skate3` 949 s, `build-debug` 394 s clean,
  then `build-debug-skate3` 290 s. The default scripts compile only `skate_source`; the `-skate3`
  ones compile all 7 Skate 3 crates. Each leaves `KH2RW Launcher.exe`, `model_inspector.exe` and
  `image_inspector.exe` and no `KH2RW.exe`; the release launcher is 66,716,672 bytes without
  `skate3` and 74,358,272 bytes with it. These four times are slower than the clean runs outside
  OneDrive above and the second release run was slower than a clean one, so they are not used to
  compare.
- `python tools/progress.py`: NOT RUN (no coverage change).

## Uncertainty

- The default release build is only about 7% faster than the `skate3` one when both are clean:
  the Skate 3 packages compile alongside Bevy, and Bevy and the `main` crate set the critical
  path. This change removes the Skate 3 work from builds that did not ask for it; it does not make
  the launcher itself compile faster.
- Switching between a default and a `skate3` build in one cache costs a Bevy rebuild the first
  time only, because the Skate 3 packages change shared features. That is inherent to the feature
  and not changed here.
- `--workspace` still includes the Skate 3 crates (Cargo's rule); the docs no longer use it for the
  default checks.
- GitHub Actions does not run for this repository; the CI file was updated but not exercised.

## Verdict and next task

PASS for the build change: builds that do not ask for `skate3` no longer compile the Skate 3
crates or a second Bevy stack, and `skate3` builds still include them; each build script leaves
only `KH2RW Launcher.exe`, with the viewers inside it; the launcher makes and lists a working
image viewer (the title screen). No gameplay gate is affected.

Next: none required; if the default launcher build should be faster still, profile `main` and the
Bevy feature list with `-Timings`.
