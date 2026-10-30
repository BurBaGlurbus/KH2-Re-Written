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

## Checks

| Check | Result |
| --- | --- |
| Unit tests: parts with one texture become one with their UVs; a model without textures is one part | PASS |
| `cargo fmt --check`, `clippy --all-targets -D warnings`, `cargo test --locked` | PASS |
| Side by side with the installed game | NOT RUN |

## Uncertainty

- This PC had every core busy with other sessions' builds through these measurements.

**Verdict:** PARTIAL. Each fix's table stands; real play is not measured.

**Next task:** see the fixes still open in this record.
