# T236 — Frame rate setting, frame-rate independence and the performance pass

Owner Claude, 2026-10-25, requested by burba in the project chat (steps 2–4 of the performance refactor
after [T230](T230-performance-baseline.md): frame rate settings, frame-rate independence, then
optimizations, "without changing gameplay behavior or visuals"; the HUD to stay pixel-identical; later "a
option in debug to enable stuff like fps gpu to show up at the top left"). Gate: P30 performance
engineering; no gameplay rule changes and no final gate inferred. Prerequisites: T230, T229. Build
identity: branch `perf/high-fps` from `36c97bb` (merged into local `main` as `efca285`), then local
`main` up to `1b92156`; package 0.1.2, Rust 1.99.0 Windows MSVC. Source: the selected Steam KH2FM build,
validated at each run; the install was only read.

The commits of this work say "T231" in their messages; that number was taken on `main` by the BAR
round-trip work meanwhile, so this record is T236.

Private evidence: `C:\Users\ANIMA\kh2-perf-out\` (frame records, run logs, captures and Chrome traces:
`cap*`, `lim*`, `det*`, `e*`, `h*`, `t*`, `q*`, `trace-*`, `ab*`, `fin-*`, `overlay-*`).

## What changed

Step 2, frame rate:
- Graphics page: Frame Rate (V-Sync, Unlimited, 30/60/90/120/144/165/240 FPS, Custom) and Custom Limit
  (30–500 FPS in steps of 5; any value in that range from `kh2fm-settings.json`), saved with the other
  settings and applied at once. V-Sync presents with `AutoVsync`, the others with `AutoNoVsync`.
- `visual::frame_limit`: the main app waits at the end of each update for the next whole-interval
  deadline (sleep, then a short spin), so the input that arrived meanwhile is read right before the next
  update. Bevy's winit `Reactive` mode was tried first and paced badly (frames of 10–98 ms at a 30 cap).
  Cap 30 held 30.0 FPS on average, cap 90 an 11.17 ms median frame.
- In the background the room caps itself at 30 FPS (not in unattended runs). `--uncapped` and
  `--frame-cap N` override the settings for checks.

Step 3, frame-rate independence (the simulation already ran on the fixed 60 Hz tick):
- The Keyblade pose that effect anchors read was kept by the per-frame presenter, so below 60 FPS a tick
  read an older pose and bound spell effects started a few units off: kept at the end of each tick now.
- The drive change's afterimage and the death blur stepped once per drawn frame, the debug and
  skateboard notices and the debug menu's held-arrow repeat counted frames: all count ticks now.
- An unattended capture (a held clock) blends Sora's motions at the tick itself.
- Motion Smoothing (Graphics page, on by default; `visual::smoothing`): the scene camera and everything
  under the scene root are drawn between the last two ticks by the fixed clock's overstep, at most one
  tick behind; cuts, slow frames and a held clock show the newest tick.

Step 4, optimizations (each its own commit):
- HUD canvas: only the 16-pixel tiles drawn in are cleared and resolved; rows go to the threads in turn;
  inside tests before divisions; then a per-tile hash of the triangles over each tile, so only changed
  tiles are redrawn; the changed tiles alone are written into the HUD's GPU texture
  (`visual::texel_upload`). A test keeps the previous rasterizer as the oracle and checks byte equality.
- Mesh uploads: skinned actors, party members and effect stand-ins are only re-uploaded when their data
  changed (`visual::room::set_positions`, `set_triangles`); the 2D overlay keeps a slot's mesh while its
  sheet and quads are the same.
- Release profile: an opt-in `release-lto` profile (thin LTO, one codegen unit). Thin LTO in the normal
  release profile made a one-file rebuild 10 min instead of 2–3, and its gain was within this PC's noise,
  so `build.bat` keeps the plain release. `panic = "abort"` was left out (Bevy reports a panicking system
  by unwinding; a research example catches panics).
- Dev profile: dependencies at opt-level 3 (a one-time 23 min rebuild; then a one-file test rebuild 8 s
  instead of 18 s, the tests 22 s instead of 39 s, the smoke run 5 s instead of 17 s). Our crates at
  opt-level 1 made a one-file test rebuild 264 s, so they stay at 0.

Debug: F3 > Sora > Display > Performance overlay (off by default, not saved, `--perf-overlay` at start)
shows FPS and its 1% low, frame time and its p99, the GPU and API, and the entity count (memory and CPU
in perf-diag builds), drawn by the 2D layer over everything. The GPU's own frame time is not shown: it
needs Bevy's render diagnostics, which can only be added with the render plugins and time every pass.

## Results

See [docs/perf/results.md](../perf/results.md). Medians of three alternating runs each: test room field
play 58.8 → 194.8 FPS (1% low 26.8 → 45.9, p99 24.7 → 12.3 ms), Twilight Town 45.5 → 147.3 FPS (1% low
14.0 → 53.6, p99 44.0 → 12.1 ms), heap allocated per frame 12.5–13.4 → 2.5–3.1 MiB; with VSync the room
now holds the monitor's 100 Hz (57.2 → 99.8 FPS).

## Checks

| Check | Result |
| --- | --- |
| `cargo fmt --all --check` | PASS |
| `cargo clippy --all-targets --locked -- -D warnings` | PASS |
| `cargo clippy -p main --all-targets --features perf-diag --locked -- -D warnings` | PASS |
| `cargo test --locked` (new tests: frame-rate choices and old files, frame caps and focus, the limiter, smoothing, the canvas oracle, overlay statistics) | PASS |
| `cargo build --locked`, smoke `cargo run -p main --locked -- --frames 180` | PASS |
| Release `tools/Build-Launcher.ps1 -Skate3` | PASS, no warnings |
| Unattended room run at 30, 60, 144, 240 FPS and unlimited: gameplay events | PASS, identical |
| Captures at equal ticks against the previous build after each optimization | PASS: equal, apart from a scene area near the top that also differs between two runs of one build |
| `cargo clippy --workspace --features main/skate3` | NOT RUN (no Skate 3 code changed) |
| Comparison with the installed game | NOT RUN |

## Uncertainty

- The owner's PC ran Roblox, Discord and other sessions' builds during every measurement; single runs
  vary by a factor of two. The comparison alternates builds and reports medians, but small effects (the
  thin-LTO gain) could not be measured.
- Smoothing shows positions up to one tick late; skinned poses and scene-space effect meshes still step
  at 60 Hz, so at high frame rates an effect can lead the interpolated body by up to a tick. Below
  60 FPS a motion blend can start from a pose one tick older than at 60 (presentation only).
- The tile hashes make a changed tile that hashes like the old one keep its old pixels; the chance is
  about 2^-64 per tile.
- Captures differ between two runs of one build in one area (effect batches are drawn in hash-map
  order, which differs per process); not changed here.

## Deferred

- GPU skinning: Sora's and the party's CPU-skinned meshes change once per tick and are now most of the
  remaining render-thread mesh uploads and per-frame allocation.
- Effect entity pooling and texture/material prewarming against the first-use hitches: they live in
  `spell_fx.rs` / `drive_fx.rs`, which another session was editing.
- Running independent `Update` systems in parallel: the main thread is about 5 ms against the render
  thread's 9 ms, so it would not raise the frame rate yet, and reordering risks behaviour.

## Verdict / next

PASS for the frame rate setting, the frame-rate independence checks and the measured optimizations; no
final gate. Next: GPU skinning or per-tick posing of the characters, and the deferred effect work once
the effect files are free.
