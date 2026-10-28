# T280 — LAN duel: the session's status in the game's message balloon, cleaner match text

Owner Claude, 2026-10-28, requested by burba in the project thread: make the multiplayer in-game UI better
using the KH2 menus. Gate: none of the roadmap's (the LAN duel is an addition outside the original game; no
fidelity claim). Prerequisites: the LAN duel (T240-T249, T279). Build identity: local `main` after `90f073c`,
package 0.1.2, Rust 1.99.0 Windows MSVC, release profile with `skate3` for the runs. Source: the selected Steam
KH2FM build, validated at each run; the install was only read.

Evidence (private, outside the checkout): `%USERPROFILE%\kh2-lan-out\ui0-*` (before), `ui1-*`, `ui2-*`
(after, two instances over loopback), `uileave*-host` (the other player leaving), contact sheets in
`kh2-lan-out\ui-shots\`; picked screenshots in `.local\test-room\screenshots\duel-ui-20261028\before|after`.

## What was there

The lobby, bans, builds, a vote, the result and the Forfeit menu were already the game's camp (T246). The rest
was drawn on the 2D layer in the system font: the session's status ("Waiting for a player on UDP port ...",
"Opponent left") as a line on a black box, the countdown, GO, K.O. and the result's title. The result's
VICTORY / DEFEAT was drawn over the open result menu, on top of the camp's party model, and the network
overlay (`--net-overlay`, F3) was drawn over every menu, hiding the camp's banner and the right end of rows.
Text over the field had no backing or shadow and was lost on the K.O. flash.

## What changed

- **Status in the message balloon.** The session's status is shown in YS::WINDOW_BALLOON, the message window
  the world map uses for its messages (`visual::dialogue::TalkWindow::balloon`): its art (msg/us/fontinfo.bar
  and wm.bar's window), the event font and the world map's text colour (03system `ftst` 5, world 15), read once
  per session on a thread. `TalkWindow::notice` is the balloon without the advance arrow and without the
  advance; `dismiss` closes it with its own close animation. A changed status closes the old window and opens
  the new one when it has gone. Texts: "Waiting for a player to join. / UDP port N", "Joining ADDRESS...",
  "NAME left the duel. / Waiting for the next player.", "NAME stopped answering." (a time-out), "Esc to quit."
  on the joiner. Without the art the old line is drawn. The status gives way to any open menu.
- **The result.** VICTORY / DEFEAT and the reason stay over the field for 1.5 s (90 ticks, the last 10 fading)
  with the winner's camera, in the upper third so the Sora shows; then the result menu opens, without the
  title over it. Its Results page and help line carry the reason, as before.
- **Motion** (the owner asked for Emil Kowalski's design-engineering rules): menu navigation keeps the camp's
  own behaviour and gets nothing added. The K.O. and result titles settle to their size in 12 ticks (200 ms)
  with a strong ease-out (cubic) instead of shrinking linearly over 20-30 ticks; the result title leaves in 10
  ticks, faster than it came. The status balloon uses the window's own open and close groups.
- **Text over the field** (round, names, numerals, GO, K.O., the result's title): a dark drop shadow, so it
  reads on the K.O. flash. No box.
- **Network overlay.** Not drawn while a menu is open; below the balloon while one shows.
- Nothing changed in the camp screens themselves (lobby, bans, builds, vote, result, Forfeit): they are the
  game's camp with its cursor, sounds, transitions and help line.

## Duel choices

- The balloon for the status (the game shows such messages in it; the duel's texts are the rewrite's own).
- The world map's window art and colour, since the duel's arenas have no world of their own.
- The 1.5 s title hold before the result menu, and the titles' 200 ms entrance.

## Checks

| Check | Result |
| --- | --- |
| `visual::dialogue` unit test: a notice has no arrow, ignores the advance, closes when dismissed; a talk balloon still waits with its arrow and plays the advance sound | PASS |
| `duel_net` unit test: the departure texts name the player once; a notice's lines end in 0x02 and the text ends in 0 | PASS |
| Loopback LAN `ui1` (both scripted, One KO, net overlay on) | PASS: the waiting balloon on the host from tick 6; no overlay over the lobby, bans, ready, vote or result menus; DEFEAT / VICTORY over the field, then the result menu without it; the Results page counts as before |
| Loopback LAN `ui2` (both scripted, Best of 3, overlay off) | PASS: waiting balloon, lobby, bans, vote, countdown, GO, two K.O.s, DEFEAT over the field, Results page, the next match's lobby |
| The other player leaving (`uileave3`, joiner quits) | PASS: "JoinSora left the duel. / Waiting for the next player." at tick 610, the waiting balloon again at 926; open and close animations seen (`uileave`) |
| Balloon size against its text (layout units, one host run) | PASS: box 90-425, text 115-414 |
| The joiner's "Joining" balloon | NOT SEEN: on loopback the joiner connects before the art is read |
| Two PCs, real pad or keyboard in the menus | NOT RUN |
| Loopback LAN `ui3` after the entrance change (One KO) | PASS: K.O., DEFEAT on the host and VICTORY on the joiner at full size and readable; the result menu after the hold |
| `cargo fmt --all --check`, `cargo clippy --all-targets --locked -- -D warnings`, `cargo test --locked` (31 test binaries), `cargo build --locked`, smoke `--frames 180` | PASS |
| Release launcher (`tools\Build-Launcher.ps1 -Skate3`) | PASS |

## Uncertainty

- The balloon's timing and art are the world map's; the field's own system messages were not compared.
- The countdown, GO, K.O. and the result's title are still the rewrite's text in the system font; the game has
  no duel screens to take them from, and the Coliseum cups' or Struggle's 2D layouts were not looked into.
- The vote's coin is still the rewrite's flat disc.

## Verdict

PASS for the screens checked on loopback; two PCs NOT RUN.

## Next

- Look for a 2D layout with big numerals or titles (the Coliseum cups, the Struggle) to replace the countdown
  and result text.
- The joiner's "Joining" balloon on a slow link, and the screens on two PCs.
