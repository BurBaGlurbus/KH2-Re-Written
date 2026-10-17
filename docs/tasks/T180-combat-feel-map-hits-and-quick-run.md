# T180 — Combat feel: the Keyblade meeting the map, Quick Run's streak, and what the air combo does

Owner Claude, 2026-10-17, requested by burba in the project chat (match combat
physics, feel, effects and sounds to the installed executable). Gate: development
of the test room only; no final gameplay acceptance is inferred. Prerequisites:
T056 (player controller), T064 (attack windows), T090 (attack selection), T105
(movement), T150 (effects and sounds), T170 (lock-on camera). Build identity:
`main` at `554d3bf`, package 0.1.2, Rust 1.99.0 Windows MSVC. Source: the
selected Steam KH2FM build (manifest in the private test-room folder), validated
each run; the install was only read.

Private evidence root: `%LOCALAPPDATA%/KH2FinalRewrite/research/combat-feel-20261017/`
(headless Ghidra 12.1.4 on a read-only project copy: decompiles `g/d1`..`d25`,
disassembly `g/a1`, `g/a2`, reference lists `g/r1`..`r14`, field scans
`g/s1`..`s7`) and the earlier `playable-room-20261008/movement-01/g/` and
`guard-01/` folders. Unattended runs are in the private `kh2-combat-out/` folder.
No game data, decompiled text or captures are in Git.

## What the executable does

