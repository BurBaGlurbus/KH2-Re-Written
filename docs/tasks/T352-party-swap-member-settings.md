# T352 — A party swap waits for the next area; guests' settings; cursor memory

Owner Claude, 2026-11-01, requested by burba in the project thread ("work through issues and close them"). Issue #34 (parts). Gate: development of the camp and party in the
test room; no final gate is inferred. Prerequisites: T220, T325, T342. Build identity: local `main` after T351 plus this change, package 0.1.2. Source: the selected Steam KH2FM
build, validated each run; the install was only read. Plain Ghidra 12.1.4 headless on a read-only project copy.

Private evidence (outside the checkout): `research\issues-20261102\party34-spec.md` sections A and G; runs `gb`, `pc1` under `%USERPROFILE%\kh2-issues-out\s162\`.

## What the executable does

- A swap of slots 1 and 2 writes only the row's bytes (FUN_14036b460). The field's list keeps its order until the next area builds it: only FUN_1403c2db0 adds to it, and the
  follow angle (+0xdbc) and gauges (FUN_1401b7f70) read it. T342 had this as [I]; it is now read from code.
- A reopened camp (FUN_14036b660, FUN_140304ea0) and menu 8 (FUN_1403f29e0) read the row, so they show the new order at once.
- Guests have frequencies too: the table at 0x1407249d0 holds 29 rows, not the 7 the rewrite listed.
- A guest's act is gated by its ability (FUN_1401cccf0, FUN_1401cd270). The rewrite said it was not.
- Leaving Party with a changed order zeroes the cursor memory (FUN_14031f650, FUN_140305ea0). So does opening the camp with a changed party count, forms held, Pride Lands, world or
  row against the last close (FUN_1403030e0).
- Party's help goes by count alone (FUN_140320b30).

## Changes

- `camp_tick`: the close no longer moves the room's members. The camp builds its party in the row's order, with `Data::party_models` mapping each card to its actor.
- `second_deck::room`: the members in the row's order.
- `party_members::frequency_index`: the whole table. `Record::ability`, `record_frequency`; `party_roster` names each act's ability.
- `Memory::clear`, `party_left`, `CampRes::memory_key`, `memory_row`.
- `party.rs`: the help by count.
- `--party-camp-tour` reopens the camp after the close; a `party_row` line logs the row and the field.

## Checks

| Check | Result |
| --- | --- |
| `cargo fmt --all --check`, `cargo clippy --all-targets --locked -- -D warnings`, `cargo test --locked` | PASS |
| Release `--party-camp-tour` (`pc1`) | PASS: `party_row` row [0,2,1,18] with field [2,3]; reopened, the panels read Sora, Goofy, Donald with their own models; the verdict passes |
| `KH2RW_PARTY=P_BB000,P_EX030` with `--enemy-combat-tour` (`gb`) | PASS: Beast still acts under the gate: `party_bash` rush x10. His `shout` didn't come in this run |
| Side by side with the installed game | NOT RUN |

## Uncertainty

- [U] Riku's abilities 0x1b4 and 0x1b6 have no entry of his class in the scan. They are in the frequency table but gate nothing.
- A guest whose record doesn't equip an ability stops using that act, as the code says. Which plrp rows equip which wasn't listed row by row.
- The "same row" test compares types, as FUN_1403e3670 does, not lock bits.

## Verdict

PASS for these parts. #34 stays open for the guests themselves and the event notice.

## Next

Guests: the reserve card, actors from the row, the hot swap, then the 0x15 notice.
