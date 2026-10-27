# T261 — Moving while attacking (range 41) and root motion in the air

Owner Claude, 2026-10-27, requested by burba in the project chat ("wisdom form doesnt flow and combat like the
original i cant shoot and walk at the same time, final form combos arent like the original effects are weird, air
combos play but they are stuck in one place. you need to make sure its 1:1"). Gate: development of the test room's
combat; no final gate is inferred. Prerequisites: T259 (form probes), T260 (body push), the movement work of
`docs/research/movement-feel-1to1-plan.md`. Build identity: local `main` after `d7c2bff`, package 0.1.2, release
launcher with the `skate3` feature. Source: the selected Steam KH2FM build, validated each run; the install was only
read. Ghidra 12.1.4 headless on private read-only project copies.

Private evidence (outside the checkout): `%LOCALAPPDATA%\KH2FinalRewrite\research\wisdom-air-20261027\` (decompiles
`g\a`, vtable symbols `g\sym1.txt` / `g\sym2.txt`, the vtable dump `g\tab1.txt`) and the earlier
`research\drive-magic-20261026\g\range\` (every function of 0x1403a0000–0x140430000); this session's scratch folder:
the motion-set range dumps (`ranges.py`, `magf_all.txt`) and the probe logs named below.

## What the executable does

- **The stick's request.** FUN_1403a89a0 (Sora's per-frame command step) first calls YS::PLAYER vtable +0x128
  (FUN_1403a78f0 → FUN_1403a8fa0), which writes the stick's request into obj+0x10 (direction) and obj+0x1c (walk
  speed below a 0.8 stick, run speed above), then runs the commands, then clears the request only when no range
  0–2 holds (`obj+0x18c & 0x1c`) **and** obj+0x18c bit 31 is clear. Bit 31 is motion range 41
  (FUN_14040d810 case 0x29).
- **Who reads it.** Mode 0 (FUN_140422350) turns the request into the ground velocity every physics step, eased
  through FUN_1403b7ad0 with PRTY words 10/11 under range 42; mode 1 (FUN_1403cb360 → FUN_1403b7ad0, 0.9 / 0.95)
  steers the air velocity with it. Neither checks the attack state, so under range 41 the stick moves Sora while the
  motion plays.
- **The facing.** The stick turns the facing through FUN_1403d6870 / FUN_1403d4de0 only when YS::PLAYER vtable +0x108
  (FUN_1403a7830 → FUN_1403d65f0) allows it, and FUN_1403d65f0 refuses while bit 31 is set: under range 41 Sora moves
  with the stick but keeps facing the target (range 28 turns him toward it).
- **Which motions carry range 41** (every range record of each form's motion set, parsed exactly): Wisdom Form's
  shots and spells (`P_EX100_MAGF.mset` 151 Shoot ticks 0–46 with range 42 throughout, 154 finisher 10–80, 181 air
  shot whole; spells 56–73 from their release, e.g. Fire 56 from 24, 71/72 0–8) and Final Form's swings and spells
  (`P_EX100_ULTF.mset` 151 and 152 whole, 153 from 32, 154 16–36, air 181 16–36 and 182 from 16, spells 56–73
  whole, and its jump/land motions 3, 5, 17, 19 briefly). Master Form's 184 (0–200) and Limit Form's 246 (19–100)
  have it too; base Sora, Valor and Antiform have none.
- **Root motion in every mode.** FUN_1403b89a0 adds FUN_1403b7ca0 — the motion root's change this frame (obj+0x254),
  turned by the facing, all three axes — to the mode's displacement whatever the mode is. Final's air swing 182 moves
  200 forward by its root, its air finisher 183 300 forward and 300 down.

## What the rewrite did

- Range 41 was not read. Every attack and cast dropped the stick (the request was only taken under range 0–2), so
  Wisdom stood still while shooting and Final's swings could not be steered; in the air Wisdom's shot hung in place.
- Root motion was applied only to ground attacks, the dodge roll and scripted motions. Air attacks, air casts and
  ground casts ignored theirs, so Final's air swings stayed where they started.

## Changes

- `Clip::stick_move` (range 41) is read from the motion triggers.
- `player::Player::tick`: under range 41 the ground request is the stick (walk / run speed by the 0.8 threshold),
  eased by range 42 as before; in the air the stick feeds mode 1's steering. The stick does not turn the facing
  under range 41, nor does a chained attack take the stick's direction as its facing (the room's carry-over).
- `Player::root_delta`: the root's change (FUN_1403b7ca0) for attacks and casts on the ground and in the air; the
  air keeps the vertical part (the arc absorbs it while gravity runs, as the exe's absolute arc does).
- The form probe (`--magic-tour --form-moves`) gains a ground and an air combo mashed with the stick held right.

## Checks

Form probes `--magic-tour --form-moves --magic-form n` (scratch logs `mv-f0`, `mv-f2`, `mv-f5`; the strafe segments
hold the stick right with the Shadow 110 ahead; positions from the `move_probe` events):

| Check | Result |
| --- | --- |
| Wisdom, ground shots with the stick held (151 ×1, finisher 154) | PASS: Sora moves about 11 a tick (Wisdom's run is 12) through the shots and the finisher, 1442 units over 215 ticks, while range 28 keeps turning him toward the Shadow; before, Sora stood still for the whole string |
| Wisdom, air shot 181 with the stick held | PASS: drifts 6–10 a tick with the stick through the shot (easing up to speed through 0.9 / 0.95) (before: hung in place) |
| Wisdom, the same strings without the stick | PASS: unchanged, no drift (no request) |
| Final, ground combo without the stick (151, 152, finisher 153) | PASS: 151 closes from 110 to the Shadow's body (centre distance 62–66) as in T260; 152 and 153 stay at it |
| Final, ground swings with the stick held | PASS: 151 and 152 follow the stick on top of their root motion (7–14 a tick, eased by Final's 0.94 / 0.99) |
| Final, air combo (182, 181, 182) | PASS: 182 now travels by its root (200 forward) and its hop to the Shadow (centre distance 110 → 23); before, about 6 units in 30 ticks |
| Final, air swing with the stick held | PASS: 182 drifts with the stick from tick 16 |
| Base Sora (no range 41) | PASS: unchanged; the air combo still hops 181 → 182 → 183 onto the Shadow and lands, the ground swings do not move by the stick |
| New unit tests `range_41_lets_the_stick_move_an_attack_without_turning_it`, `an_air_attack_moves_by_its_root_curve` | PASS |
| `cargo fmt --all --check`, `cargo clippy --all-targets --locked -- -D warnings`, `cargo test --locked` | PASS |
| Release launcher `cargo build --release --features skate3 --locked` | PASS |
| `cargo build --locked`, smoke `cargo run -p main --locked -- --frames 180` | PASS |
| Side-by-side with the installed game | NOT RUN (no save with Drive forms on this PC) |

The three probes ran at the same time: each reached its scheduled `exit` event, but they returned exit code 1 and
wrote 1×1 captures. Final run alone (`fin-f5`) exits 0 with full captures and the same movement.

Final Form's effects, from the `fin-f5` captures: the swing trails, sparks and the keyblade-grip elements 52–56
(PAX data 14, started by 152's frame triggers) draw and now follow Sora as he moves. During 152 and the finisher
153 a large tan, translucent disc of elements 52–56 surrounds Sora (capture 07). Its blend comes from the traced
ALPHA rule (`effect_draw::Blend::of`, FUN_1404b6860), so the room draws what the data asks through the current path,
but whether the original shows the same disc could not be checked: NOT RUN (no footage or save to compare). The
refraction programs 145/160 behind Final's element 29 are still not drawn (#148).

## Uncertainty

- Range ends: FUN_14040e740 treats a range as `start <= t < end`; the room tests `start <= t <= end` everywhere. Not
  changed here.
- The order of FUN_1403b4980 (which also clears the request) relative to FUN_1403a89a0 within a frame was not traced
  from the frame loop; it is inferred from locomotion working at all (the request must reach the mode).
- Final Form's "weird" effects: see Checks. The refraction pass (#148, effect programs 145/160, Final 29) is still not
  drawn.
- Nothing was compared side by side with the installed game: no save with Drive forms exists on this PC.

## Verdict

PASS for the traced rules in the room: under motion range 41 the stick moves Sora through Wisdom's shots and
spells and Final Form's swings, on the ground and in the air, without turning him; attacks and casts move by their
root motion in the air as on the ground, so Final's air combo travels. Final Form's effect colours are NOT RUN
against the original. No final gate; nothing was compared side by side with the installed game.

## Next

Compare Final Form's effects (the elements 52–56 disc, the trails) with footage or a save with Drive forms; the
refraction pass (#148); range ends as `start <= t < end` (FUN_14040e740) across the room.
