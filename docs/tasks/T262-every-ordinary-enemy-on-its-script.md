# T262 — Every ordinary enemy fights on its own object script

Owner Claude, 2026-10-27, requested by burba in the project thread ("start work on all enemy AI and animations 1:1
using ghidra", then "go ahead"). Gate: development of enemy AI in the test room; no final gate is inferred.
Prerequisites: T125 (the battle actor host), T133 and T190 (the object-script engine and its services), T206 and
T219 (boss framework, map body), T260 (body push). Build identity: local `main` after `42f8e02`, package 0.1.2,
release launcher with the `skate3` feature. Source: the selected Steam KH2FM build, validated each run; the install
was only read. Ghidra 12.1.4 headless on private read-only project copies (plain Ghidra, no MCP).

Private evidence (outside the checkout): `%LOCALAPPDATA%\KH2FinalRewrite\research\enemy-1to1-20261027\` (the
battle-class vtable dumps `g\vt1.txt`, `g\vt2.txt`, `g\vtcmp.txt`), the earlier
`research\drive-magic-20261026\g\range\` decompiles (every function of 0x1403a0000–0x140430000) and
`research\enemy-ai-20261016\bdx\` (the installed object scripts, disassembled in this session's scratch folder).
Run logs: a private run folder outside the checkout (`roster-04.jsonl`, `roster-05.jsonl`, `combat-01.jsonl`,
`combat-02.jsonl`, `seph-01.jsonl`).

## What the executable does

- **An enemy's AI is its object script.** FUN_1403df930 builds an object by its objentry type: 3 and 12 as bosses
  (FUN_1403f8da0), 4 as YS::ENEMY (FUN_140419e30, +0x6c8 |= 0x800, the "zako" bit) and 21 on top of it
  (FUN_14041ab50), all on the battle object FUN_1403db840, which starts with FUN_140411700 (13 too, through
  FUN_14041a210). The script then runs on the interpreter T133 translated.
- **The script binds its own sheet and prize.** Soldier's init (`M_EX010` word 1458) calls its setup with ENMP row
  33 and prize 1, which `btlobj_set_sheet` (2:9) binds and the setup keeps at work offset +28; Shadow passes 34 and
  2, Dusk 41. The death act calls `obj_scatter_prize` (1:92, FUN_14042c960 → FUN_1403b6170) with that prize row,
  which drops it through FUN_1403dc170 at the object (above it by +0x73c, then a line test, FUN_14040c2b0).
- **The map body.** FUN_140411700 sets the body class (1, or 2 with objentry +0x48 bit 3) for every battle object it
  builds, so ordinary enemies have the same map body as bosses (T219 kept it to type 3).
- **The ordinary enemy's death.** YS::ENEMY's vtable (0x1405d2d68) forwards almost every slot to the shared battle
  object routines (hit FUN_1403dbb90, reaction FUN_1403d2fe0, HP FUN_1403d2eb0); its vt+0xb0 is its own,
  FUN_140419d10: event signal 0x4b with the spawn record's serial (FUN_1403b5bf0, +0x9f0 → +0x1e), motion 16 when
  the record behind +0xc has type 0x17, then the shared death FUN_1403dbaf0. Its vt+0x38 (FUN_140419b40) makes the
  object entry +0xd20 (FUN_1403f8310) when +0x9b8 bit 2 is set, before the usual FUN_1403d36d0.
- **Shapes.** FUN_1403c94c0 tests two collision shapes by kind: 3 against 3 as spheres, 0 with 0 or 3 as an
  ellipsoid of the summed radii and half heights (FUN_1403c9890), 1 with 1 as upright cylinders, and 1 against 0 or 3
  with FUN_1403c9780 (the round shape's reach narrows above the cylinder's half height). FUN_1403ba550 says whether a
  shape's group is on: the script's {on, off} words at +0x5a8 (traps 1:52–54), then the motion's at +0x5a0 (range
  events, reset each tick), then the collision table's default mask.

## What the rewrite did

- Only Soldier, Shadow, Dusk and Sephiroth could fight, and the first three ran hand-written brains unless
  `KH2RW_ENEMY_AI=script`; their ENMP rows and prizes were constants in the host.
- The death drop came from the host at the kill, not from the script's `obj_scatter_prize`.
- Ordinary enemies had no map body. An enemy attack shape that was not a sphere stopped the enemy with an error;
  HIT shapes outside group 0, shape kind 1, motions whose first key is negative and attack windows naming a group
  with no shapes made the enemy refuse to load.

## Changes

- `enemy_battle::supported`: Sephiroth and every objentry type 4 or 21. Every one runs on its object script; the
  hand-written brains stay behind `KH2RW_ENEMY_AI=port` for comparisons. The host starts from the fallback row 0x45
  (FUN_1403ebf30), runs the script's init and reads the assets again with the row the script bound.
- `ScriptBrain::prizes` takes `Event::ScatterPrize`; the room drops that przt row (FUN_1403ecc70, absent rows fall back
  to 0x48) where the object stands when the script asks. The hand-written brains still drop at the kill.
- `enemy::damage::Object::die`: an ordinary enemy posts signal 0x4b with its spawn serial (FUN_140419d10).
- `enemy_combat::map_body`: objentry types 3, 4, 12, 13 and 21.
- `player::body_push::overlap`: kind 1 (upright cylinder) and FUN_1403c9780; the enemy hit tests (Sora's attacks on
  an enemy's body, an enemy's attacks and guards against Sora) use it with each shape's own kind.
- Loader: shape kind 1 admitted; HIT shapes of every group kept and hit only while FUN_1403ba550 says their group
  is on (`ScriptBrain::group_on`); motions whose first key is negative admitted (clock 0 is that key); a window that
  names a group without ATTACK shapes hits nothing instead of refusing the enemy.
- `KH2RW_TOUR_ROSTER=1` with `--enemy-combat-tour`: every enemy that can fight, one at a time, 720 ticks each (AI on
  at 10, Sora attacking from 390), with an `enemy_roster_slot` summary (load error or fault, acts, motions, hits both
  ways, death, orbs, distance moved, untranslated services). `KH2RW_TOUR_MODELS` narrows it.
- Unattended runs open their window unfocused and behind other windows and read no keyboard, mouse or controller
  (`visual::input::Devices::None`; a controller reaches every window). The enemy combat tours skip the room's
  scripted camp check.
- `target_set_before_player` (1:162): a null object (a search that found nothing) only clears the gone entries,
  as FUN_14040c9d0 does, instead of faulting the script.
- `source/data/examples/collision_dump.rs`: prints a model's collision table from the install.

## Checks

| Check | Result |
| --- | --- |
| Roster, previous build (`roster-04`, all 99 type 4/21 entries with a model and motions) | 69 loaded and ran on their scripts; 30 refused by the loader checks fixed above; 12 of the 69 stopped at their first non-sphere swing; 3 script faults |
| Roster retry of the 45 failed or faulted entries (`roster-05`), then the 5 still faulting after the null-object fix (`roster-06`) | 30, then 4 more, fight without a fault: 88 of the 99 in all |
| Combat tour (`--enemy-combat-tour`, Shadow, Dusk, Soldier now on their scripts by default) | PASS: 16 hits on the enemies, 2 on Sora, Scan, lock and cycle checks pass |
| Sephiroth tour to his defeat (`KH2RW_SEPH_TOUR_END=3000`, `_LOW=1800:30`) | PASS: defeated at 2773, mission clear at 2960, exit 0 |
| New unit tests `an_ordinary_enemys_death_posts_its_spawn_serial`, `upright_cylinders_test_their_gap_and_reach`, `a_null_object_only_clears_the_gone_entries` | PASS |
| `cargo build --locked` | PASS |
| Smoke `cargo run -p main --locked -- --frames 180` | NOT RUN (it opens a focused window on the owner's desktop) |
| `cargo fmt --all --check`, `cargo clippy --all-targets --locked -- -D warnings`, `cargo test --locked` | PASS |
| Release launcher (`tools\Build-Launcher.ps1 -Skate3`) | PASS |
| Any enemy side by side with the installed game | NOT RUN |

Not running after this task (11 of 99): EH_G_EX290 (script fault at word 3482, `Memory(0)`, not traced); five objects
with no primary HIT or TARGET shape in group 0 (No. 13, B_EX220, B_EX220_LV99, B_EX350, EH_G_EX120); three with
raw-matrix motions (No. 4, B_EX210, B_EX380); B_AL110 and B_AL120 (no model entry). Among those that run, some hit
Sora and some were hit by his attacks in their slot (Large Body was defeated at 676): the census only shows that the
scripts run, act, move and attack in the room.

## Uncertainty

- Cannon Gun, Camo Cannon, Air Pirate and Aerial Viking share a "teamwork" block and look up the object of spawn
  serial 92 for `target_set_before_player`; the room has none. In the executable the search's miss is a null
  object that FUN_14040c9d0 skips, which the engine now does too. What serial 92 is in the game's areas is not
  known.
- The prize drop's position: the room uses its own floor probe, not FUN_1403b6170's offset and line test.
- FUN_140419d10's motion 16 (the +0xc record of type 0x17) and FUN_140419b40's made object are not done.
- Non-sphere attack shapes are swept between ticks like spheres (the executable's sweep of moving shapes was not
  read); the party still treats every enemy attack shape as a sphere.
- Motion sets with stance variants (MSET header bit 0, FUN_1403c7c50) are still refused; raw-matrix motions and
  objects without a primary HIT/TARGET shape too.
- The roster is a census in a flat debug arena with Sora standing still. A clean run does not show the enemy
  behaves as in the game.

## Verdict

PARTIAL. Every ordinary enemy with a model, motions and a supported motion clock can fight on its own script in the
room, binding its own sheet and scattering its own prize; the loaders follow the executable's shape and group rules.
The enemy AI gate stays NOT RUN: no enemy was compared with the installed game.

## Next

Motion sets with stance variants for enemies (FUN_1403c7c50 / FUN_1403c88c0); raw-matrix motions and objects without
a group-0 HIT shape; EH_G_EX290's fault; the area's spawn data so partners and groups exist; FUN_140419b40's made object and FUN_140419d10's motion 16; then per-family checks of
Shadow, Soldier, Dusk, Creeper, Large Body and the other first-world enemies against footage or the installed game.
