# T343 — The enemy level from the story's `btlv` rows; `dsa` op 2's second message

Owner Claude, 2026-11-01, requested by burba in the project thread ("commit to local main and continue working on issues on guthub"). Issue #162
(parts). Gate: development of the field and world map; no final gate is inferred. Prerequisites: T173, T269, T323. Build identity: local `main`
after `7f3d659` plus this change, package 0.1.2. Source: the selected Steam KH2FM build, validated each run; the install was only read. Plain
Ghidra 12.1.4 headless on read-only project copies.

Private evidence (outside the checkout): `research\issues-20261101\small\small-spec.md`, `small\dec\` (FUN_1403ee4a0, FUN_1403ee1d0, FUN_1403ee560,
FUN_14039b0e4); runs `lv1`, `lv2` under `%USERPROFILE%\kh2-issues-out\s161\`.

## What the executable does

- Every flag set runs FUN_1403ee4a0: each `btlv` record whose flag matches sets its bit at save+0x3724. The bits stay when a script clears the flag.
- FUN_1403ee1d0, at the end of the area loader: the area's op 0x1e level, or else the sum of the set records' levels for the current world; 0
  becomes 1. FUN_1403ee560 gives the world map's panel the same sums.
- `dsa` op 2 passes three words (FUN_14039b0e4): the entry, its message, a second message.

## Changes

- `Progress`: `BattleRow`, `read_battle_rows`, `set_battle_rows`, `battle_sums`, `battle_level`; the bits are saved and loaded (`state::BATTLE_BITS`).
- The room takes its enemy level from it at the start and on every area change. Without an area the config's level still stands in.
- The world map's panel reads the bits, not the flags.
- `dsa` op 2 stores the second message, which `blocked_message` reads.
- `area_start` logs `enemy_level`.

## Checks

| Check | Result |
| --- | --- |
| `cargo clippy --all-targets --locked -- -D warnings`, `cargo test --locked` | PASS |
| Unit test: the sum, the floor, op 0x1e winning, a bit staying | PASS |
| Release runs (`lv1`, `lv2`) | PASS: tt area 9 with the room's story flags gives `enemy_level` 6 with no op 0x1e; hb area 1 with no story flags gives the floor, 1 |
| Side by side with the installed game | NOT RUN |

## Uncertainty

- The width of the bit array is taken from the highest bit in the table; the executable's own width was not read.
- An enemy already standing keeps the level it was built with until its area reloads.
- The rest of FUN_14039afc0's tail (Torn Page pairs, the Atlantica chain) and FUN_1403e66c0 need item and level access in `Progress`. They only
  matter once events set flags, so they wait for #25.

## Verdict

PASS for the level and the message. PARTIAL for #162.

## Next

The flag tail with #25.
