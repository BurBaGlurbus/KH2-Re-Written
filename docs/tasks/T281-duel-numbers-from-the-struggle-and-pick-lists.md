# T281 — LAN duel: the game's own countdown, clock and titles (the Struggle's), and pick lists in the lobby

Owner Claude, 2026-10-29, requested by burba in the project thread: use the actual KH2 assets for the numbers,
found from the executable with Ghidra, and make the multiplayer menus (readying up, choosing a Keyblade and the
rest) better: not every choice should need Right on the D-pad. Gate: none of the roadmap's (the LAN duel is an
addition outside the original game). Prerequisites: T280 (ba829cc). Build identity: local `main` after
`ba829cc`, package 0.1.2, Rust 1.99.0 Windows MSVC, release profile with `skate3` for the runs. Source: the
selected Steam KH2FM build, validated at each run; the install and the executable were only read.

Evidence (private, outside the checkout): headless Ghidra outputs and a summary in
`%LOCALAPPDATA%\KH2FinalRewrite\research\duel-ui-20261029\` (`duel-ui-spec.md`); two-instance loopback runs
`%USERPROFILE%\kh2-lan-out\ui4-*` (One KO), `ui5-*` (Time limit, 60 s), `ui6-*` (lists); screenshots in
`.local\test-room\screenshots\duel-ui-20261029\before|after`.

## What the executable showed

- **dk::TIMER** (vtable 0x1405b23d0) is the field's time-limit readout: shown by FUN_14018d1b0, laid out by
  FUN_14018d5c0, stepped by FUN_14018d830. It splits the frames left (FUN_1401575d0: minutes, seconds,
  hundredths), draws each digit as group 0xc + digit (table 0x14071bc38) with the marks 0x17 and 0x18, every
  sprite after the last by its group's pitch, and in the last three seconds plays the big numerals 0x1f, 0x1e,
  0x1d (3, 2, 1, each with se000 0x2b) and 0x1c (0). It draws from the sheet a mission's timer widget hands it
  (FUN_1403f9ec0 -> FUN_14017b8a0).
- **Mission files** (msn/us/*.bar, 139 in the install) carry their own 2D sheets: `ti_?` (timer), `st_?` (start),
  `ed_?` (end), `ct_?` (counter), `gg_?` (gauge). The Struggle's (TT05_MS401) are the game's one-on-one match:
  `ti_t` (the timer and its 3-2-1-0), `st_t` ("READY", "STRUGGLE!"), `ed_t` ("YOU WIN!", "YOU LOSE" on their bar).
- sa::COUNTDOWN / sa::ROUND_NUMBER are the Atlantica musical's and were not used.

## What changed

- `visual::mission_timer`: dk::TIMER translated (stateless, from each sprite's age): the readout, the time
  split, the last seconds' numerals. `hud::load_nth` reads one of several same-named sheets of an archive.
- The duel reads the Struggle's three sheets once a lobby opens (on a thread) and draws:
  - the countdown: the timer's big 3, 2, 1, one a second (the names line moved above them);
  - GO: "STRUGGLE!" as it bursts (`st_t` group 6, 45 frames, GO's time);
  - a timed fight: the Struggle's TIME readout at the top centre, its 3-2-1 and 0 in the last seconds; the
    last-seconds beep now comes with 3, 2 and 1 as the executable plays it (it came with 2, 1 and 0);
  - a round's end and the result: "YOU WIN!" or "YOU LOSE" on its bar (`ed_t` groups 3 and 1 / 2), from this
    player's side, held through the result; the reason stays a line of text under it. A drawn round keeps the
    text. The result's hold is a second longer when the match ended without a round's end (a player left or
    forfeited), so the banner has time to come in.
  - Without the art (an install without the file) the old text is drawn.
- Lobby menus (the camp's Config screen, `pause_menu`):
  - Decide on a row now works: Ready toggles, the bans' Confirm confirms, a two-way row (Ruleset, Arena) flips;
  - a row with more choices (Keyblade, Ban 1/2, Mode, Damage, Magic damage, Hit cap, Time limit) opens them as
    a list in place of its page: the row's name on the page's tab, the cursor on the current one ("Now"), the
    help line naming each Keyblade's Strength and Magic; Decide takes one, Cancel goes back to the row;
    L2/R2 stay on the page meanwhile; Left and Right still step every row;
  - a page longer than the panel scrolls with the cursor (8 rows on screen).
  - The help lines say so.

## Duel choices

- The Struggle's sheets for every duel (its 3-2-1, "STRUGGLE!" for GO, "YOU WIN!" / "YOU LOSE") — the game's
  only one-on-one match with a win/lose title. KH2 has no "K.O." or "VICTORY" title; Christmas Town and Space
  Paranoids have READY/3/2/1/GO! sets but only CLEAR/FAILED ends.
- Pick lists in place of a page, "Now" on the current choice.

## Checks

| Check | Result |
| --- | --- |
| `visual::mission_timer` unit tests (the time split, the numerals' seconds) | PASS |
| `duel_lobby_room` unit test (which rows list, which toggle) | PASS |
| Loopback LAN `ui4` (One KO, both scripted) | PASS: countdown numerals, "STRUGGLE!" at GO, "YOU LOSE" with the reason after the K.O.; the joiner picked its Keyblade from the list (`duel_pick`), the host readied with Decide on Ready |
| Loopback LAN `ui5` (Time limit 60 s, damage x0.5) | PASS: the TIME readout counted down (00'58"43 ... 00'00"00), 3, 2, 1 and 0 in the last seconds, "YOU LOSE" after time up with "Time up: the larger share of HP left" |
| Loopback LAN `ui6` (lists) | PASS: Ban 1's list ("Now" on the current pick) and Cancel back to the row; the Keyblade list with Strength / Magic help; rows past 8 scroll and hide |
| Real pad or keyboard presses in the lists | NOT RUN (scripted presses through the camp's own input path only) |
| Two PCs | NOT RUN |
| `cargo fmt --all --check`, `cargo clippy --all-targets --locked -- -D warnings`, `cargo test --locked`, `cargo build --locked`, smoke `--frames 180` | PASS |
| Release launcher (`tools\Build-Launcher.ps1 -Skate3`) | PASS |

## Uncertainty

- The Struggle's start and end are played by the mission's events (`re` entries, event `re_mg01`); those were not
  traced, so which groups play when (and the bar's timing against the text) is read from the sheets themselves.
- The second `ed_t` of TT05_MS401 does not decode with the replacement the loader picks; only the first is used.
- The readout's slot in the HUD stack is taken as the top (no other mission widget in a duel).
- The KO flash and the result's camera are the rewrite's (T246), not the Struggle's.

## Verdict

PASS on loopback for the screens checked; real input and two PCs NOT RUN.

## Next

- Trace the mission start/end events (`re_mg01`) to play the Struggle's banners on the game's own timing.
- Real pad / keyboard pass through the lists; two PCs.
