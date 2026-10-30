# T283 — Every placed enemy run on a map where it spawns: the tools

Owner Claude, 2026-10-30, requested by burba in the project thread ("keep working on getting all enemy ai working for
every enemy in the game starting with all heartless then all nobody's"). Gate: development of enemy AI in the test
room; no final gate is inferred. Prerequisites: T269 / T270 / T272 (an area's enemies on its map), T276 (the roster's
diagnostics). Build identity: local `main` after `f69c1b4`, package 0.1.2, release launcher with the `skate3` feature.
Source: the selected Steam KH2FM build, validated each run; the install was only read.

Private evidence (outside the checkout): the placement list, the area cover and the sweep's run logs in a private run
folder.

## Changes

- `source/game/area/examples/enemy_placements.rs`: from the installed ARDs, the map and battle programs that place each
  ordinary enemy (objentry types 4 and 21), through `script::run` (FUN_1403a2170). It prints names only.
- `source/game/enemy/examples/implemented_traps.rs`: every trap the engine implements, to compare with the traps the
  object scripts call.
- The area tour (`KH2RW_AREA_ENEMIES`): `KH2RW_TOUR_END` sets its length; Sora is put in each group point's zone in
  turn, so a program's groups all come; he attacks from tick 300 and is topped up, as in the roster; the end of the run
  logs one `area_enemy_model` per model (acts, motions, motions without a clip, faults, unsupported services, motion
  events nothing carries out, how far it rose and sank, how long it stood still and where).
- `boss_state` carries the program counters of the main and act threads.

## Findings at this stage

- Static census of the object scripts: every native the `M_*` and `EH_G_*` scripts call is implemented except `0:0`
  (`trap_puti`, a debug print) and `4:0` / `4:1`, whose slots in the executable's table are empty.
- Area cover: 74 program runs place every enemy with a placement.

## Checks

| Check | Result |
| --- | --- |
| `cargo clippy --all-targets --locked -- -D warnings` | PASS |
| First sweep runs | see the family records that follow this one |
| Side by side with the installed game | NOT RUN |

## Verdict

PARTIAL. Tools only; the fixes they lead to have their own records.

## Next

Enemies the sweep shows frozen: M_EX210 and M_EX110 holding motion 44, M_EX890 in `jump`.
