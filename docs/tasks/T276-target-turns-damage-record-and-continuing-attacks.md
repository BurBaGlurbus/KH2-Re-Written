# T276 — Enemies turn toward their target, read the damage record, continue attacks, and make objects in OBJ_INIT

Owner Claude, 2026-10-28, requested by burba in the project thread ("keep working dont idle"). Gate: development of
enemy AI in the test room; no final gate is inferred. Prerequisites: T133 (the engine's motion events), T262 / T263
(every type 4 / 21 enemy on its script), T274. Build identity: local `main` after `e81c14c`, package 0.1.2, release
launcher with the `skate3` feature. Source: the selected Steam KH2FM build, validated each run; the install was only
read. Ghidra 12.1.4 headless on private read-only project copies (plain Ghidra, no MCP).

Private evidence (outside the checkout): `%LOCALAPPDATA%\KH2FinalRewrite\research\drive-magic-20261026\g\range\`
(FUN_14040d810, FUN_14040dd70, FUN_1403dafe0, FUN_1403d25d0, FUN_1403db080), `research\combat-feel-20261017\g\`
(FUN_1403d12c0, FUN_1403ce740), `research\boss-01\out\h1\` (FUN_1403cf830), `research\enemy-1to1-20261027\g\vt98\`
(the vt+0x90 / +0x98 / +0xa0 slots of the BTLOBJ, ENEMY, BOSS and ZAKO vtables) and `g\ev\` (the frame-event
routines named under Next); the object scripts M_EX350_10, B_EX380, M_EX880_DANCER and M_EX870 disassembled in this
session's scratch folder; run logs in a private run folder outside the checkout (`roster-08`, `combat-11`, `tt05-07`,
`seph-07`, `hb08-09`, `roster-09`).

## What was found

A roster run of every enemy (`roster-08`, 175 slots, 165 without a fault) logged the motion events each enemy's motions
passed to the engine and nothing carried out, and the script faults:

- Range 0x16 (61 models) and frame event 0xf (12) were not carried out for enemies.
- Range 0x21 (29 models) was not read as an attack window.
- M_EX350_10, B_EX380 and the three M_EX880 Dancers faulted reading their damage handler's argument at +0x10.
- M_EX870 faulted on a null object: its OBJ_INIT makes an object (`sysobj_appear`) before the host gave the
  actor its spawner.

## What the executable does

- **Turning toward the target.** Range 0x16 (FUN_14040d810 → FUN_1403d4b90) and frame event 0xf (FUN_14040dd70 →
  FUN_1403d4b80), for a battle object (`+0x6c8` bit 1), turn toward the object's own target record (`+0xbf8`) through
  FUN_1403d3a90: beyond 50 units horizontally the heading turns by up to the first parameter in degrees; a frame
  event's 0 faces the target at once; a valid record sets the motion's `+0x3c` (sys `+0x194`) bit 3.
- **The damage record.** OBJ_DAMAGE receives the damage record (FUN_1403d12c0 → FUN_1403ce740). Its +0x10 is the
  attack's +0xc, the owner's handle (FUN_1403cf830), and +0x14 the struck object's handle. The scripts build an AI
  object from +0x10 and ask `sysobj_is_exist` / `sysobj_is_player` of it.
- **Continuing attacks.** Range 0x21, like range 10, goes to FUN_1403dafe0 for objects with `+0x6c8` bit 0. That
  routine marks the collision group in the motion's mask (`+0x5a0`) and calls the class's vt+0x98 (FUN_140419c90 for
  ZAKO, FUN_1403db600 for ENEMY, FUN_1403f8be0 for BOSS), which is FUN_1403d25d0 on the object's `+0xc10` attack set.
  Range 0x21's third parameter is FUN_1403d25d0's last argument. When it is set, the new attack takes the set's `+4`,
  the hit id of the object's last attack, so a target that attack struck is not struck again.
- **Objects made in OBJ_INIT.** `sysobj_appear` (1:0, FUN_1403df930) can run in the new object's OBJ_INIT.

## Changes

- `enemy::object`: range 0x16 and frame event 0xf turn battle objects toward their own target, beside the
  range 28 / frame 21 turn Sora's object already had. A frame event of 0 degrees faces the target at once, and every
  valid record sets `+0x194` bit 3.
- `enemy::damage` / `world`: the damage record's +0x10 (the attacker, `Sysobj::damage_attacker`) and +0x14 (the
  struck object) read as engine handles.
- `test_room::enemy_combat` / `enemy_battle_hits`: range 0x21 windows are attack windows. A continuing window shares
  the hit id of the window that started last before it (`hit_keys`), and that id lives while any window sharing it is
  active.
- `test_room::enemy_script`: `ScriptBrain::load` takes the spawner, so OBJ_INIT can make objects, and a rebuild
  (respawn) keeps it. Enemy battle actors get the objentry spawner at load.
- Roster diagnostics: each slot lists the motion events the engine passed on that nothing here carries out. Range 10
  / 0x21 and frame event 8 are left out of that list, because the host reads them from the clip.
- Unit tests `a_battle_object_turns_toward_its_target_under_range_0x16_and_frame_0xf`,
  `obj_damage_reads_the_attacker_from_the_damage_record`,
  `a_continuing_window_shares_the_hit_id_of_the_last_one_started`.

## Checks

| Check | Result |
| --- | --- |
| Roster of every enemy before the changes (`roster-08`) | 175 slots ran, 165 without a fault; the findings above |
| Roster of the faulting and range-0x21 enemies after (`roster-09`: M_EX350_10, B_EX380, M_EX880_DANCER, M_EX870, M_EX570, M_EX680, M_BB010_SWORD, M_EX050, M_EX640) | PASS: 9 of 9 without a fault (4 faulted before), no unhandled motion events left on them; hits on Sora / on the enemy 9/0, 9/4, 1/6, 0/8, 0/8 for the five range-0x21 users (9/1, 6/4, 2/0, 4/4, 1/7 before) |
| Combat tour in the flat arena (`combat-11`) | PASS: 16 hits on the enemies, 2 on Sora (as `combat-10`) |
| tt05 (`tt05-07`) | PASS: 3 Dusks to 1800, no fault, on the map (y no lower than −10); 8 hits on the Dusks, 4 on Sora (as `tt05-06`) |
| Sephiroth tour to his defeat (`seph-07`) | PASS: defeated at 2293, mission clear at 2480, exit 0 (2682 / 2869 before; he now turns toward Sora in his range 0x16 windows) |
| hb08 group pop-in (`hb08-09`) | PASS: 4 actors |
| New unit tests (above) | PASS |
| `rustfmt --check` on the changed files, `cargo clippy --all-targets --locked -- -D warnings`, `cargo test --locked` | PASS |
| Release launcher (`tools\Build-Launcher.ps1 -Skate3`) | PASS |
| Side by side with the installed game | NOT RUN |

## Uncertainty

- Hits reach an enemy here from Sora's side only, so the damage record names Sora as the attacker. A party member's
  hit should name that member.
- The other damage record words (+0x18 flags, +0x1c the attack, +0x20 the parameters) still do not read.
- A continuing window takes the hit id of the motion's window that started last. In the executable it is the id of
  the object's last attack of any kind, including script attacks.
- `motion_attack_windows` (summons' windows) does not read range 0x21.
- Sephiroth's earlier defeat and the changed hit counts follow from the turns and windows; they were not checked
  against the game.

## Verdict

PARTIAL. The roster's turn, damage-record, continuing-attack and OBJ_INIT faults are fixed and the tours pass; not
compared with the installed game.

## Next

Still not carried out, by roster count. Frame events 13 / 14 (9 models): the class's vt+0xa0 → FUN_1403dad90 →
FUN_1403ce580 posts a prioritised request to the object `+0xbb4` names. Range 0x17 (7): FUN_1403cd890 →
FUN_1401a8a20 on the effect set at `+0x7b0`. Frames 26 / 27 (6): FUN_1403cd670 / FUN_1403cd4c0, a colour fade
through FUN_1403b7810. Frame event 25 (0x19, M_EX630): vt+0x90 → FUN_1403d2510, which gives the attack set a new hit
id (FUN_1403cf490), so later attacks can strike the same targets again. Range 0x33 (3, FUN_1403db080). Frames 23 / 24.
The physics modes frame event 0 switches to.
