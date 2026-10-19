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
