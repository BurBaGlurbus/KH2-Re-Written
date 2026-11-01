# T335 — A world point marks its area when the area loads

Owner Claude, 2026-11-01, requested by burba in the project thread ("get more issues on github done"). Issue #143 (one item). Gate:
development of the field and world map; no final gate is inferred. Prerequisites: T324. Build identity: local `main` after `be25005` plus
this change, package 0.1.2. Source: the selected Steam KH2FM build, validated each run; the install was only read.

Private evidence (outside the checkout): `research\issues-20261101\g\a3\refs2.txt` (callers of FUN_1403f2270), run `wp1` under
`%USERPROFILE%\kh2-issues-out\s161\`.

## What the executable does

FUN_140418c20 and FUN_140418e30, the world point object's set-up, call FUN_1403f2270: the bit of the area's point in `save+0x1ef0` is set
when the object is made. It does not wait for a touch or a press. The world map reads these bits.

## Change

`Progress::enter_world_point`, called when the room's first area is made and on every area change when the area holds a WORLD_POINT object.
`world_state::enter_area_with_world_point` already held the rule.

## Checks

| Check | Result |
| --- | --- |
| `cargo clippy --all-targets --locked -- -D warnings`, `cargo test --locked` | PASS |
| Release `--save-point-tour` in tt area 9, map program 6 (WORLD_POINT), `--save-dump` (`wp1`) | PASS: the image's `save+0x1ef6` holds 0x01 |
| Live chain across windows, played by hand | NOT RUN |
| Side by side with the installed game | NOT RUN |

## Verdict

PASS for the item. #143 stays open only for playing the chain across windows by hand.

## Next

The owner plays save point → World Map → landing → field once.
