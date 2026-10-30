# T296 — Final Form's effects in their places, Limit Form's limits against real targets

Owner Claude, 2026-10-30, requested by burba in the project chat ("some of final forms attack effects are really weird
and are not in the right places, limit forms limits dont act correctly like the original"). Gate: development of the test
room's combat and effects; no final gate is inferred. Prerequisites: T113, T210, T218, T225, T261. Build identity: local
`main` at `80925ea` plus this change, package 0.1.2, release launcher with `--features skate3`. Source: the selected Steam
KH2FM build, validated each run; the install was only read. Plain headless Ghidra 12.1.4 on read-only project copies
(`ghidra-kh2-dm1`, `dm2`); the Ghidra MCP was not used.

The checkout was being edited by other threads and did not compile for most of this work, so the change was built and
run from a private export of `HEAD` with only this change applied, and committed from there; no other thread's
uncommitted work is in this commit. The checkout's own files carry the same edits.

Private evidence (outside the checkout): `%LOCALAPPDATA%\KH2FinalRewrite\research\drive-fx-20261030\` (`g\a`, `g\b`,
`g\dis`, `g\lim`, `limit-audit.md` with its table of all 93 traps of `limitform`); runs, captures and reference sheets in
`runs\` beside them (logs and contact sheets of `final-a`..`final-e`, `limit-a`..`limit-d`, `bat-1`, `bat-2`, `live2`, `ref`); selected screenshots in
`.local\drive-fx-review\`.

## What the executable does

### How an element binds (FUN_1402c6b30, FUN_1402c58f0, FUN_1402c49d0, FUN_1402c4da0, FUN_1402c7300)

- Flag 1 binds to the bone; flag 2 stops following once the start wait has passed (the element then stays in the world);
  flag 4 takes only the position through the bone and keeps the element's own rotation and scale; without flag 0x100 the
  scale is the element's, not the bone's; flag 0x20000 binds to the camera (the view's inverse times the element).
- Flag 0x20 registers the instance on the object's motion (FUN_1403c8000). The next motion start (FUN_1403c8a40 ->
  FUN_1403c7300) marks it (+0xc bit 25); FUN_1402c7300 then ends the loop (flag 0x80) or fades it over the element's own
  fade frames, PAX +5 (flag 0x40).
- Flag 0x800 takes the alpha of what it is bound to (FUN_1403cc950): a weapon slot's, 0 when the weapon is gone.

### The weapon codes (FUN_1403b50f0 -> FUN_1403b5450 -> FUN_1403d7600 -> FUN_1403b5d40)

- The slot matrix is the **weapon object's own bone**: the bone of its first enabled collision shape of kind 15 (bone 2 on
  W_EX010 and W_EX010_U0, read from the mdlx), not the hand. The value returned for 0x4003 / 0x4007 is **that shape's x**
  (`MOVSX EAX, word [RBX+8]` at the end of FUN_1403b5d40, which the decompiler drops): -78 on the Kingdom Key, so 0x4003
  sits at (-78, 0, 0) on the bone and 0x4007 at half.
- A trail's ratios multiply a different number, FUN_1401a7af0's return: the bone record's +0x1c, 21.107 (21.107 × -3.7 =
  -78).

### Limit Form

- `limitform` calls 1:83 (`obj_apply_bone_matrix`) and reads attached objects' positions (FUN_1403b5b40: +0x70), so its
  thrown Keyblade and shots start at the hand and the blade top.
- 2:56 (FUN_140431ad0) is &(obj+0xdc8)->+4, the target holder 6:67 reads, locked or not. 1:120 (FUN_1403bdde0) is the
  TARGET shape's centre, and FUN_1403d7520 measures the hop from Sora's own centre.
- Range 28 / frame 21 / ranges 0x16 and frame 0xf (FUN_1403d3a90 -> FUN_1403b9f70): up to **twice** the parameter a tick
  (DAT_1405b18ec = 2.0), and FUN_1403b6560 makes the new heading the wished one. Sonic Rave's wish of turning round is
  overwritten by range 28 as soon as a target stands beyond 50 units.
- FUN_1403b89a0 adds a battle object's knock-back (FUN_1403d4570, gated on +0x6c8 bit 1) and the motion root in every mode.
  `obj_hitback` (L2242, L2407, L2494: 50, 100, 200) moves Sora during Infinity.
- FUN_1403d1730: ATKP +0x2f comes back as HP through the owner's vt+0xe8 (Sora: FUN_1403a75f0 -> FUN_1403d5e50). Rows:
  1857 10, 1858-60 5, 1861 15, 1866/7 8, 1878/9 5, 1883 3, 1884-88 1, 1889-94 2, 1895 5.
- A hit outside a motion's range 27 plays a damage motion; trigger 22's motion test (L5792, L5820) then fails and the
  limit ends. FUN_140400990 fades both party slots in all four limits.
- The colour filter (FUN_1403f7040 -> FUN_1401626f0) multiplies the light table, the object colour list (FUN_14016f2b0),
  the fog, and the **map's** effects. It never reaches an object's effects.

## What the rewrite did

- `drive_fx` read only flag 1: every bound element followed its bone with the bone's rotation for its whole life, and
  nothing ended an element with its motion. Final's bursts (52..56, flags 0x243: they stay where the swing leaves them)
  rode the Keyblades, its discs (eff 26, a flat ring at (-144, 0, 0) on the blade) turned with them.
- The weapon codes used the hand joint and the farthest vertex. With Final's floating Keyblades the hand joint's axes have
  nothing to do with the blade, so every 0x4005 / 0x4006 element at (-29, 0, 0), (-60, 0, 0), (-144, 0, 0) landed in the
  wrong place and turned the wrong way. `spell_fx` guessed the same.
- The limit scene's host had no hand and no bones: Strike Raid's Keyblade and Infinity's shots started at Sora's feet. The
  Keyblade then climbed out of the map (its fly loop lifts it, in T218's log 28.9 up a tick), so Strike Raid only hit at
  point-blank range, and all ten shots hit on their first tick.
- The scene's only target and victim was the room's Shadow: against a battle enemy the hops and turns went to the Shadow
  or nowhere, and Raid and the shots could not hit it.
- Limit Form tinted the **whole composed frame**, effects included: Infinity's white star came out green, Strike Raid's
  rings purple. The Drive change's grey did the same.
- No HP drain anywhere; a hit during a limit was overwritten on the next tick and the limit went on (even at HP 0); the
  recoil never moved Sora; a limit that ended in the air stood him on nothing; 1:149's element sat on Sora instead of the
  Keyblade; 0:52 was dropped, so Infinity's orb never faded; Donald and Goofy stayed visible and frozen.

## Changes

- `drive_fx`: flags 2, 4, 0x100, 0x20000, 0x20 / 0x40 / 0x80 and 0x800 as above; `SoraPose::matrix` is one place for
  FUN_1403b50f0. `slot_pose` / `slot_matrix` take the blade bone and the reach; `spell_fx` uses them too.
- `character_kit::blade_reach` reads the kind-15 shape per Keyblade; `ClipPoses` keep `blade_reach` and `blade_length`;
  `weapon_trail` uses the bone length instead of measuring the mesh.
- `party_limit`: `RoomEnv::attach_point` / `bone_point` from `SoraPose`; `limit_target` (the target holder, else the
  nearest of the battle enemies and the Shadow) with the TARGET centre; the scene's enemy object always exists, out of the
  searches when there is nothing; `lockon_target` is the holder; attack objects land on the Shadow with spark, victim
  sound, drive, HP drain and connect, and reach battle enemies through `party_volumes` (`hand_over`, `SCENE_VOLUME`); a hit
  ends the limit; the end falls when airborne; 1:149's element follows the Keyblade; 0:52 fades.
- `enemy`: range 28 / 0x16 / frame 0xf at twice the parameter and setting the wish; knock-back stepped outside reactions
  for battle objects; `Env::own_target_lift` for the hop's rise.
- `player`: `face_target` at twice the parameter.
- ATKP HP drain in `combat_tick`, `enemy_battle_hits` and `land_attacks`.
- `boss::requests` multiplies the Drive's two filters into `scene_tint`; `boss::map_tint` tints every textured material
  (map and actors) instead of the composed frame; the fades keep the tint.

## Checks

| Check | Result |
| --- | --- |
| `cargo fmt --all --check` | PASS for this change; FAIL at `source/visual/src/room.rs` lines 992 and 1034, which `HEAD` already has and this change does not touch |
| `cargo clippy --all-targets --locked -- -D warnings` | PASS |
| `cargo test --locked` | PASS, 1309 passed, 1 ignored as before. New: `range_28_makes_the_new_heading_the_wished_one`, `a_battle_object_slides_by_its_knock_back_without_a_reaction`, `a_bound_element_takes_the_bones_turn_and_flag_4_only_its_place`, `an_element_ends_with_its_motion_as_its_flags_say`; the weapon code and range 28 tests carry the new numbers |
| `cargo build --locked`, smoke `cargo run -p main --locked -- --frames 180` | PASS |
| Release `cargo build --release --features skate3 --locked -p main --bin KH2RW` | PASS (from the private export of `80925ea` with this change) |
| `--limit-form-tour` on that release (`limit-d`) | PASS: the four limits start and end, no faults, no missing traps, `exit` code 0 |
| `--magic-tour --form-moves --magic-form 5`, with and without `--form-moves-chain`, on the build before the last rebase (`final-b`, `final-d`, `final-e`) | PASS: 151, 152, 153, 181, 182 and 183 play with their elements at the places below; no unsupported program |
| Real launcher path (`room` on `test-room-combined.json`, the owner's settings), F3 into the form, posted keys, game-side screenshots (`live2`, sheets in `.local\drive-fx-review`) | PASS: Final's ground combo; Sonic Rave, Last Arcanum, Strike Raid and Infinity through the Limit menu and Triangle. `PrintWindow` returned white in this build, so these runs used a local build with an uncommitted capture hook, otherwise this change |
| Real map, real enemies (`area-hb08-01.json`, `--enemy-combat-tour` with `KH2RW_AREA_ENEMIES`; the limit requested by the same local hook; `bat-2`) | PASS for Infinity: its shots hit and defeated placed battle enemies. Sonic Rave started in the air and ended on landing (see Uncertainty). Strike Raid's request was refused while the tour's Sora attacked: NOT RUN against a battle enemy |
| Real map, no battle enemy (`area-hb20-01.json`, floor at y = -800) | PASS: Strike Raid runs its whole length at the floor's height |
| Against public footage (YouTube, "Drive Form Breakdown: Final Form" and "Limit Form", "All Drive Forms"; sheets in `.local\drive-fx-review`) | Looks alike: Final's blue ring and lightning on the ground finisher, the four white columns of the air finisher, the blue orbs of the first swing; Limit's white star and comets on Infinity, the rings of Strike Raid. Not a controlled comparison |
| Controlled comparison with the installed game | NOT RUN: no save with Drive forms on this PC |

## Per attack

Motions from the probe logs; "matches" means placement and look against the public footage named above, not a controlled
comparison.

| Attack | Elements | Verdict |
| --- | --- | --- |
| Final ground 1 (151) | 52..56 at the blade ends, stay in the world | Changed. Bursts along the blades as in the footage. 57 (frame 84) is cut when the next swing starts at 84 under a mash |
| Final ground 2 (152) | swing trails only | No element of its own. The wide cream arcs still there are the Kingdom Key's element 10 trail on the far-reaching blades; the footage (another Keyblade) shows a similar arc |
| Final ground finisher (153) | 20 (fades with the motion), 21, 22..24 fixed ahead | Changed. Blue ring and lightning match the footage |
| Final second finisher (154) | 25..40 | NOT RUN: the probes did not reach it. Same code path |
| Final air 1, 2 (182, 181) | 62, 63; 58, 59, 60, 61 | Changed; follow Sora as flags say |
| Final air finisher (183) | 42..51 | Changed. The four white columns over and under Sora at frames 36..40 match the footage |
| Sonic Rave | 698 -> 699 ×5 -> 700 | Changed: range 28 aims each dash; hits, target. Effects white and gold, not tinted. 7 hits in the tour. Live: PASS, all six presses |
| Last Arcanum | 701 -> 702 ×5 -> 703 | Changed: as above. Live: PASS |
| Strike Raid | 704 -> 705 ×3 -> 706 | Changed: the Keyblade leaves the hand, homes on the target holder, hits at range (1866 ×4, 1867 in the tour; before, only at point-blank) with spark, sound and drive. Live: PASS |
| Infinity | 683 -> 684 (Shoot) | Changed: shots leave the blade top and spread; recoil moves Sora; the orb fades; a fall after it instead of standing in the air. Tour: 1862 ×7. Live: PASS. On hb08 the shots hit and defeated placed battle enemies (`gallery_hit` 1862, 12 hits) |
| Zantetsuken (154) | PTYA record 3 | Unchanged; plays in the tour's ground combo. Motion 155 (record 2) is NOT RUN |

## Uncertainty

- [I] The hand point for an attached object is the hand joint's origin; the exe reads the weapon's own matrix there.
- [I] The filter multiplies materials by colour; a material created during a tint keeps white until the next change. The
  clear colour and the map's effects are still not tinted.
- [I] Limit attacks' hits use the row's first step for drive and drain (all have one).
- A limit ended by a hit ends its scene at once, so a Keyblade in flight returns to the hand at once.
- The scene's searches see one enemy object: scatter shots aim at the chosen enemy, though they hit whatever they cross.
- A limit started in the air with no invulnerable landing ends when Sora lands (seen in a run): his land motion fails trigger
  22's motion test, as a damage motion does. Whether the original lets a limit start in the air was not traced.
- A hit that interrupts a limit, and HP drain, are not observed in a run: the tours' enemies did not hit Sora outside the
  windows, and Sora was at full HP.
- Not done: motion 235 on the thrown Keyblade; the first motion step 0 -> 2 in the limit's `ClipView`; range ends as
  `start <= t < end` (`Clip::within`, `combat_tick`).
- The held reaction command: after a finisher the slot keeps showing it (mode 8, T211) until the next command. Not changed;
  whether the original clears it at the limit's end (the callers of FUN_1403a81c0) is not traced.
- Frame timing and damage were not compared with the installed game: no save with Drive forms on this PC. The footage is
  public video of the game, not this build.

## Verdict

PARTIAL. The binding rules, weapon codes, limit targets, hits, drain, interruption and tint follow the executable. Final
Form's effects and the four limits run in the real launcher path and look like the public footage; nothing is compared
frame by frame with the installed game, and no gate is inferred.

## Next

Trace what clears the held command at a limit's end; motion 235 on the thrown Keyblade; every battle enemy in the scene's
searches; a controlled comparison once a save with Drive forms exists.
