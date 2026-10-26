# T260 — Battle objects push each other apart (Final Form's combo no longer runs through the enemy)

Owner Claude, 2026-10-26, requested by burba in the project chat ("final forms combo isnt like the original he
just attacks in place you need to recrate it 1:1 using ghidra"), with the coordinator's note to check Valor, Master
and Antiform the same way. Gate: development of the test room's combat; no final gate is inferred. Prerequisites:
T259 (form probes), the movement work (`docs/research/movement-feel-1to1-plan.md`), the townspeople's push
(look-02). Build identity: local `main` after `45c8080`, package 0.1.2, release launcher with the `skate3`
feature. Source: the selected Steam KH2FM build, validated each run; the install was only read. Ghidra 12.1.4
headless on private read-only project copies.

Private evidence (outside the checkout): `%LOCALAPPDATA%\KH2FinalRewrite\research\drive-magic-20261026\g\range\`
(every function of 0x1403a0000–0x140430000 decompiled), `g\phys`, `g\pair`, `g\dd-w.txt`; this session's scratch
folder: `far-f*.jsonl` (before), `push2-f*.jsonl` (after), the trigger dumps of each form's motion set
(`msetall.py`) and `far_summ.py`.

## What Final Form's combo is made of (executable and data)

- **Records** (00battle `ptya` set 5): the ground strings are 151 (target 180–400 ahead), 152 (the close second
  hit, an ALWAYS record) and the finisher 153; the hop speed band of every ground record is [0, 0], so the attack
  start's lunge (FUN_1403d78d0's non-hop branch, ((dist − reach) + PTYA +0x3c) × SSTM +0xa8 clamped to the band)
  gives no speed. Air records hop 14–47 toward the target ([0, 16] speed).
- **Motion** (`P_EX100_ULTF.mset`): 151's root motion is 400 forward, 152's none, 153's 100; range 28 turns toward
  the lock-on target by its parameter × 0.0174533 (22°) a tick once the target is farther than 50
  (FUN_1403d4b10 → FUN_1403d3a90, `DAT_1405b15b4`, `DAT_1405b532c`), measured from the position after this tick's
  root motion; range 42 makes mode 0 steer with PRTY words 10/11 (0.94 / 0.99 for Final); range 41 (obj+0x18c
  bit 31) only keeps the stick from moving him (FUN_1403d65f0). The request is cleared every frame
  (FUN_1403b4980), so an attack never moves by the stick.
- **Bodies** (FUN_1403b89a0 → FUN_1403b9090): every object's movement runs FUN_14040ad70 against every other
  active object's kind-1 collision shapes (obj+0xa00, FUN_1403ba970). Overlapping shapes (FUN_1403c94c0) are
  separated (FUN_1403c90b0, 5 past touching) and the separation is split by weight (FUN_1403b5e00: objentry
  +0x50, 10000 under motion range 44); a weight of 0, range 31, an attached object or a mode with flag bit 2 is
  immovable (FUN_1403ba7d0).

The rewrite already played the records, root motion, range 28 and range 42. It had no body push between Sora and
battle enemies (only the townspeople's), so Final's 400-unit swing carried Sora straight through the Shadow: with
the Shadow 300 ahead, 151 ended with Sora 89 past its centre, 152 swung at nothing, and 153 ran into it again
(centre distances 300 → 17 → 89 → 6). The same happened to every form's lunging swings and finishers (Valor 155
passed 99 → 13 → 80; Anti 155 → 150; Master 184 → 182).

## Changes

- `player::body_push`: FUN_14040ad70 with FUN_1403c94c0 / FUN_1403c90b0 for cylinders and spheres, the weight
  split and the immovable cases, with a unit test.
- The test room places each object's kind-1 shapes (the collision table's default groups) around it
  (`test_room/actor_push.rs`) and pushes Sora against the room's Shadow (`combat_tick`) and against every battle
  actor of the arena (`enemy_battle::tick`). Sora's shapes and weight follow the shown kit (Antiform weighs 25);
  motion ranges 44 (heavy) and 31 (immovable) are read into `Clip`.
- Sora's weight is now his own objentry's (`P_EX100`, 100). It was read from entry 0x236, which is only the
  scripts' alias for the player (FUN_1403b4110) and weighs 0; the vacuums (FUN_1403f9130) scale with the same
  weight, so their pull on Sora now uses 100 as in the executable.

## Checks

| Check | Result |
| --- | --- |
| `--magic-tour --form-moves --magic-form n`, Shadow 300 ahead then 110 (forms 0–6) | PASS: Sora closes in and stays at the Shadow's body (centre distance 57–66) through Final's 151, 152, 153; hits on the Shadow rise in every form (Final 61 → 67, Valor 23 → 25, Master 43 → 49, Anti 44 → 50) |
| New unit test: the weight split, an immovable body, bodies apart | PASS |
| `--enemy-combat-tour` (three arena battle actors) | PASS for the push itself: 108 pushes, about 1 unit a tick while an actor walks into Sora, up to 11 when one runs into him; the fight plays out differently (Sora lands 4 hits instead of 14 and is defeated at tick 1684, so the unattended run stops on the Game Over screen; before, it ended at 1800) |
| `cargo fmt --all --check`, `cargo clippy --all-targets --locked -- -D warnings`, `cargo test --locked` | PASS |
| Side-by-side with the installed game | NOT RUN (no save with Drive forms on this PC) |

## Uncertainty

- The push is resolved once a tick after both objects moved, not inside each object's own movement in object
  order; the pushed positions do not run the wall sweep again.
- Shapes on a bone (rather than the object frame, 0x4000) are taken at the object's origin; the bodies read here
  (Sora's forms, the Shadow) all sit on 0x4000.
- Immovable modes (flag bit 2), the pair filter FUN_140549690 (same group when both carry obj+0x6c8 bit 27) and
  FUN_1403baad0 (stopped objects) are not modelled; Donald and Goofy are not pushed by enemies here.
- Whether the original's Final 152 also stands still at close range is read from the data (no root motion, no
  lunge speed), not from footage.

## Verdict

PASS for the traced rule in the room: Sora and battle enemies push each other apart by weight, so the forms'
lunging swings stop at the enemy instead of passing through it; no final gate, not compared side by side with the
installed game.

## Next

Push between Sora and Donald/Goofy and between enemies; the wall sweep after a push; a side-by-side check once a
save with Drive forms exists.
