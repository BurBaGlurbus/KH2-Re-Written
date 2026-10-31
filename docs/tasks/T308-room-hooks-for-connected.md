# T308 — The room opens up: a library, hooks and extra state for Kingdom Hearts: Connected

Owner Claude, 2026-10-30, requested by burba in the project thread. Multiplayer and everything from KH3 move out of this repository into Kingdom Hearts: Connected. Gate: none of the roadmap's; single player must not change. Prerequisites: the enemy and KH3 threads stopped at 87dc536. Source: Steam KH2FM build as before, only read.

This is the engine half. The duel code still sits in this crate, on the new hooks, until the new project runs alone; a later record removes it.

## What changed

- `main` is a library (`kh2rw`, `src/lib.rs`) and a thin `KH2RW` binary. The room's items are public.
- `test_room/extension.rs`: `RoomHooks`, a table of plain functions (who drives the room Sora, its step, its casts, a replica's input, cameras, drawing, bullets, debug rows); `RoomExtension`, what a crate does while the room starts; `RoomSet`, the named places in the schedule. `RoomHooks::NONE` is the single-player game.
- `RoomPlugin` holds the room's schedule. The duel's systems left its `.chain()` and order themselves around `RoomSet`.
- `Game` keeps neutral fields (`versus`, `replica`, `local_slot`, `camp_request`, `menu_held`, `music_held`) and a slot, `ext`; `Fighter` has a slot of its own in place of its KH3 fields.
- `rival.rs` keeps what the engine needs of another Sora (his state, `swap`, his TARGET centre). Putting him in the room, his scripts and his drawing are `rival_play.rs`.
- 35 multiplayer fields of `Game` and the room's multiplayer options and start are in `test_room/online.rs`.

The state went into the extension slot, not into separate Bevy resources as the plan said: moving it to resources would have meant threading a parameter through every duel helper, and the slot keeps each field's borrowing the same as before.

## Checks

| Check | Verdict |
|---|---|
| `cargo fmt --all --check` | PASS |
| `cargo clippy --all-targets --locked -- -D warnings` | PASS |
| `cargo test --locked` | PASS |
| `cargo run -p main --locked -- --frames 180` | PASS |
| clippy and test with `--workspace --features main/skate3` | PASS |
| `tools\Build-Launcher.ps1 -Skate3` (what `build.bat` runs) | PASS |
| Unattended runs against a build of 87dc536: `auto`, `gameover`, `deck`, `magic` (6361 records), and Test KH3 | PASS: identical, by event kind and content, leaving out wall-clock records |
| `talk`, `partyhit` against the same build | PASS: differ only by the exit tick (260 vs 263, 6714 vs 6716) and two records it brings; 78aa465 against 87dc536 shows the same exit wobble |
| Loopback duels (KH2 and KH3 rulesets) | PASS as a smoke test: both ends finish a match with similar counts. Their timing depends on the network, so the logs cannot match exactly. |

Those logs are private and stay outside the repository. No real pad or keyboard was used and the runs do not cover every room.

## Verdict

PASS for this change: single player showed no difference in the runs above. No claim about the rest of the game, and no gameplay gate moves.

## Next

Kingdom Hearts: Connected takes the room code; this repository then removes it.
