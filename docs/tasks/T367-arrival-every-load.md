# T367 — Signal 0 on every load

Owner Claude, 2026-11-02, requested by burba in the project thread ("work through issues and close them"). Issue #25 (part). Gate: development of the story in the test room; no final gate is inferred.
Prerequisites: T366. Build identity: local `main` after T366 plus this change, package 0.1.2. Source: the selected Steam KH2FM build, validated each run; the install was only read.

Private evidence (outside the checkout): `research\issues-20261101\events\events-spec.md` §1.4; configuration `test-room-tt07-95.json`; runs `sig4` to `sig6` under `%USERPROFILE%\kh2-issues-out\s162\`.

## What the executable does

Every area load raises signal 0 once the area is up (FUN_140152680 → FUN_140152cd0 → FUN_1401561b0 → FUN_1403aabe0(0, 0)). A reload of the same area, such as a mission's retry jump (signal 37), counts as a load too.

## Changes

- `AreaRes::loads` counts the areas made current.
- `story_events::tick` keys its arrival on that count, not on the world and area, and logs `story_arrival`.

## Checks

| Check | Result |
| --- | --- |
| `cargo fmt --all --check`, `cargo clippy --all-targets --locked -- -D warnings`, `cargo test --locked` | PASS |
| tt07 program 95, signal 37 at tick 200 (`sig6`) | PASS: `story_arrival` at load 0, the retry's jump, tt07 entered again and `story_arrival` at load 1 (tick 308) |
| tt07 program 106 arrival (`sig5`) | PASS: event 103 as before |
| An entry firing on a reload in place | NOT RUN: program 95 has no signal-0 entry |

## Uncertainty

- A load that ends before the arrival tick is seen (a second jump in the same step) would be counted once.

## Verdict

PASS in the room.

## Next

#25: the player's skip (FUN_1403a55b0), events with no jump (0x4c).
