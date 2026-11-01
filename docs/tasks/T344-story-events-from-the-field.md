# T344 — An area's story events start from the field (first step, behind a switch)

Owner Claude, 2026-11-01, requested by burba in the project thread ("commit to local main and continue working on issues on guthub"). Issue #25.
Gate: development of the story in the test room; no final gate is inferred. Prerequisites: T323, T324, T328. Build identity: local `main` after
`1b6929d` plus this change, package 0.1.2. Source: the selected Steam KH2FM build, validated each run; the install was only read. Plain Ghidra
12.1.4 headless on read-only project copies.

Private evidence (outside the checkout): `research\issues-20261101\events\events-spec.md` with its decompiles and data dumps; runs `ev1` to `ev5`
under `%USERPROFILE%\kh2-issues-out\s161\`; private configurations `test-room-tt07-106.json` and `test-room-tt07-95.json`.

## What the executable does

- An area's programs declare event entries (op 0x0c). Signal 0 is raised once when an area starts (FUN_140152cd0 → FUN_1401561b0), and 267 of
  Twilight Town's 529 entries use it.
- FUN_1403f60d0 picks the entry: signal and arg equal wins at once, else the last open one. FUN_1403f61a0 fires it: flag and flag script, menu flag,
  used up. That all happens when it fires, before the event.
- An event writes no story state. When it ends, FUN_1403a46e0 applies the entry's jump.
- A story chain is a run of such hops. In Twilight Town: tt07 program 106 → flag 0x81a, event 103 → program 95, where the player has control.

## Changes

- `area::script::find_entry`, `AreaSource::entry_jump`, `AreaSource::fire_entry`.
- `test_room::story_events`, on with `KH2RW_STORY_EVENTS=1`: once per arrival it applies op 0x19's flags and raises signal 0. An entry with an event
  writes the handoff image at the jump's place and starts the event program; an entry without one jumps as a gate does.
- `kh2fm event --then-room <config>`: when the event ends, not when the window is closed, it starts the room on the handoff image.

## Checks

| Check | Result |
| --- | --- |
| `cargo clippy --all-targets --locked -- -D warnings`, `cargo test --locked` | PASS |
| Unit test: the entry a signal picks | PASS |
| Release run at tt07 program 106, switch on (`ev1`) | PASS: `story_signal` flag 0x81a, event `103`, jump to tt07 programs [95, 95, 95]; the image is dumped at that place |
| Event 103 by itself (`ev2`) | PASS: Roxas and the others play to the fade |
| The room at program 95 on that image (`ev3`) | PASS for loading: the mission TT07_MS101 starts. FAIL for the mission: native 1:296 `trap_tutorial_open` has no translation, so its script stops each tick (x300) |
| `event --then-room` on the real handoff path (`ev5`) | PASS: event 103 played to its end, the event program exited and a room started on the image, first frame logged |
| The whole chain with a person, window to window | NOT RUN |
| Side by side with the installed game | NOT RUN |

## Uncertainty

- Off by default: a normal run plays no events, as before. On, every area's signal-0 entry fires, and many are built for story points the room
  does not reach in order.
- Left out: other signals (doors, missions' 37 and 38, talk), a player's skip (FUN_1403a55b0), events with no jump, events over a running field
  (`evtp` rows 11-15, 17), movie op 0x44 (event 100 plays nothing for its length), SeqDialog 0x2d. Jump kinds 3, 5 and 6 and fade flags are not read.
- `party_reset` (op 0x1d) and sub-ops 4, 5 and 7 are not applied.
- The rest of FUN_14039afc0's tail is not ported.

## Verdict

PARTIAL. The first hop of the chain runs in parts, unattended. #25 stays open.

## Next

Translate `trap_tutorial_open`, then the mission's signals 37 and 38 on program 95; doors' signal 1.
