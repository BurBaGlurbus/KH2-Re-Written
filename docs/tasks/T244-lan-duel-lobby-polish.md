# T244 — LAN duel: the lobby's polish, Leave and the result shots

Owner Claude, 2026-10-26, requested by burba in the project chat (the lobby polish the phase 3 spec asked for: a
menu in the game's look for pad and keyboard, transitions, which section the other player is in, the
Keyblade's stats with a preview, a live AP counter with clear refusals, the other player leaving mid-menu; the
coordinator's follow-ups after phase 6: Leave to a main menu, a victory pose if the game has one, the result
shots captured). Gate: none of the roadmap's (an addition outside the original game; no fidelity claim).
Prerequisites: T243. Build identity: worktree `KH2FinalRewrite-lan`, branch `feature/lan-duel`; package 0.1.2,
Rust 1.99.0 Windows MSVC, dev profile. Source: the selected Steam KH2FM build, validated at each run; the
install was only read.

Private evidence: `%USERPROFILE%\kh2-lan-out\` (run logs and captures `lobbyui3-*` … `lobbyui5-*`) and the
trace `%LOCALAPPDATA%/KH2FinalRewrite/research/playable-room-20261008/victory-pose-01/`.

## What changed

1. The lobby's rows are the game's own pause list (`visual::pause_definition::Menu`, the game-over list's
   boxes, glove, light, appear animation and se000 sounds, its art from the game-over resource), built per
   stage and updated in place (`Menu::set_text`) when Left/Right steps a value: the host's rules, then the
   bans' row, then the Keyblade, *Abilities and items* (opens the camp, as Start does) and Ready.
2. A status panel at the top left in the system font, fading in with each stage: the joiner's view of the
   rules, both players' bans, where the other player is (`Seat::section`: the list, the ban picker or the
   camp, sent with an intent), the AP used against the limit (orange with "too many abilities equipped" when
   over), and the Keyblade's name, Strength, Magic and ability. Refusals show in orange; the ban picker says
   when a third ban is refused.
3. The Keyblade's preview: Left/Right on its row puts the Keyblade on this player's Sora at once (its kit
   rebuilds on both instances), both Soras stand armed while the builds are made, and while the row is chosen
   the camera looks at this player's Sora from the front, to the left of the list.
4. The other player leaving mid-menu: an open camp closes without writing the duel's build back, and the
   camp's and the field's own state come back.
5. Leave on the result screen closes the duel; the launcher, which waits on it, comes back on its
   Multiplayer screen (hosts on the network, Host, Join) rather than its tests list.
6. The result screen: the title at the top and the numbers and choices in the lower third, so the winner's
   push-in and the loser's view show between them.
7. A victory pose: none used. The game has none for Sora or Roxas (private trace `victory-pose-01`: every
   non-combat motion of `obj/P_EX100.mset` has an owner in the exe and none is a cheer; the Struggle and
   Coliseum win paths play sounds and 2D layouts only; the Drive revert just starts idle; cutscene poses are
   event-only files). The nearest is the game's own end-of-battle motion (motion 6 in its field variant, P_EX100
   slot 27, 70 ticks, Sora rising from the battle stance), which the room's kit does not load today; left out
   [duel choice: a stand-down is not a victory pose, and loading it changes the single-player kit].

## Checks

| Check | Result |
| --- | --- |
| `cargo fmt`, `clippy -D warnings`, `cargo test` (1,089 passed) | PASS |
| Single-player tours (talk, magic, party-hit) against a build of `5d7a960` | PASS (talk and magic identical; party-hit identical but for its exit tick, 6840 against 6841) |
| Lobby on both screens in the game's list (`lobbyui3`, `lobbyui4`): the host's rule rows and Choose bans; the joiner's Keyblade (Winner's Proof), Abilities and items, Ready!; the panel with bans, the other player's section, AP 2 / 166 and the Keyblade's numbers and ability | PASS (captures) |
| Keyblade preview: armed Sora with the picked Keyblade, the camera on him (`lobbyui4`) | PASS (capture) |
| Result shots (`lobbyui5`, a knock-out): VICTORY with the winner's camera in front of him, DEFEAT with the loser's behind his Sora; then a rematch into a new lobby (match 2) | PASS (captures, before the panel moved down) |
| The result panel moved to the top and lower third | NOT RUN in a capture (the unattended runs did not end a match between captures) |
| The camp opened from the list's row, a refusal for AP over the limit, the other player leaving while the camp is open, Leave back to the launcher's Multiplayer screen | NOT RUN (needs a player at the keys; logic in place) |
| Two PCs | NOT RUN |

## Uncertainty

- The rows' texts must fit the game's boxes; the longest Keyblade names are the limit seen so far.

## Verdict / next

PASS on one PC for what the captures cover; the player-driven paths and two PCs NOT RUN. Next: what the owner
asks after trying it.
