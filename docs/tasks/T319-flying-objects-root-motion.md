# T319 — Flying objects move by their motions' root; the floor ray takes the body's class

Owner Claude, 2026-10-31, requested by burba in the project thread (issue #163). Gate: development of enemy AI in the test
room; no final gate is inferred. Prerequisites: T318. Build identity: local `main` after `1dc214c` plus this change, package
0.1.2, release launcher with the `skate3` feature. Source: the selected Steam KH2FM build, validated each run; the install was
only read. Ghidra 12.1.4 headless on private read-only project copies (plain Ghidra, no MCP).

Private evidence (outside the checkout): `%LOCALAPPDATA%\KH2FinalRewrite\research\enemy-1to1-20261027\runs-20261031` (sweeps
`f5`, `f6`, `t8`, `t9`), decompiles in `g\st`, `g\fly`.

## What was wrong

The Lords (B_AL100_FIRE, B_AL100_ICE) enter physics mode 5 in OBJ_INIT and stayed 275 above the ground for good. B_AL100_FIRE's
`atk_bound` waits for `obj_float_height` above -30, so it held motion 202 forever.

## What the executable does

- FUN_1403b89a0 adds the motion root's change in every mode. T261 did this for jumps and flight, but not for mode 5 (steering,
  `obj_fly` and `obj_hover`).
- FUN_1403b87e0 asks FUN_1403f3dd0 for the ground: a ray from 200 above the object (DAT_140585efc) down 100000 (DAT_1405b6108),
  with the body's class (+0x740 bits 3 to 6). With no hit the ground is the object's own height.

## Changes

- `Sysobj::update_steer` adds the root change.
- `float_height` uses that ray with the body's class. Without a map body it asks for the room's floor from the same height.
- `area_enemies` tour: Sora goes to the nearest boss with a character of its own, ahead of helpers and small enemies, so a
  fight's own reaction commands come up.

## Checks

| Check | Result |
| --- | --- |
| `cargo fmt`, `cargo clippy --all-targets --locked -- -D warnings`, `cargo test --locked`, `cargo build --locked` | PASS (before the one-line synthetic test was added; that test then run with `-p enemy`, PASS) |
| Volcanic and Blizzard Lord tour (`t8`) | PASS: both descend and attack, signal 4 at ticks 2574 and 4094, `DeadBoss(-1)` only on the second, `mission_clear` 4281 |
| Shadow Stalker tour (`f6`) | PASS: clear, `mission_clear` 1523 |
| Thresholder tour (`f6`) | FAIL: still stops in `freeze`; the reaction command 67 was not taken in the tour |
| The other flyers on this build | NOT RUN at the time of this commit: the root change reaches every object in mode 5 or 6 |
| Side by side with the installed game | NOT RUN |

## Uncertainty

- FUN_1403b87e0 runs its ray only when +0x744 bit 1 is set; otherwise it answers the solver's last floor (+0x738), which the room
  does not keep.
- The root change on flyers is taken from one function that serves every mode, and shown on one boss. An ordinary flyer that now
  drifts would be a regression of this commit.

## Verdict

PARTIAL. The Lords reach their clear. The sweep across the other boss programs is running.

## Next

The 66-program boss sweep, an ordinary flyer regression check, Thresholder's reaction command.
