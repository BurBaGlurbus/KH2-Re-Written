# T354 — World guests: the reserve card, the swap at the camp's close, members from the row

Owner Claude, 2026-11-02, requested by burba in the project thread ("work through issues and close them"). Issue #34 (parts). Gate: development of the camp and party in the test room; no final
gate is inferred. Prerequisites: T325, T342, T352. Build identity: local `main` after T353 plus this change, package 0.1.2. Source: the selected Steam KH2FM build, validated each run; the install was
only read. Plain Ghidra 12.1.4 headless on a read-only project copy.

Private evidence (outside the checkout): `research\issues-20261102\party34-spec.md` sections B, C, D; runs `pg1`, `pg2`, `pc2` under `%USERPROFILE%\kh2-issues-out\s162\`; private configuration
`area-bb02-guest.json`.

## What the executable does

- The party list holds the row's fourth member too; the card's colour follows its position, and the top frame caps at three.
- The Party screen edits only the camp's copy. At the close FUN_14036b460 writes the row; if the reserve came out at the player's own camp, FUN_14036b400 replaces the leaver's field object:
  nothing is preloaded, the object loads inside that task, and FUN_1403e33f0 swaps the bytes and drops both lock bits. An event's camp swaps bytes only.
- The newcomer takes the leaver's list index, so its follow angle and gauge row (FUN_1403c2db0). FUN_1403c2fc0 puts it at the leaver's place when the line to Sora is clear, else 200 to 300 from him.
- A camp model comes from the row's member object (FUN_1403dfeb0), loaded when the screen asks; a card shows none until then.
- Each area load builds the field from the row, with op 0x0f applied over it (FUN_1403e2eb0, FUN_1403e31f0).

## Changes

- `game_save::write_party_order(row, camp, field_camp)`, `camp_row`, `to_reserve`, with the five orders worked by hand as a unit test. `load_reserve`, `store_reserve`, `member_gear`.
- `party_room::Reserve`, `Game::reserve`, `make_reserve`, `keep_reserve`, `keep_actors`, `start_replace`, `finish_replace`. `MemberRt::replacing`, finished in `swap_weapons`.
- `camp_data`: the reserve joins the party as a fourth member; the close writes its record and runs the replacement. The capture saves its record.
- `menu_friend::sync`, `load_object`, `spawn_menu_slot`, `spawn_menu_parts`: members' camp models follow the field and the reserve at run time.
- `chest::sync_holdings`: the first area's preset applies to the row, as an arrival's does; `keep_reserve` and `keep_actors` run each tick.
- `Members::objects`; the name table for all twelve members; `party_camp_tour` takes `KH2RW_TOUR_KEYS` and `KH2RW_TOUR_CAPTURES`.

## Checks

| Check | Result |
| --- | --- |
| `cargo fmt --all --check`, `cargo clippy --all-targets --locked -- -D warnings`, `cargo test --locked` | PASS |
| Unit tests: the five camp orders, an event's bytes-only swap, lock bits dropped, no reserve | PASS |
| bb area 2 program 1, preset 2 (`pg2`) | PASS: row [0,3,2,1], Beast and Goofy in the field, `party_reserve` Donald; top frame Sora, Beast, Goofy with four panels; Party screen with Donald's dark blue fourth card and his model |
| The swap, the same run | PASS: choosing Goofy and then Donald, the cards swap; at the close row [0,3,1,2], `party_body` P_EX020, `party_replace` slot 1 at the leaver's place; HUD faces Beast's slot unchanged and Donald's in the leaver's row |
| `--party-camp-tour` with Donald and Goofy (`pc2`) | PASS: no reserve, swap as T352, verdict passes |
| Side by side with the installed game | NOT RUN |

## Uncertainty

- The camp closes at once; the executable holds it open until the object has loaded. So the old member stands in the field for up to a second.
- A replaced member has no battle voices until the next room load: voice banks are built at the room's start.
- `keep_actors` applies a row whose active set differs from the field while the room runs, including after a door. Fewer members than actors leaves the extra actor standing.
- Character 8's hidden parts on the camp model aren't built. Whether every guest's set holds motion 0x8b is reported by a run, not checked for all.
- Party command 0x49 and Auto Change use the same replace with a fade and aren't built. The second deck's row colour doesn't look at the reserve.
- A reserve's level stays 1 on the camp's panels, as Donald's and Goofy's do.

## Verdict

PASS in the room for the guests' card, swap and row. #34 stays open for the 0x15 notice.

## Next

The event camp's 0x15 notice.
