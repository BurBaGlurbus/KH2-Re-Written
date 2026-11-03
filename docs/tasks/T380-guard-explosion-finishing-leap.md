# T380 — Sora's guard arm, Explosion and Finishing Leap

Owner Claude, 2026-11-03, requested by burba in the project thread: "fix the ik for sora for his guard, his other arm is twisted and both hands are on the keyblade, explosion attack does no damage, finishing leap does nothing and looks incorrect". Gate: test-room combat and motion evaluation; no final gate is inferred.
Prerequisites: T090, T105, T372, T375, T377. Build identity: local `main` after T379 (1023819) plus this change, package 0.1.2. Source: the selected Steam KH2FM build; the install was only read. Existing Ghidra decompiles were read; no new Ghidra run.

Private evidence (outside the checkout): run logs and captures under `%USERPROFILE%\kh2-issues-out\sora-gef\` (`runC` guard, `runD` Explosion, `runE` Finishing Leap, with `guard-after.png`, `explosion-after.png`, `leap-after2.png`); the motion triggers in `research\playable-room-20261008\guard-01\pex100-triggers.txt`; the decompiles of FUN_1401d2c80, FUN_1403b9090, FUN_1403cb940 and FUN_14040d810 under `research\byte-match-20261025\ghidra\out-5\` and `research\drive-magic-20261026\g\range\`.

## What the executable does

1. **Constraint activations.** FUN_1401d2c80 runs before the evaluator FUN_1401d3b80 and fills one flag per constraint. A constraint with no activation entries keeps its record's byte +1. Otherwise the motion time (the time the curves are sampled at) picks an entry from the table at header +0x58: the last entry's flag from its time on, the opposite of the first entry's flag before it, and between them the entry found by halving. The evaluator reads these flags, not the record byte.
2. **Guard (P_EX100 motion 173).** The constraint that puts the left-hand target (joint 274) on the Keyblade grip (joint 282) has entries (−6e7 on, 4 off, 74 on), and the hand's orientation constraint (joint 85) has (−6e7 on, 4 off, 82 on). The clip is 60 ticks long, so from tick 4 to the end the left hand is off the Keyblade.
3. **Explosion (motion 166) and Finishing Leap (motion 167)** hit through the actor motion's range-10 windows: Explosion ATKP 911 then 912 on attack group 19; Finishing Leap ATKP 854 on group 18 at ticks 34-38. Both groups are one round shape (kind 0, radius 350, height 180, lift 120) on bone code 0x4000, the object's origin (FUN_1403b50f0). Their weapon motions have no hit windows.
4. **Finishing Leap's root** climbs to 66 by tick 24, comes back to 0 at tick 28, holds there through the strike, then climbs to 150 by tick 64. A no-gravity range covers ticks 1 to the end. FUN_1403b7ca0 adds the root's Y to the displacement in every mode. FUN_1403b9090 skips the land test while a no-gravity range holds (obj+0x18c & 0x1000020, or bit 6), so the hop's return to the floor at tick 28 does not land.

## Changes

- `data::motion_eval`: `constraint_active`, the activation pass above. The evaluator gates every constraint on it. `evaluate_with_clock` takes `timed_activations`: the game passes true, and `motion_eval_compare` passes false unless `ACTIVATIONS=1`, because the PS2 reference adapter calls the evaluator alone with the record flags.
- `character_kit::AttackShape`: the object's ATTACK shapes keep origin shapes (0x4000), with the round shape's height and lift. The Shadow arena (`combat_tick`) and the field battles (`enemy_battle_hits`) test them as ellipsoids (`volume_hits_sphere`). Before, every shape was taken as a bone sphere, and origin shapes found no bone, so they never hit.
- `player`: a ground attack whose root climbs is lifted into the jump mode with its motion still playing (`lifted`; the action stays `Attack`, so the room's trigger scans and voices do not restart). While a clip's no-gravity range holds, an airborne move that meets the floor does not land.

## Checks

| Check | Result |
| --- | --- |
| `cargo fmt --all --check`, `cargo clippy --all-targets --locked -- -D warnings`, `cargo test --locked`, `cargo build --locked`, smoke `--frames 180` | PASS on local main |
| Unit tests `timed_activations_switch_a_constraint`, `a_climbing_root_lifts_a_ground_attack_and_the_hover_skips_the_land` (generated data) | PASS |
| Guard motion offline, activations on against off | the left hand (joint 80) is up to 83 units away from the grip between ticks 4 and 40; the right arm (170-172) does not move |
| Arena run `runC`: Square at tick 760 | PASS by eye: Keyblade forward in the right hand, left arm out behind, left hand free |
| Arena run `runD` (Explosion equipped, party out) | PASS: the third hit plays motion 166; ATKP 911 hits twice for 2 each, Blow reaction, the Shadow dies |
| Arena run `runE` (Finishing Leap equipped, party out) | PASS: Square after the second hit plays motion 167; it hops, comes down without a LAND, ATKP 854 hits for 8 (Blow), Sora ends 150 up, falls and lands |
| Field rooms (`enemy_battle_hits`) | NOT RUN: same shapes and the same overlap routine as the arena path |
| Against the installed PC game | NOT RUN |

## Uncertainty

- The PS2 reference comparison of T372/T375 ran without activations. With `ACTIVATIONS=1`, the installed motions with activation entries are expected to differ from that reference (T375's research run saw guard off by 117 units that way); they were not re-run.
- Origin shapes now hit for every kit, Drive forms included. Their groups were dropped before, so any Drive motion that uses one now deals damage it did not deal before. That is the executable's rule, but each form was not re-checked.
- The other attachment codes (0x4001..0x4009, bone pairs) are still left out.
- In the exe, Finishing Leap stays in the ground mode while it rises. The rewrite uses the jump mode with gravity held. The paths agree for this motion, but the exe's ground-mode handling of a climbing root was not traced.
- The guard result was judged from captures against burba's description, not against the PC game.

## Verdict

PASS for the three reported problems in the test room. NOT RUN against the PC game.

## Next

Re-run the motion comparison with `ACTIVATIONS=1`, and spot-check the activation motions in their rooms (P_EX100 slot 30, M_EX670, P_NM000).
