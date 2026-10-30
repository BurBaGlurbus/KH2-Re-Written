# T297 — Journal scroll bars and treasure seen bits; the townspeople's other programs

Owner Claude, 2026-10-30, requested by burba in the project thread ("start working on the github issues", then "continue").
Issues #69, #161, #24. Gate: development of the pause menu and the townspeople; no final gate is inferred. Prerequisites: T164
(Journal pages), T267 (the Journal row's badge), T163 (townspeople push and fade). Build identity: local `main` after `c637b9a`,
package 0.1.2. Source: the selected Steam KH2FM build, validated each run; the install was only read.

Private evidence (outside the checkout): `research\playable-room-20261008\journal-03\files-treasures-spec.md` §1.5, §3.7, §4;
`journal-04\other-pages-spec.md`; `tt-world-01\rtn-02\rtn-programs-spec.md`; a run under `research\issues-20261030\runs\`.

## What the executable does

- **Scroll bars.** Every Journal list and reader has the bar object `DAT_140bee5d8` (FUN_1402fa930, FUN_1402facd0) on the page's
  track sprite, groups 139 / 138 / 140 from table 0x1405baad0. List pages: total max(n, 10), visible 10. Treasures: total rows,
  visible 9. Readers: total lines, visible 9. Story: total L0 + max(10, the newest entry's lines), visible 10.
- **Chest and map seen bits.** FUN_140345420's chest bit: a `trsr` row of type 0 with a real flag that is set, in an unlocked
  world, not seen in save+0x4d24. Map bit: the k-th map row, its item owned, not seen in save+0x4d64. A world's rows are marked
  seen together when the cursor rests on it (FUN_1403467a0, FUN_1403464a0). The page's NEW mark needs a row new at the opening,
  and dims once all of those are seen (FUN_140347c80 / bb0, FUN_140347360 / 290).
- **Programs.** rtn_sit, rtn_lm and n_ex740_skate_r share one code family: no turn-in-place motion, the walk motion always on a
  route, `look` turns only past 30°. rtn_sit also sets flag 4 (the talk doesn't turn it to face Sora); rtn_lm and the 740 skater
  set flags 3 and 5 (route flattened to its first point's height; physics mode 5). The other skaters and the tutor run
  rtn_default's acts unchanged.

## Change

- `Journal::bar_in`, `bar_to`; every list page and reader (the Character Files lists also get their missing track art, G95).
- `JournalData::seen_chests`, `seen_maps`, `chest_available`, `map_rows`, in `any_new`; the Treasures page marks by the rules
  above (it used to show a mark on every unlocked world) and keeps the seen rows between openings through `CampRes`.
- `npcs::Program` from the name at the start of the model's BAR type-3 program, OR-ed into the entity's flags in `spawn`;
  `FieldNpc::plain`, `keeps_facing`; the family's look, encounter and path steps.

## Checks

| Check | Result |
| --- | --- |
| Unit tests: the chest and map bits in the badge; the program name picks the family | PASS |
| Release `--journal-tour`: Limits, Minigames, Missions, Story, reader pages show the bar on the track, the thumb moves with the rows | PASS |
| Treasures page marks in a run | NOT RUN: the tour's journal has no chest flags |
| tt13's Zipper and tt06's 740 skaters on screen | NOT RUN unattended: they need story programs the room's tours don't reach |
| Side by side with the installed game | NOT RUN |

## Uncertainty

- Physics mode 5 is not modelled: whether it adds movement to flag-5 walkers is [U] in the spec. The Zipper walks by its root
  travel; the 740 skaters have none, so they stay where they spawn and turn along the path, as the spec expects.
- The talk camera flag (`(ccc & 0x50) == 0`) is not read.
- Not ported: the skaters' trigger 11 (dormant in Twilight Town's data), and the tutor's signals 39 / 40.
- Rows without a Journal entry (fixed rtn_sit NPCs) idle as before.

## Verdict

PASS for the scroll bars. PASS for the program family in unit tests only. PARTIAL for #69 and #161, which have other items.
