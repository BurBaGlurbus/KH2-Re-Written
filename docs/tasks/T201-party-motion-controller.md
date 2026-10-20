# T201 — Donald and Goofy: the motion controller, blends, stances and their effects

Owner Claude, 2026-10-19, requested by burba in the project chat ("make Donald and Goofy
1:1 with KH2FM in animation and VFX ... their animations start, loop and transition
erratically, don't blend into each other, and have no effects"). Gate: development of the
test room only; no gameplay gate is inferred. Prerequisites: T186 (members hit, KO, revive,
casts; PR 109), T200 (the branch this one is cut from; PR 115), T108 (`MemberRt.stored`).
Build identity: branch `claude/project-thread-tufhvc` cut from `claude/project-thread-qq8tbs`
at `ce960b1` (which holds the local `main` merges, PR 109 among them, plus PR 115), package
0.1.2, Rust 1.99.0 Windows MSVC. Source: the selected Steam KH2FM build 15194255, validated
at each run and only read.

Private evidence root: a private folder outside the checkout (`kh2-party-anim/`: the Ghidra
runner, decompiles `out/d1`–`out/d10`, data dumps `out/dd*.txt`, tour runs `run1`–`run4`,
`hit1`, pose sheets `sheets/`), and the earlier `party-01` specs under the research folder.
Nothing from the game is in Git; the room reads motions, models and effects from the install.

## What the executable does

- A friend's motions run on the shared motion controller at `obj+0x158`:
  `FUN_1403c86a0` (start; empties the queue), `FUN_1403c6dc0` (change: nothing when the
  slot already plays; walk and run keep their phase, `fmod(len_new × (clock + 1 + blend) /
  len_old, len_new)`), `FUN_1403c8310` (queue a motion, 4 slots), `FUN_1403c88c0` (the
  stance switch below), `FUN_1403c8a40` (a blend over 2 ticks starts the clock at 2 and holds
  the new motion's start frame until the clock reaches the blend) and `FUN_1403c6740` (one
  tick: the first step from 0 jumps to 2; at the end the queued motion starts, or the clock
  wraps to the return frame).
- The AI does not play locomotion. The think (`FUN_1401b53b0`) writes a wish (direction and
  speed 0 / walk 2 / run 8 from the follow state of `FUN_1401b41a0`, whose enter/exit
  distances are the personality header's: field 200/240/1000 and 0/160/200, toward an enemy
  0/240/1000), `FUN_1401b1b20` averages it over six ticks and snaps the speed (below half the
  walk speed none, from (walk + run) / 2 run), and the shared ground mode `FUN_140422520`
  changes to idle, walk or run with a blend of 8 (`DAT_140623bac`) while the motion's range 0
  is open, or range 2 when moving.
- Starts and queues: weapon attacks start unblended with idle queued (fall after an air
  attack; `FUN_1401b0420`); Donald's spell acts `motion_start(m, 8)` then idle (p_ex020);
  Tornado changes to 191 with fall queued, Turbo to 194 with idle, the Bash starts 192 and 193
  at 8 (p_ex030); the item motion 9 starts at 8 with idle (`FUN_1403d6fc0`); a flinch starts
  at 0 with idle, an air flinch with fall, blows and their land motions with nothing, the
  get-up 55 at 8 with idle (damage-02 section 7, `FUN_1403c3c30`); the DEAD motion is changed
  to every tick (`FUN_1403c3660`).
- Stance (`obj+0x120` bit 20): on while the battle flag is set (`FUN_1403d68b0`) and whenever
  a battle-variant slot starts (`FUN_1403c8cd0`); off at a motion's end once the flag is down
  (`FUN_1403ba350`). Slots are `4 × motion + variant`, picked in the order 0, 1, 3, 2 (battle)
  or 3, 2, 1, 0 (field) (`FUN_1403c7c50`, `DAT_1405c5d38`). Idle asked for while the playing
  slot is in the other stance plays motion 6 (the weapon drawn or put away) with idle queued.
  The weapon plays its owner's slot.
- The blend (`FUN_1401c3a30`): each joint is mixed relative to its nearest ancestor that is not
  a helper (bone record +0xc bit 0; `FUN_1401c55e0`, `FUN_1401a7a80`) by mixing the X and Y axes
  (`FUN_1401c3970`); a helper is mixed relative to its parent by the two axes that agree most
  (`FUN_1401c6150`). IK chains are mixed by position and solved again (`FUN_1401c57b0`).
- Motion keys run at 60 a second (twice the declared 30-per-second frame window).
- A hit's spark comes from the attack owner's effect container (`FUN_1403d1b40`); a member's
  swing belongs to its weapon object, whose effects are `obj/W_EX020.a.us` / `W_EX030.a.us`.
  Frame trigger 1 starts an element of the object's own container (`FUN_14040dd70` case 1).

## What was built

1. `player::party_motion`: the controller (start, change with phase, queue, advance, blend
   hold, stance switch), the wish smoothing and speed snap, the ground mode's choice and the
   follow distances, with unit tests.
2. `player::party_members::Follower` drives it: transitions post the exe's start or change
   with its blend and queue; `animate` applies them or the ground mode's choice, advances the
   clock, and the attack's tick, root motion and end follow the clock (a blended act's waits
   count the held ticks: the release is offset by the hold). Gait comes from the follow state
   and the smoothed speed; the room's "close in at a run" shortcut is gone. Stance is kept.
