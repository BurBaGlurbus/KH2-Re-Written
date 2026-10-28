# T270 — Enemy groups pop in from their zone, wave by wave

Owner Claude, 2026-10-28, requested by burba in the project thread ("continue"; "keep working dont idle"). Gate:
development of enemy AI in the test room; no final gate is inferred. Prerequisites: T269 (an area's enemies on its
map), the area crate (spawn points and zones). Build identity: local `main` after `3c837ac`, package 0.1.2, release
launcher with the `skate3` feature. Source: the selected Steam KH2FM build, validated each run; the install was only
read. Ghidra 12.1.4 headless decompiles on private read-only project copies (plain Ghidra, no MCP).

Private evidence (outside the checkout): `%LOCALAPPDATA%\KH2FinalRewrite\research\drive-magic-20261026\g\range\`
(FUN_1403fe320, FUN_1403fe6f0, FUN_1403feb50, FUN_1403b41e0); run logs in a private run folder outside the checkout.

## What the executable does

- **The zone hit.** FUN_1403ff000 calls FUN_1403fe320 for each zone of an activated spawn point the player stands in;
  for a point of kind 2 (case 1) it marks the record triggered and runs FUN_1403fe6f0, every frame the player is in the
  zone.
- **FUN_1403fe6f0, the group.** While the point's timer (+0x20) has run out, each entity of spawn type 2 whose object is
  not alive (FUN_1403b41e0 by serial), that is not on the defeated list (DAT_142ae5e60, FUN_1403f5900) and whose
  required count (+0x30) the wave (+0x2c) has reached is made: at its own place, at a free place around the player
  (spawn argument 1, FUN_1403f3fa0 / FUN_1403f4080) or within its radius (2); a made entity sets the timer to its
  respawn delay (+0x2a). With the timer out, FUN_1403feb50 may move the wave on and the round repeats.
- **FUN_1403feb50, the waves.** Below wave 200: the living entities (types 0 and 3, and type 2 of the waves reached;
  each one counted unless it is defeated and gone) are weighed by their objentry +0x54; at or under the point's limit
  (header +0x22) the wave steps on, and at header +0x23 it goes back to +0x24, taking the entities of the waves to come
  off the defeated list; it stops at the first wave some type-2 entity waits for.

## Changes

- `area::Area::group_spawn` (FUN_1403fe6f0) and `next_wave` (FUN_1403feb50) with `area::Census` (the host's alive,
  defeated, forget and +0x54); `SpawnObject::wave`; a kind-2 zone hit is `Request::Group`.
- `data::objentry::ObjectEntry::spawn_limit` (+0x54).
- `test_room/area_enemies.rs`: entities of spawn types 0, 1 and 3 stand at their places when the area is ready; the
  groups come through `group_spawn` when Sora stands in their zone; defeated serials go on the list. In the unattended
  tour Sora is put in the first group point's zone at tick 40 (an authored tour placement).
- Unit tests `a_group_makes_its_wave_then_the_next_once_the_first_is_down`, `a_respawn_delay_holds_the_next_entity`.

## Checks

| Check | Result |
| --- | --- |
| hb08 battle program 1 with Sora put in point 172's zone at tick 40 (`hb08-03`) | PASS: at tick 41 the point's group pops in (4 Soldiers, serials 34-37, wave 0); the other points stay empty while Sora is not in their zones; Sora defeats one and the next wave does not come while three stand (the point's limit, header +0x22, is 0); no fault |
| Combat tour in the flat arena (`combat-07`) | PASS |
| New unit tests (above) | PASS |
| `rustfmt --check` on the changed files, `cargo clippy --all-targets --locked -- -D warnings`, `cargo test --locked` | PASS (the whole-workspace `cargo fmt --all --check` was not run: another thread's files were mid-edit) |
| Release launcher (`tools\Build-Launcher.ps1 -Skate3`) | PASS |
| Side by side with the installed game | NOT RUN |

## Uncertainty

- Spawn arguments 1 and 2 (a free place around the player or within the radius) are placed at the record's own
  position.
- The wave loop is bounded at 400 steps (the executable's would not end for a restart wave no entity waits for).
- A made entity is counted as made even when the room's 16-actor limit stops its summon.
- The writer of the defeated list in the executable was not traced; the room adds a serial when its actor dies.

## Verdict

PARTIAL. Groups pop in from their zone and come wave by wave as in FUN_1403fe6f0 / FUN_1403feb50; not compared with
the installed game.

## Next

The free-place searches for spawn arguments 1 and 2; the battle state and music an encounter starts; per-family
checks.
