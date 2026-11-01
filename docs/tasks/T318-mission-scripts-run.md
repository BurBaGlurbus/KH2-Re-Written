# T318 — A mission's own script decides its end

Owner Claude, 2026-10-31, requested by burba in the project thread (issue #163). Gate: development of enemy AI in the test
room; no final gate is inferred. Prerequisites: T317. Build identity: local `main` after `f5e6b61` plus this change, package
0.1.2, release launcher with the `skate3` feature. Source: the selected Steam KH2FM build, validated each run; the install was
only read. Ghidra 12.1.4 headless on private read-only project copies (plain Ghidra, no MCP).

Private evidence (outside the checkout): `%LOCALAPPDATA%\KH2FinalRewrite\research\enemy-1to1-20261027\runs-20261031` (sweep
`t6`), mission scripts disassembled in `msn`, decompiles in `g\st`.

## What was wrong

T206 stood in for `ms_boss` in Rust: the first death signal cleared the mission. So the Volcanic Lord and Blizzard Lord fight
cleared on the first Lord's death. Most fights use the same `ms_boss`, but not all of them.

## What the data shows

Of the eleven missions dumped, eight share one script. The others differ:

- `AL03_MS103`: its trigger 10 counts down from 2 before it locks.
- `CA10_MS107`: its trigger 10 wants argument 1015 (B_CA010), and a thread watches the object of character 8 and fails the
  mission when it dies.
- `CA01_MS204`: it finds the Grim Reaper and his chest, shows their medals as counts, and has an OBJ_DAMAGE of its own that
  scatters medals when Sora is hit.

Natives: 4:3 FUN_1404334c0, 4:11 thunk_FUN_1403a3800, 4:16 FUN_1404334d0, 4:22 FUN_1404334f0, 4:54 FUN_140433560, 4:55
FUN_140433570, and 1:41 FUN_14042ec90, which is FUN_1403aabe0 with the code and argument.

## Changes

- `traps::mission`, new: the natives above. Table 4's calls wait in `World::mission` for the host.
- `Object::spawn_unit`: a mission script's main thread is trigger 3, its signal trigger 10.
- `ScriptBrain::mission`: the script stands in the engine's list and the fighters lend it their objects, as they do each other.
- `boss::mission_tick`: signals reach trigger 10, then the script runs, then its calls are carried out. Only
  `trap_mission_dead_boss` does anything so far; the others are logged.
- `signal_call` now posts its signal instead of reporting itself.

## Checks

| Check | Result |
| --- | --- |
| `cargo fmt`, `cargo clippy --all-targets --locked -- -D warnings` | PASS |
| `cargo test --locked -p enemy -p main` (two new synthetic tests of the natives) | PASS |
| Whole-workspace `cargo test --locked` on this build | NOT RUN |
| Dark Thorn tour | PASS: `DeadBoss(-1)` from the script at tick 1950, then the clear |
| Barbossa tour | PASS: the clear on B_CA010's death at tick 1672 |
| Volcanic and Blizzard Lord tour | PASS for the first death: no clear on it. The second death was not reached in 5400 ticks, so the clear on both is NOT RUN |
| Side by side with the installed game | NOT RUN |

## Uncertainty

- FUN_1403aabe0 also passes the signal to FUN_1403f4980, FUN_1403b4270 and FUN_1403f60d0. Only the mission gets it here.
- `mission_failed`, `mission_complete` and `mission_exit` do nothing yet, so a mission that fails does not end the fight.
- CA01's OBJ_DAMAGE is not fired.

## Verdict

PARTIAL for the fights it serves. The rule itself now comes from the game's own scripts.

## Next

Thresholder's stop, then the Grim Reaper's medals and the failed mission's end.
