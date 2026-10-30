# T302 — General lag, the biggest frame costs first

Owner Claude, 2026-10-30, requested by burba in the project thread ("please make the game less lagging and more optimized"), after
T298. Gate: P30 performance engineering; no gameplay gate is inferred. Prerequisites: T236, T237, T298. Build identity: local `main`
from T298's commit on, package 0.1.2, Rust 1.99.0 Windows MSVC. Source: the selected Steam KH2FM build, validated at each run; the
install was only read.

Private evidence (outside the checkout): the private output folder `kh2-gc-out`: traces `tr-*`, runs `f1-*` and later.

## Where the time goes

A `perf-trace` build over 40 s of the soak (`--soak`), with other sessions' compilers on all cores, so the proportions count more
than the milliseconds. Per frame:

| Span | Test room | Twilight Town |
| --- | --- | --- |
| Main thread | | 23 ms |
| Render schedule | | 25 ms |
| `hud_present` (software HUD canvas) | 5.5 ms | 11 ms |
| `mesh::allocator::allocate_and_free` (render) | 2.4 ms | 4.3 ms |
| `drive_fx::tick` (per tick) | | 2.0 ms |
| `texel_upload::write` (HUD tiles to the GPU) | 0.9 ms | 2.8 ms |

## Fixes, each with its own commit

### 1. A model's parts merged by texture

Each model part was its own mesh, entity and upload, and a skinned character re-uploaded every part each tick.
`InspectionModel::merge_parts_by_texture` makes one part per texture for models drawn whole: Sora and his forms, Keyblades, the
party, NPCs, map objects, the room's enemy. Code that selects parts (cutscenes' `mesh_group`, the world map's attributes) reads the
models unmerged.

Three alternating pairs, 40 s each, median of medians, compilers running:

| | Room before | Room after | Twilight Town before | Twilight Town after |
| --- | --- | --- | --- | --- |
| Meshes re-uploaded per frame | 161.8 | 11.5 | 184.1 | 22.7 |
| Entities | 677 | 427 | 1,076 | 791 |
| Meshes | 401 | 152 | 787 | 502 |
| Frame median (ms) | 30.0 | 20.6 | 29.2 | 31.9 |
| Frame p99 (ms) | 84.3 | 68.0 | 81.3 | 85.3 |
| Heap allocated per frame (MiB) | 4.59 | 3.86 | 11.96 | 12.17 |

The upload and entity counts do not depend on the noise. Twilight Town's frame times swung from 18.5 to 37 ms between runs of one
build, so its medians say nothing either way. Captures of Twilight Town and Final Form look right.

<<<<<<< HEAD
### 2. The HUD canvas on its own thread

`hud_present` waited for the software rasterizer. `visual::gauge::CanvasWorker` holds the canvas on a thread of its own: the main
thread builds the draw list and hands it over, and takes the finished tiles on a later frame, so the HUD is up to a frame or two
behind the tick. One job runs at a time and the next draw waits for it. Each sheet the HUD uses is copied once for the worker.

Three alternating pairs at high process priority (the compilers go behind the game), 40 s each, median of medians. At that priority
both builds sit at about 60 FPS, the median at 16.7 ms whatever the build, for a reason not traced (an older build does the same), so
only the tails show the difference:

| | Room before | Room after | Twilight Town before | Twilight Town after |
| --- | --- | --- | --- | --- |
| 1% low (FPS) | 33.5 | 40.4 | 30.6 | 32.0 |
| Frame p99 (ms) | 26.2 | 20.6 | 29.2 | 27.1 |

A `perf-trace` run of the after build at high priority, per frame: the rasterizer `hud_raster` takes 2.2 ms in the room and 4.4 ms in
Twilight Town, now off the main thread.

| Per frame, after both fixes, high priority | Room | Twilight Town |
| --- | --- | --- |
| Main app | 4.7 ms | 6.9 ms |
| Render schedule | 7.4 ms | 9.9 ms |
| `mesh::allocator::allocate_and_free` | 1.7 ms | 3.1 ms |
| `drive_fx::tick` | | 1.3 ms |
| Update / FixedUpdate schedules | 1.5 / 1.1 ms | 1.5 / 2.2 ms |

### 3. No copies of an effect's mesh data

The two effect presenters copied each batch's vertex lists into the new mesh, then dropped them; they now move them. Two alternating
pairs, high priority: heap allocated per frame 3.59 to 3.73 MiB in the room and 6.75 to 6.82 MiB in Twilight Town, p99 24.6 to 22.2 ms
and 28.4 to 27.8 ms. All of that is within this PC's noise: no measurable gain, a simpler loop.

## Not done, with why

- **GPU skinning:** the characters' uploads are down to about 12 to 23 meshes a frame; what they still cost is most of the mesh
  allocator's 1.7 to 3.1 ms. Worth doing only if the render thread becomes the limit.
- **Effect prewarm:** the run logs show no spikes at a first cast above the noise (hitches fall on ticks with no first effect), so
  there is no measurement to fix against yet.
- **Splitting the chained Update:** the Update schedule takes 1.5 ms a frame; there is little to gain.

=======
>>>>>>> upstream
## Checks

| Check | Result |
| --- | --- |
| Unit tests: parts with one texture become one with their UVs; a model without textures is one part | PASS |
<<<<<<< HEAD
| The canvas's tests; captures of Twilight Town show the HUD whole with the worker | PASS |
| `clippy --all-targets -D warnings`, `cargo test --locked` | PASS, but for `kh3_menu`'s test, in another session's work under way |
=======
| `cargo fmt --check`, `clippy --all-targets -D warnings`, `cargo test --locked` | PASS |
>>>>>>> upstream
| Side by side with the installed game | NOT RUN |

## Uncertainty

- This PC had every core busy with other sessions' builds through these measurements.

**Verdict:** PARTIAL. Each fix's table stands; real play is not measured.

<<<<<<< HEAD
**Next task:** measure on a quiet PC; find what holds an unattended `--uncapped` run near 60 FPS at high priority.
=======
**Next task:** see the fixes still open in this record.
>>>>>>> upstream
