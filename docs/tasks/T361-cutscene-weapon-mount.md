# T361 — Keyblades mounted on the hand without the axis flip

Owner Claude, 2026-11-02, requested by burba in the project thread ("work through issues and close them"). Issue #27. Gate: cutscene playback development; no final gate is inferred.
Prerequisites: T099, T072 motion oracle. Build identity: local `main` after T360 plus this change, package 0.1.2. Source: the selected Steam KH2FM build, validated each run; the install was only read.

Private evidence (outside the checkout): `%USERPROFILE%\kh2-issues-out\anim72\` (event command dump, the model and motion payloads, the PS2 evaluator's matrices `r-rox3`, `r-rox5`, `r-w3`, `r-w4`); captures `ev613`, `ev613b`, `ev613d` and sheets `ev613c`, `ev613e`, `ev613f` with the reference sheets `ref613a`, `ref613b` under
`%USERPROFILE%\kh2-issues-out\s162\`. Reference footage: a public PS2 KH2FM video of the Roxas and Axel fight.

## What was wrong

In tt19 event 613 the Keyblades (`w_ex010`, actors 1 and 2) are bound to Roxas's bones 63 and 172 by AttachEffect type 1. Around frames 745-805 Oathkeeper's keychain stood out straight towards Axel, and in the earlier shot (651-713) the chains rose above the hilts. The footage shows the Keyblades held out to the sides with the chains hanging.

Roxas's ETT61303A and ETT61304A and the weapon's `anm_tt_l` motions all match the PS2 evaluator over the shots (worst 0.02 units), so the motions were not the cause. Composing the PS2 matrices both ways put the cause in the mount: with the y and z flip that playback applied between the hand bone and the weapon, chain bones 3-11 ran sideways or upwards; without it, they hang straight down from the hilt, at every sampled tick of both shots except mid-swing.

## Change

`cutscene.rs`: the child of an AttachEffect type 1 sits at `placed * M_bone`, with no flip. The exe's two conversions (the bone read through FUN_1401a7af0 and the child's vertices) cancel in the space the poses are drawn in. The test room's hand mounts already used the bone matrix directly. Effects bound to bones keep their flip.

## Checks

| Check | Result |
| --- | --- |
| `rustfmt --check` on the changed file, `cargo clippy --all-targets --locked -- -D warnings`, `cargo test --locked` | PASS |
| PS2 evaluator against our motions, Roxas and the weapon, both shots | PASS (worst 0.0214) |
| Event 613 captures at 600, 640, 690, 750, 765 and 790 against the footage at 19-26 s | PASS for the Keyblades: at 750-765 the left one points at Axel and both chains hang; at 690 the raised Keyblade's chain hangs below it |
| Other events with weapons bound by AttachEffect type 1 | NOT RUN |
| Side by side with the installed game | NOT RUN: footage only |

## Uncertainty

- The footage is 640x360, and its picture is cropped from a 4:3 screen, so the framing differs from ours. Which Keyblade is which colour isn't readable at that size.
- Roxas is washed nearly white in the 746-826 shot here, while the footage shows his jacket. That comes from the shot's effects or glow, not the mount, and isn't part of this change.

## Verdict

PASS for the weapon mount in event 613.

## Next

Roxas's brightness in the 746-826 shot.
