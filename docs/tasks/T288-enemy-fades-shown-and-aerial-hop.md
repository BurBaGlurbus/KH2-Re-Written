# T288 — Enemies' alpha fades and hiding are drawn; an aerial hit hops from any mode

Owner Claude, 2026-10-30, requested by burba in the project thread. Gate: development of enemy AI in the test room; no
final gate is inferred. Prerequisites: T287. Build identity: local `main` after `57f68f2`, package 0.1.2, release
launcher with the `skate3` feature. Source: the selected Steam KH2FM build, validated each run; the install was only
read. Ghidra 12.1.4 headless on private read-only project copies (plain Ghidra, no MCP).

Private evidence (outside the checkout): `%LOCALAPPDATA%\KH2FinalRewrite\research\enemy-1to1-20261027\g\` (`dd70`, the
disassembly of FUN_14040dd70; `s3`, FUN_140409ad0 / FUN_140409b00); run logs in a private run folder (`probe2`).

## What the executable does

- **Frame events 0x17 / 0x18** (FUN_14040dd70): unless leaving (`+0x9b8` bit 4), FUN_1403b7810 on the alpha at `+0xa08`,
  1.0 to 0.0 or 0.0 to 1.0, over the first parameter in ticks. M_EX910 uses them.
- **An object's alpha** starts at 1.0. `obj_fadein` / `obj_fadeout`, the frame events and the leaving code all run
  through it.
- **Slot 3 of every physics mode** is FUN_140409ad0 (FUN_1403cbb00, then mode 1). So FUN_1403d4800's aerial hit makes a
  jump from any mode, armed like any jump (land motion 5, no fall motion). T287 hopped only from jump modes, which left
  objects in a range-held motion with their acts gated.

## Changes

- `motion::Effect::AlphaFade` for frame events 0x17 / 0x18.
- `Sysobj`: the alpha starts at 1.0.
- `enemy_gallery::present`: each part's material takes the object's alpha (blended below 1.0, opaque again at 1.0);
  a class-hidden object (`obj_hide`, `obj_model_hide`) is not drawn. Until now every enemy was drawn opaque whatever
  its script asked.
- `Sysobj::exec_damage_small`: the aerial hit's jump from any mode.
- `boss_state` carries the turn rate.

## Checks

| Check | Result |
| --- | --- |
| Probes on he09, hb25, bb00, lk04 (`probe2`) | PASS: no faults; M_EX910's frame events no longer unhandled |
| `cargo test --locked -p player -p enemy -p area` on the staged changes | see below |
| Release launcher (`tools\Build-Launcher.ps1 -Skate3`) | PASS |
| The fades looked at | NOT RUN: no capture was reviewed |
| Side by side with the installed game | NOT RUN |

## Uncertainty and open behaviour

These enemies still stand still for long in the tours, and the cause is not found:

- **M_EX630 `atk_kiriage`.** Its fall motion 210 carries a range 0x1e over the whole clip (`+0x18c` bit 24), and
  FUN_1403cb100 takes no vertical move under that bit, so it hangs at the apex. Nothing in its script or the motion's
  frame events ends the jump.
- **M_EX910 `form` / `atk_side`.** Its motion 4 carries the same range, and an act with flag bit 0 clear is gated by
  FUN_1403ba9a0 while that motion plays (FUN_1403ca550).

Either the executable has a path out that was not found, or these really behave so. Only a comparison with the game can
tell; nothing was invented for them.

## Verdict

PARTIAL. Fades and hiding are drawn and the aerial hit follows the executable; not compared with the installed game.

## Next

The two open behaviours above; the rest of `sweep2`.
