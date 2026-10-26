# T237 — HD remaster textures everywhere, less memory, faster loads

Owner Claude, 2026-10-25/26, requested by burba in the project chat ("i want everything to use the hd
textures, also focus on adding less memory usage and faster loads times in rooms, menus, and general
optimzations"; later "keep working"). Gate: P30 performance engineering, with the PC's texture choice
followed where the install has one; no gameplay rule changes and no final gate inferred. Prerequisites:
[T236](T236-high-frame-rate.md), [T228](T228-effect-textures-uvs-blends.md). Build identity: local
`main` from `1b92156` (end of T236) to `8b2985a`; package 0.1.2, Rust 1.99.0 Windows MSVC. Source: the
selected Steam KH2FM build, validated at each run; the install was only read.

Private evidence: the private output folder outside the checkout (frame records, run logs, captures, load-stage
timings and a Chrome trace: `lt-*`, `hd*`, `ap-*`, `st-*`, `fx-*`, `chk-*`, `ff-*`, `trace-start-1`,
`f2-*`). Executables used for the comparison: `KH2RW-final.exe` (before, `1b92156`),
`KH2RW-after.exe` (`8b2985a`) and `KH2RW-nohd.exe` (`8b2985a` with a temporary, uncommitted patch that
turns the 3D HD images off; the HUD's are required by the source check).

## What changed

HD remaster textures (the PC draws the replacement image where exactly one matches; otherwise the
original, as before):
- Models (Sora, party, NPCs, weapons, event actors) and enemies bind the HD replacement of each
  environment; each image is uploaded once per texture set and kept on the GPU only. Event actors'
  environments with texture animation (eyes, mouths) keep the original, whose pixels the patches paint.
- System, map and party-limit effects attach their HD pages like the spell and drive effects already did.
- Field maps and skies: a map's HD images are per environment region (its CLAMP crop), mostly at the
  original size in true colour and some at 4×, stored as BC3 mip chains; the base level replaces the
  cropped original. Environments with TEXA animation keep the original. Maps carry more than 64 images,
  so they read under a separate, still finite bound (1,024 entries, 1 GiB).
- Map objects: their HD images are region crops too; each is placed into a copy of its original page
  (scaled by the whole multiple), so the model's UVs address it unchanged. Other sizes keep the original.

Memory and load (each its own commit; behaviour checked after each):
- Sora's motion frames are skinned when shown instead of all at load; his kit, Donald's and Goofy's
  motions, the party-limit sheets, townspeople and map-object animations evaluate on several threads.
- Donald and Goofy load on their own threads, alongside Sora's kit; camp, journal and music, the world's
  footstep and ambient banks, and the effect sources with the enemies' sound banks are read on threads
  started at the beginning of the load and joined where they were used before (same registration order).
- The GPU device is created on a thread while the data decodes (Bevy's own `initialize_renderer` with
  its default settings, handed to `RenderPlugin` as manual creation; a failure falls back to Bevy's own
  creation), in the field, world map, events and title. This was about a second after the decode before.
- An area's room, HD textures and map objects decode alongside its townspeople on a door change.

## Results

Medians of three alternating runs per build and scene, uncapped, the same autoplay as T236; see
[docs/perf/results.md](../perf/results.md#hd-textures-memory-and-loads-t237).

| | Test room before | after | Twilight Town before | after |
|---|---|---|---|---|
| Data decoded (ms after start) | 7,039 | **2,870** | 4,161 | **2,205** |
| First frame (ms after start) | 8,450 | **3,525** | 5,342 | **2,903** |
| Door change, background decode (ms) | — | — | 2,150 | **777** |
| Process memory, peak (GiB) | 1.92–2.08 | **1.39–1.57** | 1.61–1.63 | **1.24–1.27** |

With the 3D HD images off (same build otherwise), process memory peaked at 1.43–1.46 GiB in the room and
1.28–1.29 GiB in Twilight Town: the HD images cost no measurable process memory, since they are kept
only on the GPU. GPU memory was not measured.

Frame rate is unchanged within this PC's noise: field play 221.7 → 205.5 FPS in the room and 147.1 →
157.4 in Twilight Town (medians; one "after" room run averaged 125), while the no-HD batch, run later
with the same build, averaged 171 and 131. The camp menu's 282 → 259 FPS in the room is inside that
spread; it was not traced further.

### After the comparison (`2a7b2a9`–`3e470b0`)

Measured against the build before each change, three alternating runs each (single runs on this PC
vary by up to a factor of two, so only consistent shifts are claimed):
- Party limit files read from the start of the load: room decode 2.7–3.9 s → 2.5–2.8 s.
- Townsperson kinds load on several threads: Twilight Town's door change 0.69–0.71 s → 0.51–0.55 s.
- Model HD replacements decode on several threads (admitted in order, first error in order wins): the
  models stage about 100 ms shorter (Sora's six HD images took about 170 ms one after another).
- Motion sets decoded once per load (the last three kept, released after the room's load; the source
  checks still run on every read): Sora's kit 0.85–0.90 s → 0.61–0.70 s; room decode about 2.3–2.5 s.
- Tried and left out: spell effect files and limit files on further threads. Each stage left the main
  path, but the decode total did not move: the kit's own work is the critical path now.

## Checks

| Check | Result |
| --- | --- |
| `cargo fmt --all --check` | PASS |
| `cargo clippy --workspace --all-targets --features main/skate3 --locked -- -D warnings` | PASS |
| `cargo test --workspace --features main/skate3 --locked` (1,063 tests; new: HD-or-original binding, region placement, map replacement bound, ordered parallel replacement decode) | PASS (`frame_limit::the_app_holds_the_cap` failed once with the PC under heavy load, then passed alone and in the full run) |
| `cargo build --release -p main --bin KH2RW --features skate3,perf-diag --locked` | PASS |
| Captures: Twilight Town map, map objects, arena, world map, an event, with HD | PASS: textures in place, nothing shifted; the map's stone and floor are cleaner |
| Gameplay events, unattended room run, after each load change | PASS, identical (`event_diff.py`) |
| Captures at equal ticks after each load change | PASS at ticks 144 and 1900; a strip near the top at tick 1200 also differs between reruns of the unchanged build (3 of 3), so it is not attributed |
| Same adapter and backend with the early device | PASS (RTX 2070 SUPER, Vulkan, as Bevy picks it) |
| `cargo build --locked`, smoke `cargo run -p main --locked -- --frames 180` | PASS |
| Comparison with the installed game | NOT RUN |

## Uncertainty

- Which replacement the PC draws is inferred from the replacement's original-asset offset (the same
  rule as the model textures, `model_texture_routing`); a map image that is not exactly one match, or
  not a whole multiple of its region, keeps the original. Not compared with the installed game.
- The early device's adapter is requested without the window's surface, which does not exist yet. On a
  PC with several GPUs it could pick an adapter Bevy would not have; it falls back only on failure.
- Loads now overlap; on a busy CPU the stages slow each other (Sora's kit took 0.6 s → 0.8 s while the
  party loaded beside it), and single runs on this PC vary by up to a factor of two.
- The decode of HD map images is on the area's worker, adding about 0.1–0.2 s there before the overlap.

## Not done

- GPU skinning: KH2 skins each vertex as the sum of `matrix × (weighted position)` over its influences,
  each with its own position, which Bevy's skinning (one position, weighted matrices) cannot express; it
  needs a custom vertex shader and per-entity matrix buffers. Several systems also read the CPU-skinned
  positions (effect anchors, afterimage, diagnostics), and GPU arithmetic would not be bit-identical.
- Effect prewarming/pooling: the frame records show no slow frames at the first casts (Thunder at ticks
  77 and 318, Fire and others later), so there is nothing to fix there yet.
- Parallel `Update`: most systems share `ResMut<Game>`; splitting it is a large change, and the main
  thread is not the limit at the measured frame rates.
- Menus: the camp opens and pages with no frame over 31 ms; its data is loaded at start.

## Verdict / next

PASS for the measured load and memory work and for the HD texture paths checked in captures; PARTIAL for
HD fidelity (not compared with the installed game). Next: Sora's kit (0.6 s, its own clip evaluation),
Bevy's first-frame pipeline compiles (about 0.2 s), and a frame-for-frame HD comparison with the
installed game.
