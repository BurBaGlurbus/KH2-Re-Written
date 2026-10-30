# T303 — The Journal's Puzzles: menu, board, tutorial, solved banner, List of Pieces

Owner Claude, 2026-10-30, requested by burba in the project thread ("start working on the github issues", then "continue").
Issue #68. Gate: development of the journal in the test room; no final gate is inferred. Prerequisites: T164 (the journal's
category pages), T297 (the grid page's scroll bar and seen bits). Build identity: local `main` after `4793a0f`, package 0.1.2,
release launcher with the `skate3` feature, built from a private export of `HEAD` with only this change applied. Source: the
selected Steam KH2FM build, validated each run; the install was only read.

Private evidence (outside the checkout): `research\playable-room-20261008\journal-04\puzzles-spec.md` and its decompiles;
`research\issues-20261030\runs\puzzles\` (the tour's captures and log).

## What the executable does

`jiminy.bar`'s `puzz` table has six rows: a shape (3 x 4 tiles of 84, or 6 x 8 of 42), whether pieces rotate, a name, a reward
item and a file. `15jigsaw.bin` has 158 records for 144 pieces. Each picture is `menu/jm_puzzle/<file>.bin`, one IMGD of 256 x 384.

States: 0x1b the menu, 0x1c the list, 0x1d the board, 0x1e / 0x1f the tutorial, 0x20 the banner, 0x21 and 8 the List of Pieces
(the Treasures page's third kind). When the journal opens, `FUN_140341280` puts every owned piece not yet on a board into a
random empty slot of its puzzle. Grab, place, swap and rotate follow `FUN_140340b80` and `FUN_14033e650`; the puzzle is solved
when each slot holds its own piece at rotation 0. The banner then gives the reward (`FUN_1403c3f00`) and the board locks.

## Change

- `journal_table`: `read_puzzles`, `read_pieces`; `menu_screen::load_file` for the piece table in `kh2_first`.
- `pause_menu::journal::puzzles`: the states, `PuzzleSave` (the save's fields), `place_pieces`; `treasure.rs` gains the pieces.
- The slices are plain quads with the rotation in their corners, since a `Quad` has one UV box (`FUN_14033daa0`'s four tables).
- The piece bits now count in the camp's Journal NEW mark (#161).
- `picture::Picture::set_groups` for the banner's item picture groups 4 / 5.
- The host loads the pictures, hands the rewards to the stock, and keeps `PuzzleSave` for the session.
- The Debug Menu's Sora tab has "Give all puzzle pieces", since no room has piece objects yet.
- `--journal-tour` goes on through these pages.

## Checks

| Check | Result |
| --- | --- |
| Unit tests: placement (once only, not on the own place, rotations), a rotated slice's corners and UV, the OBTAINED line | PASS |
| Release `--journal-tour`: menu with NEW marks, list, first-time tutorial with scroll bar, board, grab, carry, place, solved banner with the item picture, solved board with the OBTAINED line, R2 to a rotating board and a 6 x 8 board, List of Pieces with 38/144 | PASS |
| The reward in the stock after the journal closes | NOT RUN (the log doesn't show it; it's three lines in the host) |
| Side by side with the installed game | NOT RUN |

## Uncertainty

- The board's cursor hand, group 115, has alpha 0 in every animation, so it draws nothing here. The exe sets `+0x108 |= 8` and
  `+0x1f1 = 1` on it, whose meaning is unread [U]; those flags could make it visible in the exe.
- Text sits about 15 units left of the spec's anchors on these pages, as on the other list pages; not explained or compared [U].
- The item description is laid out left-aligned as one block although its alignment is 2; not compared [U].
- Placement uses a CRT-style `rand` of its own, because the exe's shares its state with other callers: same algorithm, different
  draws [I].
- The completion tally (`FUN_140348e10`, its consumers are outside the journal) isn't built.

## Open

- Picking pieces up in the field (`FUN_14041ac80`, `FUN_14041ad30`): no room has the objects yet.
- `PuzzleSave` isn't written to a save file yet (#79).

## Verdict

PASS for the pages in the test room. Next: the piece objects, once rooms field objects by program.
