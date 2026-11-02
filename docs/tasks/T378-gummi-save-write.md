# T378 — The Gummi result's save write

Owner Claude, 2026-11-02, requested by burba in the project thread (the Gummi mode in pieces). Issue #142. Gate: development of the Gummi result in the world map program; no final gate is inferred.
Prerequisites: T363, T374, T376. Build identity: local `main` after T377 plus this change, package 0.1.2. Source: the selected Steam KH2FM build, validated each run; the install was only read.

Private evidence (outside the checkout): `research\issues-20261102\gummi142\gummi-spec.md` §1.5 and §3.5, `gummi-result-layout.md` §2; run `gm10` under `%USERPROFILE%\kh2-issues-out\g142\`.

## What the executable does

At the result (FUN_140210730 state 4) the unlocks (FUN_140209b50) run on the save as it was. On the panel's decide, the record write (FUN_140209750) keeps the best of each value and sets cleared. It writes the level's treasure share (FUN_1402083d0: obtained bits at save +0x10f60 over all of the level's treasure.bin rows, normal and EX) to the record and to its other-kind twin.

## Changes

- `gummi::record`: `TREASURE_BITS`, `obtained`, `read_records`, `write_records` (a round-trip unit test).
- `GummiData::treasures` from `gumibattle/treasure.bin`.
- World map:
  - keeps the loaded save (`gummi_save`);
  - closing the finished result panel applies `unlock`, then `write_clear` with the share, to that save and to the image the field starts from (`FieldHandoff`), seals them and logs `gummi_save_write`.

## Checks

| Check | Result |
| --- | --- |
| `cargo fmt --all --check`, `cargo clippy --all-targets --locked -- -D warnings`, `cargo test --locked` | PASS |
| Unit test: all 54 records round-trip; treasure bits by word and bit, short images | PASS |
| Level 2 at rank 6 (`KH2RW_GUMMI_RESULT=800:12:44:6:1`, `gm10`), generated save with level 2 at score 500, rank 1 | PASS: the record now holds score 800, 44 destroyed, medals 12, rank 6 with rank-up, cleared; unlocks `ex: [false, true, false]` (level 2's EX); share 0 (no treasure bit is set in the generated save), written to both kinds |
| A save the installed game wrote | NOT RUN |
| Side by side with the installed game | NOT RUN |

## Gaps

- **Not built:**
  - the prizes, so no treasure bit or item changes here;
  - the achievements (FUN_140210540).
- **Not refreshed:** the world map's route panel keeps the records it loaded until the next start.
- **Not written to a file:** the save write reaches the field only through the existing hand-off when a landing follows.

## Verdict

PASS in the world map program for the record write and the unlocks.

## Next

#142: the prizes (treasure.bin slots, the HUD item window), the notices, the pause menu.
