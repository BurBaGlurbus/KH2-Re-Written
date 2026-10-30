# T306 — Fighters' OBJ_INIT at the first update; the last untranslated natives; refusals gone

Owner Claude, 2026-10-30, requested by burba in the project thread. Gate: development of enemy AI in the test room; no
final gate is inferred. Prerequisites: T305. Build identity: local `main` after `43c9bd1` plus this change, package 0.1.2,
release launcher with the `skate3` feature. Source: the selected Steam KH2FM build, validated each run; the install was
only read. Ghidra 12.1.4 headless on private read-only project copies (plain Ghidra, no MCP).

Private evidence (outside the checkout): `%LOCALAPPDATA%\KH2FinalRewrite\research\enemy-1to1-20261027\g\miss\`, `g\miss2\`,
`g\boss\`; the object scripts disassembled in a private run folder; run logs there (`sweep7`, `sweep8`, `boss3`, `boss4`,
`probe12`).

## What was wrong

The boss sweep behind `KH2RW_ENEMY_BOSSES` showed:

- Six natives had no translation in the runs, and a census of all 797 scripts found about 50 more, mostly in field gimmick
  and party scripts.
- B_CA050's OBJ_INIT searches for its F_CA690_BTL, and an object of a spawn set could run before the rest existed.
- A window naming an ATKP row the table lacks refused the object (B_EX110), and so did a set with no motion 0 (N_CM020_BTL).
- `TR16`'s third argument, an effect, is copied by B_EX390 as an AI object.

## What the executable does

The natives are in `traps/gimmick.rs` and `traps/service.rs`, each citing its address. Those that matter to fights:
`obj_serial` (FUN_1403b5bf0), `obj_is_hover` (FUN_1404229d0, mode 6), `damage_is_normal` (FUN_1403cee00),
`attack_exec_reflect` (flag bit 6), `target_clear_before_player` (FUN_1403bd290), the status bits (FUN_140430800,
FUN_140430930) and `obj_is_hook` (FUN_14053ede0). FUN_1403dacb0 starts motion 0 only if the set has it.

## Changes

- `traps::gimmick`, new; `traps::service`: requests for hosts not here report themselves. Answers that stand in are marked
  in the code.
- `ScriptBrain::build`: a fighter's OBJ_INIT waits for its first update (`Object::spawn_later`). `enemy_battle::tick`: a new
  fighter waits while its spawn set is still being made (`spawns_pending`), and the sheet row its OBJ_INIT bound is read after
  its first update.
- `enemy_combat::windows`: a window on a row the table lacks is left out. `BattleActor`: a set with no motion 0 poses its
  first motion; a slot the set lacks keeps the last pose.
- `world.rs`: word +4 of an effect handle names the object running.

## Checks

| Check | Result |
| --- | --- |
| Ordinary sweep on T305 (`sweep7`, 81 programs) | PASS: 136 models, no faults, no missing clips |
| Ordinary sweep on this build (`sweep8`) | PASS so far at the time of this commit: 41 programs, 103 models, no faults. NOT RUN to the end |
| Boss sweep on this build (`boss4`) | At the time of this commit, 49 of 70 programs, 80 models, no refusals. FAIL for two: B_EX180 (`Memory(0)` at word 10031) and B_EX390 (`Memory(0)` at word 583). NOT RUN to the end |
| `cargo test --locked -p enemy -p area` | PASS |
| `cargo clippy --locked -p enemy --all-targets` | PASS |
| Release launcher (`tools\Build-Launcher.ps1 -Skate3`) | PASS |
| Side by side with the installed game | NOT RUN |

## Uncertainty

- `answer_zero`'s natives (listed in `traps/gimmick.rs`) answer 0 for state the room does not keep.
- Boss runs prove only that their scripts run. Their missions, cameras and cutscene requests still only report themselves, and
  none of this is a playable fight.
- The effect word was added for B_EX390, and that object still faults; the next step is to find out why.

## Verdict

PARTIAL. Runs in tours; not compared with the installed game.

## Pause

Work paused at the owner's request on 2026-10-30. Both sweeps were stopped unfinished: `sweep8` at 44 of 81 programs (105 models, no faults), `boss4` at 51 of 70 programs (84 models, no refusals, the two faults above). The private run folder is now `%LOCALAPPDATA%\KH2FinalRewrite\research\enemy-1to1-20261027\runs-20261030`.

## Next

Everything left is listed in issue #163.