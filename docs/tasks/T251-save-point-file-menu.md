# T251 — The save point opens the Save file menu in the field (issues #139, #79, part)

Owner Claude, 2026-10-26, requested by burba in the project chat ("continue working on more
issues"). Gate: development of the test room only; no final gameplay acceptance is inferred.
Prerequisites: T155 (save container, file menu), T211 (save points and the "Save" reaction).
Build identity: local `main` after `d03170f`, package 0.1.2, Rust 1.99.0 Windows MSVC. Source:
the selected Steam KH2FM build, validated each run; the install was only read.

Evidence: unattended runs `save1` and `save2` (Twilight Town area 9) in the private
`kh2-issues-out/`.

## What the executable does

Choosing "Save" at a save point (reaction command 0x37) runs `FUN_1403a8b10` case 0x37, which
opens the file module's save point screen (`FUN_1403a5650(7)`, `FUN_1402e4950`, flow
`FUN_1402e78e0`): the Save / Title Screen rows (a world point adds World Map), the 99-slot list,
the overwrite question, the saving message.

## What the room did

The press only logged `save_menu_requested`; the file menu ran only on the title screen and in
its `--autoplay-savepoint` capture.

## Rewrite

- The camp hosts the save point's file menu: on "Save" it reads `menu/us/save.2ld`, the texts
  and the place names the first time, opens `Kind::SavePoint { world_map: false }` and feeds it
  the camp's pad while the field waits (like the camp).
- Interactive runs save into the rewrite's own container under the data folder (the original's
  file is only read, as on the title); unattended runs use the generated container in memory.
- What a save holds is a first snapshot: the save point's world and area, Sora's level, the
  play time (room ticks) and the difficulty, sealed. The rest of the state is #79's open line.
- The file cards refresh after a save or a delete (`Menu::revision`); before, the saved slot
  kept showing "Save new file." until the cursor moved.
- `--save-point-tour` now goes on into the menu: Save, up to the empty slot, save, back out;
  its verdict requires one written file.

## Checks

| Check | Result |
| --- | --- |
| `cargo fmt --all --check`, `cargo clippy --all-targets --locked -- -D warnings`, `cargo build --locked`, smoke | PASS |
| `cargo test --locked` | PASS (the save test also checks the revision) |
| Release `--save-point-tour` on Twilight Town area 9 (private `save2`) | PASS: the menu opens the tick after the press with se000 #11, Save → list (#2), up (#1), save into slot 08 (#2, then #12 at 449), back twice (#3), closed at 607; `save_point_tour_verdict` pass; the capture after the save shows slot 08 as "Twilight Town / Central Station, LV 1, 0 Munny, 0:00" |
| Release `--save-point-tour` before the card refresh (private `save1`) | the saved slot still read "Save new file." (fixed above) |

## Uncertainty

- Field save points have no World Map row (`world_map: false`): the save point object opens the
  menu with mode 2, which hides it, and only WORLD_POINT uses 0 (`FUN_1402eb490`, private
  `save-20261016/save-spec.md` §4-5).
- Title Screen and World Map answers close the menu and are only logged: the room cannot go back
  to the title or jump to the world map.
- The message window art came in T253 and the save point's glow in T252.
- Only the card's fields are captured; loading such a save restores nothing yet (#79).
- Not compared with the installed game.

Verdict: PARTIAL for #139 and #79: the field save point opens a working Save menu that writes
the rewrite's own file; the full state capture remains (the glow is T252, the window art T253).
Next: capture the live state into the save image (#79).
