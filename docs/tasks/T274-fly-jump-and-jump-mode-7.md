# T274 — The fly-jump arm, frame events at a motion's start, and Dusk's jump (physics mode 7)

Owner Claude, 2026-10-28, requested by burba in the project thread ("keep working dont idle"). Gate: development of
enemy AI in the test room; no final gate is inferred. Prerequisites: T133 / T190 (the engine's motion events and
jump modes), T269 (an area's enemies on its map). Build identity: local `main` after `39d9e6e`, package 0.1.2, release
launcher with the `skate3` feature. Source: the selected Steam KH2FM build, validated each run; the install was only
read. Ghidra 12.1.4 headless on private read-only project copies (plain Ghidra, no MCP).

Private evidence (outside the checkout): `%LOCALAPPDATA%\KH2FinalRewrite\research\enemy-ai-20261016\out\traps-all\`
(trap 1:168, FUN_14042cf50), `research\drive-magic-20261026\g\range\` (FUN_1404233f0, FUN_1403cbb00, FUN_1403c8a40,
FUN_14040e960, FUN_14040dd70, FUN_1403b4da0, FUN_1404229f0, FUN_140422af0, FUN_140422b30, FUN_1403cbd30),
`research\enemy-1to1-20261027\g\` (`modes.txt`, the physics-mode table at 0x140750d30; `vt7.txt`, mode 7's vtable
0x1405cd880; `c4.txt` DAT_140623c48 = -1.0; `c5.txt` DAT_140623b98 = 5.0, DAT_1405b1c74 = 0.95, DAT_1405aaa40 = 0.9);
the Dusk script M_EX990 disassembled in this session's scratch folder; run logs in a private run folder outside the
checkout (`tt05-01` to `tt05-05`, `combat-09`, `seph-05`, `hb08-07`).

## What was wrong (found on tt05's three Dusks)

Two of the three Dusks of tt05's battle set `b_42` stood in their `jump` act at their spawn points for the whole run.
The act is `obj_flyjump(self, v, 155, 0, 1)` and then waits for `obj_is_jump` to turn on and off.

## What the executable does

- **The fly-jump arm.** `obj_flyjump` (1:168, FUN_14042cf50) zeroes the vector's NaN lanes, runs FUN_1404233f0 — the
  jump's own arming FUN_1403cbb00 (its parameters; the fall and land motions back to none and 5) followed by
  `+0x648` = 3 — then starts the motion and, with the last argument, sets the fall and land motions to slot + 1 and
  slot + 2. The engine stored the fly parameters but never set the pending mode, so a frame-0 take-off could not enter
  it.
- **Frame events at a motion's start.** FUN_1403c8a40, starting a motion without a blend, calls
  FUN_14040e960(start, -1.0), which fires the frame events exactly at the start frame; with a blend they fire at its
  end. The engine's first clock step goes from 0 to 2 and never reached a frame-0 event.
- **Dusk's jump is physics mode 7.** Motion 155 has no take-off event: its frame event 3 at frame 26 is
  FUN_1403b4da0(obj, 7) (FUN_14040dd70 case 3). Mode 7 (flags 0x101) enters as the jump does (FUN_1403cbd30) on the
  armed jump; its step (FUN_1404229f0) rises with no horizontal move, then steers (FUN_1403b7ad0 with 0.95 / 0.9) and
  falls, holding while motion 156 has not reached its loop (FUN_1403c7e70); its fall-motion slot (FUN_140422af0)
  changes 155 to 156 five ticks before the apex; its land (FUN_140422b30) returns to mode 0 with motion 157, idle
  queued behind it, and `+0x120` bit 27.

## Changes

- `traps::object::fly_jump`: arms the jump (`launch_jump`) and sets pending mode 3.
- `Sysobj::motion_start`: a motion started without a blend fires its frame-0 events.
- `motion::Effect::JumpMode7` (frame event 3), `Sysobj::enter_mode7`, `update_mode7`, and mode 7's landing in `land`;
  the flag is cleared wherever a jump is cut short.
- `source/data/examples/motion_triggers.rs`: prints a motion set's range and frame triggers from the install.
- The area tour's checkpoints carry the engine state of each actor (`boss_state`, with the motion clock, the pending
  mode and the act manager's flags).
- Unit tests `a_fly_jump_waits_for_the_take_off_then_flies`, `an_unblended_start_fires_the_frame_events_at_frame_0`,
  `mode_7_rises_straight_turns_to_156_and_lands_on_157`.

## Checks

| Check | Result |
| --- | --- |
| tt05, battle program 86 (`tt05-05`) | PASS: all three Dusks leave their `jump` act (leaps up to y 586), then `move_target`, `footwork`, `atk_kick`, `atk_walk`, `atk_craw`, `mode_revenge`; 3 hits on Sora, 8 on the Dusks; no fault (before: two stood in `jump` for 1800 ticks) |
| Combat tour in the flat arena (`combat-09`, with a Dusk) | PASS: 14 hits on the enemies |
| Sephiroth tour to his defeat (`seph-05`) | PASS: defeated at 2182, mission clear at 2369, exit 0 |
| hb08 group pop-in (`hb08-07`) | PASS |
| New unit tests (above) | PASS |
| `rustfmt --check` on the changed files, `cargo clippy --all-targets --locked -- -D warnings`, `cargo test --locked` | PASS |
| Release launcher (`tools\Build-Launcher.ps1 -Skate3`) | PASS |
| Side by side with the installed game | NOT RUN |

## Uncertainty

- In `tt05-05` one Dusk falls without end from about tick 1200 during `mode_revenge`, off the edge of the map; whether
  a blow, the mode-7 steering or a missing wall lets it out was not found.
- The effects of frame events fired at a motion's start are carried out with the next tick's.
- Mode 7's steering takes no move request (as the room's mode 1 does); FUN_1403b7ad0's request terms were not read
  for it.

## Verdict

PARTIAL. Fly-jumps, frame-0 events and Dusk's jump mode follow the executable, and tt05's Dusks fight; not compared
with the installed game.

## Next

The Dusk that falls off tt05's map; the other physics modes the scripts' frame events name (the 0x14, 0x28, 0x2c,
0x31, 0x3f modes of FUN_14040dd70 case 0, and case 4's / 5's routines).
