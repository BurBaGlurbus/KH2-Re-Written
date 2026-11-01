# T345 — The big chest's event camera and its window; the mission's tutorial native

Owner Claude, 2026-11-01, requested by burba in the project thread ("commit to local main and continue working on issues on guthub"). Issues #67
and #25 (parts). Gate: development of the field in the test room; no final gate is inferred. Prerequisites: T338, T344. Build identity: local `main`
after `071cb4e` plus this change, package 0.1.2. Source: the selected Steam KH2FM build, validated each run; the install was only read. Plain Ghidra
12.1.4 headless on read-only project copies.

Private evidence (outside the checkout): `research\issues-20261101\chest\chest-spec.md` §3.2, §3.3; `research\issues-20261101\g\h1\`, `h2\`
(FUN_140158ee0, FUN_140159ae0, FUN_140159210, FUN_1403db2f0, FUN_1403ad270, FUN_140406a90, FUN_1404316e0); runs `ch8` to `ch10`, `ev6` under
`%USERPROFILE%\kh2-issues-out\s161\`.

## What the executable does

- A big chest starts its object's event `00` (FUN_1403db2f0): a 90-frame camera based on the chest's matrix (FUN_1403ad270, state 2 reads +0x808 each
  frame).
- The event's op 0x3b at frame 75 opens state 7 (FUN_140158ee0): `gaug` group 0x4d with the name, `ipic` group 2, the description below (FUN_140159210,
  the group's +0x20), se000 #10. A confirm plays #24 and closes on groups 0x4e and 3 (FUN_140159ae0).
- Native 1:296 (FUN_1404316e0) makes a menu::TutorialWidget.

## Changes

- `ChestKind::event`, `boss::Overlay::at` and `view_at`: the event's camera while the opening runs (`Game::chest_camera`).
- `visual::treasure_info`: the event window (`push_event`, `update_event`), with `Game::decide_now` as the confirm. An unattended run confirms on its own.
- A big chest's item goes to that window, not the small one.
- 1:296 reports itself and answers nothing, so the mission's script runs on.

## Checks

| Check | Result |
| --- | --- |
| `cargo clippy --all-targets --locked -- -D warnings`, `cargo test --locked` | PASS |
| Release `--chest-tour` in tt area 27 (`ch10`) | PASS: the camera frames Sora and the chest through the flourish; "OBTAINED", Tower Map, its picture and "A map of the area." in the lower box; se000 #10 at the window, #24 at the confirm |
| The room at tt07 program 95 (`ev6`) | PASS: the mission's script no longer stops on 1:296 |
| Side by side with the installed game | NOT RUN |

## Uncertainty

- [I] The camera's base is the chest's yaw plus half a turn: the first run, with the mission overlay's rule alone, put the camera behind the tower's
  wall, and this one frames the scene. +0x808's matrix was not read, so the half turn is fitted to that result.
- [I] Event frames run at half the tick rate (90 frames over the 180-frame clip).
- Sora is not attached at bone 0x4002 as the executable does, and the world is not frozen under the window. Whether state 7 freezes it was not read.
- Other parts of the event (op 0x10's fade-in) are not played.
- A decide press that closes the window also reaches the field.
- The Tutorial widget itself is not built.

## Verdict

PASS for the big chest in the room, apart from the items above. #67's chest item is done. No claim about the installed game.

## Next

The rest of the pass.
