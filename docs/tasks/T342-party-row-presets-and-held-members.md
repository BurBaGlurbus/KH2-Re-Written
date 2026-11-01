# T342 — The party row keeps a swap through op 0x0f; held members are locked

Owner Claude, 2026-11-01, requested by burba in the project thread ("commit to local main and continue working on issues on guthub"). Issue #34
(parts). Gate: development of the party in the test room; no final gate is inferred. Prerequisites: T325, T328. Build identity: local `main`
after `7f3d659` plus this change, package 0.1.2. Source: the selected Steam KH2FM build, validated each run; the install was only read. Plain
Ghidra 12.1.4 headless on read-only project copies.

Private evidence (outside the checkout): `research\issues-20261101\small\small-spec.md` with its decompiles; run `pc1` under
`%USERPROFILE%\kh2-issues-out\s161\`.

## What the executable does

- ARD op 0x0f applies its preset over the world's own row (FUN_1403e31f0, an unset row starting as FUN_1403e2e10's), so members already in the
  row keep their order. The room started from a new row each time and lost a Party swap at the next area with the op: 889 programs hold one.
- FUN_140400880's list, which T325 could not place, is the members a Drive form or Summon took out of the field (FUN_140400990, FUN_140400a30,
  FUN_140400bb0). FUN_1403e37c0 counts them as locked, so the Party screen goes dark for them.

## Changes

- `Members::presets`, `game_save::row_with_presets`, applied on an area's arrival.
- `game_save::party_locked` takes the held members' types, from `MemberRt::stored`.

## Checks

| Check | Result |
| --- | --- |
| `cargo clippy --all-targets --locked -- -D warnings`, `cargo test --locked` | PASS |
| Unit test: a swapped row keeps its order through a preset; an unset row; a held member is locked | PASS |
| Release `--party-camp-tour` (`pc1`) | PASS for no regression: Donald and Goofy still change places when the camp closes |
| A swap surviving a door in a run | NOT RUN: only the unit test |
| The dark card while a form holds a member | NOT RUN: no tour opens the Party screen in a form |
| Side by side with the installed game | NOT RUN |

## Uncertainty

- The research found ([I]) that a swap of slots 1 and 2 moves nothing in the field until the next area load. I did not change this: the room's
  camp builds its pages from the room's members, so leaving them would show the old order when the camp reopens and swap the row back. It needs
  the camp to read the row first.
- Guests and the event camp's 0x15 notice stay open: the guest needs the reserve card and a preloaded actor, the notice needs event-end jumps.
  FUN_14036b400 is not a stock notice, as #34 said: it replaces the field object at once.

## Verdict

PASS for the two rules. PARTIAL for #34.

## Next

Guests in slot 3.
