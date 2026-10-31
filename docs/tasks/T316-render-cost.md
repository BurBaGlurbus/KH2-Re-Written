# T316 — Rendering refactor: kept effect meshes, one staging belt, flat skinning, shared vertices

Owner Claude, 2026-10-31, requested by burba in the project thread: "refactor the code to make it even more optimized for rendering and research online what you can do to optimize games in rust bevy". Gate: P30 performance engineering; no gameplay gate moves. Prerequisites: T312, T313. Build: local `main`, base `d905230`, package 0.1.2, Rust 1.99.0 Windows MSVC, Bevy 0.19.1, wgpu 29. Source: the selected Steam KH2FM build, validated at each run; the install was only read.

Numbers are in [the results](../perf/results.md#rendering-refactor-t316). Private evidence stays in an output folder outside the checkout (`p-*`, `o*-prof`, `cap-*`, `tour-*`).

## How the costs were found

A private build carried a sampling profiler that suspends each thread and unwinds its stack, searching each module's unwind table itself (the system's lookup takes a lock a suspended thread may hold). That gave inclusive milliseconds per frame by function, on top of `perf-trace`'s per-system spans.

## Changes

Milliseconds are per 60 Hz frame from the stack samples, one run each, Station Plaza on the owner's settings, with the profiler's own cost. They show where time moved; the table in the results is the measure of the whole.

| Cause, as measured | Change | Before | After |
|---|---|---|---|
| Drive and spell effects spawned an entity and a mesh for every batch every tick (about 7 meshes added per frame while standing in the room) | `effect_mesh`: one entity and mesh per batch key, kept between ticks; unused vertices repeat the first position, so their triangles have no area; hidden when not drawn, gone after 600 drawn ticks unused | `allocate_and_free_meshes` 0.38 | 0.01 |
| `Skinning::pose` built a full `ModelPose` through a list of lists and bit conversions, and threw most of it away | Flat influences; `positions_from_worlds` returns only vertices. The same operations in the same order: the same bits. The JSON form is unchanged | 0.43 | 0.12 |
| `mesh_upload::collect` packed each vertex attribute by attribute, then walked the positions again for bounds | One pass for bytes and bounds | 0.55 | 0.14 to 0.23 |
| `queue.write_buffer` made a staging buffer per mesh part, created and destroyed each frame | One `StagingBelt`, kept between frames | `write_buffer` 0.39 plus most of `maintain` 0.18 and `EncoderInFlight` drop 0.17 | `write` 0.36 to 0.45; `maintain` 0.12, drop 0.12 |
| Model parts were triangle soups: each corner its own vertex | `room::part_mesh`: a corner with the same model vertex, UV and colour shares its mesh vertex; same triangle order; `set_positions` still takes positions corner by corner | part of the `collect` and `write` rows | part of the `collect` and `write` rows |
| The HUD canvas hashed texture fingerprints and each triangle with SipHash | `FixedHasher`, for hashes that live only inside a session | fingerprints 0.09 | 0.04 |
| The Skate HUD's camera drew a transparent layer and blended it over the frame every frame, whether or not Skate was on | The camera is active only while the HUD draws something | a camera pass | none outside Skate (by the code; passes were not counted again) |
| Two inactive 3D cameras reset thousands of light clusters each frame | `ClusterConfig::None` on them, as the main camera has | `reset_for_new_frame` 0.05 | not measured alone |

A mesh that takes the fast path keeps Bevy's retained sort data from its first upload. Transparent items sort by the centre of their bounds, read from the render mesh and copied into the retained phase item, so reused effect meshes sorted by a stale distance: the capture comparison showed prize orbs green instead of yellow, in every run. `mesh_upload::recentre` now refreshes both, and the same captures match the base. This also repairs the same stale centre that T312 left on skinned parts.

## Researched and not used

| Item | Result |
|---|---|
| GPU skinning | NOT DONE. The format lets each vertex have up to 32 influences, each with its own position, so Bevy's four-weight `SkinnedMesh` does not fit: it needs a custom vertex shader and storage buffer. What it could remove is now about 0.7 ms of about 7, and effects read CPU vertices. GPU rounding would need its own proof against the bit-exact pose |
| `NoIndirectDrawing` on the cameras | Tested: 0.452 against 0.453 running samples per sweep. No gain; removed |
| `use_gpu_instance_buffer_builder: false` | Tested: Bevy logs an error per batch and frame time collapses. Rejected |
| `prepass_enabled: false` | Tested: 0.453 against 0.456. Within noise; removed |
| Bindless materials, draw batching | NOT DONE: `FogExt` is a custom shader, so it needs `#[bindless]` and shader changes with visual risk. Command encoding is about 0.7 ms and the driver about 1.2 ms, so this is the largest thing left. Draw calls were not counted |
| `desired_maximum_frame_latency`, pipelined rendering off | NOT RUN |
| mimalloc | NOT RUN: it needs a C compiler in the build. The allocator is about 0.2 to 0.3 ms |
| Thin LTO | Not run again: `Cargo.toml` records 28 min builds and no measurable gain |
| Memory hints, unit clustering, task pool caps, single-threaded schedules | Already done in T312 |

