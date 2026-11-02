# T359 — Tinker Bell's revive of Sora

Owner Claude, 2026-11-02, requested by burba in the project thread ("work through issues and close them"). Issue #67 (part). Gate: development of summons and the game over in the test room; no final gate is
inferred. Prerequisites: T140, T207, T336. Build identity: local `main` after T358 plus this change, package 0.1.2. Source: the selected Steam KH2FM build, validated each run; the install was only read.

Private evidence (outside the checkout): `research\issues-20261101\ko\ko-spec.md` section 2; `research\enemy-1to1-20261027\runs-20261030\lst\P_EX340.lst`; runs `sm2` to `sm5` under `%USERPROFILE%\kh2-issues-out\s162\`.

## What the executable does

FUN_140404d60 chooses this path when Sora falls with Peter Pan out and Tinker Bell unused (FUN_1403de320). The death event plays and the white holds, then FUN_1403fcec0 state 2 spends the one use, makes objentry 0x4f4 (P_EX340) and starts its act `reraise`. The act captures Sora in motion 252, pushes idle, runs trap 1:277 (FUN_1403aa8d0: the dead bit clears, HP gains its maximum), plays Tinker Bell's motion 232, and leaves.

## Changes

- `summon::{tink_ready, start_reraise, TINKER_BELL}`, `Summoned::{tink, aside}`. `spawn` takes a row and an act.
- `ScriptBrain::reraises` from trap 1:277. The summon's tick plays the act's captures on Sora and applies the revive.
- `game_over::Phase::Reraise`, in place of the list. P_EX340's motion 252 joins Sora's kit in `boss::capture_motions`.
- `KH2RW_TOUR_DEFEAT=<tick>`: Sora falls then, in any tour.

## Checks

| Check | Result |
| --- | --- |
| `cargo fmt --all --check`, `cargo clippy --all-targets --locked -- -D warnings`, `cargo test --locked` | PASS |
| `--summon-tour`, `KH2RW_TOUR_DEFEAT=5600`, with Peter Pan out (`sm5`) | PASS: `game_over_start`, the event, then `Reraise`; at tick 5643 `boss_capture` slot 252 played and `sora_reraise`, `game_over_end`; Sora lies with sparkles and gets up in the captures, idle at 5982 |
| A second fall in the same summon | NOT RUN: the code spends the one use, so the list would open |
| Side by side with the installed game | NOT RUN |

## Uncertainty

- Peter Pan is set aside while the act runs, and isn't drawn or ticked. The executable leaves him out [U].
- The white comes off at once. FUN_140155790, which the state calls, isn't read. Sora's floor snap and the zeroed physics aren't built.
- The act's camera start isn't honoured, and Tinker Bell herself wasn't made out in the captures, only her sparkles.
- `CAPTURE_OBJECTS` limits Sora's kit: P_EX340 takes one of its entries.

## Verdict

PASS in the room for Tinker Bell. #67 stays open for Mickey's revive, and positional sounds.

## Next

The item shop (#33).
