# T312 — The game costs far less to run: 60 FPS by default, no baked poses, the save point's glow

Owner Claude, 2026-10-31, requested by burba in the project thread: "the regular game on steam runs with like 3 percent cpu and basically uses nothing to run ... there is no way in hell i should be getting like 250 fps while the game uses 40 percent of my cpu, 1.2 gb of ram". Gate: P30 performance engineering; no gameplay gate moves. Prerequisites: T236, T237, T298. Build: local `main` from `804e6a3` to `9ec10aa`, package 0.1.2, Rust 1.99.0 Windows MSVC. Source: the selected Steam KH2FM build, validated at each run; the install was only read.

Numbers are in [the results](../perf/results.md#a-game-that-costs-little-t312). Private evidence (runs, captures, heap dumps, decompiles) stays in an output folder outside the checkout (`f-*`, `cap-*`, `soak*`, `h*`, `s*`, `gh1`).

## Causes and changes

| Cause, as measured | Change |
|---|---|
| The owner's saved Frame Rate was Unlimited; the default was V-Sync (100 FPS on a 100 Hz monitor); title, events, world map and launcher had no limit but V-Sync | The default is the original's 60. A settings file from before this moves to it once (`Settings::pacing`); choosing another rate afterwards stays. Every program holds 60 by itself. A cap of 60 or less presents with V-Sync, so it does not tear |
| The limiter spun for the last 1.5 ms of each frame | It sleeps; a late wake stays on the deadline schedule |
| Frames of 0 and 2 ticks at 60 FPS (about 10% of frames) | While frames arrive on the tick's interval each runs exactly one tick; the clock stays within 4 ms of real time and catches up past that (`frame_limit::lock_ticks`) |
| The save point's glow: an emitter that births at frame 0 and loops from frame 1, with children that live forever. `vertex_ap`, `born_rnd5` and `born_rnd6` birthed during a loop's catch-up, which `FUN_1404d3b30`, `FUN_1404d3e00`, `FUN_1404d4ea0` and `FUN_1404d51a0` refuse (`DAT_142b0efe4`). Every 200 frames added 4 parts. After 5 minutes in a room with a save point: 217 parts, 113 MiB allocated per frame, a main thread at 98% | The check, as in the exe |
| Bevy's multi-threaded executor woke pool threads about 350 times per frame for chained or tiny systems | `low_cost`: single-threaded schedules (main and render), one thread per task pool |
| GPU light clustering ran though everything is unlit | Off (`GlobalClusterSettings`, `ClusterConfig::None`) |
| The HUD canvas started 8 OS threads per draw, its work estimate counting whole triangles for about 90 changed tiles | Threads only when over 1,024 tiles change. Tile by tile with a small scratch (22 MiB of colour and depth gone); finished tiles kept in two generations of up to 32 MiB, since the HUD's animations loop; clear texels skip the blend. The byte-match oracle with the pre-tile rasterizer still passes |
| Skinned meshes marked modified each tick were cloned for the render world, freed, reallocated and re-specialized (8.6 MiB per frame, 2.5 ms in the allocator) | `mesh_upload`: `set_positions` updates the mesh untracked and only its vertex bytes go to its place in the buffer; bounds refreshed as before |
| Party members baked all vertices for every tick (492 MiB), plus joints and weapon vertices (52 MiB); Sora's joints and Keyblade vertices (205 MiB); map objects (114 MiB) | Evaluated and skinned when shown, as the original does (about 100 µs for Sora's 239 joints). The results are the same bits: the same `evaluate` and `pose` over the same inputs |
| Render targets kept a zeroed CPU copy (21 MiB+); the HUD image kept its first pixels | No CPU copy |
| GPU memory blocks sized for speed | `MemoryHints::MemoryUsage` |

Also found: `overlay::present` had no order against Bevy's asset events. On the single-threaded schedule a slot's rewrite reached the events a frame late, the mesh's data was already taken, and the renderer freed the mesh without a new copy: most of the camp menu vanished. The capture comparison caught it; `present` now runs before the events. The old build logged the same error twice per run.

## Checks

| Check | Verdict |
|---|---|
| `cargo fmt --all --check` | PASS |
| `cargo clippy --all-targets --locked -- -D warnings` | PASS |
| `cargo test --locked` | PASS, with new tests: one tick per frame on real time, a looping animation's tiles, a loop restart births nothing at frame 0 (fails without the fix: 15 parts instead of 2), the settings' move to 60 |
| `cargo run -p main --locked -- --frames 180` | PASS |
| `tools\Build-Launcher.ps1 -Skate3` | PASS: only `KH2RW Launcher.exe` left |
| Captures at 7 ticks, old against new, before the glow fix, test room | PASS: identical except tick 1790, which also differs between two runs of the old build (4,350 px, same box) |
| The same, Twilight Town | PASS: all 7 identical |
| After the glow fix | PASS by eye: frames match in layout; effect sparkles sit elsewhere because the effects' shared random stream advances differently without the extra births |
| `--save-point-tour`, old against new | PASS: the menu screens identical; the capture during the menu's opening slide is a frame apart |
| `--party-anim-tour`, `event --autoplay`, `wm --autoplay` on the release build | PASS: clean exit, 0 errors; 268, 11 and 14 captures |
| A 6-minute soak in Station Plaza | PASS: parts stay at 9, about 5K allocations a frame, heap flat at 565 to 590 MiB |

One run of a capture build differed at two ticks (a HUD region, Sora's pose in a blend) and the repeat matched the old build exactly, so about 1 run in 3 had a difference at one or two ticks. The old build's two runs did not show this; three runs each do not separate a low rate from none.

## Uncertainty

- The Steam game's 3% and low memory are the owner's figures: the installed game was not run here.
- Which part of this PC's private memory the GPU driver holds was not attributed beyond a map of the process: about 340 MiB is write-combined mappings in 32 MiB blocks. Heap is about 540 MiB.
- GPU load and memory were not measured.
- The glow's look was compared with the old build, not the installed game. The first wave is unchanged; later waves no longer stack up.
- The title's and launcher's frame rates were not counted (they have no per-frame record); the limiter holds them at 60.
- The launcher hidden behind a running game was not measured, only idle on its own screen.
- Tick lock at 60 FPS was checked on this 100 Hz monitor; a 60 Hz or 59.94 Hz monitor was not available.
- Kingdom Hearts: Connected reads `ClipPoses::joints`, `weapon_frames` and `weapon2_frames` (`kh3_model.rs`, `rival_play.rs`). They are now `posed` and `weapon_frame`. Its pinned engine commit still builds; its local `.cargo/engine-path.toml` build does not until those three places change.

## Verdict

PASS for the measured work: Station Plaza on the owner's settings from 35.8% to 5.5% of all cores, Task Manager's memory from 1,663 to 779 MiB. It does not reach the Steam game's reported 3%. No gameplay gate moves.

## Next

The remaining cost has no single large item: CPU skinning's vertex uploads, Bevy's per-frame work, the driver. The largest remaining memory is the world's ambient sound bank (51 MiB), the party limits' HUD sheets (45 MiB), kept model textures and the driver's mappings.
