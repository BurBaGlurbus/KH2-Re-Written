# T277 — Motion frame events 13 / 14 request the object's voice

Owner Claude, 2026-10-28, requested by burba in the project thread ("keep working dont idle"). Gate: development of
enemy AI in the test room; no final gate is inferred. Prerequisites: T276 (the roster's list of motion events nothing
carries out). Build identity: local `main` after `5b09ea5`, package 0.1.2, release launcher with the `skate3` feature.
Source: the selected Steam KH2FM build, validated each run; the install was only read. Ghidra 12.1.4 headless on
private read-only project copies (plain Ghidra, no MCP).

Private evidence (outside the checkout): `%LOCALAPPDATA%\KH2FinalRewrite\research\drive-magic-20261026\g\range\`
(FUN_14040dd70), `research\enemy-1to1-20261027\g\vt98\` (the vt+0xa0 slots), `g\ev\` (FUN_1403dad90,
FUN_1403ce580), `g\bb4\` (FUN_1403d5840, FUN_1403ce260, FUN_1403ce5c0); run logs in a private run folder outside the
checkout (`roster-08`, `roster-10`, `combat-12`).

## What the executable does

FUN_14040dd70 cases 0xd and 0xe (objects with `+0x6c8` bit 0) call the class's vt+0xa0 with the first parameter and,
if the record has at least two, the second (0 otherwise). This is the same slot `obj_voice` (1:262) calls. For the
ZAKO and ENEMY classes the slot is FUN_1403dad90 (FUN_140419cc0 / FUN_1403db630), and FUN_1403dad90 passes the request
to the object's voice player at `+0xbb4` (the YS::VOICE_IOP that FUN_1403d5840 sets up). FUN_1403ce580 keeps the
request when nothing is queued or when its priority is at least the queued one's, and FUN_1403ce260 plays it on the
next update.

## Changes

- `enemy::motion::Effect::Voice` for frame events 0xd / 0xe; `Object::carry_out_motion_effects` posts it as the same
  `Event::Voice` `obj_voice` posts, so the host's priority voice slot takes it.
- Unit test `frame_events_13_and_14_request_a_voice`.

## Checks

| Check | Result |
| --- | --- |
| Roster of three frame-13 users (`roster-10`: B_CA020, N_EX500_BTL, B_LK110_PHANTOM) | PASS: no faults; frame 13 is gone from their unhandled events (range 0x17 remains on two); 20 voice requests logged |
| Combat tour in the flat arena (`combat-12`) | PASS: 16 hits on the enemies, 2 on Sora |
| `rustfmt --check`, `cargo clippy --all-targets --locked -- -D warnings`, `cargo test -p enemy --locked` | PASS |
| Release launcher (`tools\Build-Launcher.ps1 -Skate3`) | PASS |
| The voices heard | NOT RUN: the room has no voice bank for these objects (`bank: null`), so nothing plays |
| Side by side with the installed game | NOT RUN |

## Uncertainty

- The `+0x6c8` bit 0 test is not modelled; every class that reaches here derives from STDOBJ.
- The bosses' class slot (FUN_1403f8c10) was not read; it is assumed to be the same request.

## Verdict

PARTIAL. The requests reach the voice slot; no voice was heard, and nothing was compared with the installed game.

## Next

The voice banks of these objects. Range 0x17 (FUN_1403cd890 → FUN_1401a8a20, the effect set at `+0x7b0`).
Frames 26 / 27 (the colour fades). Range 0x33 (FUN_1403db080, a sound slot of the object). M_EX630's frame 25 needs
nothing more: it sits at frame 0, where hit ids already start fresh for each motion.
