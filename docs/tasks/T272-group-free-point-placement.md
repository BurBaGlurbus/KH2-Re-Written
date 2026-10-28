# T272 — Group entities that appear around Sora or within a radius

Owner Claude, 2026-10-28, requested by burba in the project thread ("keep working dont idle"). Gate: development of
enemy AI in the test room; no final gate is inferred. Prerequisites: T270 (group pop-in and waves), T190 (the free-point
search FUN_1403f3fa0 / FUN_1403f4080 translated for the scripts' `bg_get_random_pos`). Build identity: local `main`
after `c884871`, package 0.1.2, release launcher with the `skate3` feature. Source: the selected Steam KH2FM build,
validated each run; the install was only read. Ghidra 12.1.4 headless on private read-only project copies (plain
Ghidra, no MCP).

Private evidence (outside the checkout): `%LOCALAPPDATA%\KH2FinalRewrite\research\drive-magic-20261026\g\range\`
(FUN_1403fe6f0) and `research\enemy-1to1-20261027\g\c2.txt` (its constants); a private room configuration
`.local/test-room/area-hb20-01.json` (hb 20, programs [0, 86, -1]); run logs in a private run folder outside the
checkout (`hb20-01`).

## What the executable does

FUN_1403fe6f0 places a group entity by its spawn argument:

- **1, around the player:** the search centre is the player's position 200 above him (`DAT_140623c98` = -200 in the
  engine's Y-down frame); FUN_1403f3fa0 with 150 to 450 (`DAT_1405b5578`, `DAT_1405c9ef0`) and mask 0x10001, or for an
  objentry with +0x48 bit 3 FUN_1403f4080 with the same ring, -100 to 100 (`DAT_1405b22e8`, `DAT_140623be4`) and mask
  0x10002;
- **2, within its radius:** around its own position, 0 to the record's radius, mask 1 (or FUN_1403f4080 with 0 to 0,
  mask 2);
- **otherwise** at its own position; the made object is created there (FUN_1403fe650).

## Changes

- `test_room/area_enemies.rs`: `free_point` runs `enemy::traps::random_position::ground_point` / `air_point` with
  those values against the room's map (a small `Env` whose line test is the script brain's: mask bit 16 is the attack
  ray), with this module's own engine generator and lap counter.
- `test_room/area_enemies.rs` placement takes that point for group entities.

## Checks

| Check | Result |
| --- | --- |
| hb20, battle program 86 (`hb20-01`) | PASS: point 47's Dusks (argument 0) stand at their places; point 46's Armored Knight 12 (argument 1) appears 346 from Sora, inside the 150-450 ring; 13 actors run to 1800 without a fault, hits land both ways |
| hb20, the first Knight (11) | The group popped at tick 30 while Sora still stood at the configuration's spawn (an hb01 position, off this map); no ground was found and it stood at the search centre, as FUN_1403f3dd0's fallback gives |
| hb20 again with Sora at the area's own player spawn (the configuration's `spawn` removed; `hb20-02`) | PASS: both Knights (11, 12) come out on the floor the search finds around Sora, the lower level at y -800 where their own records stand too, and walk there for the whole run; 13 actors without a fault; 5 hits on Sora, 17 on the enemies |
| `rustfmt --check` on the changed files, `cargo clippy --all-targets --locked -- -D warnings`, `cargo test --locked` | PASS |
| Release launcher (`tools\Build-Launcher.ps1 -Skate3`) | PASS |
| Side by side with the installed game | NOT RUN |

## Uncertainty

- The search's generator is this module's own, not the engine's shared one, so the drawn points differ from the
  game's.
- A room configuration's `spawn` overrides the area's player spawn (`test_room.rs`); area configurations should leave
  it out, as the private hb08 and hb20 ones now do.

## Verdict

PARTIAL. Group entities with spawn arguments 1 and 2 are placed by FUN_1403fe6f0's free-point searches; not compared
with the installed game.

## Next

The battle state and music an encounter starts.
