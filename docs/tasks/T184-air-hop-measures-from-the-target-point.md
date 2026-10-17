# T184 — The air hop's height and cap, measured as the executable does

Owner Claude, 2026-10-17, requested by burba in the project chat (issue #99: the
air combo should match the original). Gate: development of the test room only; no
final gameplay acceptance is inferred. Prerequisites: T180, T181. Build identity:
branch `claude/combat-feel-hop-height` on PR 104's branch (`main` at `554d3bf`),
package 0.1.2, Rust 1.99.0 Windows MSVC. Source: the selected Steam KH2FM build,
validated each run; the install was only read.

Private evidence root: `%LOCALAPPDATA%/KH2FinalRewrite/research/combat-feel-20261017/`
(disassembly `g/a2/1403d7520.asm`, `1403be0f0.asm`, `1403c9c00.asm`, the earlier
`ghidra-kh2-a3/guard-01/pb/` and `cmd-04/` decompiles); unattended run `hop2` in the
private `kh2-combat-out/`.

## What the executable does

`FUN_1403d78d0` starts an attack record; for one with a Jump field above zero
(PTYA +0xc) and a valid target it calls `FUN_1403d7520(Sora, target, &vector)`:

- the vector is the target's TARGET point minus Sora's own TARGET point (both from
  `FUN_1403c9c00`: the shape's bone-turned offset with its height subtracted from
  y), not minus Sora's feet; the return value is the two shapes' radii (the u16 at
  +0x10 of each record) added;
- the hop's height is the vector's rise (y turned up) clamped to [PTYA +0x14, +0x10],
  and when the record's flags bit 1 is set the cap +0x10 is raised by
  `FUN_1403b5a40` (Sora's jump height: High Jump's row, else PRTY +0x130);
- the horizontal speed is `(distance - radii + PTYA +0x3c) / time(height)` clamped
  to [+0x18, +0x1c] toward the target's direction.

The branch for a record with no Jump field (grounded attacks) moves Sora toward the
target only in a physics mode whose flags have bit 1 or with `obj+0x18c` bit 6;
Sora's modes 0, 1 and 4 have neither, so grounded swings do not lunge.

## What the room did

The rise was the target's TARGET point above Sora's feet and the cap ignored the
jump-reach flag. Sora's TARGET point is 70 units above his feet, so every rise was
70 too high and caps with the flag (air combo hit 1 among them) too low by his jump
height.

## Rewrite

- `Input::hop_vector` replaces `target_rise`: the target's point minus Sora's point
  (`enemy_target::player_targetcentre`, the routine the lock-on already uses), in
  scene axes; `lift` takes direction, distance and rise from it.
- `AttackStep::hop_jump_reach` from the PTYA flag bit 1 (`ptya::flag::JUMP_REACH`),
  and `Player::jump_reach_height` (the jump height of the last tick, High Jump
  aware): the cap is `+0x10 + jump height` for those records.

## Checks, uncertainty, verdict and next task

PASS unit tests `aerial_hop_heads_for_the_target` (unchanged expectations through
the new vector) and `a_jump_reach_hop_may_rise_by_the_jump_height_above_its_cap`.
PASS the three-actor combat tour (private `hop2`): the logged hop vectors differ from
the old feet-relative rises by Sora's 70-unit target height (for example 127.5
became 57.5).

Not verified: no installed side-by-side of the hop's arc; the owner's report ("Sora
falls too fast at the end of an air combo") is explained by T181's hover end, and this
change only matches the hop's start. The record's second jump-reach use (the
selection floor, `FUN_1404027f0`) was already in `ptya.rs`.

Verdict: PARTIAL for the hop's start (matches the traced routine); with T181 it
accounts for everything issue #99 listed.
Next: owner checks the air moves against the original.