- **A Keyblade swing that meets the map.** `FUN_14040f610` (the weapon attack
  volume's per-frame update) calls `FUN_1403d0bb0` for the volume's first sphere
  from its second frame on: segment from the sphere's previous position to its
  current one, `FUN_140171090` with class 4 (the map's polygons with attribute
  bit 7, polygon filter cleared, so nibble-4 polygons count; the existing
  `collision::World` ray). It runs only for an ATKP row with flags bit 0 (+0x12),
  never for a volume that already met the map (volume flags +8 bits 3/4) or
  hit an actor (bit 1), and a row with a reach (+0x28) that missed tests a second
  segment of that length below the sphere. Sora's rows with the bit are ATKP 126
  (ground combo hit 1), 128 and 131 (air combo hit 1); all have reach 0.
  `FUN_1403d1960` then: sets flags bit 3 or 4 by the surface nibble; starts the
  attacker's own hit element whose category holds the ATKP hit effect's bit above
  bit 32 and `0x100` (nibble 1) or `2` (any other), at the contact, turned by the
  polygon normal (`FUN_1403b7710`: pitch `atan2(-ny, hypot(nx, nz))`, yaw
  `atan2(nx, nz)`); plays `FUN_1403dadf0(…, 0x7a12a)` (sound 500010, the
  equipped Keyblade's bank) when the volume belongs to a weapon; and calls the
  owner's callback (class `YS::PLAYER` vtable +0xd8 = `FUN_1403d63e0`): the
  swing counts as connected (obj+0x120 bit 25), the combo step byte is lowered
  by one if non-zero, and `FUN_1403d4650(-1, volume, 0, -1, -1)` recoils once per
  volume: on the ground motion 0x29, a knock-slide backward of strength 50
  (`DAT_1405b532c`) and the idle next; in the air motion 0x2b, the fall next and
  the aerial hit's hop (height 50, speed 50 × SSTM HitBackToJump backward).
  The Keyblade PAX's element 3 (category `0x300000102`) is the spark for hit
  effects 1 and 2 on both surface classes.
- **Quick Run's streak.** `FUN_1403a96c0`, `FUN_1403a9830` and the Square path
  `FUN_1403a81f0` start motion 0xcb (203), mode 0x10, and for an ability level
  above 1 start element `level + 0x6c` of Sora's own effects through
  `FUN_1403b6670`: elements 110 and 111 (sound 999018) of `p_ex100`.

## Rewrite

1. `collision::World::attack_ray` and `AttackHit` (the class-4 segment with the
   plane normal and surface nibble) with a unit test; `MapWorld::attack_hit` in
   scene coordinates; `data::battle_tables::Attack::env_reach` (+0x28).
2. `test_room/wall_hit.rs`: the per-volume test with the executable's gates, the
   effect request `EffectRequest::Wall` (category from the ATKP effect and the
   surface class, turned by the normal), the clank, and the recoil. It runs for
   every room with a map, independent of an enemy runtime, so the Twilight Town
   rooms show it; the generated arena has no class-4 polygons.
3. `player::Player::wall_recoil`, `Action::WallRecoil` (motion 0x29, locked until
   its range-2 tick like the guard recoil, so the combo window it carries still
   chains), motion 0x2b as a scripted motion with the fall queued, and the
   kits load both motions (`character_kit`, all Drive forms through the base
   fallback).
4. `Player::quick_run_effect` and `drive_room::motion_effects`: the streak
   starts bound to Sora with its sound.

## Investigated and not changed (the air combo's drop)

Reported: Sora drops to the ground too fast at the end of an air combo. What was
checked, all against the executable:

- The mode-1 arc is the whole of the air physics: gravity `DAT_1405ce184` is read
  only by the arc routines (`FUN_1403cb100/360/500`, `FUN_1403cbb00/c30`,
  `FUN_1403b4780`, `FUN_1403b8150`), the blow flight and the start-up; FallMax is
  the SSTM value; nothing scales it during attacks. The port matches these.
- Sora's air-combo motions (181 to 184) have no range-3 "no gravity" window, no
  range-24/30 window and zero root translation (private `guard-01/pex100-triggers`
  and a per-frame root dump), so nothing in the animation data holds him up.
- The game-speed scale `DAT_140717424` is written only by engine start-up code,
  not by hits.
- The attacker-side hit routine (`FUN_1403d12c0`) sets flags, sounds and effects
  only; it does not touch the arc.
- Each air attack with a Jump field relaunches the arc (`FUN_1403d78d0` →
  `FUN_140409ad0` → `FUN_1403cbb00`) exactly as the port does, so a single swing
  started at the jump's apex lands about 40 ticks into the 60-tick animation in
  both.
- One real difference was found in the hop's target height (`FUN_1403d7520`,
  disassembled): the executable measures the target's TARGET point relative to
  Sora's own TARGET point (`target − Sora's shape point`), the room measures
  from Sora's feet. Issue #99 has the detail; the room is unchanged.

No executable mechanism for extra hang time was found, so none was added (issue #99 asks what the original shows).

## Checks, uncertainty, verdict and next task

PASS unit tests: `attack_ray_hits_only_class_four_polygons`,
`a_swing_that_meets_the_map_recoils_on_the_ground`,
`a_swing_that_meets_the_map_in_the_air_hops_back_and_falls`,
`quick_run_starts_its_level_effect_from_the_second_level`.
PASS unattended run in the Twilight Town room with the installed source (private
`kh2-combat-out/wall1`): at tick 889 the first ground combo swing met a wall,
`wall_hit` logged material 2, effect 1, recoil `WallRecoil`; element 3's effect
started at the contact (`effect_number` 3), sound 500010 played, and Sora was
pushed back along 4.8, 4.4, 4.0 … units a tick for the motion's first frames.
PASS Quick Run at level 3 (private `qr2`): element 111 started bound to Sora and
sound 999018 played one tick after the dash began.
Engineering checks (fmt, clippy, workspace tests, build) are recorded in the pull
request.

Not verified: no pixel capture of the spark was inspected, and no installed side-
by-side comparison exists. The two-sphere order of the executable's per-volume
loop is read from `FUN_14040f610`; only the first sphere tests the map. The
volume flag bit 8 (set only by the spell volumes) is taken as clear for the
Keyblade. Dynamic collision objects (the list `FUN_140163fc0` walks) do not exist
in the supported rooms and are not cast against. The victim-side hit sound
(`FUN_140410d60` → `FUN_1403dadf0`, bank × 1000 + alternating bit + class) is not
played: the per-object bank value comes from a resource record whose source was
not traced (issue #100, which also lists the unmodelled frame triggers 16 and 31 and range triggers 24 and 30).

Verdict: PARTIAL. The map hit and Quick Run's streak execute traced executable
logic in the development room; the air-combo hang time is unresolved and the
victim hit sound is not built.
Next: decide the air-combo reference with burba (what the original shows), then
trace the resource record behind the victim hit sound (`FUN_14039da60`).
