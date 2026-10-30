# T287 — Flyers know they fly, fly-jumps end, and the hit reactions follow the executable

Owner Claude, 2026-10-30, requested by burba in the project thread ("all enemy ai working for every enemy in the game
starting with all heartless then all nobody's"). Gate: development of enemy AI in the test room; no final gate is
inferred. Prerequisites: T283 (the sweep), T274 (the fly-jump arm). Build identity: local `main` after the T286 commit,
package 0.1.2, release launcher with the `skate3` feature. Source: the selected Steam KH2FM build, validated each run;
the install was only read. Ghidra 12.1.4 headless on private read-only project copies (plain Ghidra, no MCP).

Private evidence (outside the checkout): `%LOCALAPPDATA%\KH2FinalRewrite\research\enemy-1to1-20261027\g\` (`fl`, `m11`,
`xd`, `xd2`, `m4`, `c6.txt`, `vt8.txt`, `ms`, `ms2`, `refs-pm.txt`, `refs-m10.txt`); the object scripts disassembled in
a private run folder; run logs there (`sweep`, `probe1`, `sweep2`).

## What the sweep showed

The first sweep ran 74 programs. Of the 114 models it placed, none faulted and none played a motion without a clip.
What remained was behaviour:

- **Flyers hung in the air.** M_EX210 and M_EX110 held motion 44 for the whole run, their act threads in a wait for
  `!(!obj_is_entry_fly && obj_is_air)`.
- **M_EX890 hung at the top of its fly-jump** with `obj_is_jump` on for the rest of the run.
- **`enemy_exec_damage_blow`, `_small`, `_hitback` and `_large` did nothing** but report themselves, so the hit took no
  reaction in M_EX050, M_EX700, B_CA020, EH_G_EX250 and others.
- **hb24, hb25 and others placed nobody**: their group points start disabled and wait for a mission signal.

## What the executable does

- **`obj_is_entry_fly`** (1:124 → FUN_1403ba700) is bit 3 of the objentry's +0x48 word.
- **Mode 11, the fly-jump.** Enter (FUN_140423410) keeps the mode it left at +0x108 and the height above the floor at
  +0x100. The fall motion near the apex is the armed one at +0x118, none by default (FUN_1403cb680 → FUN_1403cb610).
  Back at its height after the rise (FUN_140423360) it lands (FUN_1404233e0 → FUN_1403cb2a0): the armed land motion
  at +0x11c (motion 5 only if the set has it) with idle queued, the mode at +0x108, and `+0x120` bit 27.
- **Reactions** (FUN_1403d2fe0):
  - Class 2 is FUN_1403d43b0: motions 10 to 13 by side and height (DAT_14074c390), with the other height when the set
    lacks one.
  - Class 3, and `enemy_exec_damage_small`, is FUN_1403d4260. In the air (FUN_1403ba9a0) it is FUN_1403d3840 (motion
    14 or 15, DAT_1405c63f8, heading set, motion 4 queued) and FUN_1403d4800 (a hop 50 high at the strength times
    HitBackToJump). On the ground it leaves a flying blow if one is running (FUN_140410300), flinches and slides
    (FUN_1403d4960). A mode with flag bit 4 takes the aerial flinch with `+0xd08` queued, and the slide.
  - Class 4, and `enemy_exec_damage_blow`, is FUN_1403d3f50: the heading set, one of two motions by side, the velocity,
    then FUN_140409d30 picks mode 4, or mode 8 for an object flying or in a range with `+0x18c` bit 6 (0x15, 0x2d and
    0x21 for classes that do not reach here). Landing plays the motion after the blow's (FUN_140422700).
  - Class 14, and `enemy_exec_damage_large`, is FUN_1403d4140: motion 23 or 24 by side, idle behind it, the slide.
    `enemy_exec_damage_hitback` is FUN_1403d4120, the slide alone.
- **Mode 8, the flying blow.** Enter (FUN_140410460): stop speed = first speed × SSTM +0x40. FUN_140409d30 scales the
  upward speed by +0x44. Each step (FUN_140410330) scales the velocity by +0x3c. Below the stop speed (FUN_140410390)
  it plays the next motion, stands still and goes to mode 5, or mode 0 under `+0x18c` bit 6.

## Physics modes on the open list

`FUN_14040dd70` case 0's 0x14, 0x1a, 0x28, 0x2c, 0x31 and 0x3f are not reachable by the ordinary enemies. The pending
modes come only from:

- mode 10's and mode 22's jump slots (FUN_140423210, FUN_1404241b0); mode 10 is entered only for character 0x4ff in
  the party class (FUN_1403d4bd0) or from Sora's routines (FUN_1403d6470);
- a Sora reaction (FUN_140405ed0, FUN_140406020, reaction 0x1a5);
- Sora's own routines (FUN_140536330, FUN_14053b6f0, FUN_14053bc00, FUN_14053d230, FUN_14053d340).

Nothing is ported for them here. (Correction, T289: the Gambler's `player_dice` and `player_card` put Sora in modes 0x2b and 0x2e, and those lead to 0x2c and 0x31, so M_EX950 does reach them.)

## Changes

- `ScriptSetup::entry_flags`, the catalog's `Entry::flags` and the object spawner set `Sysobj::entry_fly`.
- `Sysobj::update_fly`: the armed fall and land motions, the mode it left; `Fly::steer`.
- `damage.rs`: `flinch`, `air_flinch`, `exec_damage_small`, `exec_damage_blow`, `exec_damage_hitback`,
  `exec_damage_large`; `react` goes through them; the record's direction and height in `Sysobj`; `Reaction::Blow` names
  its land motion; `Reaction::FlyBlow`, `FlyBlow`, `update_fly_blow`, `end_fly_blow`; the flinch's queued motion.
- `traps::state`: traps 2:1, 2:2, 2:3 and 2:87 call them.
- `actor_physics::Sstm`: `fly_blow_brake`, `fly_blow_stop`, `fly_blow_up`, read from +0x3c, +0x40 and +0x44.
- The area tour (tour setup only) opens group points that start disabled.
- Unit tests: `a_fly_jump_plays_its_own_motions_and_goes_back_to_flying`,
  `the_scripts_blow_plays_its_own_motion_and_lands_on_the_next`, `a_flying_object_takes_the_flying_blow_and_flies_on`,
  `a_small_hit_in_a_jump_flinches_in_the_air_and_hops`.

## Checks

| Check | Result |
| --- | --- |
| Sweep with these changes (`sweep2`, first 33 programs) | PASS so far: M_EX110 travels 35143 units (326 before), M_EX210 19685 (1620 before), M_EX890 32138 (9368 before), and none holds still past 700 ticks; hb25's M_EX900, M_EX910, M_EX930, M_EX940 appear; no faults |
| `cargo test --locked -p player -p enemy -p area` on the staged changes | PASS |
| `cargo clippy --all-targets --locked` | NOT RUN to PASS: `source/data/src/kh3` (another thread's) stops `-D warnings`; no warning in the changed files |
| Release launcher (`tools\Build-Launcher.ps1 -Skate3`) | PASS |
| Side by side with the installed game | NOT RUN |

## Uncertainty

- FUN_1403ba190's gate on the knock and the hop is not modelled.
- A hop (FUN_1403d4800) is applied only in a jump mode. The other modes' slot 3 was not read.
- Mode 8's 0x21, 0x2d and 0x15 variants of FUN_140409d30 are not modelled.
- Class 3 under a mode with flag bit 4 queues motion 0, not `+0xd08`.

## Verdict

PARTIAL. The flyers and fly-jumps move and the reactions run, in tours. None of it is compared with the installed game.

## Next

The enemies that still stand still long in `sweep2`; the alpha fade of frame events 23 and 24 and enemies' fades shown
at all; the rest of `sweep2`.
