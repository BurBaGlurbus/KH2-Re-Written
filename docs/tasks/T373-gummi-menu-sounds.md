# T373 — The Gummi menu's sounds

Owner Claude, 2026-11-02, requested by burba in the project thread (the Gummi mode in pieces). Issue #142. Gate: development of the Gummi menu in the world map program; no final gate is inferred.
Prerequisites: T370, T371. Build identity: local `main` after T372 plus this change, package 0.1.2. Source: the selected Steam KH2FM build, validated each run; the install was only read. Plain Ghidra 12.1.4 headless on a read-only project copy.

Private evidence (outside the checkout): `research\issues-20261102\gummi142\gummi-result-layout.md` with its `res2\` folder (the sound id decode); run `gm4` under `%USERPROFILE%\kh2-issues-out\g142\`.

## What the executable does

A Gummi sound id is bank × 1000 + entry (FUN_140271ca0 for the menu, FUN_140212650 in flight), so 0xf3689 is se997 #1. The menu mode loads `gumibattle/se/se997.win32.scd` (FUN_140243bc0).

## Changes

- The world map program also loads se997.
- The Gummi menu's sound events go to the world map's sound requests, which already resolve ids that way (`visual::sound::Banks`).
- The T370/T371 "silent" gap is closed.

## Checks

| Check | Result |
| --- | --- |
| `cargo fmt --all --check`, `cargo clippy --all-targets --locked -- -D warnings`, `cargo test --locked` | PASS |
| `KH2RW_GUMMI_KEYS=450:4,540:80` (`gm4`) | PASS: ten menu sounds requested (decide 997002, mission 997121, mission open 997124, next level 997125, move 997001, cancel 997003), each resolving to one se997 layer |
| Heard against the installed game | NOT RUN |

## Uncertainty

- The top menu's open sound (0xf36fe, stored at +0xf0) isn't played: when the exe plays it isn't traced.
- Volume and priority come from the bank as for other menus.

## Verdict

PASS in the world map program.

## Next

#142: the result screen and the after-clear and game-over menus (`gummi-result-layout.md`).