Sources: [Bevy 0.19 release notes](https://bevy.org/news/bevy-0-19/), [Bevy 0.16 release notes](https://bevy.org/news/bevy-0-16/), [Bevy profiling guide](https://github.com/bevyengine/bevy/blob/main/docs/profiling.md), [Bevy cheatbook on performance](https://bevy-cheatbook.github.io/pitfalls/performance.html), [wgpu best practices](https://wgpu.rs/doc/wgpu/documentation/best_practices/dos_and_donts/index.html), [wgpu `StagingBelt`](https://wgpu.rs/doc/src/wgpu/util/belt.rs.html). These are claims about the engine: the decisions above rest on the measurements here.

## Checks

| Check | Verdict |
|---|---|
| `cargo fmt --all --check` | PASS |
| `cargo clippy --all-targets --locked -- -D warnings` | PASS |
| `cargo test --locked` | PASS: 1,153 passed, 0 failed, with new tests: a batch keeps its entity and mesh and hides when not drawn; unused vertices have no area; triangles pack as the engine packs them; a part mesh draws the same corners; weighted vertices sum as the full transform does; skinning serializes as before |
| Clippy and test with `--workspace --features main/skate3` | PASS: 1,175 passed, 0 failed |
| `cargo run -p main --locked -- --frames 180` | PASS |
| Release build with Skate 3 | PASS: only `KH2RW Launcher.exe` left |
| Captures at 7 ticks, test room, 3 runs each, against the base | PASS: 2 of 3 runs identical at all 7; the third differs at one tick. The base's own second run differs from its first at one tick |
| The same, Station Plaza | PASS: all 3 runs identical at all 7 ticks |
| Fullscreen, borderless, window on the owner's settings | PASS: 0 errors, clean exit |
| Live window resize at four sizes | PASS: 0 errors |
| Settings changed during play in a private build, every 5 s for 75 s: all six render scales, anti-aliasing on and off, all three display modes | PASS: 14 changes logged, still running, 0 errors. The switch was injected in code, not made through the menu |
| 90 s soak with spells, Drive forms and 5 area changes | PASS: 0 errors; effect entities drawn stay at 4 to 6 and materials grow from 57 to 61 over the last 35 s |
| `--save-point-tour`, base against new | PASS by eye: one capture of 8 differs, at most 3 levels at the screen's edge during a menu slide, as T312 recorded |
| `--magic-tour`, 180 captures | See below |

## Uncertainty

- `--magic-tour`: two runs of the base differ at 15 of 180 captures, two runs of the new build at 9. Base against new differ at 27 to 31, of which 12 to 15 are large like the run-to-run ones. The other 16 or so are 1 or 2 pixels, always the same captures, and they do not occur between two runs of one build. So this change does cause them. Turning off skinning, packing, the belt, the re-sort, the HUD hash and the camera together did not remove them, nor did turning off vertex sharing or the pool's reuse alone. The likely cause is untested: reused entities change which of two transparent sprites with exactly the same centre draws first, and the differing pixels move like a 1-pixel far-away sprite. It is NOT PROVEN.
- The Steam game's field is still not measured, so no row here compares play with play.
- Draw calls were not counted. GPU time was not measured; GPU memory was.
- The CPU figures are 2 runs each with other programs running (system about 30% busy). Frame times are one run each.
- The 14 setting changes were in Station Plaza only. Their switch does not go through the Graphics menu's own code.
- Kingdom Hearts: Connected: `pose_from_worlds` stays, `room::mesh` stays, but `spawn_model`'s parts are now indexed. Any code there that writes `ATTRIBUTE_POSITION` directly on a part must use `room::set_positions`. Not built here.

## Verdict

PASS for the measured work: Station Plaza on the owner's settings from 5.85% to 4.6% of all cores, 100% scale from 6.1% to 4.9%, the scripted battle from 4.45% to 3.85%; the title is unchanged. Frame pacing is unchanged. Memory moved within noise. It does not reach the Steam menu's 1.3% to 2.8%, which is a different screen. No gameplay gate moves.

## Next

Bindless for the fog and effect materials, with draw calls counted first: encoding and the driver are now the largest cost left.