3. `test_room/party_room.rs`: keys sampled one per tick (the members played every motion at
   half speed and only its first half); battle and field variants loaded, the weapon at the
   owner's slot; per-tick joints for body and weapon; the presentation shows the controller's
   clip and frame and, during a blend, mixes the joints the switch left behind toward the new
   start frame (`data::motion_eval::blend_poses_with_helpers`, the helper-aware blend); hit
   spheres, guard spheres, windows and voices read the shown clip. Effects: frame trigger 1
   elements of the member's own container (Goofy's Tornado and Turbo; the Bash's engine starts
   its own), sparks of member weapon hits from the weapon's container on the Shadow (with the
   victim's own hit sound) and on gallery enemies (they took Sora's Keyblade sparks before).
4. `--party-anim-tour <dir>`: Sora walks and runs toward the camera, stops and turns; the
   Shadow comes, the members fight with ordered acts, it leaves and Sora walks on. A tour-only
   camera frames both members. The log records each controller change (`party_anim`), the
   motion elements (`party_motion_effect`) and pose jumps at switches (`party_pose_jump`).

## The roster

- `FUN_1401b06e0` gives each character id its personality class: 2 Donald (0), 3 Goofy (1),
  5 Auron (2), 15 Pin (3), 6 Mulan (4), 7 Aladdin (5), 8 Jack Sparrow (6), 9 Beast (7),
  10 Jack Skellington (8), 11 Simba (9), 12 Tron (10), 13 Riku (11) (Pride Lands Donald and
  Goofy 0xc/0xd). Each class's command table (0x48-byte entries) is filled by a static
  initializer at start-up (`FUN_14000ac90` … `FUN_14000d2c0`); emulating them gives the
  tables: the weapon attacks, the two wait commands, the HP/MP items, and specials that are
  all `FUN_1401cc650` starting a named act of the member's own script (Auron `yukizakura`;
  Mulan `mushufire`, `flametongue`; Aladdin `slashrush`, `quickplay`; Sparrow `slash`,
  `shadowsnipe`, `bonecrusher`; Beast `shout`, `rush`; Jack `slidingfire`,
  `backstepblizzard`, `airthunder`; Simba `rush`, `quake`, `cure`; Tron `normal_attack1-4`,
  `overflow`, `thunder`; Riku `cure`). Voice names come from the table at 0x14074beb0.
