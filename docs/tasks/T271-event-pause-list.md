# T271 — The event pause menu on pause.2ld (Axel and Roxas cutscene)

Owner Claude, 2026-10-28, requested by burba in the project thread ("work on more issues and closing them").
Issue #28 (pause menu item). Gate: development of the event player; no final gate is inferred. Prerequisites: T099
(event player), T140 (the pause-definition list, `visual::pause_definition`). Build identity: local `main` after
`3c837ac`, package 0.1.2. Source: the selected Steam KH2FM build, validated each run; the install was only read.

Private evidence (outside the checkout): `research\playable-room-20261008\cutscene-01\audio\audio-spec.md` §5 (pause
during events); runs in `C:\Users\ANIMA\kh2-issues-out\` (`ev-pause1`, `ev-pause2`).

## What the executable does (audio-spec §5)

- The event start (`FUN_1403ac5a0`) pushes pause definition **4** for an event whose `evtp` flags have bit 0
  (613: 0x65). Start opens it through `FUN_1402e4c20` (game state 8).
- Definition 4 (14mission.bar `slct`) lists Continue (result 6, sys 0x8ab4) and Skip Scene (result 5, sys 0x8ab3).
  The list is `menu/<lang>/pause.2ld` with the system font (`FUN_14031d200` / `FUN_14031ee60`). Opening plays se000
  #3, confirm #2, cursor #1. The cursor starts on the last chosen row (DAT_140744cf0).
- While it is open the event clock, voices, sound effects and music hold; Continue resumes them; Skip Scene runs the
  end of the event.

## Change

- `cutscene::load`: `PauseSource` (pause.2ld, definition 4 with its messages, the system font); se000 is loaded with
  the event's banks.
- `cutscene`: a `Pause` resource drives `visual::pause_definition::Menu`, the list the game-over screen already uses,
  in place of the plain two-row list. Results 6 / 5 continue or skip; Start and Backspace close it as Continue. The list's
  sounds play on their own mixer key while the event's sounds are held. The list draws through the 2D frame with the
  pause.2ld art and the system font sheets.
- `kh2fm event --autoplay <dir> --pause-at <frame>` (unattended): opens the menu at that frame, moves the cursor
  down and back, captures `event-pause-00/01.png`, then chooses Continue. [Test setup.]

## Checks

| Check | Result |
| --- | --- |
| Release `event --autoplay --pause-at 200` (`ev-pause1`, `ev-pause2`) | PASS: the list opens at frame 200 over the held event; captures show Skip Scene selected, then Continue (glove, light, red selected box); Continue resumes the event (frame 260 captured); sounds se000 #3, #1, #1, #2 resolved and played |
| `cargo fmt --all --check`, `cargo clippy --all-targets --locked -- -D warnings`, `cargo test --locked` | PASS |
| Skip Scene by the list | NOT RUN unattended (the script picks Continue); the skip path is the one the plain list used |
| Side by side with the installed game | NOT RUN |

## Uncertainty

- Which se000 numbers the cursor and cancel play is [I] in the spec (T[] indices 1 / 3); the list's constants were
  kept.
- Keyboard only: the event player reads no pad. There is no auto-repeat on held arrow keys.
- What the event's own curtain does over the open list (draw order with layer C) was not checked against the
  original.

## Verdict

PASS for the pause list; #28's other items (frame-by-frame checks of blur, face animation and mesh groups, the ribbon
pair mode #63) remain.
