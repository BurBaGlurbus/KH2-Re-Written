# T377 — The IK chains' own blend on a motion switch

Owner Claude, 2026-11-02, issue #73. Gate: motion-switch blend against the executable's own blend routine; no final gate is inferred.
Prerequisites: T201 (the helper-aware blend), T263, T375. Build identity: written against local `main` 9a654a1 in a separate clone, applied after T376 (2207369), package 0.1.2. Source: the selected Steam KH2FM build; the install was only read. Plain Ghidra 12.1.4 headless on a read-only project copy; the game was not run.

Private evidence (outside the checkout): `%USERPROFILE%\kh2-ik73\` — `harness\` (the emulator script `BlendOracle.java` and the case generator/comparer), `scratch\` (generated cases, the exe's outputs, logs) and `real\` (cases built from evaluated installed motions, `compare-01.txt`, `regress-01.txt`). Decompiles and disassembly under the research folder `issues-20261102\b73\`.

## What the executable does

FUN_1401c3a30 (called through FUN_1401a4cd0 from the motion controller FUN_1403c6aa0) blends two pose snapshots. A snapshot (FUN_1401c5d00) keeps each joint's scale with its X rotation channel in word 3, the local matrices, and a chain list that the motion builds (FUN_1401d2810, interpolated-motion vtable slot 0x18):

- every joint-order entry of IK level 2 or 3 on a model bone, at most 32, sorted by joint;
- per chain, in the frame of the top's nearest non-helper ancestor (scaled matrix inverted): the top position, the point one middle-joint length along the middle joint's X, and the top's Y and Z rows.

A snapshot of an object that was itself blending has no chains (FUN_1401a4cd0 clears the object's motion).

The blend walks the joints in index order with a cursor into each list. A top that both lists hold with the same level takes the chain branch (0x1401c3e6a..0x1401c4bb6) for itself and the next joint, and the walk resumes after that middle joint; a listed chain whose top is a middle joint is never reached and holds back every later chain of that list. In the branch:

- the records are mixed linearly and taken through the blended matrix of the parent's nearest non-helper ancestor into the chain parent's unscaled frame (on every installed chain checked the parent is a helper, so this is the record's own frame);
- the pole is the mixed Y row when the Y rows agree at least as much as the Z rows (dot products) and it is not within 0.96 (DAT_1405b3fb8) of the aim; otherwise the normalised mixed Z row crossed with the aim;
- FUN_1401c57b0 solves with lengths of bone length (+0x1c) times the X scale, the arm bend for level 3, no roll on the top and the middle joint's X channels mixed the short way round as its roll;
- the top keeps the mixed position, the middle joint sits one top length along the top's X; the end joint blends as a plain joint against the middle joint.

A foot table from the caller (+0x14 flags, 0x200000 end, +0x1c joint, bits 0x80000/0x100000) moves a chain's target onto the ground by a cast (FUN_1401712b0); the rewrite has no such table, so that step is left out.

## What changed

- `motion_eval::blend_poses_with_chains` (the blend above), with `ChainBlend`, `ChainTop` and `bone_lengths`; `Evaluated::chains` carries each evaluation's list (filtered to the shared bones in `evaluate_shared`). The two-joint solve is shared with the evaluator (`two_joint_turns`, same arithmetic).
- Callers pass the chains and lengths, and keep no chains for a blended pose: NPCs (`npcs.rs`), gallery and battle enemies (`enemy_gallery.rs`), Donald and Goofy, body and weapon (`party_room.rs`), and Sora (`test_room.rs`). Sora used the blend without helper joints; it now uses the helper-aware blend as the others do (the exe has one blend for every object).
- Not changed: the world map's actors and `pose_sheet` still use `blend_poses` (their driver was not checked).

## Checks

| Check | Result |
| --- | --- |
| `cargo fmt --all --check`, `cargo clippy --all-targets --locked -- -D warnings`, `cargo test --locked`, `cargo build --locked` | PASS |
| Smoke `--frames 180` | NOT RUN |
| Exe FUN_1401d2810 + FUN_1401c3a30 in Ghidra's emulator against `blend_poses_with_chains`, generated 35-joint skeletons (helper and plain tops, arm chain, a parent that is not a helper, level mismatch, stuck walk, the Z fallback), unit and uniform scales, 38 cases at weights 0.25-0.75 | PASS: every joint within 4.6e-5 translation, 2e-6 rotation; the old blend was off by up to 65 units on chain joints and equal on the rest |
| The same on evaluated installed motions: P_EX100, P_EX100_MAGF, P_EX020 (24 cases) | PASS: every joint within 8.1e-5 translation, 3e-6 rotation (old: up to 4.3, 16.6 and 6.3 units on the chains) |
| P_EX030 (9 cases) | Chains and the joints below them within 5.3e-5 translation, 1e-6 rotation; joints 42/43 differ in one pair, equally for the old and new blend (below) |
| B_EX100 (6 cases) | FAIL outside the chains, equally for the old and new blend (non-uniform scales, below); its chains are 3.2 units off, inherited from above them (old: 4.8) |
| Evaluator after the solve was shared: PS2 reference, 14 chain motions, 84 samples | Reports byte-identical to 9a654a1 |
| PS2 reference for the blend itself | NOT RUN: the reference adapter recompiles only the motion evaluator's own code, and its RAM image holds no code, so the PS2 blend cannot be called |
| Against the installed PC game | NOT RUN |
| On local main after T376: `cargo fmt --all --check`, `cargo clippy --all-targets --locked -- -D warnings`, `cargo test --locked`, smoke `--frames 180` | PASS |
| Room tour tt14 → tt13 on local main (`ik73` under `%USERPROFILE%\kh2-issues-out\s162\`) | PASS: Sora walks and swings as in T372's tour |

## Uncertainty and open differences

- Outside the chains, the plain blend still differs from the exe where scales are not uniform. The exe mixes local scales and multiplies them down the hierarchy, and its helper re-expression (FUN_1401c55e0) multiplies unscaled local matrices. B_EX100's x-only scales on joints 2, 5, 123 move most of its joints by up to 41 units. Generated poses whose scales differ between the poses show the same.
- P_EX030 slot 6 to slot 8: joints 42/43 rotate up to 0.0055 off the exe, equally in the old and new blend (plain path, not chains).
- Not reproduced: the foot table's ground placement and the attachment list pass (snapshot +0x20, FUN_1401c5190). A chain branch on level 1 cannot occur (the builder skips it), so its stale-joint path is not modelled.
- The emulator answers sqrtf/sinf/cosf in Java; differences there are at float rounding.

## Verdict

PASS: the chain blend matches the executable's own routine on generated and installed poses with uniform scales.

## Next

The plain path's scale handling (local scale mix, FUN_1401c55e0's matrix products) for non-uniform scales; the foot table once ground placement exists.
