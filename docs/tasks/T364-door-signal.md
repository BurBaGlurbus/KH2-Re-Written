# T364 — A door raises signal 1 before its jump

Owner Claude, 2026-11-02, requested by burba in the project thread ("work through issues and close them"). Issue #25 (part). Gate: development of the story in the test room; no final gate is inferred.
Prerequisites: T344, T345. Build identity: local `main` after T363 plus this change, package 0.1.2. Source: the selected Steam KH2FM build, validated each run; the install was only read.

Private evidence (outside the checkout): `research\issues-20261101\events\events-spec.md` §1.4 and §2.2, `jumps_tt.txt` (the tt entry census); private configurations `test-room-tt14-18.json` and `test-room-tt03-4.json`; runs `door1` to `door4` under `%USERPROFILE%\kh2-issues-out\s162\`.

## What the executable does

An exit point (FUN_1403ffb40, also FUN_1403ff2a0) stores its target in `DAT_140717028`, then raises signal 1 with its point id (record +2). If an entry answers (FUN_1403f60d0: an exact id first, else one with a negative id), it fires: its flag, its event and its jump. Jump kind 3 goes on to the stored door target. If no entry answers, the door's own jump runs (FUN_140152990). In tt, 8 entries answer doors, for example tt14 program 18 (any door: flag 0x8eb, then tt03) and tt03 program 4 (points 58, 42 and 60: event 605, then tt02, tt04 or tt06).

## Changes

- `area::Request::Jump` carries the exit point's id.
- `story_events`:
  - `door` raises signal 1 before a door's jump, behind `KH2RW_STORY_EVENTS` as before.
  - The arrival's entry handling moved into `run_entry`, which both signals share.
  - Kind 3 resolves to the door's target.
- The room's exit handler (`test_room.rs`) asks `story_events::door` first and makes the plain transition only when no entry took it. The `exit` log record names the point.

## Checks

| Check | Result |
| --- | --- |
| `cargo fmt --all --check`, `cargo clippy --all-targets --locked -- -D warnings`, `cargo test --locked` | PASS |
| tt14 program 18 with the switch on, autoplay to the nearest exit (`door1`) | PASS: exit point 41 (to tt13) at tick 35, `story_signal` signal 1 with flag 0x8eb, the room entered tt03 entrance 0 instead of tt13 |
| tt03 program 4 (`door2`) | PASS: exit point 58 at tick 118 matched its own entry: flag 0x883, `story_event` `tt:3:605` towards tt02 entrance 0 (an unattended run stops at the hand-over, as in T344) |
| Arrival, signal 0, after the change: tt07 program 106 (`door3`) | PASS: event 103 and the jump to program 95, as in T344 |
| The same door with the switch off (`door4`) | PASS: tt13 entrance 1, no signal |
| Kind 3, the other door shapes, a played chain | NOT RUN: tt has no kind-3 door entry |
| Side by side with the installed game | NOT RUN |

## Uncertainty

- FUN_1403ff2a0's own door path (the second raiser) isn't separated from FUN_1403ffb40's here: both reach the room as the same exit request.
- The rule that redirects any target to program 0x7a once flags 0x86e-0x871 are set and 0x872 isn't (FUN_1403a46e0) isn't built, for doors or arrivals.
- An entry this step can't carry out (an event over the running field) leaves the door its own jump and stays unfired. The exe would fire it.

## Verdict

PASS in the room for door signal 1, behind the story-events switch.

## Next

#25: the mission signals 37 and 38, talk signals, the player's skip and events with no jump.
