# T264 — Leaving the ground and landing: the act and script events, the kept momentum

Owner Claude, 2026-10-27, requested by burba in the project thread ("start work on all enemy AI and animations 1:1
using ghidra"; "you can do it"). Gate: development of enemy AI in the test room; no final gate is inferred.
Prerequisites: T217 (leaving the ground), T219 (map body), T262. Build identity: local `main` after `8bea096`,
package 0.1.2, release launcher with the `skate3` feature. Source: the selected Steam KH2FM build, validated each
run; the install was only read. Ghidra 12.1.4 headless on private read-only project copies (plain Ghidra, no MCP).
Part of GitHub issue #155.

Private evidence (outside the checkout): `%LOCALAPPDATA%\KH2FinalRewrite\research\drive-magic-20261026\g\range\`
(FUN_1403b9090, FUN_1403ca1d0, FUN_1403e1840, FUN_1403b7850, FUN_1403cbc30) and the installed object scripts under
`research\enemy-ai-20261016\bdx\`; run logs in a private run folder outside the checkout (`combat-03`, `seph-02`).

## What the executable does

- **Leaving the ground** (FUN_1403b9090, grounded mode, no gravity or hold range, the solve found no floor and range
  0x1d does not hold): the velocity +0x20 becomes this tick's move with its length capped at 8 a tick
  (`DAT_140623bac`), +0x2c = 1; `+0x120` bit 26 is cleared; the act manager's event 1 (FUN_1403ca1d0 → FUN_1403caf80:
  the running act's word at record +0xc, with the AI object, unless no act runs or its first run is gated); then, while
  bit 26 stays clear, the script's trigger 0xd (FUN_1403e1840, no arguments), the mode's slot +0x28 and the class's
  vt+0x50 (FUN_1403b7850: the mode's enter with height 0, then bit 26 set).
- **Landing** (not rising, FUN_1403ba1e0, the solve found a floor): `+0x120` bit 27 cleared; act event 2 (record
  +0xe); while bit 27 stays clear the script's trigger 0xe, the mode's slot +0x30 and the class's vt+0x58.
- **The scripts use it.** 71 of the 131 `M_EX*` scripts have a trigger 0xd and 10 a trigger 0xe. Soldier's
  (`OBJ_FALL`, word 429) is `obj_sysjump(self, 0)`, `motion_start(4, 8)`, `obj_exec_fall(self)`: it starts its own fall
  with its fall motion, and `obj_exec_fall` (1:65) sets bit 26 so the engine's own leave handler is skipped
  (`obj_exec_land`, 1:66, bit 27 likewise). The fall enter (FUN_1403cbc30) leaves +0x20 alone, so the fall keeps the
  velocity.

## What the rewrite did

- The engine left the ground or landed on its own; no act event or script trigger was sent, so Soldier and the other
  71 scripts never ran their fall handler (no fall motion), and `obj_exec_fall` / `obj_exec_land` were read as "start
  a fall" / "land" requests.
- A fall started with no horizontal speed.
- The act record's +0xc and +0xe words were stored as unused.

## Changes

- `enemy::sysobj`: `Ground` (Leave, Land); with `defer_ground` (set by every scripted object) the leave and land
  branches record the transition instead of finishing it; `finish_leave` and `land` are the engine's own handlers.
  The leave branch sets the velocity (capped at `LEAVE_SPEED_CAP`) and the next fall starts with it.
- `enemy::object::Object::finish_ground`: after the physics, act event 1 or 2, then trigger 0xd or 0xe, then the
  engine's handler, each only while bit 26 or 27 stays clear.
- `enemy::act::Entry`: `leave` (+0xc) and `land` (+0xe); `Manager::ground_event`.
- Unit tests `leaving_the_ground_runs_trigger_0xd_before_the_engine_falls` (a synthetic script whose trigger 0xd
  calls `obj_exec_fall`, and one without it) and `walking_off_the_floor_carries_the_move_into_the_fall_capped_at_8`.

## Checks

| Check | Result |
| --- | --- |
| New unit tests (above) | PASS |
| `cargo fmt --all --check`, `cargo clippy --all-targets --locked -- -D warnings`, `cargo test --locked` | PASS |
| Release launcher (`tools\Build-Launcher.ps1 -Skate3`) | PASS |
| Combat tour (Shadow, Dusk, Soldier on their scripts) | PASS: 14 hits on the enemies, 2 on Sora |
| Sephiroth tour to his defeat (`KH2RW_SEPH_TOUR_END=3000`, `_LOW=1800:30`) | PASS: defeated at 2182, mission clear at 2369, exit 0 (2773 before: his script now gets the ground events too) |
| An enemy walking off a ledge in the room | NOT RUN (the debug arena is flat) |
| Side by side with the installed game | NOT RUN |

## Uncertainty

- The displacement the leave branch measures (`local_78`) is taken as the tick's move; the decompiler did not name it.
- The jump or hop mode ends, and the feet are put on the floor, before a landing's events, even when a script cancels
  the landing with `obj_exec_land`.
- The land branch for an object in a grounded mode under a range (mode slot +0x30 and vt+0x58 of the ground mode and
  the battle classes) is not done; only falls, jumps and hops land through it.
- Mode slots +0x28 / +0x30 are taken as the existing fall start and FUN_1403cb940; the classes' vt+0x50 / vt+0x58
  besides FUN_1403b7850 were not read.

## Verdict

PARTIAL. Leaving the ground and landing send the act event and the script trigger in the executable's order and the
scripts can take the move over; a fall keeps the tick's momentum. Not compared with the installed game.

## Next

The ground-mode land branch and the classes' vt+0x58; grounded edge refusal (FUN_1403bab20 / FUN_1403ba750); a room
with ledges to see the fall handlers run.
