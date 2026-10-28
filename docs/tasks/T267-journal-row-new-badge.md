# T267 — The camp's Journal row NEW! badge

Owner Claude, 2026-10-27, requested by burba in the project thread ("work on some github issues"). Issue #161, first
item. Gate: development of the camp's top level; no final gate is inferred. Prerequisites: T255 (camp top-level
badges), T164 / journal pages (their seen marks). Build identity: local `main` after `820f55d`, package 0.1.2. Source:
the selected Steam KH2FM build, validated each run; the install was only read.

Private evidence (outside the checkout): decompiles `research\ghidra-kh2-a3\camp-04\` (FUN_140345420, FUN_140346b30),
`research\ghidra-kh2-a1\camp-02\` (FUN_140305dc0, FUN_140304ea0, FUN_140306790), `research\ghidra-kh2-a1\jm-03\`
(FUN_1403489e0, FUN_140348400, FUN_140346950, FUN_1403479d0, FUN_140346ed0, FUN_1403450d0, FUN_140345210);
journal-01 spec §3.6, camp-01 spec §3.3. Runs in `C:\Users\ANIMA\kh2-issues-out\` (`i161-badge2`, `i161-journal2`).

## What the executable does

- The camp task (FUN_140305dc0) sets DAT_140beec27 = FUN_140346b30() each time it starts; FUN_140304ea0 (the camp
  rebuilt after the journal) clears it and starts that task again, so it is recomputed on every camp opening.
- FUN_140346b30 runs FUN_140345420 (each journal table's "available" bits minus the save's seen bits) and answers
  whether any bit is set: characters, Ansem reports (13 bits), limits, missions (status not 0 or 3), minigames (record
  state above 1), treasures, map items, the four Synthesis Notes lines, puzzle pieces, and the album and story bits of
  journal worlds 0..0xe.
- FUN_140306790 gives entry 5 (Jiminy's Journal) group 50 (the pulsing NEW!) when the flag is set; it never uses the
  dim group 49.

## Changes

- `JournalData::any_new` (visual, journal): the predicate over the categories the rewrite keeps. `flag` and
  `char_available` moved onto `JournalData`; the journal's own methods call them.
- The Ansem reports' seen rows (save+0x4d74) are now kept between openings (`JournalData::seen_reports`,
  `Journal::seen_reports`, `CampRes::seen_reports`); before, every owned report showed NEW at each opening.
- `pause_menu::Data::journal_new`, filled by `camp_data` from the journal's data at every camp opening; entry 5 shows
  the pulsing badge.
- `--journal-tour` captures the camp's top level first (tick 130).
- Unit test `the_camp_badge_asks_for_an_available_unseen_entry`.

## Checks

| Check | Result |
| --- | --- |
| Release `--badge-tour` (`i161-badge2`): the room's journal has nothing available | PASS: Items and Abilities rows badged, Journal row not |
| Release `--journal-tour` (`i161-journal2`, show-all flag and tour extras) capture 0 | PASS: the Journal row shows the pulsing NEW! |
| New unit test | PASS |
| `cargo fmt --all --check`, `cargo clippy --all-targets --locked -- -D warnings`, `cargo test --locked`, `cargo build --locked`, smoke `--frames 180` | PASS |
| Side by side with the installed game | NOT RUN |

## Uncertainty

- Treasure, map item and puzzle piece bits are not part of the predicate: the rewrite's Treasures and Maps page does
  not keep per-entry seen bits (save+0x4d24 / +0x4d64; it marks whole worlds viewed for the session), and puzzle
  pieces are not built.
- The badge disappearing after every entry is seen is checked only by the unit test; no tour sees every entry.

## Verdict

PARTIAL for issue #161: the Journal row badge follows FUN_140346b30 for every category the rewrite keeps; the forms'
ability lists and the Status form and Summon rows remain.

## Next

Treasure and map seen bits on the Treasures page (FUN_1403377c0); then the rest of issue #161.
