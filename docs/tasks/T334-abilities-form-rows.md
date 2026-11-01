# T334 — The camp's Abilities lists the Drive forms and Lion

Owner Claude, 2026-11-01, requested by burba in the project thread ("get more issues on github done"). Issue #161 (last item). Gate:
development of the camp in the test room; no final gate is inferred. Prerequisites: T255, T323, T329. Build identity: local `main`
after `812ffbd` plus this change, package 0.1.2. Source: the selected Steam KH2FM build, validated each run; the install was only read.
Plain Ghidra 12.1.4 headless on a read-only project copy.

Private evidence (outside the checkout): `research\playable-room-20261008\camp-06\abilities-spec.md` §1.3, §1.5, §2.2 to §2.6;
`research\issues-20261101\g\a1\` (FUN_140368ed0, FUN_14036a500, FUN_14036a2b0, FUN_14036a0b0, FUN_140360cb0, FUN_140361200,
FUN_14035f130, FUN_140363c70, FUN_140363770); runs `t2`, `t3` under `%USERPROFILE%\kh2-issues-out\s161\`.

## What the executable does

- The list runs Sora, the owned forms (FUN_1402fc500: none in worlds 10 and 11), Lion in world 10, then the party. Form rows use groups
  129, 130, 131 and 128 and are indented; their help is message 0xbb8c.
- A form's row lists Sora's equipped supports (with their NEW! marks on Sora's list), Sora's weapon, the form's own weapon (Valor, Master,
  Final) and the form record's 24 slots at no cost. There is no AP gauge, AP cell, △ or popup.
- FUN_140368ed0 keeps an obtained-order list per form at save+0x4798 + 0x60·block, with seen bits at +0x424c, bit 0x30·block + k. Lion uses
  block 5. A list already full when the camp opens gains no new marks.
- L2 and R2 step through every row, forms included (FUN_14035f130).

## Changes

- `pause_menu::abilities`: `FormAbilities`, `form_entries`, row mapping, the view-only rules, the rows' art and help, NEW! marks by owner.
- The top level's Abilities badge also counts the forms (FUN_140306790).
- Fixed: T132 listed every form weapon's ability on Sora's own row. FUN_140361200 lists a form's weapon only on that form's row.
- `new_marks`: the full-list rule, which also applies to Sora and the party.
- `test_room::status_forms::ability_forms`, `CampRes::form_marks`, saved and loaded by `game_save` (`state::ORDER_FORMS`,
  `state::SEEN_FORM_ABILITIES`).
- `--party-camp-tour` also opens the last form's row; its timing for Donald's row now allows for the extra rows.

## Checks

| Check | Result |
| --- | --- |
| `cargo fmt` on the changed files, `cargo clippy --all-targets --locked -- -D warnings`, `cargo test --locked` | PASS |
| New unit tests: a form's entries and their order, the full-list rule, the form marks' save bytes | PASS |
| Release `--party-camp-tour`, fresh room (`t3`) | PASS: Valor to Final listed and indented with NEW!; Final's preview, help line and list (Action, Growth, Support) with no gauge, AP cells or △; the cursor's mark dims; R2 reaches Donald with his gauge back |
| Lion's row | NOT RUN: no tour stands in world 10 with Lion's record filled |
| Side by side with the installed game | NOT RUN |

## Uncertainty

- A fresh room's forms come from its development grant, so all their abilities read NEW! the first time. A loaded save reads its own bits.
- The world map's camp does not list forms, as with its Equip page (`CampSide` is empty there).

## Verdict

PASS for building and running the rows in the room. #161's three items are built. No claim about the installed game's look.

## Next

The rest of the pass: #17, #67, #69.
