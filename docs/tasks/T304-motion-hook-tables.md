# T304 — Motion hook tables: the second forms of the Devastator, Trick Ghost, Creeper Plant and Gambler; `obj_is_air`

Owner Claude, 2026-10-30, requested by burba in the project thread. Gate: development of enemy AI in the test room; no
final gate is inferred. Prerequisites: T262, T287, T299. Build identity: local `main` after `9224112`, package 0.1.2,
release launcher with the `skate3` feature. Source: the selected Steam KH2FM build, validated each run; the install was
only read. Ghidra 12.1.4 headless on private read-only project copies (plain Ghidra, no MCP).

Private evidence (outside the checkout): `%LOCALAPPDATA%\KH2FinalRewrite\research\enemy-1to1-20261027\g\hook\`
(FUN_1403c7c10, FUN_1403c7c50, FUN_14040d7a0, FUN_1403a6420, FUN_1403c8cd0, FUN_1403c8a40, FUN_14040d7d0, FUN_14042f460,
FUN_14042f490, FUN_14042f4c0, FUN_14042efc0, FUN_1403cbdd0); the object scripts disassembled in a private run folder; run
logs there (`probe8`, `probe9`, `dbg2`).

## What was wrong

`Env::motion_hook` had no implementation, so `obj_motion_hook` did nothing and `obj_motion_is_hook` was always false. Every
act behind that test never ran: the Devastator's `atk_ball`, `atk_cannon` and `atk_round`, and Trick Ghost's whole second
form (`atk_small_b`, `move_jump_b`, `move_roll_b`, `move_target_b` and others). The coverage of acts seen in the sweeps
pointed at it.

## What the executable does

- **The table.** `obj_motion_hook(n)` (1:75 FUN_14042f460 → FUN_1403c7c10) sets `+0x160` to entry `n` of the object's
  motion set. FUN_14040d7a0 calls FUN_1403a6420 with type -1, so `n` counts every entry of the set, not only motions. In
  the sets scripts name, that entry is type 20 and a whole nested motion table of its own: slot 161 in M_EX500, M_EX680 and
  M_EX750, slot 251 in M_EX950.
- **Lookups.** FUN_1403c8cd0 looks in the hook table first, then in the set.
- **The playing motion keeps its table.** FUN_1403c8a40 stores the clip's and its trigger table's pointers at the start
  (`+0x18`, `+0x20`). A hook or unhook therefore changes only the next motion. A first version switched at once, which
  left Trick Ghost in a jump it could not land from.
- **`obj_is_air`** (1:60 FUN_14042efc0) is FUN_1403ba9a0, or the mode's flag bit 1, or `+0x18c` bit 6. The port read a
  separate flag that a hover left stale, so the Devastator waited forever after `atk_change`.
- **Range 0x18's parameter** (FUN_14040d810) is `+0x108`, the steering inertia of FUN_1403b7ad0 in FUN_1403cbdd0 and
  FUN_140421c80, not a height. It is not ported here.

## Changes

- `enemy_catalog::Kind::hook_tables`: the type 20 entries that are whole motion tables, with their motions.
- `enemy_battle::Clips` holds, for each hook table, the set as that table leaves it; `build_clip` is split out of
  `build_clips`.
- `sysobj`: `MODEL_ENTRY_BITS`, `model_entry`: the model word's top bits name a table for `Env::clips`.
  `Sysobj::set_motion_hook`, `playing_model`; `Motion::table`, set when a motion starts. `begin_tick` and `end_tick` read
  the playing motion's table.
- `BrainEnv::motion_hook`, `clips`; `Brain::motion_table`; the host poses from the playing motion's table.
- `traps::object::is_air` as the executable has it.
- Unit test `a_hook_changes_only_the_motion_started_after_it`.

## Checks

| Check | Result |
| --- | --- |
| Runs on he09, he11, tr02 (`probe8`, 3000 ticks) | PASS: M_EX500 plays both forms with `_b` acts and motions 2 to 5 of its table; M_EX680 runs `atk_ball`, `atk_cannon`, `atk_round`; no faults and no missing clips |
| `cargo test --locked -p enemy` | PASS |
| Release launcher (`tools\Build-Launcher.ps1 -Skate3`) | PASS |
| Side by side with the installed game | NOT RUN |

## Uncertainty

- A table counts only with at least 100 entries: the smaller type 20 entries (`AA00`, `AB40`, `AC60`) are not named by any
  hook call in the scripts. What they are for was not established.
- `obj_motion_hook` on another object answers from the running fighter's own set.
- M_EX950's hook is now live, but its dice and cards are still not ported.
- In he11 Trick Ghost's direction of flight has a rising height. Whether it climbs that much in the game was not established.

## Verdict

PARTIAL. The second forms run in tours; not compared with the installed game.

## Next

Range 0x18's steering inertia; the models no sweep has run yet (`M_EX570`, `M_EX660`, `M_EX120`); the per-family report.