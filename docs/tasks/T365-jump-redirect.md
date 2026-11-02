# T365 — The jump redirect to program 0x7a

Owner Claude, 2026-11-02, requested by burba in the project thread ("work through issues and close them"). Issue #25 (part). Gate: development of the story in the test room; no final gate is inferred.
Prerequisites: T364. Build identity: local `main` after T364 plus this change, package 0.1.2. Source: the selected Steam KH2FM build, validated each run; the install was only read. Plain Ghidra 12.1.4 headless on a read-only project copy.

Private evidence (outside the checkout): `research\issues-20261101\events\g\d1\1403a46e0_FUN_1403a46e0.c`, `jumps_tt.txt`; configuration `test-room-tt14-18r.json` (tt14 program 18 with flags 0x86e-0x871 set); runs `door5` and `door6` under `%USERPROFILE%\kh2-issues-out\s162\`.

## What the executable does

FUN_1403a46e0, which carries out an entry's jump, checks before a jump to a place (kinds 1, 2 and 3, and kind 4's special targets) whether flag 0x872 is clear and 0x86e, 0x86f, 0x870 and 0x871 are all set. If so, the target is replaced by FUN_14039fbc0(0x7a, 2): world 2's program 0x7a, at entrance 0. In tt that program's arrival entry (tt10, event 506) sets 0x872, which ends the redirect. The jump is carried out when the event ends (FUN_1403ac750 case 9), after the entry's flag was set.

## Changes

- `AreaSource::redirect`, applied by `signal_jump` and `entry_jump` (gates and story entries) and by the door's kind 3.
- `story_events::run_entry` reads the jump again after firing the entry, so an entry's own flag counts.

## Checks

| Check | Result |
| --- | --- |
| `cargo fmt --all --check`, `cargo clippy --all-targets --locked -- -D warnings`, `cargo test --locked` | PASS |
| Unit test: the redirect's target from a generated program table, and none without world 2's 0x7a | PASS |
| tt14 program 18 with 0x86e-0x871 set, switch on (`door6`) | PASS: the door's jump to tt03 became tt10 programs 122 (0x7a). tt10's arrival fired flag 0x872 and event 506, whose own jump (tt10 entrance 50) wasn't redirected |
| `door5`, before the re-read | FAIL, fixed: event 506's jump was also redirected, because it was read before the flag |
| Kind 4's world-map cases (flags 0x1071, 0x1072, 0x140f, 0x1822, 0x200b, 0x8cd, 0x3834, 0x1c1c) | NOT RUN: not built |
| Side by side with the installed game | NOT RUN |

## Uncertainty

- Kind 4 still always opens the world map. Its special targets, which the redirect also applies to, aren't built.
- The arrival in tt10 ran map program 0, as the room's programs table gives for 122.

## Verdict

PASS in the room for the redirect.

## Next

#25: mission signals 37 and 38, the player's skip, events with no jump.
