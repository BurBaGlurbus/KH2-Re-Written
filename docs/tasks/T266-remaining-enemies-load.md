# T266 — The last ordinary enemies load: raw-matrix motions, shape-less bodies, vector_to_angle

Owner Claude, 2026-10-27, requested by burba in the project thread ("you can keep going"). Gate: development of
enemy AI in the test room; no final gate is inferred. Prerequisites: T262. Build identity: local `main` after
`7ea730b`, package 0.1.2, release launcher with the `skate3` feature. Source: the selected Steam KH2FM build,
validated each run; the install was only read. Ghidra 12.1.4 headless on private read-only project copies (plain
Ghidra, no MCP).

Private evidence (outside the checkout): `%LOCALAPPDATA%\KH2FinalRewrite\research\enemy-ai-20261016\out\traps-all\`
(trap 0:102, FUN_14041d980), `research\drive-magic-20261026\g\range\` (FUN_1403b7710), the installed object scripts
under `research\enemy-ai-20261016\bdx\`; run logs in a private run folder outside the checkout (`roster-07`,
`combat-05`).

## What the executable does

- **vector_to_angle** (0:102, FUN_14041d980): the argument vector's NaN lanes are set to 0 in place, then
  FUN_1403b7710 gives (pitch `atan2(-y, |xz|)`, yaw `atan2(x, z)`, 0, 1) in the trap's static vector DAT_1407535c8.
  EH_G_EX290 copies that vector (word 3479); the engine had no translation, so the script read address 0 and faulted.
- **Raw-matrix motions** (No. 4, B_EX210, B_EX380) are motions like the others for the motion player; their root
  comes from their own matrix table.
- **Objects with no HIT or TARGET shape** (No. 13, B_EX220, B_EX220_LV99, EH_G_EX120): the attack and target tests walk
  the object's shapes, so such an object is simply never struck on its body or locked on.

## Changes

- `traps::math::vector_to_angle` (0:102).
- `enemy_battle::build_clips`: raw-matrix motions admitted; their roots from `RawMotion::sample`.
- `enemy_combat::collision_shapes`: an object without HIT or TARGET shapes is kept.
- Unit test `vector_to_angle_gives_pitch_and_yaw_and_zeroes_nan_lanes`.

## Checks

| Check | Result |
| --- | --- |
| Roster of the 9 entries still refused or faulting after T262 (`roster-07`) | 8 run (No. 13, No. 4, B_EX210, B_EX220, B_EX220_LV99, B_EX380, EH_G_EX120, EH_G_EX290 without a fault); B_EX350 has no object script of its own |
| Roster total of the 99 type 4/21 entries | 96 run on their scripts; B_EX350 (no script), B_AL110 and B_AL120 (no model entry) do not |
| New unit test | PASS |
| `cargo fmt --all --check`, `cargo clippy --all-targets --locked -- -D warnings`, `cargo test --locked` | PASS |
| Release launcher (`tools\Build-Launcher.ps1 -Skate3`) | PASS |
| Combat tour (Shadow, Dusk, Soldier) | PASS |
| Side by side with the installed game | NOT RUN |

## Uncertainty

- Raw-matrix poses are not blended (the blend needs joint transforms; T263 falls back to the held start frame).
- B_EX350 and the B_EX220 pair are parts of larger fights; on their own in the room they idle, which says nothing about
  their fight.
- Running without a fault in a flat arena does not show the behaviour matches the game.

## Verdict

PARTIAL. 96 of the 99 ordinary enemy entries run on their own scripts in the room; not compared with the installed
game.

## Next

Ledges and spawn data for the room (T264/T265 handlers, partners); motion sets with stance variants; per-family checks
against footage or the installed game.
