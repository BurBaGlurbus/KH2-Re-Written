# T275 — Blows and knock slides move through the map solve

Owner Claude, 2026-10-28, requested by burba in the project thread ("keep working dont idle"). Gate: development of
enemy AI in the test room; no final gate is inferred. Prerequisites: T219 / T262 (map bodies for battle objects), T265
(edge refusal), T274 (tt05's Dusks). Build identity: local `main` after `e184ba4`, package 0.1.2, release launcher with
the `skate3` feature. Source: the selected Steam KH2FM build, validated each run; the install was only read.

Private evidence (outside the checkout): `%LOCALAPPDATA%\KH2FinalRewrite\research\drive-magic-20261026\g\range\`
(FUN_1403b9090); run logs in a private run folder outside the checkout (`tt05-05`, `tt05-06`, `combat-10`, `seph-06`,
`hb08-08`).

## What was wrong

In tt05 a Dusk struck by Sora's combo finisher (ATKP 129) at tick 1114 flew out through the area's edge and fell
without end (y −612 at 1200, −10212 at 1800). The engine moved a blow's flight and a knock slide with the room's plain
floor-following (`Env::movement`), not with the map body's solve, so walls did not stop them, a blow's landing ignored
the solve, and T265's knock-back edge probe (which lives in `apply_movement`) never ran for a knock slide.

## What the executable does

Every physics mode's displacement goes through FUN_1403b9090's solve (FUN_14016fec0 for a body); the blow and the knock
slide are no exception.

## Changes

- `Sysobj::update_reaction`: the blow's step and the knock slide go through `apply_movement`; a blow with a map body
  lands where the solve finds a floor (`touchdown`), one without keeps the room's floor test from above its start.

## Checks

| Check | Result |
| --- | --- |
| tt05 (`tt05-06`) | PASS: the Dusk struck by the finisher stays on the map (y 0 to −10 for the rest of the run) and fights on; all three Dusks to 1800 without a fault; 4 hits on Sora, 8 on the Dusks |
| Combat tour in the flat arena (`combat-10`) | PASS: 16 hits on the enemies |
| Sephiroth tour to his defeat (`seph-06`) | PASS: defeated at 2682, mission clear at 2869, exit 0 (2182 / 2369 before: his knocks and blows now meet the map) |
| hb08 group pop-in (`hb08-08`) | PASS |
| `cargo clippy --all-targets --locked -- -D warnings`, `cargo test --locked` | PASS |
| Release launcher (`tools\Build-Launcher.ps1 -Skate3`) | PASS |
| Side by side with the installed game | NOT RUN |

## Uncertainty

- The solve is asked as for the object's mode (`air_query`); whether the executable solves a blow as an air move was not
  read from the blow mode's flags.
- Sephiroth's later defeat in the tour is the expected effect of the solve, not checked against the game.

## Verdict

PARTIAL. Blows and knock slides meet the map's walls, floors and edges as other moves do; not compared with the
installed game.

## Next

The other physics modes that frame events switch to (FUN_14040dd70 case 0's 0x14, 0x28, 0x2c, 0x31, 0x3f; cases 4/5).
