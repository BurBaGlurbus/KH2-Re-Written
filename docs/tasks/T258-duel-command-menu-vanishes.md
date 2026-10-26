# T258 — The command menu vanished during a LAN duel

Owner Claude, 2026-10-26, requested by burba in the project chat ("in multiplayer the command menu sometimes
disappears randomly"). Gate: development of the LAN duel (T240 onward); no gameplay rule of the installed game changes
and no final gate is inferred. Prerequisites: T202 (the second deck), the LAN duel merged into local `main` at
`7405db8`, T256 (PC keyboard and mouse). Build identity: local `main` after `f85bac1`, package 0.1.2, Rust 1.99.0
Windows MSVC, release launcher with the `skate3` feature. Source: the selected Steam KH2FM build, validated each run;
the install was only read.

Private evidence: the before and after loopback runs (`deckgone-before-*`, `deckgone-after-*`: captures, event logs,
stderr) in the private LAN output folder outside the checkout; the two verification executables in this session's
private scratch folder.

## Cause

A networked duel turns the second deck off (`game.second_deck = None` at the session's start: no summons, party or
limits), but the deck swap stayed live. The swap is logical pad bit 0x10000: D-pad left, the Left arrow key, the
mouse's back button, or L2 held with the right stick (or the mouse) to the left. One press showed the second deck,
which had no rows: only the COMMANDS header stayed, and with no rows Attack did nothing either, until the swap was
pressed again. On a controller or a mouse with side buttons that press is easy to make by accident, which is why it
looked random. Single-player rooms load a second deck, so the swap there shows its rows.

The other suspects were read and ruled out for this symptom: the camp, game-over and HUD-setting gates hide the whole
HUD (not the menu alone) and a duel has no game over; the battle style is constant in a duel; the rival's tick swaps
his own state but never touches the HUD; the joiner's snapshots do not write the menu.

## What the rewrite does now

`CommandMenu::hold_first_deck` [rewrite choice]: with no second deck the swap is refused and its pointer hidden, as
FUN_1403b2780's lock does, and a menu already on the second deck goes back to the first. `simulate` calls it every
tick while `game.second_deck` is `None`, and clears the lock otherwise.

## Checks

| Check | Result |
| --- | --- |
| Loopback duel, host and joiner (`--rival-script`), pre-fix code with a temporary hook pressing the deck swap at tick 900 | FAIL as reported: `command_deck` kind 6 on both sides; captures at 960 and 1200 show only the COMMANDS header through the fight on both |
| Same run on the fixed code, presses at 900 and 1300, joiner with `--net-sim 60,15,3` | PASS: no swap on either side; the four rows stay at 928, 1187, after the KO and in the rematch countdown (2990) |
| New unit test: an empty second deck takes the rows away; held on the first deck the rows return, left no longer swaps and decide runs Attack | PASS |
| `cargo fmt --all --check` | PASS |
| `cargo clippy --all-targets --locked -- -D warnings` | PASS |
| `cargo test --locked` | PASS |
| `cargo build --locked` | PASS |
| Smoke `cargo run -p main --locked -- --frames 180` | PASS |
| Release launcher with `skate3` | PASS |

The hook was removed before the commit; the shipped code has no test-only input.

## Uncertainty

- The owner's own disappearance was not watched; the reproduced one matches the report and is the only path found
  that removes the menu alone. If it happens again with no `command_deck` line in the log just before it, it is
  another cause.
- Physical presses (the arrow key, a mouse side button, a controller) were not made; the hook queues the same bit
  the pad pipeline does.

## Verdict

PASS for the reproduced disappearance in the LAN duel. No final gate.

## Next

None for this bug unless the owner sees it again without a `command_deck` line in the log.
