# T253 — The file menu's message window from save.2ld (issue #80)

Owner Claude, 2026-10-26, requested by burba in the project chat ("continue working on more
issues"). Gate: development of the title and test room; no final gameplay acceptance is
inferred. Prerequisites: T155 (file menu), T251. Build identity: local `main` after `4a1b4a8`,
package 0.1.2, Rust 1.99.0 Windows MSVC. Source: the selected Steam KH2FM build, validated each
run; the install was only read.

Private evidence: `%LOCALAPPDATA%/KH2FinalRewrite/research/issues-20261026/g/` (`w1`, `w2`:
FUN_1402e78e0, FUN_1402eae10, FUN_140322690, FUN_140322dd0, FUN_140322f80, FUN_140323250;
`w3.txt` references; `w4.txt`, `w5.txt` data at 0x1405ba638 and the group tables); unattended
runs `save4`..`save8` and `title1` in the private `kh2-issues-out/`.

## What the executable does

The save point screen's init (`FUN_1402eae10`) sets the window descriptor `DAT_140bef750` to the
table at 0x1405ba638 (`FUN_140322220`). Its entry +0x28 points at 0x1405ba528, whose shorts are
4, 6, 5: the message window is created by `FUN_140322dd0` → `FUN_140300620(window, 0x756d, …,
group 4, queued 5, …)` on sequence 0 (`FUN_140322210(0, 0)`), and re-opened with group 4 when it
stands on group 6; the message text is the sprite's text (+0x1f8, `FUN_140322690`). The groups'
parts (read from the installed save.2ld) are a fixed box about 386 x 186 around the animation's
translation (256, 192), plus a one-pixel part; nothing resizes it.

## What the rewrite did

The messages were drawn on a translucent dark panel sized to the text (the earlier attempt with
talk-window groups 67 / 73 and a nine-slice fit did not give the art), and the text was centred
with widths in the font's reference units, which made it wider than drawn.

## Rewrite

- `visual::file_menu`: the message window is sequence 0's group 4 queued 5, at its own
  translation; the message lines are centred in the box with the widths scaled to the drawn
  size (`TEXT_SIZE / 18`).
- The panel and its white sheets are gone from `Draw`, the title and the test room.
- `--save-point-tour` captures the "Saving data" window (tick 436).

## Checks

| Check | Result |
| --- | --- |
| `cargo fmt --all --check`, `cargo clippy --all-targets --locked -- -D warnings`, `cargo test --locked`, `cargo build --locked`, smoke | PASS |
| Release `--save-point-tour` (private `save8`) | the "INFORMATION / Saving data to PC." window is the dark red bordered box centred on the screen with its text inside |
| Release title `--autoplay-savepoint` (private `title1`) | the "Jump to World Map?" and "QUESTION / Overwrite existing file?" windows draw the same box; Yes / No rows below as before |
| Before: nine-slice fit of groups 4..6 (private `save4`) | broken: the parts are not in nine-slice order; and without the group's own translation (`save5`) the box sat at the bottom right |

## Uncertainty

- The window ends with the dialog at once; the close group 6 is not played.
- [I] The text block is centred in the box; the executable's text origin inside the sprite
  (the group's +0x10 / +0x14 = 64 / 130) was not traced.
- Not compared frame for frame with the installed game.

Verdict: PASS for issue #80 in the development room and title (the window is the save.2ld box
the executable selects); PARTIAL against the installed game.
Next: the close animation and the exact text origin.
