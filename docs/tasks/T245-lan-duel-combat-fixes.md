# T245 — LAN duel: combos that connect, magic checked spell by spell

Owner Claude, 2026-10-26, requested by burba after testing the duel ("comboing doesn't work on another player
because they have normal iframes when getting hit so its only 1 hit basically and magic doesn't work at all").
Gate: none of the roadmap's (an addition outside the original game; no fidelity claim). Prerequisites: T244.
Build identity: worktree `KH2FinalRewrite-lan`, branch `feature/lan-duel`; package 0.1.2, Rust 1.99.0 Windows
MSVC, dev profile. Source: the selected Steam KH2FM build, validated at each run; the install was only read.

Private evidence: `C:\Users\ANIMA\kh2-lan-out\` (bench logs `combo1-*` … `combo7j-*`, `combo7h-*`).

## Combos

What stopped them, found with a bench that has one Sora combo an idle one (`--duel-bench` / `--duel-idle`)
and records every refused contact (`duel_hit_refused`):

1. The victim's hit record was the player's: a player-type record locks out every other attack group for 30
   ticks after a hit, and the lock does not run while he is in a damage motion (`HitRecord`, FUN_1403cf4b0).
   Each combo step is its own group, so only the first step landed.
2. Once that lock was gone, a swing hit on every tick of contact: the duel marked a swing's instance as seen
   only when it landed, and with the room's clock (`simulate` has advanced it when the duel's hits run) one
   tick ahead of the retire's (enemy_battle), so every instance's attack id was freed each tick.

The change (`test_room/duel_hits.rs`, `duel_loadout.rs`, `rival.rs`):

- [Duel rule] Each Sora takes the other's hits on a target's record (`HitRecord::new(false)`), as an enemy
  takes Sora's: no group lock, so each step that connects counts; the attack rows' own re-hit intervals still
  space multi-hits (each rival's record now advances once a tick, as the room Sora's does).
- [Duel rule] A Sora's flinches (hurt motions 0–5, ground and air) have no invulnerable frames in a duel; a
  blow (a finisher's launch or a knock-down), its landing, the recovery and the knock-out keep the frames the
  game's motions give them, as do dodges and attacks with frames of their own, so a launched Sora gets up
  before he can be hit again. This is the closest model to KH2's own: the player's damage protection is the
  group lock and those motion frames; a target (an enemy) has neither, and KH2 has no player-against-player
  rule to copy.
- Every live swing and spell volume marks its instance on the simulated tick, so a swing hits a Sora once per
  contact window.
- The bench presses inside a ground combo's reach (a press from farther starts the running lunge, which is what
  made the scripted rival look as if only step 1 landed).

Results at ×3 (an idle target; full Kingdom Key string: three steps, attack rows 126, 128, 147, then the
finisher 129):

| Attacker → victim | Keyblade | Steps 1–3 | Finisher | Full string |
| --- | --- | --- | --- | --- |
| Host → joiner | Kingdom Key | 15 | 12 | 27 |
| Joiner → host | Winner's Proof (the lobby script's pick) | 27 | 24 | 51 |

170 HP is about six to seven Kingdom Key strings, about 15 s of uninterrupted strings on a target that never
moves or guards; with players missing, guarding and moving that is in the 30–60 s range, so the default stays
×3 [duel choice].

## Checks

| Check | Result |
| --- | --- |
| `cargo fmt`, `clippy -D warnings`, `cargo test` (1,089 passed) | PASS |
| Single-player tours (talk, magic, party-hit) against a build of `5d7a960` | PASS (identical) |
| Bench, joiner → idle host and host → idle joiner (`combo7j`, `combo7h`): full strings land, no refused contact, one hit per step | PASS |
| Two PCs, players at the keys | NOT RUN |

## Verdict / next

Combos PASS on one PC. Next: magic, every spell from both sides.
