# T317 — Boss fights by default: a launcher list, script-only objects, captures read from the sets

Owner Claude, 2026-10-31, requested by burba in the project thread ("work on enemy ai"; issue #163). Gate: development of
enemy AI in the test room; no final gate is inferred. Prerequisites: T206, T306. Build identity: local `main` after `ca39491`
plus this change, package 0.1.2, release launcher with the `skate3` feature. Source: the selected Steam KH2FM build, validated
each run; the install was only read. Ghidra 12.1.4 headless on private read-only project copies (plain Ghidra, no MCP).

Private evidence (outside the checkout): `%LOCALAPPDATA%\KH2FinalRewrite\research\enemy-1to1-20261027\runs-20261031`
(`spawn_all.txt`, sweeps `f5`, `t5`, launcher-path run `bf`), decompiles in `g\medal`, `g\st`.

## What was wrong

- Boss types (objentry 3 and 12) ran only behind `KH2RW_ENEMY_BOSSES`, and only Sephiroth had a way in from the launcher.
- `obj_search_by_part` found nothing, because a fighter's `entry_part` was never set. B_EX390's OBJ_INIT looks for its own
  character (word 583, the `Memory(0)` of T306).
- `motions_available` left out every object whose objentry row names no motion set, and an archive with no model entry was
  refused. So Possessor (B_BB130), the window (F_BB090), Storm Rider's parts (F_MU070_BOSS) and the Lords' pillars (F_AL050)
  never existed, and the bosses waiting on them stood still.
- Sora's captured motions came from a table with one boss in it.

## What the game data shows

- `search_by_part` (FUN_14042de30 -> FUN_1403b4180) matches objentry +0x4c.
- A capture slot's first nested entry names the player's rig: `0010` on Sora's. B_EX100's are `0140` and B_LK120's `0200`.
  [I] Those are Roxas and lion Sora, from the worlds the fights are in. `0080` is one slot of B_CA010's and is not identified.
  Their skeletons do not fit Sora's kit, which refused the whole room until they were left out.
- FUN_14042cbd0 / FUN_14042cc00 (`obj_set_stealth`) set bit 4 per mesh group through the model's vtable (FUN_1401a8990,
  FUN_1401a85e0). Not ported.
- The Grim Reaper's medals are prizes of their own: FUN_14040b5a0 scatters them from +0xd1c, FUN_14040bd40 and FUN_14040bde0
  pull them to an object, FUN_14040c1f0 sets the pull. Not ported.

## Changes

- `enemy_battle::supported`: types 3 and 12 fight by default; the objects a fight searches for (8, 13, 23) in a boss fight or
  with `KH2RW_ENEMY_BOSSES`.
- `--boss-fight` and `boss_fight.rs`: the area's own program places everything (`area_enemies`), with the mission's clear or,
  with no mission, the last boss's fall ending the room.
- Launcher: a "Boss fights" list of the journal's boss Heartless and Nobody, each at the program that places it.
- `entry_part` set from objentry +0x4c for fighters and made objects.
- `InspectionModel::empty`, `SelectedMotion::held`: a script-only object has no bones and keeps its motions' clocks and
  events; an object with bones and no set holds its bind pose. [I] With no bones its shapes stand on the object's origin.
- `boss::capture_motions` reads every object of the area's sets, slot 240 upward, for `0010` entries. The area now loads
  before Sora's kit when its enemies are on.
- `KH2RW_TOUR_LOW=<tick>` for tours, and `enemy_summon_rejected` and the placed event's `skipped` list in the log.

## Results by fight

Unattended tours, `KH2RW_TOUR_LOW=1500`: at tick 1500 each boss drops to 1 HP and Sora still has to land the blow. "Clear"
means the death signal, the mission's overlays and `mission_clear` all ran. **None of this is compared with the real game, and
no fight has been played by hand.**

| Fight | Result in the tour | Open |
| --- | --- | --- |
| Dark Thorn (bb05) | Clear | Stealth |
| Prison Keeper (nm03) | Clear | |
| Barbossa and Illuminator (ca10) | Clear on Barbossa's death | Illuminator's darkness not checked |
| Twilight Thorn (tt34) | Clear | Fought as Sora: Roxas's captures are left out |
| Storm Rider (mu08) | Death and mission start reached on an earlier build (`f4`); on `f5` still at 1 HP at tick 5400 | Tour rarely reaches him |
| Volcanic Lord and Blizzard Lord (al03) | **FAIL:** the clear starts on the first Lord's death | Mission's own rule not run |
| Shadow Stalker (bb05) | Possessions run, Sora lands no hit in the tour | Tour stands below him |
| Thresholder and Possessor (bb11) | Both act; Thresholder stops in `freeze` at 1 HP | Not understood |
| Groundshaker (lk15) | Attacks run, Sora lands no hit; asks for slot 252 every tick | Needs lion Sora's captures |
| Grim Reaper (ca01) | Runs, cannot be hurt | Medals not ported |

## Checks

| Check | Result |
| --- | --- |
| `cargo fmt --all --check`, `cargo clippy --all-targets --locked -- -D warnings`, `cargo test --locked`, `cargo build --locked` | PASS |
| Release launcher (`tools\Build-Launcher.ps1 -Skate3`) | PASS |
| Room as the launcher starts it, `--boss-fight --frames 240`, no environment switches (Dark Thorn) | PASS: objects placed, mission loaded, exit 0 |
| Same fight to the win (`bf`) | PASS: `mission_dead_boss` 1950, `mission_clear` 2137, `boss_fight_won` 2138 |
| B_EX390 and B_EX180 in the first 1800 ticks of their programs | PASS: no fault. B_EX180's second instance needs a longer run |
| The launcher's list screen on screen | NOT RUN |
| Other bosses' programs (the 70 of T306) on this build | NOT RUN |
| Side by side with the installed game | NOT RUN |

## Uncertainty

- The room's Sora has the config's level, not the story's at that fight, so fights can be far too hard or too easy.
- The Dark Thorn capture shows white halos round the chains and a black floor. Neither was traced.
- HP bars were not looked at in these runs.
- Opening cameras and message windows only report themselves.

## Verdict

PARTIAL. Boss fights start from the launcher on their own scripts, and four of ten reach their clear in tours. The other six
are listed above.

## Next

Volcanic and Blizzard Lord's clear, Thresholder's stop, Grim Reaper's medals, then the rest of issue #163.
