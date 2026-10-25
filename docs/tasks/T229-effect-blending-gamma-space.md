# T229 — Effects shaded and blended in the PC's gamma space

Owner Claude, 2026-10-25, requested by burba in the project chat after T228 ("it's alright but I need you to fix
the thunder, it still shows big white sheets, and fix overall particles, they are still square sheets").
Gate: development of the test room, world map and cutscene effect presenters; no final gameplay acceptance is
inferred. Prerequisites: T228. Build identity: branch `claude/vfx-particles-thunder` from local `main` at `79b8b78`
(T228 merged locally), package 0.1.2, Rust 1.99.0 Windows MSVC. Source: the selected Steam KH2FM build, validated
each run; the install was only read.

Private evidence root: `%LOCALAPPDATA%/KH2FinalRewrite/research/vfx-fix-20261024/` (Ghidra outputs `g/a02`, `g/d08`,
`g/d09`, `g/dd1`, `g/refs2`); runs, captures, page dumps and contact sheets in the private `kh2-vfx-out` folder
outside the checkout (`dense-base`, `dense-b`, `f5-*`, `donald-*`, `prize-*`, `census5.txt`, `pages/`, `ref/`).

## What the executable does

- Effect sprites and models are shaded by pixel shader 026: texel × vertex colour, both on the stored values. The
  colour is the PS2 bytes scaled by 1/128 (FUN_14013fcc0 called with 0.0078125 from FUN_1404ae220 and
  FUN_1404b6860), so 0x80 = 1.0. The shader then applies the GS alpha test.
- The effects' GS TEST is 0x7000d (FUN_1404af5e0 copies it from 0x1405fa950 to DAT_142b1f110; FUN_1404adde0 and
  FUN_1404dc760 restore it): alpha test GREATER with AREF 0, so only texels whose alpha is 0 are dropped
  (FUN_1404724e0 maps ATST to the D3D comparison table at 0x1405e6490).
- The result goes through the D3D blend of the draw's ALPHA byte (T228) into the back buffer without sRGB
  conversion, so every blend adds or mixes the stored (gamma) values.
- FUN_1404dc760 only applies a draw's depth-only (+0x80) or always-pass (+0x81) TEST on the direct path,
  descriptor flag 4 (pppDrawMdl3 key byte +8 bit 1). No spell, Sora, Keyblade or Donald draw sets key byte +0xd
  (census of every DrawMdl3 key), so no Thunder plane is depth-only: its untextured planes are additive glows with
  dim or fading vertex colours.

## What was wrong

The rewrite blended effects in linear light (sRGB render target, vertex colours raised to 2.2). N overlapping
additive layers then add up to only about N^0.45 times one layer, and a faint edge (alpha 5%) shows at about 25%
brightness on a dark background. Soft puffs became flat, evenly lit slabs with visible quad outlines, while the
dense cores of Thunder, Fire, Magnet and Reflect stayed dimmer than the game's white-hot flashes.

## What the rewrite does now

- `visual::effect_draw::EffectMaterial` (`effect.wgsl`): texel × vertex colour on stored values (pages are
  `Rgba8Unorm`, colours 0x80 = 1.0), alpha test alpha > 0, clamp, then alpha (straight), add (premultiplied, alpha
  0), replace (premultiplied, alpha 1) or subtract (the multiply pipeline turned into a reverse subtract). The
  subtractive draws (0x42) that T228 left out are drawn.
- `scene_compositing()`: the test room scene camera, its cross-fade snapshot camera, the world map camera and the
  cutscene camera composite in sRGB space (`CompositingSpace::Srgb`, an `Rgba8Unorm` main texture). `EffectPlugin`
  patches Bevy 0.19.1's `tone_mapping` once its shader library loads so that a camera whose colour grading carries
  the mark (every section's gamma 2.2) encodes the PBR output with the exact sRGB curve. Opaque models and rooms
  keep their look; every blend in those cameras now runs on sRGB values.
- A draw whose texture is missing is logged once (`effect texture missing: …`) and left out; a missing DPD gives a
  clear page instead of Bevy's white default image.
- `pppDrawMdl3` depth-only needs key byte +8 bit 1 as well as +0xd.
- `--capture-at` overrides the magic tour's own shots (frame-by-frame checks). `KH2_FX_TRACE=1` logs every running
  spell effect's sprite count, nearest sprite depth and model draws per tick (`fx_trace`).
- The private census example also lists every OMD model (header, meshes, extent) and can dump remastered pages
  (`KH2_DUMP_PAGES`, private output only).

## Checks

| Check | Result |
| --- | --- |
| `cargo fmt --all --check` | PASS |
| `cargo clippy --all-targets --locked -- -D warnings` | PASS |
| `cargo test --locked` | PASS (new tests: blend pipelines and shader modes, the scene camera's compositing mark) |
| `cargo clippy --workspace --all-targets --features main/skate3 --locked -- -D warnings` | PASS |
| `cargo test --workspace --features main/skate3 --locked` | PASS |
| `cargo build --locked` | PASS |
| Smoke `cargo run -p main --locked -- --frames 180` | PASS |
| Release `tools/Build-Launcher.ps1 -Skate3` (private target dir) | PASS |
| `python tools/progress.py`, `python tools/test_progress.py` | PASS |
| Unattended `--magic-tour` with `--capture-at` every 2–4 ticks over the six ground casts and the three air casts, before / after (private `dense-base`, `dense-b`, 395 frames each) | exit 1 from the frame limit as intended, every capture written; no missing-texture report; opaque scene unchanged; see the verdict |
| Final Form (`--magic-form 5`) Thunder ground and air every 3 ticks, Donald's casts, `--prize-tour`, before / after (private `f5t229-*`, `donaldt229-*`, `prizet229-*`) | exit 1 from the frame limit (prize tour exit 0), every capture written, no missing-texture report; prize, heal and drive sparkles keep soft shapes |
| Comparison with KH2FM HD footage (a public particle reference video of the -ga spells, frames sampled in the browser, kept private) | Thundaga floods large parts of the screen with soft white light and white puffs, Firaga's puffs are near white, Reflega starts as an almost opaque white sphere, Magnega is a bright white dome; the gamma-space result has the same character, the old linear one was dimmer and flatter |

## Uncertainty

- [I] The PC's back buffer format was not read from the swap-chain creation; the shader and blend states only
  imply a non-sRGB target. The footage comparison supports it.
- [I] Bevy's PBR colour path decodes sRGB textures and the encode inverts that exactly; vertex colours raised to
  2.2 are re-encoded with the sRGB curve, which differs slightly from the gamma 2.2 they were raised with
  (unchanged from before).
- The air-cast Thundaga still turns the screen white for a few frames while its puffs pass close to the camera;
  not compared with the game's air cast. Whether the PC culls sprites that close was not traced.
- Final Form Thundaga strikes around the arena; when one lands beside the camera, the camera is inside the strike's storm dome (model 7, additive, texcommon's grey cloud page) and sees a grey slab. It is drawn as authored and looks the same before this task; not compared with the game.
- The colour depth of Magnet's and Reflect's cores where several spells overlap was not compared with the game.
- World map and cutscene effects were not toured after this change (presenters and cameras changed).

## Verdict

PARTIAL. Effect shading, the alpha test and every blend follow the executable and now run in the PC's gamma space;
Thunder has no hard-edged sheets and particles keep their soft shapes in every captured frame. Nothing is compared
frame for frame with the installed game.

## Next

Trace the PC's swap-chain format and whether sprites near the camera are culled; tour world map and cutscene
effects; compare the air casts with footage.
