# T301 — A script-made fighter becomes a fighter of its own

Owner Claude, 2026-10-30, requested by burba in the project thread. Gate: development of enemy AI in the test room; no
final gate is inferred. Prerequisites: T290, T299. Build identity: local `main` at `6f02dd3` plus this change, package
0.1.2, release launcher with the `skate3` feature. Source: the selected Steam KH2FM build, validated each run; the
install was only read.

Private evidence (outside the checkout): the object scripts disassembled in a private run folder; run logs there
(`probe7`, `sweep5`).

## What was wrong

The journal's Sorcerer is M_EX960, and no spawn set places it: M_EX870, which spawn sets do place, makes it with
`sysobj_appear` in its OBJ_INIT. T290 drew the Sorcerer from the object in its maker's world. So it had no attack windows,
no map body and no lock-on, and it was left out of the logs.

The engine has one object list, and an object is an object whoever made it. T299's shared list means a made object need not
live in its maker's world.

## Changes

- `ScriptBrain::load` takes an object already built; `adopt` gives it a world of its own, the entry's fields and, if its
  script bound no sheet, the host's stats. `build` and `adopt` share `entry_fields` and `fresh_world`.
- `ScriptBrain::take_made`: the object leaves the maker's world.
- `enemy_gallery`: an actor made for such an object (`GalleryActor::made`) asks for battle at once and takes the object;
  from then on it runs as any fighter, and its maker reaches it by the same handle through the lent list.
- Two clippy findings in the T290 and T299 changes.

## Checks

| Check | Result |
| --- | --- |
| hb25's run with M_EX870 (`probe7`, 2400 ticks) | PASS: M_EX960 runs as a fighter of its own with its acts in the log; `funnel_shot` and `funnel_shield` shouts are made; no faults |
| `cargo clippy --locked -p main --features skate3` | PASS for the changed files |
| Release launcher (`tools\Build-Launcher.ps1 -Skate3`) | PASS |
| Sora locking on and hitting a Sorcerer in that run | NOT RUN: the tour did not reach one |
| Side by side with the installed game | NOT RUN |

## Uncertainty

- M_EX960's move acts (motions 152 to 157) barely change its position in the tours: it stayed at one place for 2400
  ticks, with no frame or range events on those motions. Whether its movement in the game comes only from the pose was
  not established against the game.
- Attacks and memory blocks its OBJ_INIT made in the maker's world stay there.
- If the load is refused after the object is taken, the object is lost. Only types 4 and 21 are taken, and no run refused.

## Verdict

PARTIAL. Runs in a tour; not compared with the installed game.

## Next

The rest of `sweep5`; the per-family report; the Gambler's dice and cards.