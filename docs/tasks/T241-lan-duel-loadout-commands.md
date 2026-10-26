# T241 — LAN duel, phase 2: the duel build and the full command set

Owner Claude, 2026-10-26, requested by burba in the project chat (phase 2 of the LAN duel, after T240; the
owner's answers of 2026-10-26: each Keyblade different, limits and summons off, Anti Form triggers, 99 of every
item, host-chosen rules, a damage multiplier). Gate: none of the roadmap's (an addition outside the original
game; no fidelity claim). Prerequisites: T240. Build identity: worktree `KH2FinalRewrite-lan`, branch
`feature/lan-duel`; package 0.1.2, Rust 1.99.0 Windows MSVC, dev profile. Source: the selected Steam KH2FM
build, validated at each run; the install was only read.

Private evidence: `C:\Users\ANIMA\kh2-lan-out\` (run logs `duofx2-*.jsonl`, `p2s2c-*.jsonl`, their captures,
the scripts `runs.ps1`, `duo.ps1`, `compare.py`, and the built executables).

## What changed

1. `a851c21` The duel build (`test_room/duel_loadout.rs`), made from the installed tables and kept for the
   session only: level 99 from LVUP, every bonus level, every form at its cap with what its levels teach, full
   MP and Drive gauge, stocked items; the room's own build comes back when the session ends.
2. `53a37e2` The damage rule: the game's formula, then ×`MatchRules.damage` (default 3, from a scripted
   bench aimed at 30–60 s fights), host-authoritative and sent to the joiner over the reliable channel;
   `--duel-damage`, F3 → Sora → Duel on the host.
3. This step: the host runs the joiner's magic, items and Drive forms.
   - Each rival Sora has his own magic, item and Drive state and kit (`rival::Own`), swapped into the room's
     fields for his tick so the room's own code runs his commands; his effect keys are tagged by slot.
   - Spell volumes hit the other Sora (Reflect guards take them); a spell's spark comes from its own PAX.
   - The joiner's Magic, Items and Drive commands travel with its buttons (`RivalPad.command`).
   - Snapshots carry the form shown and the item slots; the joiner shows the form with its kit
     (`drive_room::show_form`) and preloads every owned form.
   - The host's effect starts and spell effect calls go to the joiner as reliable events naming the effect
     file (`magic:<file>`, `system`, `form:<n>`, `weapon:<slot>`), played with the snapshot of their tick
     (`test_room/duel_fx.rs`).
   - Duel rules: Valor, Wisdom, Master and Final owned, Limit Form off; every category-0 consumable in stock
     at 99 (Potion to Megalixir). Category 1 stays out: the stat boosts (equal stats), and Tent, Drive Recovery
     and High Drive Recovery, which the game only uses from the camp's menu (item slots take category 0 only,
     their command field is 0, their description says so; private trace `drive-recovery-01`), and the camp
     does not open in a duel.
   - A duel is a battle throughout for the Drive state (Antiform can come, as in the game's battles); the
     host seeds each Sora's Drive roll from the clock when a player joins, and the form it rolls reaches the
     joiner with the snapshots.
   - `--rival-script` with `--join` scripts the joiner's buttons (test only).
4. The other Sora's own sounds: each rival runs the room Sora's frame-trigger scan (swings, battle voices,
   cloth, footsteps on his own floor) and ground dust with his own scan state; the host sends every Sora's
   motion and command sounds (swing, voice, move_se, footstep, spell_voice, drive_start, revert, antiform,
   form_level, wall_hit) as events and the dust as an effect on the map's container; the joiner no longer
   scans its own Sora from snapshot state.
5. The opponent's HP on the HUD: in a duel the game's Scan gauge (GAUGE_ENEMY, top right) shows the other
   Sora's HP for the whole match, Scan or not. MP and Drive have no gauge of their own in the game's HUD for
   another fighter, so they are not shown [duel choice]. The network overlay moves below the gauge.

## Checks

| Check | Result |
| --- | --- |
| `cargo fmt --all --check`, `cargo clippy --all-targets --locked -- -D warnings`, `cargo test --locked` | PASS |
| Wire round trips for the new events and snapshot fields (`duel_wire` tests) | PASS |
| Loopback duel, scripted joiner (`duofx2`): the joiner's Fire and Thunder cast on the host, effects drawn on the joiner (`spell_effect` records), Valor requested (host tick 803), shown on the host (963) and on the joiner (`duel_form_shown`), Valor hits land for 9 | PASS |
| Single-player tours (talk, magic, party-hit) against local main `8b2985a` (`compare.py`), before and after the merge of `b7ddcbd` and with steps 4–5 | PASS (identical) |
| Loopback duel (`duo34`): the joiner plays 29 forwarded Sora sounds and none of its own from snapshots; the opponent's HP gauge top right on both | PASS |
| Two PCs on a real LAN | NOT RUN (one PC here) |

## Uncertainty

- Unattended runs hold each instance's clock for captures, so the two instances drift; the joiner's
  "skipped snapshots" warnings and the early "other player left" (the joiner's run ends first) come from that.
- Wisdom's shots follow their bullets on the host only; on the joiner a bullet's element is not forwarded.
  Weapon trails and footstep dust of the other Sora are not drawn.
- Antiform in a duel follows the game's roll (counter, fmlv chance) with the duel always in battle; no run
  has rolled it yet, so the joiner's view of Antiform is NOT RUN.

## Verdict / next

PASS for phase 2 on one PC (loopback); two PCs NOT RUN; Antiform on the joiner NOT RUN. Next: phase 3, the
lobby (host rules, bans, abilities within AP, Keyblades, item slots, ready, the vote).
