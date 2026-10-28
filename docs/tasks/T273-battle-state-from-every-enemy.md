# T273 — The battle state follows every enemy; battle objects are team 2

Owner Claude, 2026-10-28, requested by burba in the project thread ("keep working dont idle"). Gate: development of
enemy AI in the test room; no final gate is inferred. Prerequisites: T262, T270 (groups that pop in), the room's
battle music and the party's battle flag (both already following FUN_1403ab6f0 for the room's own Shadow). Build
identity: local `main` after `dd05762`, package 0.1.2, release launcher with the `skate3` feature. Source: the
selected Steam KH2FM build, validated each run; the install was only read. Ghidra 12.1.4 headless on private read-only
project copies (plain Ghidra, no MCP).

Private evidence (outside the checkout): `%LOCALAPPDATA%\KH2FinalRewrite\research\drive-magic-20261026\g\range\`
(FUN_1403ab6f0, FUN_1403ab6b0, FUN_1403baa60, FUN_1403db9d0, FUN_140411700) and
`research\enemy-1to1-20261027\g\c3.txt` (DAT_1405b557c = 1200, DAT_1405b67f0 = 45, DAT_1405b5330 = 120); run logs in
a private run folder outside the checkout (`hb08-05`, `hb08-06`, `combat-08`, `seph-04`).

## What the executable does

- FUN_1403ab6f0, every frame: walks the enemy-class objects (FUN_1403db9d0, +0x6c8 bit 4). A boss-class one (+0x6c8
  bit 10) puts Sora in battle; so does one of team 2 (+0x4dc) within 1200 of him horizontally, or anywhere when
  FUN_1403baa60 holds (character id 0x417 to 0x423). Then (unless the state is a mission's forced 2) the wanted state
  also follows FUN_1403d8ad0 and FUN_1403c5bd0, and the switch waits 45 frames (to battle) or 120 (to the field).
- FUN_140411700, at the start of every battle object's constructor (objentry types 3, 4, 12, 13, 21): team 2 unless
  FUN_1403baa60.

## What the rewrite did

The battle music and the party's battle flag looked only at the room's own Shadow; battle actors kept team 0, so an
area's groups would not have counted even when tested.

## Changes

- `ScriptSetup::character`; the script brain's object gets team 2 for those objentry types unless its character is
  0x417 to 0x423.
- `Brain::keeps_battle` (FUN_1403ab6f0's test of one object); `music_tick` and the party's battle flag
  (`party_room`) count every battle actor (the party side through `gallery_targets`, horizontally within 1200).

## Checks

| Check | Result |
| --- | --- |
| hb08 group pop-in with the battle state (`hb08-05`, before the team fix) | The battle music went back to the field at 159 with four Soldiers beside Sora: their team was 0 |
| The same after the team fix (`hb08-06`) | PASS: the battle state stays on through the fight; no fault |
| Combat tour in the flat arena (`combat-08`) | PASS: 14 hits on the enemies |
| Sephiroth tour to his defeat (`seph-04`) | PASS: defeated at 2182, mission clear at 2369, exit 0 (as before) |
| `rustfmt --check`, `cargo clippy --all-targets --locked -- -D warnings`, `cargo test --locked` | PASS |
| Release launcher (`tools\Build-Launcher.ps1 -Skate3`) | PASS |
| Side by side with the installed game | NOT RUN |

## Uncertainty

- FUN_1403d8ad0 (`DAT_142a24cc0`) and FUN_1403c5bd0 (a party member in a list at DAT_142a23a70) are not modelled.
- The party's flag sees a battle actor through the targets the hit pass lists, so the boss and character rules apply
  there only through those targets.
- The scripts' own battle fact (`HostFacts::battle`) is still on for every battle actor.

## Verdict

PARTIAL. Every battle actor now counts for the battle state as in FUN_1403ab6f0, and battle objects are on team 2 as
FUN_140411700 makes them; not compared with the installed game.

## Next

One battle state shared by the music, the party, the Drive gauge and the scripts; FUN_1403d8ad0 and FUN_1403c5bd0.
