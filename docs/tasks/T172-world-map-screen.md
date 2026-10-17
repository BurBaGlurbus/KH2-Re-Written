# T172 — World map: screen, cursor flight, symbols and camera

Owner Claude (thread "World map rewrite"), 2026-10-16. Issues #85 and #86
(part of #81). Gate: none passed; PROGRESS toward the world map work.
Prerequisites: T171 (world state and progress tables), T097 (area loader),
the actor physics port (player::actor_physics), the room and map-object
renderers of the test room. Branch `claude/project-thread-9zxl2w-screen`,
stacked on `claude/project-thread-9zxl2w` (T171) at `0bc7fcf`. Selected KH2FM
Steam app 2552430 build 15194255 English remains a quarantined candidate.

Private evidence root: `%LOCALAPPDATA%/KH2FinalRewrite/research/worldmap-20261016/`
(`worldmap-spec.md`; decompiles of the world map object dispatch, symbol,
Gummi, cursor, ActionShip and ObjCamera routines). Captures of the runs below
are in a private folder outside the checkout. The installed source stayed
read-only.

## Steps and implementation

1. `source/game/world_map/` (new package):
   - `objects.rs`: the world map objects by objentry (cursor 0x3a1, symbols
     0x3a2–0x3ac and 0x456, Gummi 0x3ae) with each symbol's world and draw
     group (FUN_140390150); a symbol's look from its world record — drawn,
     open/closed motion 0xa1/0x97, state tint, group switch, effect slots
     (FUN_140390510); the symbol-slot map objects and draw groups per slot
     state (FUN_14038df20 with FUN_1403906d0/6f0/6e0/700); the Gummi route
     lines per route state (FUN_140397490); the world-object table
     (FUN_14039a410).
   - `camera.rs`: worldmap::ObjCamera (FUN_140397c70 / FUN_14038ebc0): eye
     and target offsets from the cursor, the 65° view, the depth clamp, the
     southern blend and the blend toward The World That Never Was once its
     symbol is open, with the 60-tick sine blend.
2. `area`: `WORLD_MAP_CURSOR`; the player spawn falls back to the cursor in
   the world map area.
3. `kh2fm wm` (`source/main/src/world_map.rs`) and a launcher entry "World
   map": loads area wm00 at the entrance of the world left (FUN_140154e20),
   runs the queued world events' scripts (FUN_14038dc60), spawns the map, the
   map objects and the world map objects of the area's spawns, and applies
   the world state in the area start's order (dark set FUN_1403f0b70, object
   looks, slots). The cursor flies like worldmap::ActionShip: the stick's
   walk/run request (FUN_1403a8fa0), the turn toward it (FUN_1403b4780),
   FUN_1403b7ad0's steering with the PRTY record its objentry names (inertia
   +0x28, drag +0x2c), run every second frame (FUN_1403b89a0), collision
   with its object-collision body, and the fast/idle motion choice
   (FUN_140563880). Motion numbers of the world map's motion sets are BAR
   entry indices (FUN_1403c7c50). `--load-only` prints the loaded state;
   `--autoplay <empty dir>` flies a fixed route and captures six frames.
4. Shared model reader: each part keeps its mesh group's header word 0
   (`attributes`); the world map draws bit 0 groups in the translucent pass,
   additive when bit 21 is set.

## Checks

- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`,
  `cargo test --workspace --locked`, `cargo build --workspace --locked`: see
  the PR for the run on this branch.
- New unit tests on generated values: world map objects and camera (7), the
  program's field-of-view conversion, story range parsing and raw motion-set
  indexing (3).
- Installed run (`kh2fm wm <Steam root> <manifest> --load-only`, story
  0x800–0x895): the area loads at entrance 1; cursor, 12 symbols and 9 Gummi
  route objects spawn; Twilight Town and Hollow Bastion symbols open (0xa1),
  Beast's Castle and The Land of Dragons drawn closed (0x97), the rest
  hidden; the cursor reads PRTY record 59 (walk 15, run 30, inertia 0.85,
  drag 0.8) and the object-collision body r 90, h 90, centre 115.
- Installed unattended run (`--autoplay`): the opening view shows the ship at
  Twilight Town with its ring and the route line toward Hollow Bastion; the
  cursor flies north, east and south-west and stops; the symbols' glow cards
  and the map's aurora objects draw translucent.
- NOT RUN: any comparison with the installed game's world map (screen
  captures side by side, cursor speeds or camera path measured in the game).

## Uncertainty

- The meaning of mesh header bits 0 and 21 (translucent, additive) follows the
  community model layout; the PC renderer's reader of them is not traced.
- `WM_SYMBOL_CA`'s texture set does not decode with the current texture reader
  and draws untextured (the run's log names it).
- The map-object index the slot and route switches count is taken as the
  area's placement order [I].
- The Gummi route objects have no model of their own; their lines are map
  objects. The symbols' effects, the cursor's motion blend (8 ticks), the
  landing prompt and every menu are not here (#87–#89).

## Verdict

PROGRESS (no gate). Next: landing on a world and the room select (#87).
