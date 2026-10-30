# T292 — The map sun's lens programs and its visibility test

Owner Claude, 2026-10-30, requested by burba in the project thread ("start working on the github issues and getting them
out of the way"). Issue #62 (and #23 merged into it). Gate: development of map effects; no final gate is inferred.
Prerequisites: T150 (effect VM), T216 (lens programs 188/189/191), T291 (the glow camera). Build identity: local `main` after
`e31a43c`, package 0.1.2. Source: the selected Steam KH2FM build, validated each run; the install was only read.

Private evidence (outside the checkout): `research\playable-room-20261008\tt-world-01\pax-01\pax-spec.md`,
`research\issues-20261030\g\lens\` (FUN_1404d6e10, FUN_1404d6ef0, FUN_1404d6f90, FUN_1404d6290, FUN_1404d65a0, FUN_1404d6660,
FUN_1404d6970, FUN_1404d6a30, FUN_1404e2010, FUN_1404e2370, FUN_1404d7120); the program table rows at 0x140779a00 + kind·0x50;
census and runs in `C:\Users\ANIMA\kh2-issues-out\i30\`.

## What the executable does

The map suns' effects (every Twilight Town outdoor map, di00, B_EX390) use a second family of lens programs:

- **221 pppKeLnsLpT** — as 189, plus a key's block size and period; its draw then runs the visibility test.
- **223 pppKeLnsClmT** — as 191, keeping the key's size and fade scale.
- **224 pppKeLnsRayT** (name from its behaviour) — a sprite stretched along the flare's axis by the light's screen distance.
- **225 pppKeLnsFlsT** — a flash sprite on the light, its size and alpha falling with the distance.
- **FUN_1404d7120, the visibility test** — draws a small block (key bytes: 32 x 8 pixels on the suns) at the light's screen
  point with the depth test, reads it back and takes `share = white / (white + black)`; the strength takes
  `min(previous · 0.5 + share, 1)`. An instance whose number does not fall on its period keeps the last value.

## Change

- `effect::lens`: the four programs, translated.
- The read-back: the VM publishes the block (`lens::take_probe`), `visual::lens_probe` writes the depth under it to a small
  colour target after the main pass (a depth texture can only be copied whole), copies that to a buffer, counts the pixels
  behind the light when the GPU is done, and hands the share back (`lens::set_seen`). A block off the screen gives 0, as the
  exe's early exit does.

## Checks

| Check | Result |
| --- | --- |
| `effect_census` against the install: 221, 223, 224, 225 | PASS: none listed (118 effects before). Map effects left: 229 in he05 (#65) |
| Unit tests: the block's position and the light's depth; the share counts pixels behind the light | PASS |
| Release run in tt06 with a test-only log: the share follows the view, 1 with open sky at the block, fractions at the walls' edges, 0 behind them | PASS |
| The flare's sprites seen in a capture | NOT RUN: no unattended camera in tt01, tt05, tt06 or tt08 faced the sun with a clear view |
| Side by side with the installed game | NOT RUN |

## Uncertainty

- The share arrives a few frames late: the GPU read is asynchronous. The exe stalls for it in the same frame.
- The block's size is taken in output pixels, the device scale the exe uses (`dev+0xa0`, 1.0).
- Keys' second word at +0x12 on 225 (the two axis modes) was read from the draw's use.
- The name of 224 is not from the exe's string table in this record; the table row is 0x14077e000.

## Verdict

PASS for the programs and the visibility count; the flare on screen is not yet seen in a capture.
