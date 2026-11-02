# T363 — Gummi result rules and tables

Owner Claude, 2026-11-02, requested by burba in the project thread (the Gummi mode in pieces). Issue #142, piece 3 (the rules before any screen uses them). Gate: development of the Gummi rules; no final gate is inferred.
Prerequisites: T362. Build identity: local `main` after T362 plus this change, package 0.1.2. Source: the selected Steam KH2FM build; the install was only read. Plain Ghidra 12.1.4 headless on a read-only project copy.

Private evidence (outside the checkout): `research\issues-20261102\gummi142\gummi-spec.md` §1.4-1.6 and §3, decompiles in its `score\`, `ses\` and `res\` folders, asm checks in `asm\`; the extracted tables and the check's output `gcheck.txt` under `%USERPROFILE%\kh2-issues-out\g142\` (the check program sits in the session scratchpad, outside the checkout).

## What the executable does

- **Rank** (FUN_140207400): every frame before the goal, unless it's the route's first level 1 flight, the rank goes up one step when the ranked value reaches the current rank's threshold. The cap is 6 on levels 1 and 2 and 16 on level 3. The value is the medal value, ships destroyed or the score, by the row's kind.
- **Medals** (FUN_140207c30): every tenth pickup raises the value, with a sound by value. The first pickup makes it 1, and it stops at 30.
- **Score** (FUN_140207a50, FUN_140207a90): past 99,999,990 only the last digit is kept.
- **Prizes** (FUN_1402082e0, FUN_1402081e0): prize n is slot 6 − n, for n up to the rank (6 at most). A first flight gives only the first. Item 0x62 also gives 0x6a-0x6d.
- **Treasure share** (FUN_1402083d0): normal and EX treasures together.
- **Result, state 4** (FUN_140210730): the unlocks (FUN_140209b50) run before the record write (FUN_140209750), which happens on decide. Unlocks:
  - a first clear opens levels 1 and 2;
  - otherwise a normal flight at rank above 5 opens its EX;
  - levels 1 and 2 both above rank 2 open level 3, with the flight's rank standing in for its own level.
- **Record write**: the best of each value, cleared set, and the share written to the record and to its EX or normal twin.

## Changes

- New package `source/game/gummi` (`gummi`):
  - `record` holds the 54 records, moved from `world_map::gummi` with every field.
  - `rules` holds `max_rank`, `rank_value`, `rank_step`, `shown_threshold`, `Medals`, `add_score`, `bump_last_digit`, `first_flight`, `PRIZE_SLOTS`, `prize_slots`, `prize_items`, `table_level`, `treasure_share`, `unlock` and `write_clear`.
- `data::gummi_table`: the rank.bin and treasure.bin readers.
- `world_map::gummi` re-exports the records. `folder-guide.md` gains the package.

## Checks

| Check | Result |
| --- | --- |
| `cargo fmt --all --check`, `cargo clippy --all-targets --locked -- -D warnings`, `cargo test --locked`, `cargo build --locked` | PASS |
| Unit tests on generated values: rank steps and caps, the medal steps and sounds, score cap, prize order, share, first clear, EX and level 3 unlocks, record write | PASS (8 in `gummi`, 1 in `data`) |
| The installed tables through the readers (private check) | PASS: 27 rank rows, one per route and level, each with kind = level − 1, as many rising thresholds as the level's cap; 544 treasure rows with 544 distinct bits; level 1 has all seven prize slots, levels 2 and 3 slots 0-5, EX slot 0 only; the share of route 0 level 1 is 0 with nothing owned and 100 with everything |
| A flight using the rules | NOT RUN: no flight yet |
| Side by side with the installed game | NOT RUN |

## Uncertainty

- How the rank row is chosen (FUN_1402073e0) isn't read. The table has one row per route and level, so EX is taken to share the normal row [I].
- The medal pickup counter (+0x12) and the other u16 counters aren't traced beyond being kept at their best.
- The exe divides by the count of treasures with no zero check. `treasure_share` does the same and gives NaN for a route and level with no rows; the installed table has none like that.
- Level 2's and 3's lack of slot 6 means rank 0 brings them no prize. That follows from the table and the lookup, and hasn't been seen in play.

## Verdict

PASS for the rules and readers in isolation.

## Next

#142: the result panel and the after-clear menu over these rules (piece 4), or the top screen once `gummi_menu_lay`'s groups are read.
