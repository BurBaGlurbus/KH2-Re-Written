# T371 — The Gummi menu's mission select (2D)

Owner Claude, 2026-11-02, requested by burba in the project thread; the coordinator asked for the 2D screens as separate verified commits. Issue #142. Gate: development of the Gummi menu in the world map program; no final gate is inferred.
Prerequisites: T370. Build identity: local `main` after T370 plus this change, package 0.1.2. Source: the selected Steam KH2FM build, validated each run; the install was only read. Plain Ghidra 12.1.4 headless on a read-only project copy.

Private evidence (outside the checkout): `research\issues-20261102\gummi142\gummi-menu-layout.md` §1.4 and §3 with its `lay\` folder; run `gm3` with `gm3sheet.png` under `%USERPROFILE%\kh2-issues-out\g142\` (generated save `gummi.sav`: every route's levels unlocked, EX only on level 1).

## What the executable does

Start leads to mission select when level 1 has been cleared (FUN_14024ee90, FUN_14024e9e0, FUN_140277e30).
- **Shared items:** the layout plays "top" → "mission" (scenes 16, 17). The course banner moves (record 0x1c6), and the help window reads 0x1904.
- **Levels:** two levels, and a third once it's unlocked. The screen always opens at level 1.
- **Description panel** (records 0x10-0x12) shows the route table's text: 0x483a + level, or 0x4f8d + 3 × route + level for EX.
- **The level's record** shows its rank as one digit (styles 2 or 5), then its value:
  - level 1: medal value (0x156, style 1, two digits);
  - level 2: ships destroyed (0x158, style 3, seven digits);
  - level 3: score (0x159, style 0, ten digits).

  Then its treasure share (0x157, style 4, three digits) and "%" (0x1c0).
- **Digits:** the counters' digits come from `gumimenu.bar` `0005`'s fonts, right to left, leading zeros hidden.
- **Changing level** (repeat bits 0x80 and 0x40):
  - the panel slides with a transition record (0x14, 0x248, 0x249, 0x15, 0x16, 0x17, 0x152, 0x153);
  - its text changes 10 frames in;
  - the record values refresh;
  - input waits 36 or 24 frames for the carousel;
  - EX drops off on a level whose EX twin is locked.
- **EX** (bit 0x10000), when the EX twin is unlocked, shows the badge (0x262), switches the prompt (0x263: 0x5014 / 0x5015) and the description, and the values come from the EX record.
- **Decide** closes everything towards ship select with the level and EX. **Cancel** goes back to the top menu: "mission" → "top" (15, 35), the banner moves back (0x1c7), and the rows reopen.

## Changes

- `gummi::menu`: `level_count`, `ex_available`, `turn_frames`, `LEVEL_PANELS`, `level_slide`, `level_value`, `rank_digit`, `level_text`, mission pad bits and sounds.
- `data::gummi_table`: `DigitStyle`, `read_digit_styles` (`0005`).
- `visual::gummi_menu`:
  - `TopMenu` became `GummiMenu`, with the help window and banner kept from screen to screen;
  - digit counters on their item;
  - re-skins;
  - mission select.
- World map: the menu gets the route's records, and the digit fonts are loaded.

## Checks

| Check | Result |
| --- | --- |
| `cargo fmt --all --check`, `cargo clippy --all-targets --locked -- -D warnings`, `cargo test --locked` | PASS |
| Unit test `mission_select_rules`: level count, EX availability, turn frames, slides, values, rank digit, texts | PASS |
| `KH2RW_GUMMI_KEYS=450:4,540:80,640:80,720:80,760:10000` (`gm3`) | PASS: Start opens mission select (MISSIONS title, level 1 panel, rank A, MEDAL LV 0, TREASURES 42%, "Normal/EX Mission" prompt). Next shows level 2 (rank E, DESTROYED 0, 7%) and level 3 (rank S, SCORE 900, 100%, "Level 3 missions are high score runs…"). Next wraps to level 1. EX shows the badge, the EX condition "Fly a Gummi Ship with 50 or more speed!" and the EX record's values. The run's own cancel at tick 800 returned to the top menu |
| Decide towards ship select in a run | NOT RUN (unit-tested choice only) |
| Side by side with the installed game | NOT RUN |

## Gaps

- **Not drawn (3D):** the carousel's panels and the room. The turn's easing (FUN_140288090) isn't traced, so input just waits its frames.
- **Not built:**
  - the EX prompt's button glyph (FUN_1402ebe70(0x1000));
  - ship select, Teeny select and Last Check: decide ends the run as before.
- **Unknown:** which physical button is the EX bit. Run with `KH2RW_GUMMI_KEYS`.
- **Inferred:** a counter's start (x0 = slots × the digit-0 group's +0x1c) [I]. The rank letter overlaps the end of "COMPLETION RANK" in the captures, which suggests this start is off.
- **Silent:** the sounds, as in T370.

## Verdict

PASS in the world map program for mission select's 2D side, with the counter start uncertain.

## Next

#142: the result screen and after-clear menus (T363's rules), then ship select once its selectors are traced.
