# T341 — The Character Files' Slain counts and reaction rows, and what feeds them

Owner Claude, 2026-11-01, requested by burba in the project thread ("commit to local main and continue working on issues on guthub"). Issues
#69 and #79 (parts). Gate: development of the Journal in the test room; no final gate is inferred. Prerequisites: T093, T164, T323. Build
identity: local `main` after `150c951` plus this change, package 0.1.2. Source: the selected Steam KH2FM build, validated each run; the install
was only read. Plain Ghidra 12.1.4 headless on read-only project copies.

Private evidence (outside the checkout): `research\playable-room-20261008\journal-03\files-treasures-spec.md` §2.3, §3.4;
`research\issues-20261101\g\f2\`, `f3\`, `f5\`, `e5\` (FUN_1403a1520, FUN_1403a1470, FUN_1403a1430, FUN_1403db680, FUN_1403a8b10, FUN_1403aa370,
FUN_1403b6ca0); run `j6` under `%USERPROFILE%\kh2-issues-out\s161\`.

## What the executable does

- A defeated object adds 1 to its kill counter, capped at 9999 (FUN_1403db680 → FUN_1403a1520). The counter is a `u32` at save+0x3748 + 4·(key −
  1000) for keys 1000..1071 and save+0x38c8 + 4·(key − 3000) for 3000..3011, where the key is objentry +0x4c.
- A reaction command adds 1 to its use count, a `u16` at save+0x3948 + 2·(+0x26) (FUN_1403a8b10, FUN_1403aa370 → FUN_1403a1590).
- Both count only once story flag 0x2d0 is set (FUN_14039ac70).
- A `char` row with flag 0x800 is seen once its counter is not 0, and its page shows "Slain" and the number (G78, G149). The Heartless and
  Nobodies pages also list the object's `rcct` commands (G79, G149): the name once used, "?????" unused under show-all, nothing otherwise.

## Changes

- `state::kill_counter_at`, `COMMAND_USES`, `COUNTERS_FLAG`; `CampRes::kills` and `command_uses`, saved and loaded.
- `chest::count_journal`, fed by `Game::defeated_keys` and `command_presses`; log lines `kill_counted`, `command_counted`.
- `journal_table::read_rcct`, `Char::object`; `JournalSource::char_keys`, `key_commands`; `JournalData::kills`, `file_rows`.
- `files.rs` draws the rows; `char_available` counts the kills.
- `--journal-tour` takes `KH2RW_JOURNAL_KEYS=<tick>:<bits>,...` in place of its route.

## Checks

| Check | Result |
| --- | --- |
| `cargo clippy --all-targets --locked -- -D warnings`, `cargo test --locked` | PASS |
| Unit test: the counters' offsets | PASS |
| Release `--journal-tour` on a crafted image with the counters filled (`j6`) | PASS: Shadow reads Slain 101, Soldier Slain 100, with Cyclone 4 on its row; the rows are named once their counter is not 0 |
| A kill and a press raising the counters in a run | NOT RUN: no tour stands in a room with story flag 0x2d0 and a Journal open afterwards |
| Side by side with the installed game | NOT RUN |

## Uncertainty

- `apply_camp` returns early for an image with no game record (`max_hp == 0`), so a bare dump from a fresh room does not load the counters.
- The tour's detail pages show an empty portrait frame. That is not from this change and was not looked into.
- Which defeated objects the room reports: its Shadow and the battle actors' `defeat_reward`. Others do not count.

## Verdict

PASS for the page on a crafted save. The raising path is covered only by reading its code.

## Next

The portrait on the Heartless pages; the 3D model pages stay open.
