# T358 — Sora's motion clock: the first step and the hold during a blend

Owner Claude, 2026-11-02, requested by burba in the project thread ("work through issues and close them"). Issue #73. Gate: development of Sora's motion timing in the test room; no final gate is inferred.
Prerequisites: T141, T201, T263. Build identity: local `main` after T357 plus this change, package 0.1.2. Source: the selected Steam KH2FM build, validated each run; the install was only read. Plain Ghidra 12.1.4
headless on a read-only project copy.

Private evidence (outside the checkout): T201's decompiles of FUN_1403c6740 and FUN_1403c8a40 as `player::party_motion` translated them; `research\issues-20261102\g\h5`; runs `abl2`, `ecb2`, `seph2` under `%USERPROFILE%\kh2-issues-out\s162\`.

## What the executable does

Sora and the party share one motion object (T201). FUN_1403c6740 advances its clock: the first step from 0 jumps to 2, and a finished blend continues from where it held. FUN_1403c8a40 gives a blend longer than 2 ticks a clock starting at 2, and holds the new motion at its start until the clock reaches the blend.

So an attack (blend 0) reaches frame 2 on its first tick, one tick earlier than the rewrite did, and every later window with it. A locomotion change or the item motion (blend 8) holds for six ticks, then goes on.

## Changes

- `Player::{advance_frame, begin_blend, passed, prior_frame, blend_clock}`. Root motion measures from the prior frame, not `frame - 1`, so the skipped frame's movement isn't lost.
- A trigger compared with one exact frame now asks whether this tick's step passed it: the item's use, the roll chain, the Wisdom shots.
- The player crate's tests that encoded the old timing are updated, each with the reason.

## Checks

| Check | Result |
| --- | --- |
| `cargo fmt --all --check`, `cargo clippy --all-targets --locked -- -D warnings`, `cargo test --locked` | PASS |
| `--ability-tour` (`abl2`) | PASS: verdict passes, ground and air combos 4, finishers 2 |
| `--enemy-combat-tour` (`ecb2`) | PASS: verdict passes; hits land both ways |
| `--sephiroth-tour`, to the mission's clear (`seph2`) | PASS: `mission_clear` |
| Timing against the installed game | NOT RUN: every attack window moved one tick earlier, and only the executable's code says so |

## Uncertainty

- This was left alone before because it moves every attack window. The code is read and shared with the party, but no controlled comparison with the installed game has been run. Attacks are worth checking by hand first.
- Sora's own blends, not the party's, set when the clock holds: a cast starts with blend 0 here, per the record at `blend_for`, where the party's spell acts blend 8.
- A fractional blend's remainder isn't kept, and the motion resumes at its held frame.
- Other frame readers (`voices`, dust and vibration scans) were already range-based. A unit test covers the windows, and the tours above cover hits and combos, not every trigger.
- The IK chains' own blend (FUN_1401c57b0 solves a two-bone chain with the law of cosines; its caller wasn't read) isn't built.

## Verdict

PASS in the room for the clock. #73 stays open for the IK chains.

## Next

The item shop.
