# T324 — The world map and the field share the save; world points have their row

Owner Claude, 2026-11-01, requested by burba in the project thread ("work on issues on the github"). Issue #143, with #79.
Gate: development of the world map and the test room; no final gate is inferred. Prerequisites: T323, T212, T214. Build identity:
local `main` after `2817929` plus this change, package 0.1.2. Source: the selected Steam KH2FM build, validated each run; the
install was only read.

Private evidence (outside the checkout): `research\save-20261016\out\callers-4950\` (FUN_1403a8b10), runs `w1`, `w2`, `w3`, `p1`
under `C:\Users\ANIMA\kh2-issues-out\s79\`.

## What the executable does

The world map is an area of the one running game, so nothing has to cross. FUN_1403a8b10 case 0x37 opens the menu with mode 0 at a
WORLD_POINT (objentry 0x239) and mode 2 at a SAVE_POINT (0x23a), and mode 2 hides the World Map row (FUN_1402eb490).
FUN_140154e20 then stores world 0xf, area 0 and the entrance beside the world left, and jumps.

## Change

- The rewrite's programs run one window each, so the save image crosses between them (`saves/handoff.sav`).
- `kh2fm wm --load-image <file>`: the story state, play times, camp and its panels come from the image; the map's start is the
  world left. `--save-dump <file>` writes a landing's image to a file too.
- A landing writes the image with the target's place, what the arrival and the camp changed and the play times, and starts the
  field on it. The landing's room config keeps the battle tables, so EXP and munny go on counting there.
- WORLD_POINT objects now count as save points with their own `obj/WORLD_POINT.a.us` effects (the room took only SAVE_POINT, so
  the row never showed). Their menu has the World Map row, which writes the image and starts the world map.
- With no save behind the map, the image gets Sora's record at the room's start, and a room given an image with no record keeps
  its own numbers.
- `KH2RW_SAVE_TOUR_KEYS=<tick>:<bits>,...` gives `--save-point-tour` another route through the menu.

## Checks

| Check | Result |
| --- | --- |
| Release `wm --load-image a2.sav --touch 47,48,50,52 --autoplay --decide --save-dump` (`w1`) | PASS: lands at 2/9/99; level 7, 4321 munny, 500 EXP, Keyblade, HP/MP and gauge stay, play time counts on |
| Release `room <landing config> --load-image w1.sav` (`w2`) | PASS: `save_applied` with those values, Sora at the entrance's spawn |
| Release `--save-point-tour` at Twilight Town area 9, map program 6, on the World Map row (`p1`) | PASS: the row shows, Yes logs `world_map_jump` from 2 with entrance 1; the image holds 15/0/1 |
| Release `wm --load-image p1.sav` (`w3`) | PASS: starts beside Twilight Town and lands back |
| The live chain room → map → field in interactive windows | NOT RUN (unattended runs do not start the next program) |
| Side by side with the installed game | NOT RUN |

## Uncertainty

- The windows change between programs, so the fades are each program's own, not the one jump the game makes.
- [I] FUN_1403f2270 (touching a world point) is not fired by the room; the map's cells still come from what the image holds.
- [I] WORLD_POINT uses SAVE_POINT's reaction shape (`obj/SAVE_POINT.mdlx`); not compared.
- The Gummi routes (#142) still end the map.

## Verdict

PARTIAL for #143: the state crosses both ways and the World Map row works; the interactive chain was not driven by hand.

## Next

The party order and world parties (#34).
