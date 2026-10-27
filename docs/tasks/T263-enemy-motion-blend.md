# T263 — Enemy motion blends: the executable's clock and the blended pose

Owner Claude, 2026-10-27, requested by burba in the project thread ("start work on all enemy AI and animations 1:1
using ghidra"). Gate: development of enemy animation in the test room; no final gate is inferred. Prerequisites:
T133 (enemy script engine), T201 (the party's motion controller, which already follows FUN_1403c8a40), T262.
Build identity: local `main` after `42f8e02`, package 0.1.2, release launcher with the `skate3` feature. Source:
the selected Steam KH2FM build, validated each run; the install was only read. Ghidra 12.1.4 headless on private
read-only project copies.

Private evidence (outside the checkout): `%LOCALAPPDATA%\KH2FinalRewrite\research\drive-magic-20261026\g\range\`
(FUN_1403c8a40, FUN_1403c6740, FUN_1403c73c0, FUN_1403c88c0, FUN_1403c7c50) and this task's
`research\enemy-1to1-20261027\`; run logs under a private run folder outside the checkout.

## What the executable does

- **Starting a blended motion.** FUN_1403c8a40 (called by FUN_1403c86a0 / FUN_1403c88c0) takes a blend longer than
  2 ticks (`DAT_140623b18`) only when motion-player +0x168 bit 0 is set: it stores the blend at +0x48 and the start
  frame at +0x4c and sets the clock +0x44 to 2.0 at once. A blend of 2 or less starts the clock at the start frame.
  It also sets +0x34 to 1, the request to rebuild the pose (cleared by FUN_1403c73c0), which is not a pause.
- **Advancing.** FUN_1403c6740 adds one tick to the clock. While +0x48 is positive the motion holds: once the clock
  reaches the blend, +0x48 is cleared and the clock becomes `(clock − blend) + start`; the frame events between the
  start and that clock then fire. A plain step from exactly 0 to 1 is raised to 2.
- **The shown pose.** FUN_1403c73c0, with a blend running, keeps the pose the switch left (FUN_1403c8070) and the new
  motion evaluated at its start frame (FUN_1401a4d40 with +0x4c), and mixes them joint by joint — the same blend
  (FUN_1401c3a30 / FUN_1401c3970) Sora and the party use, with the weight clock / blend.

## What the rewrite did

- `enemy::sysobj` started its blend count at 0 and set it to 2 on the first advance, one tick behind the
  executable's clock, so every blended motion of a script enemy (and of Sephiroth) began its own frames a tick late.
- The room showed a battle actor's new motion at its start frame for the whole blend: the pose jumped at every
  switch.

## Changes

- `enemy::sysobj::Sysobj::motion_start`: the blend clock starts at 2 (FUN_1403c8a40); `begin_tick` ends the blend
  when the clock reaches it and continues from the start frame. `Motion::blend_weight` gives clock / blend while it
  runs.
- `enemy_gallery::GalleryActor::battle_pose`: when a script actor's blend runs, the joints shown last are kept as the
  blend's start, the new motion is evaluated at its start frame, and `motion_eval::blend_poses_with_helpers`
  mixes them (the helper joints from bone record +0xc bit 0, as the party's); the result is skinned with
  `pose_from_worlds`. A blend cut short by another starts from what was shown. Root motion is taken out of both
  poses first, as the room's sampling already does.
- Each motion switch logs `enemy_pose_switch` (farthest vertex move and the blend weight) in unattended runs;
  `KH2RW_ENEMY_NO_BLEND` shows the held start frame instead, for comparison runs.

## Checks

| Check | Result |
| --- | --- |
| New unit test `a_blend_starts_its_clock_at_two_and_holds_the_start_frame` (synthetic clip) | PASS |
| `cargo fmt --all --check`, `cargo clippy --all-targets --locked -- -D warnings`, `cargo test --locked` | PASS |
| Release launcher (`tools\Build-Launcher.ps1 -Skate3`) | PASS |
| Pose jumps at blended motion switches, combat tour (Shadow, Dusk, Soldier on their scripts), `enemy_pose_switch` | PASS: farthest vertex move median 26.1, max 60.5 over 31 switches (`combat-01`); with `KH2RW_ENEMY_NO_BLEND` median 97.7, max 177.2 over 35 (`combat-02`) |
| Combat tour verdict with the blend | PASS (both runs) |
| Sephiroth tour to his defeat with the new clock | PASS (see T262) |
| Side-by-side with the installed game | NOT RUN |

## Uncertainty

- +0x168 bit 0 (blends allowed) is taken as set for every enemy; its writer was not traced.
- The clock is whole ticks: a fractional blend's overshoot is dropped, and the motion speed +0x150 is taken as 1.
- When in the frame the pose update (FUN_1403c73c0 through FUN_1403bfd30) runs relative to the script was not traced:
  the room shows weight 2/blend on the tick the motion starts.
- The hand-written Soldier, Shadow and Dusk brains (`KH2RW_ENEMY_AI=port`) keep their own clock and show no blend.

## Verdict

PARTIAL. The blend clock follows FUN_1403c8a40 / FUN_1403c6740 and battle actors show the blended pose; not compared
with the installed game.

## Next

Motion sets with stance variants (FUN_1403c7c50 / FUN_1403c88c0 for enemies), the motion speed +0x150, and ranges
evaluated at a motion's start (FUN_1403c8a40 → FUN_14040e740, issue #155).
