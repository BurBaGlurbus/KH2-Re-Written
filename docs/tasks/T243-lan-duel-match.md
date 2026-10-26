# T243 — LAN duel, phases 5 and 6: the countdown and the match

Owner Claude, 2026-10-26, requested by burba in the project chat (phases 5 and 6 of the LAN duel: a synced
3-2-1-GO from a host start tick, the match flow decided on the host, win and lose sequences, a results panel,
rematch or exit; the owner's answers of 2026-10-26: the host chooses the mode, a disconnect mid-match is a win
for the one still there, the game's own sounds for the countdown and the victory). Gate: none of the roadmap's
(an addition outside the original game; no fidelity claim). Prerequisites: T242. Build identity: worktree
`KH2FinalRewrite-lan`, branch `feature/lan-duel`; package 0.1.2, Rust 1.99.0 Windows MSVC, dev profile.
Source: the selected Steam KH2FM build, validated at each run; the install was only read.

Private evidence: `%USERPROFILE%\kh2-lan-out\` (run logs `match1-*` … `match4-*`, their captures) and the
trace `%LOCALAPPDATA%/KH2FinalRewrite/research/playable-room-20261008/duel-cues-01/`.

## What changed

1. The match (`test_room/duel_match.rs`): Countdown → Fight → RoundOver → (next round's Countdown) → Over,
   stepped by the host on its tick. 3, 2, 1 a second each, then GO; a knock-out ends the round; the timed mode
   ends it at the clock with the larger share of HP left (equal shares draw); Best of 3 plays rounds until two
   wins (drawn rounds count for nobody, at most five rounds); a player who leaves loses. Damage dealt and hits
   landed are kept per player.
2. The match in the room (`test_room/duel_match_room.rs`): the host starts it when the lobby's players are both
   ready, refills both Soras (HP, MP, Drive gauge, out of any form) and puts them at the arena's starts for
   every round, and sends it reliably on every change. Both instances read it on the host's clock (the joiner
   on the snapshot it shows), so the countdown, GO and the release of the Soras fall on the same host tick on
   both screens; outside the fight neither Sora moves. A knocked-out Sora stays down for the round.
3. On screen, worked out from the time since the phase began (nothing spawned per frame): the names and the
   numerals growing in and fading, GO, the clock (red under ten seconds), the round and score, "K.O." or "TIME
   UP" with the round's winner, and the result: VICTORY / DEFEAT / DRAW, how it was decided, rounds, match time,
   damage dealt and hits landed, Rematch and Leave. Rematch on both returns to the lobby (new bans; each
   player's last build is where the camp starts); Leave closes the game.
4. Shots and effects: an intro shot of both Soras from the side, orbiting slowly, through 3 and 2 of a match's
   first countdown; at the result the winner's camera pushes in from the front and the loser's rises slowly
   behind his own Sora (`game.duel_camera`, under any mission overlay). The game's system effects: the item-use
   flash on both Soras at GO, the level-up sparkle on the winner.
5. Sounds, the game's own (private trace `duel-cues-01`: every competitive start, win and loss cue of KH2FM
   is a sound effect from a mission record or the game-over task; the duel takes the Olympus Coliseum cups'):
   the cup match's start stinger (se007 #13) with the intro; the timer's beep (se000 #43, dk::TIMER
   FUN_14018d830) for 3, 2, 1 and at 2, 1 and 0 seconds left of a timed fight; a cup round won with rounds left
   (se007 #16) at a round's end while the match goes on; a cup match won (se000 #53) on the winner's screen;
   the game over a lost cup match ends in (se000 0x13) on the loser's; the minigames' finish (se000 #44) for
   a draw. GO keeps the menus' decide [duel choice: the minigames' start #47 carries its go in a layer the
   room's sound player would start together with its beeps]. The cups' bank `se/se007.win32.scd` (larger
   than the default read limit) is read on its own thread when a duel's lobby opens and registered when read.
6. `--duel-mode stock|best3|time` and `--duel-time SECONDS` (60, 90, 120, 180, 300) set the host's rules from
   the command line.

## Checks

| Check | Result |
| --- | --- |
| Match logic and wire (`duel_match` tests: countdown, KO, Best of 3, time up and draws, leaving) | PASS |
| Best of 3 at ×10 (`match1`): countdown, round 1 to the joiner by KO, reset and round 2, the match 2–0; VICTORY and DEFEAT panels on the two screens with the same numbers | PASS |
| One KO (`match3`): KO, result, Rematch on both, a new lobby (match 2, new bans), a second match started; the joiner leaving then scored as the host's win | PASS |
| Leaving mid-match (`match2`): the joiner's run ended during a round, the host's result: win, "left" | PASS |
| Intro shot of both Soras through 3 and 2, K.O. banner (`match4`) | PASS (captures) |
| The game's cues (`match6`, Best of 3 at ×10): se007 registered as bank 7 on both; start stinger, six beeps, two GOs, the round's end after round 1, se000 #53 on the winner's screen and 0x13 on the loser's | PASS |
| `cargo fmt`, `clippy -D warnings`, `cargo test` (1,089 passed) | PASS |
| Single-player tours (talk, magic, party-hit) against a build of `5d7a960` | PASS (talk and magic identical; party-hit identical but for its exit tick, 6840 against 6841, the known run-to-run noise) |
| Two PCs on a real LAN | NOT RUN |

## Uncertainty

- No victory motion is used: Sora's kit as the room loads it has no win pose the duel could play; the push-in
  frames him as he stands.
- The result's push-in is not checked in a capture yet (the unattended runs end before it in the captures
  taken).

## Verdict / next

PASS for phases 5 and 6 on one PC (loopback); two PCs NOT RUN. Next: the lobby's polish (a menu in the game's
look, the opponent's section, Keyblade stats and preview, a live AP counter and clear refusals, a disconnect
in the menus).
