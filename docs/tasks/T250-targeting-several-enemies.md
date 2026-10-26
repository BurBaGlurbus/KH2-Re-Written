# T250 — Targeting and lock switching between several enemies (issue #20)

Owner Claude, 2026-10-26, requested by burba in the project chat ("continue with more
issues"). Gate: development of the test room only; no final gameplay acceptance is inferred.
Prerequisites: T170 (lock-on and targeting), the enemy gallery and its three-actor combat tour.
Build identity: local `main` after `5f82e90`, package 0.1.2, Rust 1.99.0 Windows MSVC. Source:
the selected Steam KH2FM build, validated each run; the install was only read.

Evidence: unattended runs `combat1` and `combat3` in the private `kh2-issues-out/`.

## What was open

Issue #20 asked for automatic targeting and switching to be tried with more than one enemy;
the room's first runs had one. The enemy gallery now summons several battle actors, and
`--enemy-combat-tour` fights three (M_EX020, M_EX990, M_EX010), but no run pressed the lock's
cycle input.

## Change

`--enemy-combat-tour` presses cycle-right at tick 165 while the lock (engaged at 150) holds, then
cycle-left at 170 if right found no other actor; it logs `enemy_combat_tour_cycle` and its
verdict now also requires the lock to move to another actor while it stays locked
(`lock_cycled`).

## Checks

| Check | Result |
| --- | --- |
| `cargo fmt --all --check`, `cargo clippy --all-targets --locked -- -D warnings`, `cargo test --locked`, `cargo build --locked`, smoke | PASS |
| Release `--enemy-combat-tour` before the change (private `combat1`) | PASS: automatic targeting moves between all three actors (Gallery 1, 2 and 3) as they move, the lock engages at 150 and releases at 180 |
| Release `--enemy-combat-tour` after the change (private `combat3`) | PASS, `lock_cycled` true: cycle-right at 165 keeps actor 1 (no actor on that side), cycle-left at 170 moves the lock to actor 2 and it stays locked; no limitation reported |
| Unit tests in `player::targeting` | PASS (manual search and cycle order, stable lock across removed actors, lock toggle) |

## Uncertainty

- Which side counts as "right" follows the room's screen projection; that cycle-right found no
  actor with two actors on the other side of the arena's X axis was not compared with the
  installed game.
- Not compared: target choice, switch timing, the reticle and the sounds against the installed
  game (issue #53 keeps that comparison).

Verdict: PASS for issue #20 in the development room (automatic targeting and the lock's cycle
work between several enemies); PARTIAL against the installed game.
Next: the side-by-side comparison in #53.