- The records (0x80 bytes) give windows, the air jump's offset (+0x20, height = offset + rise
  up to PRTY word 2) and costs; the headers give the follow distances (Beast's differ), the
  wait timers and the idle hop.
- The wait commands (`FUN_1401ccba0` hold facing the enemy, `FUN_1401ccdf0` follow) are
  chosen only while no action is possible and end when one becomes possible or their timer
  runs out. After an action the member may hop (`FUN_1401b1d40`, `FUN_1403cbec0`: chance
  +0x64, a row of heading offset, height, speed, inertia and rise time; jump, fall five
  ticks before the apex, land).
- An act on the engine ends with its thread (`yukizakura` ends by queueing the fall);
  the member's controller takes over the motions it left playing and queued, and a fall on
  the floor lands (`FUN_1403cb940`).
- The member's think runs before its script each tick and copies its wish over the movement
  request (`obj+0x10`/`+0x1c`, the end of `FUN_1401b53b0`); during an act the wish is empty
  (`FUN_1401b15f0`), so a script's `obj_set_movement` lasts one tick (Aladdin's `quickplay`
  dashes while it asks, and stands for the slashes of 193).
- `trap_friend_get_action_param` (10:35, `FUN_140536d20` → `FUN_1401cc770`) returns the
  member's own 0x80-byte record whose id matches: Tron's four swings read their motion from
  it (151, 152, 181, 182; +4) and a jump value (+0x20); the command itself
  (`FUN_1401cc650`) starts no motion.
- `btlobj_target` (`FUN_1404319d0`) searches again (kind 2, Sora) only for an object of the
  enemy class (`+0x6c8` bit 4); a member's invalid target stays invalid (Beast's and Mulan's
  acts turned to Sora when the Shadow sank).
- At a motion's end with a motion queued, `FUN_1403c6740` starts the queued one and then sets
  the end bit (`+0x18c` bit 1): a script waiting on `motion_is_end` sees it (Beast's
  `shout`).
- The hand bone comes from SKLT by the object entry's byte +7 (`FUN_1403d7620`:
  `FUN_1403b5c10`, `FUN_1403eaae0`), not the character id (no row 13 for Riku); Beast, Jack
  and Simba have bone -1 and hold nothing. Their weapon objects carry only attack spheres.
- The weapon object is a WENT set chosen by the object entry's neo-moveset; every member's
  first entry is its own `W_` object except Ping's, which is Mulan's `W_MU000`.

Built: `player::party_roster` (records and headers of every member), `Spell::Act`, the
`Personality` of the follower (waits, hops), `KH2RW_PARTY=<model>,<model>` to pick the two
members (Donald and Goofy by default), weapons without motions held on the hand, the act
runner generalised to any act, HUD faces and voices by member. Goofy's Bash record now uses
its own height window (-500/1000; PR 109 had it negated).

Then, for the guests: the act runner clears the movement request each tick, answers the
action-param trap from the member's records (`Record::raw_words`), marks the member as no
enemy and is dropped when its act finishes (the next act, often begun the following tick,
had reused the finished one and ended at once). The engine keeps the end bit after a queued
start and searches again only for enemies. The loader finds the hand bone by the entry's
skeleton byte, takes the weapon from the WENT set, accepts weapon files without a model,
reads Ping's level rows as Mulan's (`FUN_1403e60c0`), plays a guest's voices from its home
world's set (`he0_auron`, ...) and accepts textures up to 8192 a side (Mulan's 192 × 7296).

Per member (tour `--party-anim-tour` with `KH2RW_PARTY`):
- Auron (`P_HE000`, run `auron5`): field and battle locomotion, attacks (ATKP 193/194 with
  sparks), `yukizakura` (ATKP 1272, 1576; its elements 1-7) completes and lands, hops, HUD face.
- Mulan (`P_MU000`, run `roster-r7`): her texture sheet (192 × 7296) loads (the inspection
  bound is now 8192 a side); sword attacks 151/152 (ATKP 369, 371); `flametongue` hops, then
  dashes through the Shadow (ATKP 622, its fire elements) and ends; `mushufire` plays 191 and
  ends (no hit on the Shadow in the run). Earlier runs had her dash toward Sora: the search in
  her act had found her own sword as the nearest enemy.
- Aladdin (`P_AL000`, run `roster-r7`): `quickplay` dashes at the Shadow (192, a request each
  tick), stops for the slashes of 193 and ends (ATKP 1175); `slashrush` circles it (191, 194;
  ATKP 1174); five acts complete. Earlier runs had him run 3000 units off and through the
  floor: the movement request had stayed set after the dash.
