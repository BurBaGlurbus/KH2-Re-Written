# T323 — Saves hold the game: capture, load and the title's Load

Owner Claude, 2026-11-01, requested by burba in the project thread ("work on issues on the github"). Issues #79 and #32 (part),
#69 (part). Gate: development of saves in the test room; no final gate is inferred. Prerequisites: T155 (container, menu), T251
(the field's Save menu), T303 (puzzle fields). Build identity: local `main` after `e2e23b3` plus this change, package 0.1.2.
Source: the selected Steam KH2FM build, validated each run; the install was only read. Ghidra 12.1.4 headless on a private
read-only project copy (plain Ghidra, no MCP).

Private evidence (outside the checkout): `%LOCALAPPDATA%\KH2FinalRewrite\research\issues-20261031\g\` (decompiles of
FUN_1402ed110, FUN_1402ed0e0, FUN_1403a05f0, FUN_1402e5000, FUN_1403c1d90, FUN_1403a1410, FUN_1403e8380, FUN_1403edca0,
FUN_140368ed0, FUN_1403e2e20, the table at 0x1405ba910); runs under `%USERPROFILE%\kh2-issues-out\s79\`.

## What the executable does

The file menu copies the live structure into the image on save (FUN_1402ed110, which also picks the card's background) and back
on load (FUN_1402ed0e0), then FUN_1403a05f0 takes the place out of it and FUN_1402e5000 applies the config bits. Everything a
save holds is therefore one structure. The room keeps the same numbers on its own objects.

## Change

- `save::state`: the party records (all 13 of 0x114 bytes as FUN_1403c1d90 reads them), the form records (FUN_1403a1410), the
  gauge, the per-world party bytes (`FUN_1403e2e20`), bonus levels (FUN_1403edca0), world play time (FUN_1401574e0) and the
  offsets of the item counts, seen bits, journal block and puzzle fields.
- `test_room::game_save`: `capture` writes the running game over the image the room started from; `apply_progress`, `apply_game`
  and `apply_camp` read one back. Story flags, programs, world records, munny, EXP, level, stats as bonus over the level's row,
  equipment, abilities, forms, the stock by the item table, Donald's and Goofy's records, config, shortcuts, MP charge bits, the
  camp's and journal's seen marks and the puzzle boards.
- `--load-image <file>` starts the room at the image's world, area and entrance with its story state and `apply_*`;
  `--save-dump <file>` writes each save to a file too.
- The save point's entrance comes from the object's argument 1 (FUN_1403a8b10 case 0x37), so a loaded file stands Sora at the
  save point's own spawn (`m_99` in Twilight Town). The card's background follows FUN_1402ed110's table.
- Title: Load writes the chosen slot to `saves/handoff.sav` and prints `kh2fm-handoff Load`; the launcher starts the room on it.
  The save point's Title Screen row closes the room and the launcher starts the title.
- The camp's panel shows the real munny, level and play time. AP moves with the level's row.
- The journal's Limits, Missions, Minigames, Synthesis, chest and report pages read a loaded save's fields (#69) instead of the
  test-room's empty values.

## Checks

| Check | Result |
| --- | --- |
| `cargo test --locked -p save`, `-p area progress`, `-p main --lib game_save` (synthetic bytes) | PASS |
| Release `--save-point-tour --save-dump` on Twilight Town area 9 (`a1`) | PASS: image of 0x10fc0 bytes, checksum right, place 2/9/99, 168 story flags, 42 program rows, 142 stock counts |
| A crafted copy (level 7, 4321 munny, 500 EXP, other Keyblade, HP/MP, gauge, Donald's HP), `--load-image` then save again (`a2`) | PASS: `save_applied` logs them, the card reads LV 7 / 4321 munny, Sora starts at the save point's spawn, the second image holds the same values |
| The launcher's Load, interactively | NOT RUN (the title's unattended plan stops before the hand-off) |
| A save written by the installed game loaded this way | NOT RUN (#82) |
| Side by side with the installed game | NOT RUN |

## Uncertainty

- The Keyblade slot, level and AP round-trip, but the room's AP includes its development grant, which a u8 record clips.
- Party levels stay at the record's value: the room has no party level-up (FUN_1403ecf30 runs over 13 characters).
- [I] Album and story seen bits use the row's place among its world's rows as FUN_1403460d0's `ord`.
- [I] FUN_140157350's random draw for the background is not the same stream.
- Not captured: the items and places the room has no writer for keep the base image's bytes (command use counts, chests opened
  in the field, form ability orders).
- The world map's own hand-off is T324.

## Verdict

PARTIAL for #79: the game state round-trips in the room and through the launcher's path; #82's comparison stays open.

## Next

The world map side (#143), then the party order and world parties (#34), then cutscene story state (#25).
