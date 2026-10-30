# T290 — Reaction commands reach enemies with their record; summons and script-made enemies

Owner Claude, 2026-10-30, requested by burba in the project thread. Gate: development of enemy AI in the test room; no
final gate is inferred. Prerequisites: T211 (the reaction command manager), T283, T289. Build identity: local `main`
after `5c0a6b3`, package 0.1.2, release launcher with the `skate3` feature. Source: the selected Steam KH2FM build,
validated each run; the install was only read. Ghidra 12.1.4 headless on private read-only project copies (plain
Ghidra, no MCP).

Private evidence (outside the checkout): `%LOCALAPPDATA%\KH2FinalRewrite\research\enemy-1to1-20261027\g\` (`sv`, `sum`,
`sumt.txt`), `research\reaction-1to1-20261023`, `research\drive-magic-20261026\g\range\` (FUN_1403a8b10, FUN_1403b5fa0);
the object scripts disassembled in a private run folder; run logs there (`sweep4`).

## What was wrong

The tour's Sora never pressed a reaction command and stood far from most enemies. With him pressing and close by, most
enemies faulted the moment a reaction ran on them: `Memory(0)` in their trigger 11. M_EX770 faulted on Sora's
spec block. Separately, `enemy_summon_entry` and script-named prize drops only reported themselves, and an enemy a
script made (M_EX870's M_EX960) ran invisibly with no motions.

## What the executable does

- **Trigger 11's third argument** (FUN_1403a8b10 → FUN_1403b5fa0) is `+0x10` of Sora's reaction record at `+0xdd0`, a
  target record whose first word is the object named. The scripts read that word.
- **`obj_spec`** (FUN_14042faa0) hands out `+0x128`: walk, run, jump, turn, fall max and two more words.
- **`enemy_summon_entry`** (7:33, FUN_140542030) picks at random from the eight entries at DAT_140788b18 (122, 73, 123,
  125, 18, 7, 306, 305), leaving out those FUN_1403e0160 and FUN_1403a1f00 reject, else the caller's own.
- **`prize_appear`** (6:1, FUN_14053dba0) runs FUN_1403dc170 with a place and a prize row.

## Changes

- `ScriptBrain::command`: a four-word record with the object's handle and the shape's group.
- `Sysobj::peek_word`: the rest of the spec block (what a script wrote, else the defaults of FUN_14040a440).
- `traps::service::enemy_summon_entry`; `World::summoned`; `ScriptBrain::take_summoned`; `area_enemies` makes each a
  fighter of its own.
- `prize_appear` goes to the same prize path as `obj_scatter_prize`.
- `enemy_battle::MadeKind`, `load_made_kind`, `made_kind`: a made enemy's motions. `Spawner::appear` reads them for
  objentry types 4 and 21. `ScriptBrain::made_fighters`; `enemy_gallery::draw_made`: an actor stands for each, posed from
  the object its maker's script runs, with its fade and hiding. Sora's hits already reach made objects (`made_targets`).
- The area tour: Sora goes to the nearest enemy every 120 ticks; he presses every reaction command an enemy offers.

## Checks

| Check | Result |
| --- | --- |
| Sweep with these changes (`sweep4`, 3000 ticks a run, the first 10 programs, 34 models) | PASS: no faults; no enemy stands still past 500 ticks; 18 reaction commands carried out (before the record, 9 models faulted on theirs) |
| `cargo test --locked -p enemy -p area` on the staged changes | PASS |
| Release launcher (`tools\Build-Launcher.ps1 -Skate3`) | PASS |
| Side by side with the installed game | NOT RUN |

## Uncertainty

- All eight summon entries count as available; the executable's filter by resident heap and capacity is not modelled.
- A made enemy's own attack windows are not read, and its map body is not built.
- A made enemy is not lockable.
- Each actor still runs its own script world, so a shout or search across two actors does not reach.

## Verdict

PARTIAL. Reaction commands run on enemies in tours; not compared with the installed game.

## Next

The rest of `sweep4`; the Gambler's dice and cards (Sora's modes 0x2b to 0x31 and capture form 0xc).
