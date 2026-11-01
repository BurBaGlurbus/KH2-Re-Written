# T337 — A dropped box falls, bounces and flies; its name window; Donald and Goofy collect prizes

Owner Claude, 2026-11-01, requested by burba in the project thread ("get more issues on github done"). Issue #17. Gate: development of
combat in the test room; no final gate is inferred. Prerequisites: T161, T225, T228. Build identity: local `main` after `07650a1` plus this
change, package 0.1.2. Source: the selected Steam KH2FM build, validated each run; the install was only read. Plain Ghidra 12.1.4 headless
on read-only project copies.

Private evidence (outside the checkout): `research\issues-20261101\box\box-spec.md` with its decompiles; run `pz2` under
`%USERPROFILE%\kh2-issues-out\s161\`.

## What the executable does

- **Fall** (YS::ACTION_PRIZEBOX, FUN_1403f8950, FUN_1403f8500, FUN_1403f88c0): thrown ±5 across and 10..15 up. Gravity 1.5, capped at 25.
  Drag 0.97. A landing above 2.5 bounces at ×0.8, and the first two bounces play sound 14. It can be collected once it starts to fall.
- **Pickup** (FUN_1403f8770): the item is added as the box leaves the ground. A full stock adds nothing, and the box still flies and the
  name still shows.
- **Flight** (YS::ACTION_PRIZEBOX_GET, FUN_1403f8a10, FUN_1403f8590): no script. Sound 18 on entry, then toward the target's first body
  centre, read live, for clamp(|v|·0.05, 30, 60) ticks. Each tick t = 1 − sin((1 − r/r0)·π/2), position = anchor + v·t raised by sin(tπ)·50,
  yaw += (1 − t)·0.5236, and below t = 0.1 the scale is t·10·base. At the end it queues the name and goes, with no effect.
- **Name window** (dk::TREASURE_PRIZE_INFO, FUN_1401595d0, FUN_1401596c0, FUN_140159ed0): ring of 16, one at a time, se000 #10 as each
  starts, `gaug` group 0x4c, text `09 icon name` in 0x80e5e5e5 at size 16. No pause, no button.
- **Party prizes** (FUN_1403dbea0, FUN_1403dca50, FUN_1403d7bd0): Sora, Donald, then Goofy, and the first in range collects. Members pass
  over Drive prizes. HP, MP and element 6 go to the collector only, and a repeat raises the time to the larger of the two.

## Changes

- `enemy_prizes::ItemDrop` carries the fall, the bounces and the flight (`take_off`, `fly`), replacing the guessed gravity.
- `visual::treasure_info::TreasureInfo`, drawn after the level-up panels. `Game::treasure_items` hands it the arrivals.
- `prize_room`: members collect, with `MemberRt::heal_ticks` and element 6 bound to the member.
- `--prize-tour` moves one box beside Sora at tick 140 and drops an HP prize at Donald's feet at tick 200.

## Checks

| Check | Result |
| --- | --- |
| `cargo clippy --all-targets --locked -- -D warnings`, `cargo test --locked` | PASS |
| New unit tests: thrown up, not collectable until falling, two bounce sounds, rest; 30 ticks, 50 high, shrinks | PASS |
| Release `--prize-tour` (`pz2`) | PASS: bounce and flight sounds, the box flies to Sora, then the window reads "OBTAINED" and the item's icon and name, rises, holds and sinks; Donald's prize logged with his HP |
| The message's words on the bar | "OBTAINED" is the sheet's own art, not a message |
| Other worlds' and languages' `gaug` group 0x4c | NOT RUN (the room's own sheet only) |
| Side by side with the installed game | NOT RUN |

## Uncertainty

- The box's walls are not tested: it falls to the floor under it and can sail through a wall in flight. The executable's map solve for it was
  not traced.
- Whether a menu or event hides the window (layer 0xd) was not read. Here it keeps playing under the camp.
- FUN_1403c4d50 also places consumables in Sora's empty item slots first. The room still adds them to the stock.

## Verdict

PASS for the room. All of #17's open items are built. No claim about the installed game's look.

## Next

Field chests, on the same window host (#67, #79).
