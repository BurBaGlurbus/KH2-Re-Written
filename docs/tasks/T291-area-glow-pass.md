# T291 — The area glow post-effect (dk::Glow)

Owner Claude, 2026-10-30, requested by burba in the project thread ("start working on the github issues and getting them
out of the way"). Issue #21. Gate: development of the map renderer; no final gate is inferred. Prerequisites: T097 (areas),
T229 (the scene blends in gamma space). Build identity: local `main` after `e31a43c`, package 0.1.2. Source: the selected
Steam KH2FM build, validated each run; the install was only read.

Private evidence (outside the checkout): `research\playable-room-20261008\tt-world-01\draw-03\draw3-spec.md` §1 and
`draw-01\draw-spec.md` §4.7; `research\issues-20261030\g\` (FUN_1401a0c00, FUN_1401ae760: the FRAME mask that skips the
stencil in the alpha pass); runs in `research\issues-20261030\runs\`.

## What the executable does

- `dk::Glow` (FUN_140169bd0) runs after the 3D scene and before the HUD, when its flag is set: arif row flag bit 4 at each
  map load (FUN_1403a5a80), event start turns it on (FUN_1401e7d30), SeqGlow (0x43) switches it (0 = on).
- There is no threshold and no strength. Its source is a stencil written by the map's and the sky's opaque chunks
  (FUN_1401ba2c0, FUN_1401b9090): 0xff for triangles with a vertex alpha over 0x80, 0 for the rest. The alpha pass masks the
  frame's alpha (FBMSK 0xff000000), which turns the stencil path off.
- The pass: masked 1:1 copy, 4:1 shrink, six iterations of (horizontal, vertical) pairs where each is a plain copy and a 50%
  blend of a shifted copy (table 0x1405b1f80), then an additive composite at weight 1.

## Change

- `visual::glow`: the pass as the exe's quads, one vertex buffer per size, quarter-size targets, `Rgba8Unorm` so it runs on
  the stored (gamma) values like the scene. Unit tests hold the quads to the spec's per-iteration table.
- The room has no stencil, so the scene's alpha channel holds it. `spawn_room` gives map and sky opaque chunks a `UV_1`
  class per triangle, `fog.wgsl` writes it to alpha, and on a glow camera every other pipeline (characters, BOBs, the alpha
  pass, effects) masks alpha off. The camera clears alpha to 0 and the pass puts it back to 1 before the frame goes on. A glow
  camera is marked by an unused pipeline key bit (its shadow filter), so the camp's character cameras keep their alpha.
- The flag: `AreaSource::glow` over the arif row (shared `arif_row`), followed per area by `area_glow` in the room, read at
  load by the world map, and `glow_switch` over SeqGlow in events.

## Checks

| Check | Result |
| --- | --- |
| Unit tests: the blur quads, the triangle class, the arif flag | PASS |
| `cargo clippy` on the changed crates: no warnings in the changed files (HEAD's own warnings in `source/data/src/kh3` and `launcher.rs` fail `-D warnings` and are not from this change) | PASS for these files |
| Release runs in tt01, tt05, tt06, tt08 and the Axel and Roxas event on a clean export of HEAD plus this change: no validation errors | PASS |
| The tt08 autoplay's exit code | 1 with the glow on or off: its capture at tick 111 falls in the closed iris after the exit at tick 34, and a uniform capture fails the run. Not from this pass |
| tt08 captures with the glow on and off (a test-only switch in the export): the sky and horizon bloom, and light bleeds over the roof edges, only with it on | PASS |
| Side by side with the installed game | NOT RUN |

## Uncertainty

- Bilinear filtering of the targets is [I] (draw3-spec §5.1); the half-pixel shifts only blur with it.
- The texture bank's mode byte (+0xa8, `DAT_140ad7750`: 2 = 0xff for everything, 3 = two passes) is treated as 0. Its writer
  is not traced.
- BOB and character draws go through other commands; `FUN_1401bb350` also reads `DAT_140723bb0`, so a second stencil path may
  exist for some models. Not read.
- The title's and main task's glow objects (FUN_140169130) have no scene here.

## Verdict

PASS for the pass in the room, the world map and events; not compared with the installed game. Next: #22.
