# T329 â€” The camp's Status lists the forms and Summons

Owner Claude, 2026-11-01, requested by burba in the project thread ("work on issues on the github"). Issue #161 (part). Gate:
development of the camp in the test room; no final gate is inferred. Prerequisites: T255, T323, T327. Build identity: local `main`
after `180a3af` plus this change, package 0.1.2. Source: the selected Steam KH2FM build, validated each run; the install was only
read.

Private evidence (outside the checkout): `research\playable-room-20261008\camp-04\status-config-spec.md` Â§2.2 to Â§2.5,
`research\issues-20261031\g\c1\`, `c2.txt`, `c3\` (FUN_140369520, FUN_14036aba0, FUN_14036ab00, FUN_14036a260, FUN_1403e87f0,
FUN_1403e88f0, FUN_1403e8a00, FUN_14040d680, FUN_140353f10 and the tables at 0x1405bbf88..0x1405bc0c0); runs `c7`, `c8` under
`C:\Users\ANIMA\kh2-issues-out\s79\`.

## What the executable does

The list runs Sora, the owned forms, Summon when a summon is owned, then the party. A form's page shows Form LV, Experience
(FUN_1403e87f0: the record's plus the levels below it), Next LV up to the cap (FUN_1403e8a00), Form Gauge (level + 2) and Sora's
fighting numbers with that form's record linked (FUN_1403c03f0). Summon's page is the same over its own level and EXP, with the
owned summons named and no value. Sora's own page gains Drive Gauge and the form levels. FUN_140369520 marks an owned form or
summon new while its seen bit (save+0x4274) is clear; FUN_140353f10 sets a form's bit when the cursor rests on its row, and a
summon's when the cursor is on its line in the right column.

## Change

- `test_room::status_forms`: the numbers from the room, the rows with their messages and helps.
- `pause_menu`: the rows' NEW! badges (G50 then G49), the seen bits, and the Status row's badge on the camp's top level.
- `save+0x4274` is kept in `CampRes::status_seen` and saved. A fresh room's forms come from its development grant and never show
  NEW!; a loaded save shows what its bits say.
- Sora's Experience and Bonus LV rows show real numbers (they read 0).
- A save holds the items of the forms the room owns, so a loaded room keeps them (T323 lost them).
- The camp's clock runs from the save's play time; it restarted at 0 after a load.
- `--party-camp-tour` also rests on a form's row.

## Checks

| Check | Result |
| --- | --- |
| `cargo test --locked -p main --lib -- status_forms game_save` (EXP sums, the cap, rows on generated numbers) | PASS |
| Release `--party-camp-tour` on a loaded save with the seen bits clear (`c7`, `c8`) | PASS: Valor to Final listed with NEW!, dimming as the cursor passes; Final's page shows Form LV 1, Experience 0, Next LV 12, Form Gauge 3, Strength 8 with its Keyblade; its help line reads from the install |
| Summon's row in a run | NOT RUN (the room's stock owns no summon item; only the unit test covers it) |
| Side by side with the installed game | NOT RUN |

## Uncertainty

- The Summon level and EXP are read from the save's bytes; nothing in the room raises them.
- [I] Worlds 10 and 11 are taken as the worlds that hide the forms (0x1405bac00).

## Verdict

PARTIAL for #161: the Status rows are in; the Drive forms' and Lion's rows in Abilities stay open.

## Next

The Abilities page's form rows, then the rest of #69.
