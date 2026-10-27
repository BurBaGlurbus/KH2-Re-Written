# T265 — Grounded objects that refuse to step off an edge

Owner Claude, 2026-10-27, requested by burba in the project thread ("you can keep going"). Gate: development of
enemy AI in the test room; no final gate is inferred. Prerequisites: T219 (map body), T262 (map bodies for ordinary
enemies), T264. Build identity: local `main` after `119bf85`, package 0.1.2, release launcher with the `skate3`
feature. Source: the selected Steam KH2FM build, validated each run; the install was only read. Ghidra 12.1.4
headless on private read-only project copies (plain Ghidra, no MCP). Part of GitHub issue #155.

Private evidence (outside the checkout): `%LOCALAPPDATA%\KH2FinalRewrite\research\drive-magic-20261026\g\range\`
(FUN_1403b9090, FUN_1403bab20, FUN_1403ba750) and `research\enemy-1to1-20261027\g\c1.txt` (DAT_14074b060 = 0.2,
DAT_140623bac = 8.0); run logs in a private run folder outside the checkout (`combat-04`, `seph-03`).

## What the executable does

- FUN_1403b9090's grounded step (not in an air state, `+0x6cc` bit 2 clear) asks FUN_1403bab20 whether the object
  may not step off an edge: yes under range 0x1d (`+0x18c` bit 23); yes when it stands on another object (+0x9d8)
  that is not leaving it; yes for a battle object (`+0x6c8` bit 1) whose knock-back speed `+0xd30` is positive, or in
  the mode FUN_1403b5dc0 reports as 0xf.
- If so, the move (projected on the floor) is lengthened by 0.2 × the body radius (`DAT_14074b060`, +0x6f0) and that
  point is tried with a body solve from the current position (FUN_1403ba750 → FUN_14016fec0). When the probe finds no
  floor (`+0x744` bit 0), the object's target is its own position for this tick, and a battle object's knock-back
  (+0xd24..+0xd30) is cleared.

## What the rewrite did

Nothing stopped an enemy at an edge: knock-backs and range-0x1d motions slid it off like any other move.

## Changes

- `enemy::sysobj::Sysobj::refuses_edges` (FUN_1403bab20's range 0x1d and knock-back cases) and the probe in
  `apply_movement` for grounded objects with a map body; `EDGE_PROBE`.
- Unit test `an_edge_refusing_object_does_not_step_where_the_probe_finds_no_floor`.

## Checks

| Check | Result |
| --- | --- |
| New unit test | PASS |
| `cargo fmt --all --check`, `cargo clippy --all-targets --locked -- -D warnings`, `cargo test --locked` | PASS |
| Release launcher (`tools\Build-Launcher.ps1 -Skate3`) | PASS |
| Combat tour (Shadow, Dusk, Soldier) | PASS: 14 hits on the enemies, 2 on Sora |
| Sephiroth tour to his defeat | PASS: defeated at 2182, mission clear at 2369, exit 0 (as in T264) |
| An enemy knocked toward a ledge in the room | NOT RUN (the debug arena is flat) |
| Side by side with the installed game | NOT RUN |

## Uncertainty

- `+0xd30` is taken as the knock-slide speed of `actor_physics::Knock` (the four words FUN_1403b9090 clears); the
  writer of +0xd24..+0xd30 was not traced.
- The probe uses the horizontal move, not its projection on the floor normal (FUN_140142120).
- Standing on another object (+0x9d8) and FUN_1403b5dc0's mode 0xf are not modelled.

## Verdict

PARTIAL. Range 0x1d and knock-backs no longer carry a grounded enemy off an edge; not compared with the installed
game.

## Next

A test room with ledges for T264/T265; the ground-mode land branch; the classes' vt+0x58.
