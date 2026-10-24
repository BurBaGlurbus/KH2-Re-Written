# T227 — Effects bound to a battle actor's bones

Owner Claude, 2026-10-24 (T225 and T226 are claimed by #135 / #141), requested by burba in the
project chat ("keep making this until there's nothing left to do"), from T224's [I] note. Gate:
VFX (P19/P20 work; nothing here is inferred as a gate pass). Prerequisites: T224. Build identity:
branch `feature/actor-fx-bones` cut from T224's branch, merged with `main` at `2d8eb84`; package
0.1.2, Rust 1.99.0 Windows MSVC. Source: the selected Steam build 15194255, validated at every
run; the install was only read.

Private evidence (never in Git): the run `kh2-boss-out/cmp8/b`. No Ghidra MCP was used; no new
decompiles were needed.

## What the executable does

A bound element follows the object's bone matrix (FUN_1403b50f0; flags 1 / 2 / 4), read through
FUN_1401a7af0 with its y and z columns negated, as `spell_fx` already does for Sora's bones.

T224's actor anchor gave a battle actor's effects his root and heading only: an element bound to
one of his bones started at his feet.

## Steps

1. `EnemyCombat::sample_shapes` returns the model-space bone matrices it evaluates and the root
   translation taken out of them; the battle actor keeps them each tick and the room publishes
   them next to the actor's pose (`actor_bones`).
2. `spell_fx`: an actor anchor resolves a bone code below 0x4000 to root x (bone less the removed
   root) x the y/z negation.
3. The effect start log records the start position.

## Checks

`cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`,
`cargo test --workspace --locked`, `cargo build --workspace --locked`, the release helper,
`python tools/progress.py` and its tests; one Sephiroth run.

## Results

- fmt, clippy (no warnings), workspace tests (all pass; one test ignored as on `main`; the
  actor-anchor test now covers a bone), workspace build, release helper: PASS.
- Sephiroth (`cmp8/b`, 1200 ticks, exit 0): the effect bound to bone 161 at tick 767 starts at
  (-115, 140, -677) with him at (-116, 0, -679), 140 above his feet; before, his root.
- NOT RUN: the Limit Form, party-limit and summon tours (the change only reaches battle actors'
  bound effects, which those tours do not start); wrap-up was asked for.

## Uncertainty

- Nothing was compared with the original game.
- The body-centre codes (0x4001 / 0x4009) still take Sora's heights [I].
- Seen, not looked into: the Shadow Flare effects report their start at the origin before the
  script's first `effect_set_pos` (they are unbound starts placed by the script); the same on
  T224's build.

## Verdict and next task

PARTIAL. Bound effects follow the battle actor's bones; nothing is claimed 1:1 by observation.

Next: the open Sephiroth issues (#149, #151, #153, #154, #155, #156).
