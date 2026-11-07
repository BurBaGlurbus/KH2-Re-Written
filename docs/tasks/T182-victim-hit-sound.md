# T182 — The victim's own hit sound

Owner Claude, 2026-10-17, requested by burba in the project chat (combat sounds
missing, issue #100). Gate: development of the test room only; no final gameplay
acceptance is inferred. Prerequisites: T150 (effects and sounds), T180. Build
identity: branch `claude/combat-feel-victim-sound` on PR 102's branch (`main` at
`554d3bf`), package 0.1.2, Rust 1.99.0 Windows MSVC. Source: the selected Steam
KH2FM build, validated each run; the install was only read.

Private evidence root: `%LOCALAPPDATA%/KH2FinalRewrite/research/combat-feel-20261017/`
(Ghidra decompiles `g/d5`, `d12`, `d13`, `d24`, `d31`, `d32`; unattended runs
`snd1` and `tour2` in the private `kh2-combat-out/`).

## What the executable does

- Every hit the damage routine applies to an actor (`FUN_140410d60`, unless the
  hit record's flags bit 2 is set) plays a second sound besides the attacker's
  ATKP HitSound: `FUN_1403dadf0(victim, bank * 1000 + (n & 1) + class, position)`.
  `n` is the victim's hit counter byte (`obj+0x26f*4`, incremented after each
  sound); `class` comes from the ATKP reaction through `FUN_1403ce980`
  (reactions 1 and 12 are class 1; 3, 6, 8, 10, 13, 16, 18 class 3; 4, 7, 9, 11,
  15, 17 class 4): classes 1, 3 and 5 add 0, class 4 adds 2, any other plays
  nothing. `bank` is the short at +2 of the resource record behind `obj+0xbb0`
  (`FUN_14039da60`). `FUN_14039e900` fills it: it passes the record's bank file to
  the sound loader `FUN_1401dcef0`, which returns `FUN_1401e0d70`'s value for the
  file and that is what the record keeps. For a `SEDBSSCF` bank that value is
  the file's own number, the u16 in its table header (+0x36). The record's other
  short (+0x68, the loader's third argument 4, 5, 7 or 0x1e) is the bank pool
  slot kind, not the number.
- The install has sounds 201000 to 201003, the four ids this scheme gives the
  Shadow, and its effect sounds name bank 201.

## Rewrite

`test_room/victim_hit.rs` and `EffectRequest::VictimSound`: a counted hit by
Sora on the room Shadow or a battle enemy asks for the sound; the bank is the
header number of the sound bank in the victim's own object archive (the
`se/obj/seNNN` replacement asset, `visual::sound::object_bank`), read when the
room loads and registered like the exe's loader does; the counter is kept per
victim. The room now also registers the sound banks of the battle enemies so
their own effect sounds and hit sounds resolve.

## Checks, uncertainty, verdict and next task

PASS unit tests (reaction offsets; the alternating ids; a reaction with no sound
leaves the counter). PASS the three-actor combat tour with the installed source
(private `tour3`): the Soldier played 200000, 200001, 200002, the Shadow 201000
and 201003, the large enemy 214000 to 214002, each resolved to one layer of the
enemy's own bank. Engineering checks are recorded in the pull request.

Not verified: that the object archive's `se/obj/seNNN` asset is the file the
exe's record holds (it is the bank the exe's loader registers for the object by
the same header number, and the ids exist in it, but the load argument itself,
the pointer `FUN_1403e0660`'s callers pass, was not followed). Sora as a victim
is not built (his bank is unknown). The position of the sound and the
0x17-class object branch of the routine are not modelled.

Verdict: PARTIAL. The victim hit sound follows the traced routine for the three
battle enemies; Sora as victim remains open (issue #100).
Next: Sora's own bank, then frame triggers 16 and 31.
