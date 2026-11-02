# T348 — The Character Files' model pages; the NEW labels' place settled

Owner Claude, 2026-11-01, requested by burba in the project thread ("work through issues and close them"). Issue #69. Gate: development of the Journal; no
final gate is inferred. Prerequisites: T164, T297, T341, T346, T340. Build identity: local `main` after `3ebfcd2` plus this change, package 0.1.2. Source: the
selected Steam KH2FM build, validated each run; the install was only read. Plain Ghidra 12.1.4 headless on a read-only project copy.

Private evidence (outside the checkout): `research\issues-20261102\journal69-spec.md`; runs `m1`, `jd`, `cen`, `r7`, `r54`, `r61`, `r69`, `r181`, `r311`, `r326`
under `%USERPROFILE%\kh2-issues-out\s162\`.

## What the executable does

- TRIANGLE on a detail page (0x10 / 0x13 / 0x16) goes to the full page (0x11 / 0x14 / 0x17, FUN_14032fe70). The detail page stays behind a full-screen backdrop
  (G4 → G5, G6 out). Cancel returns: 0x11 to 0x10, and 0x14 and 0x17 to 0x16.
- The object loads with `<set>_MEMO.mset`, or `<model>_MEMO.mset` when its entry names no set (FUN_1403e0bd0). Its slot is the motion's variant when bit 0 of the
  set's header +0xc is set, else the motion id itself (FUN_1403e6230). No effect file, no sub-objects, no weapons.
- The `char` row holds the motion, each page's place, scale and the small page's turn, and the hidden parts (FUN_14032ee20). Lumiere's scale is a depth. The
  rows "No. 1" to "No. 13" start a TEXA number.
- x and y shift the projection, not the paste. The paste's tint is 1. Its alpha comes from an invisible frame sprite (G7 → G8, G9 out), which starts after the load.
- Held LEFT and RIGHT turn the model 2 degrees a frame with no sound, and MCP stops at ±16. They are read before cancel, so cancel does nothing while one is held.
- NEW label: its only position is the parent's context plus T(0,13), and alignment doesn't move a child. So it sits over the value text's end, as the rewrite
  already drew it. The spec's 450 for Minigames was wrong: the value's right edge is 462.

## Changes

- `journal_table::Char`: motion, places, scales, `hidden_parts`. `motion::variant_slot`, `read_memo_motion`.
- `journal::files`: `In::Triangle`, `files_model`, `enter_model`, `request_model`, `model_view`, `model_draw`.
- `test_room::journal_model`: the load off the main thread, a target and camera made when a page first asks, the pose worked out each frame, parts hidden, the TEXA
  number. The world map's camp uses the same code.
- A model whose file holds levels loads its first, with the originals' images (`scenery::level_payload` and `level_bank` are public now). The `_GM` rows need it.
- `model_inspection_textures`: up to 256 images.
- Hooks, off unless set: `KH2RW_JOURNAL_CAPTURES`, `KH2RW_JOURNAL_MODEL_ROW`, `KH2RW_JOURNAL_MODEL_CENSUS`.

## Checks

| Check | Result |
| --- | --- |
| `cargo fmt --all --check`, `cargo clippy --all-targets --locked -- -D warnings`, `cargo test --locked` | PASS |
| Unit tests: row fields, hidden parts, slot rule, set names, clock, turn, paste centre | PASS |
| Census: all 338 `char` rows through the page's own loader (`cen`) | PASS: bad 0 |
| Release `--journal-tour` on the Soldier (`m1`) | PASS: small model on the detail page; TRIANGLE fades it and the help; full page on the backdrop; LEFT turns its front to screen left; cancel with LEFT held does nothing; cancel fades back and the small model and help return |
| Full and small pages on rows 69 (No. 1, scale 2.4), 311 (Jack, hidden parts), 181 (Lumiere), 326 (MCP), 54 (Reckless, motion 0), 7 (Trick Ghost, levels) | PASS by eye: all draw, none is cut at the target's edge |
| Row 61 (Possessor) | Loads one part and shows nothing on the page. Not known whether the installed game shows more |
| Default `--journal-tour` (`jd`) | PASS: Limits and Minigames' NEW labels sit over the values' ends as the rule says |
| The world map's camp | NOT RUN: same code, no tour reaches this page there |
| Side by side with the installed game | NOT RUN |

## Uncertainty

- [U] Lighting and fog while the journal runs: the model is unlit, as the camp's is.
- [I] A hidden-part bit counts the model's mesh parts in order. Jack draws without a visible error; no row was compared.
- The executable reloads the object each way, so its model shows later than here, where the same object is kept.
- A level container reads the original images, not the remaster's. Trick Ghost's look may differ from the installed game's.
- The first page allocates a 2560x1440 target that stays until the program ends, about 15 MB.
- The screen copy under the paste (+0x1270) is left out. Translucent edges could differ.
- `sprite_player` draws these rows' text in the system font where the executable uses font 3, so text widths, and how much a NEW label covers, can differ. Not changed.

## Verdict

PASS for #69's remaining items in the room. No claim about the installed game.

## Next

The rest of the batch.
