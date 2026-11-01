# T346 — Items go where the executable puts them, boxes stop at walls, a Keyblade change keeps Sora's other motions

Owner Claude, 2026-11-01, requested by burba in the project thread ("commit to local main and continue working on issues on guthub"). Issues #79, #67,
#69 (parts). Gate: development of the field in the test room; no final gate is inferred. Prerequisites: T337, T338, T341. Build identity: local `main`
after `dec6d1c` plus this change, package 0.1.2. Source: the selected Steam KH2FM build, validated each run; the install was only read.

Private evidence (outside the checkout): `research\issues-20261101\chest\chest-spec.md` §2.3; runs `j7`, `pz3` to `pz8`, `ch11` to `ch13`, `su1` under
`%USERPROFILE%\kh2-issues-out\s161\`; crafted image `s161\sl.sav`.

## What the executable does

- FUN_1403c4d50 and FUN_1403c3f00: a type 0 item takes Sora's first slot whose item is 0 and whose auto-reload item is 0 or this item. Anything else
  counts in the stock, below 100; a full stock adds nothing while the chest's bit and window still happen.
- A non-key item also sets bit `record +0x12` at save+0x3d88.

## Changes

- `ItemRt::give`, used by chests and boxes. `game_save::note_obtained` and `state::OBTAINED_ITEMS` write the bit.
- `FormSources::base_external`: the base kit keeps its motions from other files (Limit, summons, captures, chests) and its scripted summon motions when
  a Keyblade change rebuilds it. Before this, a game loaded with another Keyblade, or a Keyblade changed in the menu, lost them all, so Sora did not
  swing at a chest. Found by the chest tour on the crafted image; older than this batch.
- `menu_screen::load_images` reads with the map limit: `menu/jmface_heart.2ld` holds 69 images, past the inspection limit, and the Heartless pages had
  blank faces.
- A dropped box stops at a wall (a ray along its move). `World::resolve_walls` does nothing on a real map, as its comment says.
- `--prize-tour` throws a box at the nearest wall and logs `box_thrown`, `box_wall`, `boxes`.

## Checks

| Check | Result |
| --- | --- |
| `cargo fmt --all --check`, `cargo clippy --all-targets --locked -- -D warnings`, `cargo test --locked` | PASS |
| Unit test: slot rule, then stock, then full | PASS |
| Release `--journal-tour` on the crafted image (`j7`) | PASS: the Soldier's portrait draws on the Heartless page with Slain 100 and Cyclone 4 |
| `--chest-tour` on the crafted image, before the kit change (`ch11`) | FAIL: `capture` false, no swing; the same tour with no image was true (`ch12`) |
| `--chest-tour` on the crafted image, after it (`ch13`) | PASS: `keyblade_kit` at tick 0, then `capture` true; Sora swings, the lid opens, "OBTAINED" Tent |
| `--summon-tour` on the crafted image (`su1`) | PASS: the summon starts, appears and runs on the rebuilt kit |
| `--prize-tour` with `resolve_walls` (`pz7`) | FAIL: the box passed the wall at (-2723, 1128) and rested at (-2768, 1083) |
| `--prize-tour` with the ray (`pz8`, tt area 13) | PASS: `box_wall` at tick 40; it rests at (-2677, 1174), on Sora's side |
| A type 0 item reaching an empty slot in a running game | NOT RUN: the unit test covers the rule; the tours' items are stock items and the images' slots are full |
| Side by side with the installed game | NOT RUN |

## Uncertainty

- [I] The box's wall stop is not traced: a ray at one height along the move with a radius of 30, and it loses its sideways speed. The executable's box
  has a map body; its solver was not ported for this, because `collide` also writes Sora's floor state.
- `resolve_walls` is also a no-op on real maps for its other callers (`magic_room`, `party_room`, `enemy_script`, `summon`), so those pushes do nothing
  there. Not changed here.
- A worn body and Drive forms keep what they had: a worn one gets the chest motions only, a form none.
- The rebuilt kit keeps a copy of those motions for the life of the run. Its memory was not measured.
- FUN_1403c3fe0's side cases (item 0x20, kinds 0x12 and 0x15) and member 1's list at save+0x36e8 are not ported.

## Verdict

PASS for the items above in the room. No claim about the installed game.

## Next

The GitHub comments and the batch's release build.
