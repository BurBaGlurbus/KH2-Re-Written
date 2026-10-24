# T226 — Refraction programs 145 / 160: the effect VM side (renderer not built)

Owner Claude, 2026-10-24, requested by burba in the project chat ("finish all summons, limits, and
party stuff until its 1:1"; "keep going"); stopped at the user's "wrap up what they're doing and put
them as issues and pull requests". Follows T225 (Drive form effects).
Gate: development only; no gameplay acceptance. Build identity: branch
`claude/refraction-programs` from `claude/project-thread-ww9y8z` after T225, package 0.1.2,
Rust 1.99.0 Windows MSVC. Source: the selected Steam KH2FM build, only read.

Private evidence root: `%LOCALAPPDATA%/KH2FinalRewrite/research/party-1to1-20261023/`:
`refraction-spec.md` with headless Ghidra outputs `out/hz/` (FUN_1404cfa90, FUN_1404ca9e0, their
GS callbacks, the frame-texture copy, the OT, decoded Quick Run data).

## What the executable does

pppKeShpDtt (160) draws an eight-sided disc facing the screen at its centre's depth, fading to its
rim, each vertex sampling a copy of the current frame at its own screen place moved by −strength ×
the matrix's Y axis (the rim also by its z); flags: rim alpha, screen mode (layer 4), no depth
test. pppKeMdlDtt (145) draws a model's untextured triangles, each vertex sampling the previous
output frame moved along its direction from the model origin by world scale × strength. Both
integrate one strength (value, rate, acceleration) in their draw, gated on pause and even part
frames, the key added after the integration.

## What the code does now

- `effect::refraction` produces both programs' geometry as `Refraction` records (vertex, the
  draw-space point it samples, colour, frame source, depth test, order); create and restart zero
  the strength; the OMD mesh reader reports each block's type (145 stops at the first non-zero
  type).
- Not built: the room's renderer does not draw `Instance::refractions` yet. The planned pass copies
  the scene image each frame and draws these meshes sampling it (one frame late for 160, whose exe
  copies the current target; 145 samples the previous frame as the exe does, without the HUD).

## Checks

| Check | Result |
| --- | --- |
| `cargo fmt --all --check` | PASS |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | PASS |
| `cargo test --workspace --locked` | PASS (new synthetic test: the Quick Run-style disc, its shift and rim) |
| `cargo build --workspace --locked` | PASS |
| Smoke `cargo run -p main --locked -- --frames 180` | PASS |
| Release `build-release-skate3.bat` | NOT RUN (work stopped at the wrap-up request) |
| A run showing the haze | NOT RUN (no renderer yet) |

## Uncertainty

- [U] The device scale values (145's v axis is divided by 448 in the exe), when the frame copy's
  flag resets, 145's inherited depth state, the blend set of ALPHA 0x44 on D3D (refraction-spec §5).
- [I] Screen mode's centre is taken as a draw-space point at depth 512.

## Verdict

PARTIAL: effect-side geometry only; nothing is drawn or compared with the original game.

## Next

The renderer pass (scene copy + refraction meshes in drive_fx and the event presenter).
