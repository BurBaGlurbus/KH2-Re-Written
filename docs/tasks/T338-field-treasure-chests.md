# T338 — Field treasure chests

Owner Claude, 2026-11-01, requested by burba in the project thread ("get more issues on github done"). Issues #67 and #79 (parts). Gate:
development of the field in the test room; no final gate is inferred. Prerequisites: T211, T323, T337. Build identity: local `main` after
`72f7c23` plus this change, package 0.1.2. Source: the selected Steam KH2FM build, validated each run; the install was only read. Plain
Ghidra 12.1.4 headless on read-only project copies.

Private evidence (outside the checkout): `research\issues-20261101\chest\chest-spec.md` with its decompiles and data dumps; runs `ch4` to
`ch7` under `%USERPROFILE%\kh2-issues-out\s161\`.

## What the executable does

- A chest is objentry type 10, class YS::TREASURE_BOX, with no script and no per-tick code. Spawn sets `m_70` and `m_71` place them. The
  `trsr` row is found by world, area and the entity's arg1 (FUN_140402200, FUN_1403a19c0). An opened chest stands on motion 152 with its
  command on the kill list.
- The offer is the ordinary reaction search: REACTION shape, argument 32, radius 150, its centre carried in `position` y. Press lands in
  FUN_1403a8b10 case 0x20 → FUN_1404025c0.
- Open: the chest plays motion 153, Sora the capture motion from slot 252 (small: blend 8, 60 frames, then motion 0). The same tick
  FUN_1403a1b60 gives the item and sets its bit at save+0x23ac. A full stock loses the item, and the bit and window still happen.
- The small chest's window (dk::TREASURE_BOX_INFO, FUN_140159550, FUN_140159970): 60 ticks later, se000 #10, `gaug` group 0x4f with the
  item's name and `ipic` group 6's picture, 85 to the left, 100 frames, no pause and no button.
- The opening motion's triggers: se000 #22 at frame 28 and effect 0 at frame 34 (small).

## Changes

- `crate::chests`: `ChestKind` (model, poses, triggers, shapes, effects), `Chest`, the `trsr` lookup. `journal_table::Treasure` gains +6 and +7.
- `area.rs`: `LoadedArea::chests` from the area's type-10 spawns.
- `test_room::chest`: the save's bit decides the pose, the press opens it, the triggers play, Sora turns and is pushed off the body.
- `reaction_command.rs`: the closed chests' shapes and the press. [I] The chest's hold is dropped because its command is on the kill list.
- `boss::capture_motions` keeps the generic `F_EX030` and `F_EX040` captures in Sora's kit, and a worn body (T332) takes them too.
- `visual::treasure_info`: the box window with its picture, drawn beside the prize window.
- `--chest-tour <dir>`: Sora before the area's first closed chest, pressing at tick 60.

## Checks

| Check | Result |
| --- | --- |
| `cargo clippy --all-targets --locked -- -D warnings`, `cargo test --locked` | PASS |
| Unit tests: the `trsr` lookup, the turn | PASS |
| Release `--chest-tour` in tt area 9 (`ch4`) | PASS: "Open" offered, Sora swings on the capture, the lid opens with the sparkle and se000 #22, "OBTAINED" + Tent's picture and name at the top left, stock +1, bit 304 |
| Release `--chest-tour` on an image with bit 304 set (`ch7`) | PASS: that chest is not offered, the tour opens the next one (bit 306, item 345) |
| Big chest in tt area 27 (`ch6`) | PARTIAL: "Open", Sora's long capture, the lid, item 533's name; see Uncertainty |
| Writing the bit into a save file in a run | NOT RUN: the capture starts from the same base image the bit is written to |
| Side by side with the installed game | NOT RUN |

## Uncertainty

- The big chest is only part built: Sora's capture and the opening play, and its item shows in the small window. NOT built: the attach at
  bone 0x4002, its 90-frame camera event, op 0x3b's state 7 window (dk::TREASURE_EVENT_INFO) with the description and a confirm.
- Lion's chests (F_EX050, F_EX060) open with no Sora motion: their captures are for another skeleton. A Drive form's capture variant was
  not traced, so a form opens a chest with no motion too.
- [I] Kinds 1 and 7 of the chest's shapes: the room pushes Sora off kind 1 as a solid body. The executable's use was not traced.
- [I] The turn's rate: 15 degrees a tick over 12 ticks, from the range 22 parameter.
- Consumables go into the stock. FUN_1403c4d50 puts them in Sora's empty item slots first.
- `save+0x3d88` (a non-key item's bit) is not written.
- Eleven spawns have no row (he15, mu01, mu07): they log and give nothing, where the game gives Land of Dragons' first and closes again.

## Verdict

PASS for small chests in the room. PARTIAL for big ones. No claim about the installed game.

## Next

Big chests' window and camera; the next areas' checks by the owner.