- Beast (`P_BB000`, runs `roster-r6`, `roster-r7`): no hand bone (SKLT -1), his weapon object
  only spheres; claw 152 (ATKP 570, 575, 220); `rush` turns on the Shadow and pounces
  (192, 193) and ends beside it; `shout` (191, ATKP 1166) ends with its motion. Earlier runs
  had the rush turn to Sora (the enemy search) and the shout hold idle for 300 ticks (the end
  bit lost under the queued idle).
- Simba (`P_LK000`, run `roster-r7`): no hand bone; `rush` is up to eight pounces of 30 ticks
  (192 restarted, held on its last frame, carried by the request each tick) and its body
  window (ATKP 1180) strikes again at the ATKP interval while it stays open: 119 hits in four
  rushes, not checked against the game; `quake` (ATKP 1179, elements 13-15) ends.
- Ping (`P_MU010`, runs `roster-r6`, `roster-r7`): Mulan's level rows (character 15 reads 6,
  `FUN_1403e60c0`) and her sword `W_MU000` (his WENT set); attacks 151-153 land (ATKP 369,
  385); no specials (his table has none). Before, he had no PLRP row and no weapon spheres.
- Jack Sparrow (`P_CA000`, run `roster-r7`): `shadowsnipe` fires its shots from range (192
  repeated, ATKP 1172 ×14), `slash` dashes through the Shadow and on (191, ATKP 1171),
  `bonecrusher` (193 ×3) plays and ends with no hit counted in the run (open); six acts done.
- Jack Skellington (`P_NM000`, run `roster-r7`): no hand bone; `slidingfire` lands (ATKP
  1178), `airthunder` and `backstepblizzard` (which ends in the air; the member falls and
  lands) play and end with no hit counted (open); five acts done.

## Checks

- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`:
  PASS. `cargo test --workspace --locked`: the new tests pass; two `enemy` tests
  (`traps::weapon::tests::*`) fail, and fail the same on the base `ce960b1` without this
  change. `cargo build --workspace --locked` and the release build: PASS.
- Joint data: skinning the stored joints reproduces the baked frames exactly; the blend's ends
  match both poses; Goofy's idle→walk blend moved up to 138 units a tick mid-way with the
  plain blend (his helper joint 122 turns 172°) and 6–8 with the helper-aware one.
- Clip ends: with one key per tick, the attacks, spells and the weapon switch end on the idle
  pose (under 0.2 units; before, 88–103).
- `--party-anim-tour` runs `run3`, `run4`: field idle/walk/run (slots 2, 7, 11) with blends
  and the walk-in/walk-out, phase kept from walk to run, the stance turned by the first battle
  motion and back to the field at a loop end once the flag dropped, Fire, Blizzard, Thunder,
  Cure, Tornado, Turbo and the Bash played, Tornado's elements 0 and 1 and Turbo's 3 started,
  sparks on member weapon hits. Pose jumps left at switches are the first blended frame (the
  blend shows at 3/8 the tick it starts) and the unblended starts the exe makes.
- `--party-hit-tour` run `hit1` (regression of T186): every case completes as in its earlier
  runs; the timer revive lands on the same tick.
- Not compared against a capture of the installed game: no frame or timing here is measured
  against KH2FM running.

## Uncertainty

- The IK chains' own blend (`FUN_1401c57b0`) is not reproduced; their joints use the axis mix.
- The formation table `DAT_14071d760` is filled at run time; the slot offsets stay the
  room's values. The AI's wait commands (entries 0 and 1) and the idle hop are not built.
- `FUN_14040d7f0` (blend forced off) and the controller's `+0x168` bit 0 (blend allowed) were
  not traced; blends are taken as allowed.
- Frame triggers 2 (footsteps, `FUN_1403b4fc0`), 8 (weapon sounds) and the members' own
  sound banks are not played: sound, not this task.
- The Bash's projectile path (T186) keeps Goofy throwing for long stretches.

## Verdict

PARTIAL. The controller, blends, stances, clip timing and the members' effects follow the
executable; nothing is compared against the running game yet.

## Next

The remaining party members (the world party rows of `memt` and their personalities), each
the same way; then the IK-chain blend and a capture comparison.
