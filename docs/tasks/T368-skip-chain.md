# T368 — The skip chain after Skip Scene

Owner Claude, 2026-11-02, requested by burba in the project thread ("work through issues and close them"). Issue #25 (part). Gate: development of the story in the test room; no final gate is inferred.
Prerequisites: T271, T344, T367. Build identity: local `main` after T367 plus this change, package 0.1.2. Source: the selected Steam KH2FM build, validated each run; the install was only read.

Private evidence (outside the checkout): `research\issues-20261101\events\events-spec.md` §1.4, `jumps_tt.txt`; configuration `test-room-tt00-51.json`; run `skip1` under `%USERPROFILE%\kh2-issues-out\s162\`.

## What the executable does

Skip Scene sets `DAT_142a104a8` (FUN_1402cfcd0 → FUN_1403a5640). While it's set, each area load (FUN_140152680 with FUN_1403a55b0) checks the signal-0 entry. If the entry's event mode has `mode & 0x3f` of 2 or 3, no 0x40 or 0x80 bit, and no mission is running, the entry fires with no event and no field, and its jump is taken at once. The chain stops at the first entry that fails the check, which then plays as usual. The field's start clears the flag.

## Changes

- `cutscene`: Skip Scene marks the event, and the room it hands over to gets `--event-skipped`.
- `Game::event_skipped`, from `--event-skipped`.
- `story_events::tick` lets an arrival pass through a skippable entry without its event (`run_entry`'s `skip`), and clears the flag at the first arrival that isn't skippable. `story_signal` records say `skipped`.

## Checks

| Check | Result |
| --- | --- |
| `cargo fmt --all --check`, `cargo clippy --all-targets --locked -- -D warnings`, `cargo test --locked` | PASS |
| tt00 program 51 with `--event-skipped` and the switch on (`skip1`) | PASS: six arrivals skipped in turn, setting their flags with no event: tt00 → tt04 program 83 → tt02 program 67 → tt06 program 168 → tt02 program 167 → tt21 program 142 → tt01 program 54. The chain stopped there at event 400 (mode 66, bit 0x40), which plays |
| Skip Scene in the event program handing `--event-skipped` on | NOT RUN: the unattended pause pick chooses Continue |
| Side by side with the installed game | NOT RUN |

## Uncertainty

- "No mission running" is read as the area declaring none (op 0x15), because the room loads its mission after the arrival [I].
- The exe's chain loads no field between its steps. Here each step is a short room load.
- `0x1403a54a0`'s same path for world 0x10 with signal 7 isn't built.

## Verdict

PASS in the room for the chain, behind the story switch.

## Next

#25: events with no jump (signal 0x4c) and events over a running field.
