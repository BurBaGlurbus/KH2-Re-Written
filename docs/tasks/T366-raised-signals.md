# T366 — Raised signals reach the area's entries

Owner Claude, 2026-11-02, requested by burba in the project thread ("work through issues and close them"). Issue #25 (part). Gate: development of the story in the test room; no final gate is inferred.
Prerequisites: T318, T344, T364. Build identity: local `main` after T365 plus this change, package 0.1.2. Source: the selected Steam KH2FM build, validated each run; the install was only read.

Private evidence (outside the checkout): `research\issues-20261101\events\events-spec.md` §1.3, `jumps_tt.txt`; configuration `test-room-tt07-95.json`; runs `sig1` to `sig3` under `%USERPROFILE%\kh2-issues-out\s162\`.

## What the executable does

FUN_1403aabe0(signal, arg), the one way every script raises a signal, does these steps in order:
1. drops the signal's reservations;
2. sends a fixed set of codes to the game-mode or menu handlers and returns;
3. gives the signal to the mission's trigger 10 and to the objects;
4. looks for the area's entry (FUN_1403f60d0) and fires it.

In Twilight Town, missions' 37 and 38 answer such entries: 37 replays the program, and 38 sets the step's flag and plays its event before the next program. For example, tt07 program 95 has 0x81b and event 104, then program 97.

## Changes

- `boss::mission_tick`: with the story switch on, the signals the mission had go on to `Game::story_signals`, and `story_events::tick` offers them to the area's entries, except step 2's codes.
- `boss::mission_tick`: the mission's own 1:41 calls (`ScriptBrain::signals`) were dropped. They now join `mission_signals` and reach trigger 10 on the next step.
- `KH2RW_TOUR_SIGNAL=<tick>:<code>[:<arg>]` raises a signal for runs.

## Checks

| Check | Result |
| --- | --- |
| `cargo fmt --all --check`, `cargo clippy --all-targets --locked -- -D warnings`, `cargo test --locked` | PASS |
| tt07 program 95, mission TT07_MS101, signal 38 at tick 200 (`sig1`) | PASS: `mission_signal` 38, then `story_signal` 38 with flag 0x81b, event `tt:7:104` and the jump to program 97 |
| Signal 37 (`sig2`) | PASS: no flag, no event, tt07 program 95 again |
| Switch off (`sig3`) | PASS: only the mission had it |
| A mission's own 1:41 call reaching the entries in a run | NOT RUN: no unattended run finishes TT07_MS101 |
| Side by side with the installed game | NOT RUN |

## Uncertainty

- In the exe, a mission's own signal reaches its trigger 10 at once. Here it reaches it on the next step.
- An area reloaded in place (37's retry) doesn't raise signal 0 again here, because the arrival is remembered per world and area. The exe raises it on every load.
- Step 1 (reservations) runs only for the boss's reserved signals, as before.

## Verdict

PASS in the room for raised signals reaching the entries, behind the story switch.

## Next

#25: the player's skip (FUN_1403a55b0), events with no jump (0x4c), and the arrival on a reload in place.
