# T299 — Fighters share one object list: shouts, teamwork blocks and searches reach each other; `target_attack`

Owner Claude, 2026-10-30, requested by burba in the project thread. Gate: development of enemy AI in the test room; no
final gate is inferred. Prerequisites: T283, T290. Build identity: local `main` after `01716cb`, package 0.1.2, release
launcher with the `skate3` feature. Source: the selected Steam KH2FM build, validated each run; the install was only
read. Ghidra 12.1.4 headless on private read-only project copies (plain Ghidra, no MCP).

Private evidence (outside the checkout): `%LOCALAPPDATA%\KH2FinalRewrite\research\enemy-1to1-20261027\g\team\`
(FUN_1403f6df0, FUN_140430510, FUN_14039c830, `refs*.txt`, `tab.txt`); the object scripts disassembled in a private run
folder; run logs there (`sweep4`, `sweep5`).

## What was wrong

Every fighter ran its script in a world of its own, with object id 0. The executable has one object list. So:

- 38 enemy scripts shout (`trap_obj_act_shout`) and no other fighter heard it.
- 33 use `trap_teamwork_alloc`, the block shared under a key, which M_EX760 and others keep their group's state in. Each
  fighter had its own block, and was always "first".
- `trap_obj_search_by_*`, `trap_enemy_each` and the searchers could only find a fighter's own script-made objects.

`sweep4` also showed M_EX750 and M_EX920 faulting with `Memory(72)` / `Memory(244)` in their reaction handler.

## What the executable does

- **Teamwork blocks** (FUN_1403f6df0, 2:21 FUN_140432d70): a table of key and block at DAT_142ae75d0, count at
  DAT_142ae7650. A new key gets a zeroed block of size + 1; a known key sets the block's first byte and returns it.
  FUN_1403f6ea0 frees them all; FUN_14039c830 calls it along with the other per-battle-world resets.
- **Shouts** (FUN_1403c9d40): every queued shout goes to every object of the list.
- **`target_attack`** (1:166 FUN_140430510): the word at +8 of the record it is given, as an object reference. In trigger
  11 that record is Sora's reaction target, and for an attack-borne reaction +8 holds the attack.
  `attack_rc_receiver` (2:52) then gives the script back the receiver it set with `attack_set_rc`.

## Changes

- `enemy::world`:
  - `Object::lent`: an object that stands in this world's list for searches and handles, which nothing here runs (ticks,
    captures, attachments and the shout job skip it).
  - `Teamwork`, `World::teamwork_block`: blocks in their own handle range; a host gives its worlds one store.
  - `World::heard`, `World::sent`: shouts across worlds. `World::in_list`: a handle to an object not here reads as gone
    instead of faulting. `World::make_id`: object ids from the host. `OBJECT_IDS`.
- `traps::state::target_attack`; `unsupported_vector` is gone, as this was the last trap using it.
- `traps::attack::rc_receiver`: an attack gone since the press gets a block to write in.
- `ScriptBrain`: fighters get list-wide ids (`next_fighter_id`, `next_made_id`), the shared store
  (`fighters_teamwork`), `lend_objects`, `take_back_objects`, `take_sent_shouts`, `hear_shouts`. `command` takes the
  attack, so the record's third word names it. Summons and party scripts keep their own worlds and id 0.
- `enemy_battle::tick`: while a fighter runs, the others' objects are lent to its world; afterwards shouts pass on and
  are logged (`enemy_shout`). The store is emptied once no actor is left. `register_fighter_kind`: a fighter's motions
  are reachable under its entry from another's environment.
- Unit tests: `a_lent_object_hears_nothing_here_and_a_shout_is_handed_on_for_the_other_worlds`,
  `a_handle_to_an_object_elsewhere_in_the_list_is_no_fault`, `nothing_runs_a_lent_object`.

## Checks

| Check | Result |
| --- | --- |
| `cargo test --locked -p enemy -p area` on these changes | PASS |
| `cargo clippy --locked -p enemy --all-targets` | PASS: no warning |
| Release launcher (`tools\Build-Launcher.ps1 -Skate3`) | PASS |
| Targeted runs (lk03, hb18, nm02, mu01, he09 ×2, hb25; 3000 ticks) | PASS: shouts cross between fighters (`funnel_shot`, `atk_down_normal`, `funnel_shield`, `atk_group`, `route_panic` and others); M_EX750 and M_EX920 no longer fault |
| Full sweep with this build (`sweep5`, 74 programs) | NOT RUN to the end when this was committed |
| Side by side with the installed game | NOT RUN |

## Uncertainty

- A fighter's own objects hear its shouts in the same tick, the others one tick later. The executable tells all in one
  frame job.
- Lending covers a fighter's tick. Callbacks outside it (Sora's hit, a reaction) see other fighters as gone. No run has
  shown a script that minds.
- A script running a callback on another fighter's object does so in the running fighter's world: its memory blocks and
  attacks land there.
- Attacks are not lent, so a script cannot read another fighter's attack by handle.
- The caller of FUN_14039c830 was not traced; "no actor left" stands in for the end of the battle world.
- Object ids wrap at 511 fighters and 3584 made objects. A collision needs that many alive.

## Verdict

PARTIAL. Group communication runs in tours; not compared with the installed game.

## Next

The rest of `sweep5`; the Gambler's dice and cards; the per-family report.
