# T336 — The party's Second Chance and Once More, Sora's KO voice, the guard's bounce and recoil

Owner Claude, 2026-11-01, requested by burba in the project thread ("get more issues on github done"). Issues #51 and #67 (parts). Gate:
development of combat in the test room; no final gate is inferred. Prerequisites: T140, T186, T327. Build identity: local `main` after
`be25005` plus this change, package 0.1.2. Source: the selected Steam KH2FM build, validated each run; the install was only read. Plain
Ghidra 12.1.4 headless on read-only project copies.

Private evidence (outside the checkout): `research\issues-20261101\ko\ko-spec.md` with its decompiles, `research\issues-20261101\g\c1\`
(FUN_1403ce580, FUN_1403d5dc0, FUN_140404d00); runs `ph1`, `go1` under `%USERPROFILE%\kh2-issues-out\s161\`.

## What the executable does

- **Second Chance and Once More** (FUN_1403d6670) are read only there, through a thunk in every player class. Second Chance: bit 127, floor
  0, HP above 1. Once More: bit 128, motion 10..39 before this hit's reaction. A veto leaves 1 HP (FUN_1403d3ba0). Neither starts an effect
  or a sound, and neither has a cooldown. So the "Second Chance and Once More effects" in #67 do not exist.
- **Sora's KO** (FUN_140404d60 → FUN_1403a81c0 → FUN_1403d5dc0): voice 2, skipped in Antiform. No effect element on Sora; element 14 is the
  friends' only.
- **Guard** (FUN_1403d61d0): a refact field of 2 passes with element 13. On the ground with a refact field: element 11 for the player
  character, 12 for anyone else, motion 42 and a slide of 200. Otherwise element 13 and FUN_1403d4650's bounce.

## Changes

- `combat::sora_veto` is now `kill_veto`, and `party_room::take_hit` uses it with the member's own ability record and motion.
- Sora's voice 2 as he dies.
- `Player::guard_bounce`, sharing `wall_recoil`'s motion and slide but leaving the combo alone, in the room's and the battle enemies' guard
  paths. It was marked "not loaded in this room" and was only a spark.
- A member's guard takes element 12 and the recoil (`Follower::guard_recoil`, `party_hit::guard_recoil`) when a refact field asks.
- `--party-hit-tour` gains a Second Chance case.

## Checks

| Check | Result |
| --- | --- |
| `cargo clippy --all-targets --locked -- -D warnings`, `cargo test --locked` | PASS |
| New unit tests: the bounce leaves the combo alone, the member recoil | PASS |
| Release `--party-hit-tour` (`ph1`) | PASS: Goofy at 2 HP with bit 127 struck by a claw is left at 1 HP, not down; the KO cases still KO at 1 HP; `party_guard` still bounces on motion 41 |
| Release `--game-over-tour` (`go1`) | PASS: one voice at the KO, sound 8 of the bank |
| Member recoil and element 12 in a run | NOT RUN: Goofy's guard ATKP 139 has refact 0, so nothing reaches that branch yet |
| Sora's guard bounce in a run | NOT RUN: no tour clashes with a refact-0 guard on Sora |
| Side by side with the installed game | NOT RUN |

## Uncertainty

- [I] The DEAD motion's voice 1 on the same tick gives way to voice 2: FUN_1403ce580 keeps the higher priority's pending request (4 against
  3). Whether the request is consumed between the two calls was not read, so the game may play both.
- Still not built, from the research: the attacker's own guard spark (FUN_1403d10c0), the once-per-pair gate (bit 0x80 of the attack),
  positional element sounds, Tinker Bell's and Mickey's revives of Sora, the pad vibration triggers.
- `test_room.rs` still stands a dead Sora up on any heal, which the executable refuses (0x6c8 bit 14 is the friends'). It is left, because
  the room's own refill without a game over depends on it.

## Verdict

PASS for what the tours reach. PARTIAL for #51 and #67.

## Next

The box flight (#17), then the chest research for #67.
