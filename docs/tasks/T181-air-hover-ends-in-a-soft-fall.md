# T181 — Air moves: a no-gravity range ends in a soft fall (the air combo's drop)

Owner Claude, 2026-10-17, requested by burba in the project chat ("the end of air
combos make you go down to the ground really fast"; then: in the original it is
"more like floating", slower than ours, and it depends on the air move). Gate:
development of the test room only; no final gameplay acceptance is inferred.
Prerequisites: T105 (movement), T180 (the earlier investigation, issue #99).
Build identity: branch `claude/combat-feel-air-hover` on PR 101's branch
(`main` at `554d3bf`), package 0.1.2, Rust 1.99.0 Windows MSVC. Source: the
selected Steam KH2FM build, validated each run; the install was only read.

Private evidence root: `%LOCALAPPDATA%/KH2FinalRewrite/research/combat-feel-20261017/`
(headless Ghidra 12.1.4 on a read-only project copy) and the earlier
`playable-room-20261008/movement-01/g/`, `guard-01/pex100-triggers.txt`.
Unattended runs `hz0` (before) and `hz1` (after) are in the private
`kh2-combat-out/` folder.

## What the executable does

- Sora's air moves differ in their ANB range triggers. The basic air combo
  (motions 181 to 184) has none, but the ability moves hold him in the air with
  range 3, the "no gravity" state (`FUN_14040d810` case 3 sets obj+0x18c bit 5):
  motion 192 (ticks 16 to 24), 193 (Horizontal Slash, 16 to 44), 194 (16 to 100),
  196 (46 to 54), 197 (16 to 82), 172 (20 to 36) and 167 (whole motion); the
  spells' air casts and Quick Run use it too. While the state holds, the arc
  returns zero vertical movement (`FUN_1403cb360`, `FUN_1403cb500`) although its
  time keeps running.
- When the state ends, the per-frame player update `FUN_1403b4780` (the first
  block: `obj+0x120 & 0x1800000` set last frame, `obj+0x18c & 0x1000020` clear
  now, bit 6 clear) re-seeds the arc instead of letting it resume: rise and span
  zero, `t = v / g` with `v` the last tick's actual vertical speed
  (`obj+0xa3c`, written by `FUN_1403b89a0` from the real displacement, so zero
  after a hover; `t = 0` when not falling), and the base chosen so the curve
  passes through the current height (`y - t·g·t/2`). The fall therefore starts
  from rest and speeds up by g a tick.
- Range 24 (bit 6) and range 30 (bit 24) hold the height the same way
  (`FUN_1403cb500` tests `0x1000020` and bit 6) and end through the same block;
  no Sora motion in the base set uses them.

## What the port did

The ranges held the height but never re-seeded the arc, so when a range ended
the curve had run ahead by the whole hover: on the next sample the target height
was far below Sora and the downward step snapped to the fall cap (16 a tick).
Horizontal Slash from the jump apex, run through the room (`hz0`): held at
y 162.43 from tick 100 to 128, then the cap at once (-16 a tick, 32 units a
two-tick sample) and landing at tick 139, eleven ticks after the range ended.

## Rewrite

- `Player::was_floating` and the re-seed in `airborne()`: when a no-gravity range
  that held last tick is gone, `Arc::fall(position, last vertical speed)` (the
  same routine that already ports `FUN_1403b4780`'s ledge fall).
- Range triggers 24 and 30 join range 3 in `Clip::no_gravity`.

After (`hz1`, same scenario): held at 162.43 from tick 100 to 128, then the speed
grows by g (0.41) a tick, -1.22 four ticks in, -4.49 at twelve and -11.0 at
twenty-eight, and Sora lands at tick 158, thirty ticks after the range ended
instead of eleven.

## Checks, uncertainty, verdict and next task

PASS unit test `a_no_gravity_range_ends_in_a_soft_fall_not_a_catch_up` (it fails
without the re-seed: 80 units dropped in six ticks, against a few with it).
PASS the unattended run above with the installed source. Engineering checks are
recorded in the pull request.

Not verified: no installed side-by-side, so the owner's "floating, depends on the
move" is matched by the traced rule, not compared with a capture. The ordering
inside one frame (range evaluation, re-seed, movement) is read from the
disassembly but a one-tick offset cannot be excluded. The moves other than
Horizontal Slash (motions 192, 194, 196, 197, 172, the spells) were not run
through the room; they take the same code. `obj+0xa3c` is taken as the last
tick's displacement per tick (`FUN_1403b89a0` multiplies by `DAT_140717488`,
1 / frame time).

Verdict: PARTIAL. The hover's end follows the traced routine in the development
room; the owner's check against the original remains.
Next: the owner tries the air moves; then the enemy side of air combos (does the
launched enemy hold the same hover) if it still looks different.
