# T305 — A made object's OBJ_INIT waits for its first update; statics across fighters; boss-class objects run; part colours

Owner Claude, 2026-10-30, requested by burba in the project thread. Gate: development of enemy AI in the test room; no
final gate is inferred. Prerequisites: T299, T301, T304. Build identity: local `main` after `88bbdec`, package 0.1.2,
release launcher with the `skate3` feature. Source: the selected Steam KH2FM build, validated each run; the install was
only read. Ghidra 12.1.4 headless on private read-only project copies (plain Ghidra, no MCP).

Private evidence (outside the checkout): `%LOCALAPPDATA%\KH2FinalRewrite\research\enemy-1to1-20261027\g\boss\`, `g\pc\`,
`research\drive-magic-20261026\g\range\` (FUN_1403b44d0, FUN_14040f020, FUN_14040efe0, FUN_1403cd670, FUN_1403cd4c0,
FUN_14040d810, FUN_1403cbdd0, FUN_1403cc3d0); the object scripts disassembled in a private run folder; run logs there
(`boss1`, `boss2`, `probe11`, `sum0`, `sum1`).

## What was wrong

Running the boss-class objects (types 3 and 12) on their scripts, behind a new switch, showed the following. The first four
also affect ordinary enemies.

- **A made object's OBJ_INIT ran inside its maker's `sysobj_appear`.** B_LK120's maker sets the new object's unit argument
  on the line after `sysobj_appear`, and the new object's OBJ_INIT branches on it. Run at once, it took the wrong branch and
  made more of itself until the depth limit (`Memory(0)`).
- **A script's static memory was per world.** After T301, a handle in a static block made in the maker's world meant nothing
  in the made fighter's (`Memory(0x60000114)`).
- **T304 regression.** A queued motion looked in the ended motion's table, so M_EX750's `route_panic` never got its motion 4
  and stood still.
- **Range 0x18's parameter was ignored.** It is `+0x108`, the steering modes' inertia, not a height (FUN_14040d810,
  FUN_1403cbdd0).
- **Three objects were refused at load:** collision shapes that name another HP pool (the hit path already carries the pool),
  and a shape of an unknown kind.
- **Part colours were not done:** `obj_set_parts_color`, `obj_reset_parts_color` and frame events 0x1a and 0x1b. B_CA050
  hides parts this way, and M_EX790 and M_EX940 use the frame events.

## What the executable does

- **FUN_1403b44d0** runs OBJ_INIT and makes the main thread. It is a class routine reached through the vtable (FUN_1404190d0,
  FUN_1403dacb0), not from the constructor FUN_140419640. The call site was not found; the script above is the evidence for
  the order.
- **Part colours.** Eight records at `+0xac4`. FUN_14040efe0 finds the record of exactly this mask, else the last free one.
  FUN_1403cd670 sets the colour and fades 0 to 1; FUN_1403cd4c0 fades from the current value to 0. FUN_14040f020, called from
  FUN_1403cc3d0 each frame, steps them, frees a running one that reached 0, and gives each channel as
  `byte((c - 0x80) * fade) + 0x80`, a multiplier with 0x80 as 1.0. Frame event 0x1a is mask `1 << p0`, colour `0x00808080`:
  no alpha at the end of its fade.
- **FUN_1403c94c0** knows shape kinds 0, 1 and 3.

## Changes

- `enemy::world`: `Object::init_pending`; `World::appear` builds with `Object::spawn_later`; `take_words`: fighters' static
  blocks come from the shared store.
- `enemy::object`: `spawn_later`, `run_pending_init`, called first in `tick` and `tick_main`; range 0x18 sets a running
  steer's inertia.
- `Sysobj::begin_tick`: a queued motion looks in the table in use now. `end_tick` starts motions from it too.
- `Sysobj::part_colors`, `PartColor`, `set_parts_color`, `reset_parts_color`, `part_tint`; `traps::service` 1:185 and 1:186;
  `motion::Effect::PartsHide`, `PartsShow`; `enemy_gallery::present` gives each part its mesh group's colour.
- `enemy_combat::collision_shapes`: shapes keep their pool; a shape of an unknown kind or no size hits nothing.
- `enemy_battle::supported`, `enemy_catalog`: `KH2RW_ENEMY_BOSSES` admits types 3, 8 and 12, for unattended runs.
- Unit test `a_part_colour_fades_in_on_its_groups_and_its_record_goes_when_it_fades_out`.

## Checks

| Check | Result |
| --- | --- |
| Boss runs (`probe11`, 2400 ticks) | PASS for B_LK120, B_MU120, B_AL020, B_EX100: no faults, no refusal. FAIL for B_CA050 before the type 8 gate: `Memory(0)` because its F_CA690_BTL was not placed |
| M_EX750, M_EX960 and their areas in the same runs | PASS: no faults; M_EX750's `route_panic` no longer stands still |
| `--summon-tour` against the build before these changes (`sum0`, `sum1`) | PASS: the same 4 summons, 5 limits, 23 follow-ups, 988 ×8. The higher counts of T207's record are not reached by either build; that gap is older than this change |
| `cargo test --locked -p enemy`, `cargo clippy --locked -p enemy --all-targets` | PASS |
| Release launcher (`tools\Build-Launcher.ps1 -Skate3`) | PASS |
| Side by side with the installed game | NOT RUN |

## Uncertainty

- Whether a made object's OBJ_INIT runs in the same frame or a later one, and in what order against its maker's rest, is inferred.
  It runs at its first `tick` here.
- `KH2RW_ENEMY_BOSSES` only lets bosses run. Their mission, camera and event requests still only report themselves, so none of
  this makes a playable fight.
- `obj_set_stealth` is still not carried out.
- The colour multiplier is applied to the material's base colour, not through the engine's list.

## Verdict

PARTIAL. Runs in tours; not compared with the installed game.

## Next

The full sweep and boss sweep on this build; `obj_set_stealth`; the Gambler's dice and cards.