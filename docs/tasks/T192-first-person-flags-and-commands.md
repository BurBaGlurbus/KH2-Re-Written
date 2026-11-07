# T192 — First person: FUN_1403ba300's flag bits and what first person blocks

Owner Claude, 2026-10-18, requested by burba in the project chat (follow-ups left open
in T183). Gate: development of the test room only; no final gameplay acceptance is
inferred. Prerequisites: T183, T190. Build identity: branch
`claude/combat-feel-first-person-flags` on PR 113's branch (`main` at `554d3bf`),
package 0.1.2, Rust 1.99.0 Windows MSVC. Source: the selected Steam KH2FM build, validated
each run; the install was only read.

Private evidence root: `%LOCALAPPDATA%/KH2FinalRewrite/research/combat-feel-20261017/`
(decompiles `g/d37`, `g/d38`, scans `g/s8.txt`, `g/s9.txt`, vtable index `g/vt1/index.txt`);
unattended run `fl1` in the private `kh2-combat-out/`.

## The two object flag bits of FUN_1403ba300

The animation manager is embedded in the object at +0x158, so its range accumulators
`+0x34` and `+0x3c` are `obj+0x18c` and `obj+0x194`.

- `obj+0x194` bit 1 is range trigger 46 (`FUN_14040d810` case 0x2e ORs 2 into `+0x3c`):
  first person is refused while a range 46 holds. None of the 79 motions of Sora's base
  set the room loads carries one, so it never blocks him here.
- `obj+0x18c` bit 14 is not set by any range case (the cases set bits 2 to 6, 9 to 11, 15,
  16, 18, 19 and 20 to 31). Its readers all substitute the object at `obj+0x2b0` for the actor
  when it is set (`FUN_14040e4e0`, `FUN_14040d810` case 0x14), which is the riding state;
  no rideable object exists in the room, so it is always clear here.

## What camera mode 1 blocks

`FUN_1401650f0` ("camera mode is 1") has four callers only: `FUN_1403a70a0` and
`FUN_1403a9250` (the idle states' turn on the right stick, ported in T183) and the two
"is moving" predicates `FUN_1403a7830` / `FUN_1403a8f00`, which return false in first
person. The first of them is the player class's virtual slot +0x108 (`YS::PLAYER`
vftable at 0x1405c4a80), called by `FUN_1403d6870`, which applies the stick's movement
(`FUN_1403b65c0`) only when it holds. Nothing else reads the mode, so attacks, jumps,
guard, magic, items and Drive are not gated by first person.

## Rewrite

`Clip::no_first_person` from range 46, `Player::first_person_blocked`, and
`first_person_allowed(player, clips)` now includes it; the doc comment records the riding
bit. No command gating was added, which is what the executable shows.

## Checks, uncertainty, verdict and next task

PASS unit test `range_46_blocks_first_person_while_it_holds`. PASS the kit summary of the installed source (private `fl1`): no loaded motion carries
range 46. Engineering checks are recorded in the pull request.

Not verified: what range 46 means in play (it appears in no motion of Sora's base set);
that the riding state is the only meaning of bit 14 (its setter was not located: no range
and no `OR` in the player code writes it); the movement predicate's other callers through
the vtable slot are assumed to be `FUN_1403d6870` only.

Verdict: PARTIAL. The first-person follow-ups of T183 are resolved as far as the code shows.
Next: none for this item.
