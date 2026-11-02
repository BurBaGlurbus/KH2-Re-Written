# T349 — A party weapon's motion set comes from the body's row

Owner Claude, 2026-11-01, requested by burba in the project thread ("work through issues and close them"). Issue #51. Gate: development of the party in the test
room; no final gate is inferred. Prerequisites: T220, T247, T248, T327, T336. Build identity: local `main` after T348 plus this change, package 0.1.2. Source: the
selected Steam KH2FM build, validated each run; the install was only read. Plain Ghidra 12.1.4 headless on a read-only project copy.

Private evidence (outside the checkout): `research\issues-20261102\lod51-spec.md` section 5; runs `w1`, `w2` under `%USERPROFILE%\kh2-issues-out\s162\`; private
configuration `area-lk05.json`.

## What the executable does

FUN_1403ef750: the set is `obj/%s.mset` of `wmst` row (the owner's objentry +0x4e) x 2 + slot. The weapon object plays no part in it: FUN_1403ef8b0 only picks the
model. An empty row gives no motions. T248's borrowing gave the right answer for Donald's and Goofy's ordinary bodies by the names alone. It was wrong for P_LK020,
P_LK030, P_MU010 and P_EH000_SIDECAR / LAST.

## Changes

- `weapon_entry::motion_set_name`, used by Sora's kit and the party's.
- `party_room` takes the row. A body with no row (the debug Outfits page) keeps the old chain, marked [I].
- Each member's load prints its weapon and set.

## Checks

| Check | Result |
| --- | --- |
| `cargo fmt --all --check`, `cargo clippy --all-targets --locked -- -D warnings`, `cargo test --locked` | PASS |
| Unit test: row by set and slot, empty row, past the table | PASS |
| Release `--party-camp-tour` (`w1`) | PASS: Donald on W_EX020, then with Hammer Staff's W_EX020_10 still on set W_EX020; Goofy on W_EX030 |
| Release room in Pride Lands, area 5 (`w2`) | PASS: P_LK020 on W_EX020_LK and P_LK030 on W_EX030_LK. The room stands 200 frames; their weapons' motions were not watched |
| Side by side with the installed game | NOT RUN |

## Uncertainty

- Slot 1 rows (Mulan's N_MU080_PLY, Ping's) are not in the room; no source mentions them.

## Verdict

PASS. #51's list is done.

## Next

The rest of the batch.
