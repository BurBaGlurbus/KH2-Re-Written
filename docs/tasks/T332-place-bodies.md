# T332 — Rooms wear the place's bodies

Owner Claude, 2026-11-01, requested by burba in the project thread ("yes do that": worlds switch costumes on their own). Gate: development
of the field in the test room; no final gate is inferred. Prerequisites: T328 (`AreaSource::members`), T330, T331. Build identity:
local `main` after `3586d7c` plus this change, package 0.1.2. Source: the selected Steam KH2FM build, validated each run; the install was
only read. Ghidra 12.1.4 headless on a private read-only project copy (plain Ghidra, no MCP).

Private evidence (outside the checkout): `%USERPROFILE%\kh2-outfit-research` (`place3`, `start1`, `tt-*`, `lk-start` captures and logs,
`place.ps1`), decompiles in `%LOCALAPPDATA%\KH2FinalRewrite\research\outfit-20261031`.

## What the original does

FUN_140405190(0) is Sora's object and FUN_140405190(form) a form's, from the member objects FUN_1403e2eb0 lays out at DAT_142a252f0 for
the place: every `memt` row whose world, flags and room variant hold overwrites them (T328). FUN_1403c32e0 is the friends'. So a place
names Sora's body, his six forms' and the party's: `P_EX100_NM` and `P_EX100_NM_BTLF`, Donald `P_LK020`, Atlantica's `N_LM010_PLY`.
World 15's row names `WM_CURSOR`, which is the map's and no body of Sora's.

## Changes

- `outfit::enter_place`, called when a room is entered: Sora wears the place's object and each friend theirs.
- `drive_room::wear_place`: rebuilds the kit on it, and a form in play stays, rebuilt on its column of the same row.
  `start_on_place`: a room that starts in a costume builds its kit with the session's load, so it is on from the first tick.
- A body chosen in the debug menu lasts until the next room change, where the place's returns. [Rewrite rule, burba's coordinator's
  choice: the original has no such menu.]
- A revert now waits for base Sora's kit: a rebuild during a form had dropped it, and the revert failed with "form kit not loaded".
  `loaded` keeps another kit's finished load, so one form's rebuild no longer blocks a revert.
- Only character ids 1 and 14 count as Sora's bodies, which leaves out the World Map's cursor.
- `--place-tour`: warps to one room of each body, drives into Valor where the game has forms, reverts; last, a warp in a form.

## Checks

| Check | Result |
| --- | --- |
| `cargo fmt --all --check` | PASS |
| `cargo clippy --all-targets --locked -- -D warnings` | PASS |
| `cargo test --locked` | PASS: 1186 passed, 0 failed |
| `--place-tour` on the Steam build: warps to Halloween Town, Christmas Town, Space Paranoids, Timeless River, Pride Lands, Atlantica, Twilight Town | PASS: each body on, no failed kit, no failed member rebuild, no panic |
| Sora's and the party's bodies and HUD faces after each warp, by eye | PASS |
| Drive into Valor and revert in the four costume worlds and Twilight Town | PASS: form changes both ways in the log, world's Valor by eye |
| Warp in a form: the form stays on the new body, and reverts | PASS |
| A room started in Halloween Town and in Pride Lands: the body from tick 0 | PASS |
| Twilight Town's story rows: flag 0x8db gives `P_EX110`, 0x8ce `P_EX110_BTLF`, 0x8d0 `P_EX120` | PASS by the log; by eye in `starts.png` |
| A debug pick lasting to the next room change | NOT RUN: `wear_place` compares and replaces it; no run picked then changed room |
| A real door or a landing between worlds | NOT RUN here: they start the room, the case the Halloween Town and Pride Lands starts cover |
| Controlled comparison with the original game | NOT RUN |

## Uncertainty

- The party that fights is fixed when the room starts (T328). A warp changes Donald's and Goofy's bodies but does not bring in or send
  away guests such as Beast or Simba.
- Pride Lands and Atlantica use their forms' default row columns if a form is forced; the game's own rule against forms there is not
  ported, and the tour does not drive there.
- A kit loads in the background on a warp: `outfit_on` came 120 to 190 ticks after the warp began. Whether the old body shows for a moment
  after the fade lifts was not measured; a door or a landing starts the room and has no such gap.
- The camp's own models keep their starting bodies (T331), and its Sora does not walk while a body is worn.

## Verdict

PASS for the field development; no gate moves. Whole-game equivalence is not claimed.

## Next

The camp's models could follow the place's bodies; the rule against forms in Pride Lands and Atlantica could be traced.
