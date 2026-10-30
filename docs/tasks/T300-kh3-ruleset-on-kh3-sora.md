# T300 — The KH3 ruleset runs on KH3's own Sora

Owner Claude, 2026-10-30, requested by burba in the project thread ("make sure it's 1:1"): close every approximation T282
left, so the KH3 option in the LAN duel behaves as KH3. Gate: none of the roadmap's. An addition outside the original
game, in the LAN duel only; no fidelity claim for the rewrite is made and single player and the KH2 ruleset are
unchanged. Prerequisites: T279-T282. Build identity: local `main` from `38a4626`, package 0.1.2. Sources: the selected
Steam KH2FM build as before; the player's Steam KH3 install, only read.

**Nothing of KH3 is compiled in.** The recipes (`source/data/src/kh3/recipes/*.tsv`) name where a value is: a DataTable
cell, a blueprint default, a clip's notify, or a file offset in the player's `KINGDOM HEARTS III.exe`. The values are read
when the ruleset loads. A digest of the bytes read from the executable rejects any other build of it, which leaves the
KH3 ruleset unavailable; the build reads no private file. Offsets were chosen by searching the executable for the same
bytes, so most are not the routine's own constant: they prove nothing about which routine holds a value, only that the
player's own executable supplies it. Literal numbers remain in translated routines (`game/kh3_sora`, `game/kh3_rig`,
`kh3_autose.rs`) and in the T279-T282 layer (`player/src/kh3/tuning.rs`, `look.rs`, `ability.rs`), under the owner's
standing permission for routines translated into Rust.

**Reference.** `StrawHatPro/kingdom-hearts-x-elden-ring` (MIT, `CREDITS.md`, `NOTICE`): `crates/kh-core` is ported as
`source/game/kh3_sora`, the solvers of `crates/kh-anim` as `source/game/kh3_rig`, the detector of `crates/kh-audio` as
`kh3_autose.rs`. Its Node tools were not run; its sheets are not copied.

## What changed

| Item | Now | From |
|---|---|---|
| Movement, states, combo rules, traction | `kh3_sora`'s state machine and movement component on the arena's geometry | the reference's reading of the executable, numbers from the install |
| Damage | `CalcDamage` at KH3's level 99 stats, HP 265; no duel scaling, so no Match-page option | same; its control flow read again in Ghidra at 0x140632240 |
| Risk Dodge | All four types; warp destination (0x1413ACA10) and over-slide steering and side test (0x1413B8280) from the decompiled routines | Ghidra, this task |
| Abilities | 62, with the survival, MP and system ones | the reference |
| Footsteps | The auto SE detector on `BoneSet_Sora1`, `FsTable_Sora` by surface, explicit notifies, enable windows | the install; Mirage Arena = marble, test room = rock, a duel choice |
| Helper bones, cloth | KineDriver (313 operators, 102 bones), Bonamik (252 particles, 86 bones), keychain as a second solver; the clips' Bonamik notify states and `Wind` curve | the install |
| Effects | `ead_*` rows by group, effects that follow a bone and end when stopped, also over the wire; per-texture address modes; loop end sounds | the install |
| GPU emitters | Their own capacity, no thinning, no mesh budget | run on the evaluator's CPU threads, not on the GPU |
| Second Form | The core's Formchange and finish | run, see below |
| Joiner | 45 s handshake while KH3 loads | T282's timeout |

## Not 1:1, and why

- **Materials and lighting**: Sora and the Keyblade still use `slot_look` on the room's material. KH3's are compiled UE4
  shaders; translating them at run time needs a shader-map reader, a DXBC translator, uniform expressions and the
  engine's deferred lighting and `KHS` post process, none of which exist here.
- **GPU emitters run on the CPU**: no cap and no thinning, but not KH3's GPU simulation. Module behaviour is the
  evaluator's.
- **Cascade defaults**: `ParticleModuleRequired`'s constructor was read (0x1442B03B0) and agrees. The others are the
  evaluator's fallbacks, not read one by one.
- **Sound**: no reverb bus or distance low-pass, whose cut-off mapping nobody has read; `STATE_SLOW_START` is not sent.
- **Other Keyblades' shotlocks and Formchanges**: Kingdom Key only. The magic sets of other forms are not in the recipes.
- **Bonamik's jitter**: seeded, because the game's thread-local stream cannot be reproduced.
- **Marked in the core**: pole grab pose, wall-run corner rolls and leaning walls, the ledge fallback's attempt 3, slope
  body pitch, adhere actors of cloud and discharge, Risk Dodge's direct-move as a velocity.
- **Not compared with KH3 running**: every check here is against the executable's or the data's numbers. KH3 itself was
  never run.

## Evidence

Private, outside the checkout: `%USERPROFILE%\kh2-kh3-ref\` (`probe\sheets.json`, `probe\sheets_exe.json`, `runs\e01-e09`,
`research_rd\`, `scripts\core\`).

## Checks

Filled in from the final run in the private tree (`HEAD` plus this task's files, without other threads' uncommitted
edits).

| Check | Result |
|---|---|
| Values read from the executable against the earlier reading | PASS: 28,846 cells, 22 differ, all within rounding of the sheet's text |
| Rig counts against the data | PASS: 313 operators, 102 bones written, 86 driven, 252 particles |
| Second Form in a run, gauge forced by a temporary edit | PASS: StyleChange, finish command, StyleFinish (`runs\e09`) |
| Clean copy `cargo build --locked` at `d53517d` | PASS, without a private file. Later commits NOT RUN at task-record time |
| `cargo clippy` | FAIL: `enemy_catalog.rs:572`, another thread's commit; none in this task's files |

## Verdict

PARTIAL. The ruleset runs on KH3's Sora with KH3's numbers read from the player's install; materials and the items above
are not 1:1.

Next: KH3's material shaders (a task of their own).
