# T376 — The Gummi after-clear and game-over menus

Owner Claude, 2026-11-02, requested by burba in the project thread (the Gummi mode in pieces). Issue #142. Gate: development of the Gummi system menus in the world map program; no final gate is inferred.
Prerequisites: T374. Build identity: local `main` after T375 plus this change, package 0.1.2. Source: the selected Steam KH2FM build, validated each run; the install was only read. Plain Ghidra 12.1.4 headless on a read-only project copy.

Private evidence (outside the checkout): `research\issues-20261102\gummi142\gummi-result-layout.md` §2 and §4 with its `res2\` folder; runs `gm8` and `gm9` with their sheets under `%USERPROFILE%\kh2-issues-out\g142\`.

## What the executable does

After the result panel the clear fades to black, and battle phase 0x12 opens system menu 0x35, unless this was the route's first level 1 clear, which returns straight to the map. A game over opens 0x34. Both draw on `file/<lang>/gm_ps.2ld` (menu file 5) with the title's list framework (FUN_1402f7720, FUN_1402f9440):
- a black backdrop: SEQD 1 group 0;
- three rows, idle groups 0x18-0x1a and selected 0x0b-0x0d, with their texts at the group anchors: 0x4abc-0x4abe (Retry, Gummi Menu, World Map) or 0x37d1-0x37d3 (Continue, Gummi Menu, World Map);
- moves that wrap on a fresh press, and no cancel.

Decide writes 1, 2 or 3 (FUN_1402f0a20). 1 retries the flight, 2 returns to the Gummi menu, 3 to the world map. The sounds are system requests open 0xb, cursor 1 and decide 2 (se000 #3, #1, #2).

## Changes

- `visual::gummi_list`: `ListMenu` with `Kind::{AfterClear, GameOver}`.
- World map:
  - loads `gm_ps.2ld` and uploads its image;
  - after the result panel, opens 0x35, except on a first clear;
  - result 2 reopens the Gummi menu's top screen, and 3 returns to the map.

## Checks

| Check | Result |
| --- | --- |
| `cargo fmt --all --check`, `cargo clippy --all-targets --locked -- -D warnings`, `cargo test --locked` | PASS |
| Unit test `row_texts` | PASS |
| `KH2RW_GUMMI_RESULT=5000:9:3:2:1`, keys to decide through the panel, then Down and decide (`gm8`) | PASS: the menu opens at tick 790 with sound 3; Down at 840 plays 1; Gummi Menu at 880 plays 2 and reopens the top screen |
| The same with Down twice and decide (`gm9`) | PASS: black backdrop, the rows with the glove on the red selected row, cursor Retry → Gummi Menu → World Map; World Map at 970 returns to the map |
| The game-over menu in a run | NOT RUN: there is no flight to lose (rows are unit-tested) |
| Side by side with the installed game | NOT RUN |

## Gaps

- **Not built:**
  - Retry: it needs the flight, so it returns to the map;
  - the fade to black between the panel and the menu;
  - the list's close animations (FUN_1402f6550's flags aren't traced);
  - the pause menu (0x33).
- **Not reached:** the world map's re-entry at entrance 13 + route, as in T370.

## Verdict

PASS in the world map program for the after-clear menu.

## Next

#142: the save write on decide (T363's `write_clear` and `unlock`), the pause menu, then the flight itself once its readers are traced.
