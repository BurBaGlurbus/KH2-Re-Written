# T289 — Range-held motions come down and land, and `obj_turn_dir` arrives

Owner Claude, 2026-10-30, requested by burba in the project thread. Gate: development of enemy AI in the test room; no
final gate is inferred. Prerequisites: T264 (the ground events), T287, T288. Build identity: local `main` after
`bd71527`, package 0.1.2, release launcher with the `skate3` feature. Source: the selected Steam KH2FM build, validated
each run; the install was only read. Ghidra 12.1.4 headless on private read-only project copies (plain Ghidra, no MCP).

Private evidence (outside the checkout): `%LOCALAPPDATA%\KH2FinalRewrite\research\drive-magic-20261026\g\range\`
(FUN_1403b9090, FUN_1403b89a0, FUN_1403b7ca0, FUN_1403b7890, FUN_1403cb100, FUN_1403b9f70),
`research\enemy-1to1-20261027\g\land`, `vt58`, `vt0.txt`, `bfa40`, `ba350`; run logs in a private run folder (`sweep2`,
`probe2`, `sweep3`).

## What the sweep showed

Three kinds of enemy stood still until the end of a run:

- M_EX420 and M_EX420_NM (`footwork`, motion 203) and M_EX910 (`atk_side` / `form`, motion 4): their next act gated by
  FUN_1403ba9a0, under a range 0x1e that lasts the whole clip.
- M_EX630 (`atk_kiriage`, motion 210): at the apex of a jump, under the same range.
- M_BB010_SWORD (`jump` / `move_roll`): in an `obj_turn_dir` loop that never ended.

T288 called the first two possibly real; that was wrong. Scripts like M_EX420's OBJ_LAND show the design: it waits for
trigger 0xe with motion 203 playing.

## What the executable does

- **The land branch.** FUN_1403b9090's land branch is the `else` of "the mode's flag bits 0 and 1 clear, no
  `+0x18c & 0x1000020`, no `+0x18c` bit 6". It is not only for jumps. It lands when the wished move is not rising,
  FUN_1403ba1e0 holds (a jump mode, or `+0x18c & 0x1000020`, or `+0x120` bit 23, and no range 3, and mode flag bit 9
  clear) and the solve found a floor (`+0x744` bit 0 clear). So an object in the ground mode, on a floor, under range
  0x1e fires the land events every tick: the act event, script trigger 0xe, the mode's slot 6 (FUN_140409b40, nothing
  for mode 0), then the class's vt+0x58 (FUN_140419bc0 / FUN_1403db530 → FUN_1403b7890: motion 5 with idle behind it
  if the set has it and not `+0x194` bit 4, mode 0, `+0x120` bit 27).
- **Root height.** FUN_1403b89a0 adds the motion's root change (FUN_1403b7ca0) in every mode, height included. Under
  `+0x18c & 0x1000020` FUN_1403cb100 adds no height of its own, so an authored fall comes down by its root. Otherwise
  FUN_1403cb100 aims at the arc's height, taking the root's height back on its next step.
- **`obj_turn_dir`.** FUN_1403b9f70 ends in FUN_1403b6560, which sets the wished heading too, and within reach it sets
  the target's heading exactly.

## Changes

- `Sysobj::range_land`, `land_on_range`, `Ground::RangeLand`; `Object::finish_ground` runs them.
- `Sysobj::update_jump`: the root's height is in the step's move.
- `Sysobj::turn_dir`: sets the wished heading; lands on the target's heading.
- Unit test `turning_takes_over_the_wish_and_lands_on_the_heading`.

## Checks

| Check | Result |
| --- | --- |
| Sweep with these changes (`sweep3`, the first 12 programs, 43 models) | PASS: no faults; no enemy stands still past 500 ticks; M_EX420_NM, M_BB010_SWORD move (before: 1395 and 1187 ticks still) |
| `cargo test --locked -p enemy` | PASS but for 2 tests of another thread's uncommitted change in `object.rs` |
| Release launcher (`tools\Build-Launcher.ps1 -Skate3`) | PASS |
| Side by side with the installed game | NOT RUN |

## Uncertainty

- The range land needs a map body here; without one no floor test is made.
- The mode's flag bit 9 in FUN_1403ba1e0 is not tested: no mode this engine runs has it.
- The jump step's root change is carried out on the arc's clock; it was not compared against a captured fall.

## Verdict

PARTIAL. The stuck acts move in tours; not compared with the installed game.

## Next

Whatever `sweep3` still shows; the Gambler's cards and dice (`player_capture_form`, `player_dice`, `player_card`:
Sora's own modes 0x2b and 0x2e, so the 0x2c and 0x31 that T287 called unreachable are reached through M_EX950);
`enemy_summon_entry` (M_EX740).
