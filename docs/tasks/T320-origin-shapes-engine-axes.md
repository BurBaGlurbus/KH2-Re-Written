# T320 — Shapes on an object's origin are in engine axes

Owner Claude, 2026-10-31, requested by burba in the project thread (issue #163). Gate: development of enemy AI in the test
room; no final gate is inferred. Prerequisites: T319. Build identity: local `main` after `cd1489a` plus this change, package
0.1.2, release launcher with the `skate3` feature. Source: the selected Steam KH2FM build, validated each run; the install was
only read. Ghidra 12.1.4 headless on private read-only project copies (plain Ghidra, no MCP).

Private evidence (outside the checkout): `%LOCALAPPDATA%\KH2FinalRewrite\research\enemy-1to1-20261027\runs-20261031` (`t10`,
`t11`), decompiles in `g\shape`.

## What was wrong

A collision shape with bone 0x4000 took its offset as a model-space point. B_BB100's attack `[0, -200, -281]` with height 200
stood under the floor. Every object's origin-bound attacks, body shapes and reaction shapes with a nonzero x or y were affected.

## What the executable does

FUN_1403c9c00 multiplies the offset by FUN_1403b50f0's matrix and subtracts the entry's height from y. FUN_1403b50f0's case 0x4000
is the object's rotation (+0x40) and position, so the offset is in engine axes: up is -y. The other bones go through the model's
matrix and +0x808.

With engine yaw pi minus scene yaw, a point local to the object has x and y of the opposite sign in the frame a scene yaw turns,
and z of the same sign. B_BB100's reaction sphere at z -150 stays where it was, 150 behind him. [I] Forward as +z in the engine
comes from the earlier note on spawn yaw; the sphere's place was not compared with the game.

## Changes

- `enemy_combat::origin_offset`, used by `transform_shapes` and `enemy_script::hit_spheres`.
- `area_enemies` tour: a boss's reaction command on offer takes Sora to its shape.

## Checks

| Check | Result |
| --- | --- |
| `cargo fmt --all --check`, `cargo clippy --all-targets --locked -- -D warnings`, `cargo test --locked` | PASS |
| Thresholder and Possessor tour (`t11`) | PASS: command 67 taken at ticks 1690 and 2910, B_BB130 appears and dies, signal 4 at 3590, `DeadBoss(-1)`, `mission_clear` 3777 |
| Boss sweep on this build's shape change (`f8`, 66 programs) | Running at the time of this commit |
| Ordinary enemies after this and T319 | NOT RUN at the time of this commit |
| Side by side with the installed game | NOT RUN |

## Uncertainty

- Attacks and body shapes on the origin moved, so hits that used to miss now land, on every enemy with such shapes. Whether their
  reach and timing are right has not been compared with the game.
- In the tour Sora is placed at the reaction shape. A player has to walk behind the boss; whether the room's map and body push let
  him there has not been tried.
- Thresholder's own HP stays at the tour's forced 1 in `release`; the clear comes from Possessor's death.

## Verdict

PARTIAL. Thresholder and Possessor reach their clear in a tour.

## Next

Read the sweep, rerun the ordinary enemies, then the Grim Reaper's medals.
