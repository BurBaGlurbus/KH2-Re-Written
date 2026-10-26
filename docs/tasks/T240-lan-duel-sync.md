# T240 — LAN duel, phase 1: two instances in sync

Owner Claude, 2026-10-25/26, requested by burba in the project chat ("Build local LAN multiplayer ... a 1v1
duel mode for two players on the same Wi-Fi/LAN"; phase 0 proposed host-authoritative replication, the owner
chose "host-authoritative + renet"). Gate: none of the roadmap's (an addition outside the original game; no
fidelity claim). Prerequisites: T236 (fixed 60 Hz tick, frame-rate independence). Build identity: worktree
`KH2FinalRewrite-lan`, branch `feature/lan-duel` from local `main` at `a70238f`; package 0.1.2, Rust 1.99.0
Windows MSVC, dev profile. Source: the selected Steam KH2FM build, validated at each run; the install was only
read. The number T240 was taken to stay clear of IDs other sessions may use on `main` meanwhile.

Private evidence: `%USERPROFILE%\kh2-lan-out\` (run logs, captures, the comparison script `compare.py`, the
run scripts `runs.ps1`, `duo.ps1`, `leave.ps1`, `launcher-shot.ps1`, and the built executables of each step).

## What changed

1. `7acb447` Sora is a per-player `Fighter` (`test_room/fighter.rs`): HP, attacker and target stats, KO timer,
   level, guard invulnerability and hit record move out of the enemy runtime.
2. `0599dfc` A second Sora (`test_room/rival.rs`): the same state machine on the room's starting kit (shared
   through an `Arc`), its own map floor state, drawn with copies of Sora's meshes and the motion crossfade, a
   lock-on target (`TargetId::Rival`); `--rival-script` or F3 > Sora > Duel for a scripted one.
3. `1452f1b` Sora against Sora (`test_room/duel_hits.rs`): Keyblade windows against HIT spheres with the
   room's rules (ATKP rows, hit record and group lock, the damage formula with both sides player-type, Second
   Chance / Once More, reactions, KO), guard clashes; duel difficulty Standard (×1). A duel never pauses.
4. `3ab69fa` The `net` crate (renet 2.0.0; our own UDP transport, hello with name and executable SHA-256,
   one joiner, Bye, timeouts, link simulator, LAN discovery on the game port) and the room's session
   (`duel_net.rs`, `duel_sync.rs`, `duel_wire.rs`): the host simulates both Soras; the joiner sends buttons
   (last six ticks repeated) and shows snapshots two ticks behind; hits, sparks, guards and KOs as reliable
   events. Launcher Multiplayer screen: host, join by address, name (saved in `launcher.json`).
5. `e56afd1` The launcher lists the LAN's hosts by name; developer F12 screenshot (`KH2RW_LAUNCHER_SHOT`).
6. `2c5a308` Clean ends: opponent left, timed out, refused or never answered; the joiner gets its Sora back,
   the host waits for the next player.
7. `b4e0331` Overlay warnings (no buttons / no snapshot for half a second) and F3 link simulator presets.

## Checks

| Check | Result |
| --- | --- |
| `cargo fmt --all --check` | PASS |
| `cargo clippy --all-targets --locked -- -D warnings` | PASS |
| `cargo test --locked` (1,064 passed, 1 ignored; baseline 1,037) | PASS; `visual::frame_limit::tests::the_app_holds_the_cap` failed once under load and passed when rerun (timing test, code untouched) |
| `cargo build --locked`, smoke `cargo run -p main --locked -- --frames 180` | PASS |
| Single-player unattended runs against `a70238f` (talk, magic, party-hit, party-limit tours identical; autoplay, deck, summon, game-over and limit-form tours differ only in the exit tick or the threaded Drive-kit tick, which also differ between two runs of `a70238f`) | PASS |
| Scripted rival: hits both ways (3 damage each) | PASS |
| Two instances on one PC over loopback (unattended scripts on both): connect, the joiner's Sora follows the host's simulation, both drawn on both sides, hits and sparks | PASS |
| Same with the bad-link simulator on both (230 ms ping, ~10% loss) | PASS (playable; snapshot skips logged) |
| Disconnects: joiner quits (host: "left"), joiner killed (host: "timed out" after 5 s), host quits (joiner: "host left", its Sora local again) | PASS |
| Launcher Multiplayer screen with a loopback host listed by name | PASS (screenshot) |
| Two PCs on a real LAN, discovery by broadcast, Windows firewall prompt | NOT RUN (one PC here) |
| `cargo clippy --workspace --features main/skate3` | NOT RUN (no Skate 3 code changed) |
| Comparison with the installed game | NOT APPLICABLE (no original multiplayer) |

## Uncertainty

- The baseline's autoplay, party-limit and limit-form tours panic at a debug-build overflow in
  `game/enemy/src/script.rs:215` on `a70238f` too; comparisons cover the ticks before it.
- The unattended runs hold each instance's clock for captures, so two instances drift apart; the joiner's
  "skipped snapshots" warnings in those runs come from that, not from the link.
- The branch rename of `game.player` touches about 300 lines across the test room; `main` has moved on since
  `a70238f` (kit loading, HD textures), so merging needs the rename re-applied where they meet.

## Verdict / next

PASS for phase 1 on one PC: two instances host/join, sync, fight and end cleanly; two-PC LAN NOT RUN. Next
(phase 2, not started): the duel loadout, then the lobby, arena, countdown and match flow.
