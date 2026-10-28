# T267 — An area's own enemies on its map, with their spawn serials

Owner Claude, 2026-10-28, requested by burba in the project thread ("continue", after the proposal to run enemies
in a real area). Gate: development of enemy AI in the test room; no final gate is inferred. Prerequisites: T262–T266,
the area crate (spawn sets, points and zones), the Sephiroth host (T206/T219, an area loaded with a battle program).
Build identity: local `main` after `820f55d`, package 0.1.2, release launcher with the `skate3` feature. Source: the
selected Steam KH2FM build, validated each run; the install was only read.

Private evidence (outside the checkout): `ard_sets` / `boss_placements` listings of the installed ARDs (hb08: map
program 1, battle program 1 naming `b_10` and `b_00`); a private room configuration `.local/test-room/area-hb08-01.json`
(the Sephiroth configuration with `area` hb 8, entrance 0, programs [1, 1, -1]); run logs in a private run folder
outside the checkout (`hb08-01`, `combat-06`).

## What the executable does

- An area's battle program names its spawn sets (hb08's program 1: `b_10`, `b_00`). `b_00`'s points are kind 2
  with one zone each; their entities (spawn type 2) pop in when the player meets the zone (FUN_1403fe320 case 5).
  Each entity's spawn record carries its serial, which `obj_search_by_serial` (FUN_1403b41e0) matches through
  +0x9f0 → +0x1e.
- hb08's `b_00` holds 7 Soldiers and 8 Shadows (serials 34–48) at three places at different heights.

## Changes

- `KH2RW_AREA_ENEMIES=1` (`test_room/area_enemies.rs`): once an area is ready, every ordinary enemy among its
  activated and zone spawns stands at its record's place and turn, with its record's serial, and gets its AI; up to
  the room's 16 actors.
- The serial travels with the summon (`EnemyGallery::queue_spawn`) into `ScriptSetup::spawn_serial` and the object's
  `spawn_serial`.
- An actor's AI is no longer refused inside an area (only Sephiroth's was allowed).
- `--enemy-combat-tour` with `KH2RW_AREA_ENEMIES`: Sora is set 200 from the first placed enemy at tick 40 (an
  authored tour placement), the area's actors are logged every 300 ticks, and the verdict asks that every placed
  actor fought without a fault.

## Checks

| Check | Result |
| --- | --- |
| hb08, battle program 1 (`hb08-01`) | PASS: 15 placed (7 Soldiers, 8 Shadows, serials 34–48), all run to 1800 without a fault; those placed above their ledge fall and land on it (y 1100 and 1200 → 900, 950 → 900); they move (up to 800 units), roll, jump and attack (`atk_craw`, `atk_jump`); Sora stays at the configuration's spawn, out of reach, so no hit lands |
| hb08 with Sora set 200 from the first enemy at tick 40 (`hb08-02`, a tour placement) | PASS: the group hits Sora 6 times, his attacks (700–1300) land 14 times, one enemy is defeated and its prize drops; all 15 without a fault |
| Combat tour in the flat arena (`combat-06`) | PASS |
| `cargo fmt --all --check`, `cargo clippy --all-targets --locked -- -D warnings`, `cargo test --locked` | PASS |
| Release launcher (`tools\Build-Launcher.ps1 -Skate3`) | PASS |
| Side by side with the installed game | NOT RUN |

## Uncertainty

- Placing every enemy at once is the host's choice: the game pops them in by zone, and its encounter rules (battle
  state, music, the respawn count of type 2) are not translated.
- The spawn record's unit arguments (`a1`, `a2`, +0x9f4) are not passed on.
- The unattended run puts Sora by the group instead of walking him there.

## Verdict

PARTIAL. An area's own enemies fight on its map at their placements with their serials; the zone pop-in and the
encounter rules are not translated. Not compared with the installed game.

## Next

The zone pop-in and encounter state (FUN_1403fe320), the unit arguments, and an unattended run that walks Sora into
the group; then hb08's second set (`b_01`: Armored Knights, Large Body) and other areas.
