# T374 — The Gummi result panel (banner and both columns)

Owner Claude, 2026-11-02, requested by burba in the project thread; the coordinator asked for the result screen as its own verified commit. Issue #142. Gate: development of the Gummi result panel in the world map program; no final gate is inferred.
Prerequisites: T363, T371, T373. Build identity: local `main` after T373 plus this change, package 0.1.2. Source: the selected Steam KH2FM build, validated each run; the install was only read. Plain Ghidra 12.1.4 headless on a read-only project copy.

Private evidence (outside the checkout): `research\issues-20261102\gummi142\gummi-result-layout.md` §1.1, §3 with its `res2\` folder; runs `gm5` to `gm7` with their sheets under `%USERPROFILE%\kh2-issues-out\g142\` (generated save `gummi.sav`).

## What the executable does

gb::MENU_RESULT (FUN_1402378a0) draws on `gumibattle/sprite/<lang>/sprite.bar`, a 2LD whose SEQD k uses image k. Each sprite plays a group's appear, then its loop.

- **Banner:** SEQD 4 group 2 "ROUTE OPENED" on a first level 1 clear, 1 "MISSION FAILED" when no rank was reached, else 0 "MISSION COMPLETE". Each has its own sound, and the banner holds 210 frames.
  - A saved record whose +0x1c is set skips the banner (asm 0x140237917).
- **Left column** (85 left; not on a first clear) shows the saved record:
  - the route name panel, the EX badge, the level plate;
  - RECORD, and COMPLETION RANK with the rank letter;
  - the level's value with its digits (medals, destroyed or score, by rank.bin kind).
- **Right column** (85 right) shows this run:
  - **Rows:** MEDAL LV at frame 0, DESTROYED at 18, SCORE at 36. Medal LV comes with its emblem (SEQD 0, group 3v + 1) and two digits. DESTROYED and SCORE reveal a digit every 8 frames (the label's +0x20) behind rolling placeholders, with leading zeros blank.
  - **Rank** at 60, on SEQD 6, with a number over S for ranks 7 to 16.
  - **New-record marks:** 0x7c to 0x7e once a row is shown, 0x7f for the rank at 300.
- Decide shows everything at once (timer 1000).

## Changes

- `visual::gummi_result`: `ResultPanel`, `ResultInput`, digit rows and reveal, banner, both columns, marks, sounds.
- World map:
  - `ui.rs` loads `sprite.bar`;
  - `world_map.rs` uploads its images and draws the panel over black.
- Run aids:
  - `KH2RW_GUMMI_RESULT=<score>:<medals>:<destroyed>:<rank>[:<rank up>]` opens the panel after mission select's decide, with those numbers, since no flight is built.
  - `KH2RW_GUMMI_KEYS` now reaches the panel too.

## Checks

| Check | Result |
| --- | --- |
| `cargo fmt --all --check`, `cargo clippy --all-targets --locked -- -D warnings`, `cargo test --locked` | PASS |
| Unit test `digits_most_significant_first` | PASS |
| `KH2RW_GUMMI_RESULT=123450:18:37:4:1` (`gm5`, `gm6`) | PASS: "MISSION COMPLETE"; the left column shows Asteroid Sweep, RECORD, rank C and medal 00 from the save; the right column shows MEDAL LV 18 with emblem, DESTROYED 0037 and SCORE 0123450 revealing, each with NEW RECORD!!; rank B zooms in under COMPLETION RANK; done at tick 1052 (columns + 300) |
| `KH2RW_GUMMI_RESULT=9999:5:120:7:0` with decide at 800 and 940 (`gm7`) | PASS: "MISSION FAILED"; decide fills everything at once (done at 800); the next decide returns to the map at 940 |
| Side by side with the installed game | NOT RUN |

## Gaps

- **Not built:**
  - the unlock notices (advice.bin, the notice window);
  - the prizes and the HUD item window;
  - the Treasure List prompt and panel, and "COMPLETION BONUS!";
  - the save write;
  - the after-clear menu (0x35, `gm_ps.2ld`): decide on the finished panel returns to the map.
- **Inferred:**
  - A digit row follows its label's current group, so it can drift slightly during the label's appear [I].
  - The left-column parts are taken to be on SEQD 7 like the right [I].
- **Unknown:** whether the +0x1c banner skip is meant to hide the banner after a first ranked clear [U].
- **No flight:** the run's numbers come from the test aid. No flight produces them.

## Verdict

PASS in the world map program for the banner and both columns.

## Next

#142: the after-clear and game-over menus on `gm_ps.2ld`, then the save write.
