# T177 — World map: object effects and the ship's ride

Owner Claude (thread "World map rewrite"), 2026-10-17. Issue #89 (part of
#81). Gate: none passed; PROGRESS toward the world map work. Prerequisites:
T176 (the events' character scenes). Branch
`claude/project-thread-9zxl2w-effects`, stacked on
`claude/project-thread-9zxl2w-widgets` (T176). Selected KH2FM Steam app
2552430 build 15194255 English remains a quarantined candidate.

Private evidence root: `%LOCALAPPDATA%/KH2FinalRewrite/research/worldmap-20261016/`
(decompiles `out/fx1`..`out/fx6`, `out/dd70`, `out/cd67`, `out/mb`, `out/pc`,
`out/lt`, `out/heap`, `out/ec08w`, `out/bdb50`, `out/vt-sym`; disassembly
`out/dis-4330`, `out/dis-calls`; dumps `out/data-t177.txt`,
`out/data-skip.txt`). The effect census and the run logs and captures
below are in a private folder outside the checkout. The installed source
stayed read-only.

## Steps and implementation

1. `world_map::objects`: the effects worldmap::Symbol holds.
   - FUN_140390510 keeps element 3 while the world record's flag bit 0 is
     set, element 5 from state 3, and The World That Never Was's element
     6 or 8.
   - Each slot's element starts only when the slot is empty and is freed
     when the look wants none (FUN_1403b6630 / FUN_1403b6670 →
     FUN_1402c6b30, FUN_1402be8a0).
   - Op 0x105's pre-refresh (FUN_1403940b0) starts element 0 on symbols
     whose record flag bit 0 became set (FUN_1403900d0). It does the same
     on Gummi ships whose slot went from 1 to 2 (FUN_140397240).
   - When the arrival's events task ends (FUN_14038de20 →
     FUN_140390460), each open symbol whose world lists a new unlocked
     episode starts element 4 once.
2. Motion frame triggers of the world map objects (FUN_14040e960 →
   FUN_14040dd70):
   - 1 starts an element of the object's PAX. A bound element (flag bit
     0) follows the object. With flag 0x20 the motion holds it
     (FUN_1403c8000), and the next motion releases it (FUN_1403c7300).
   - 8 plays a sound.
   - 26 / 27 show and hide a mesh part (FUN_1403cd670 / FUN_1403cd4c0).
   The cursor's idle, fast and ride motions and the symbols' open and
   closed motions use these.
3. Op 0x110 is not a Hollow Bastion effect. The cursor rides Hollow
   Bastion's symbol (FUN_1403b4110(0x3a3), FUN_1403d9fe0 flags 0x4002),
   turned by π (FUN_1403b65f0), on motion 0x9c (FUN_1403c86a0).
4. `kh2fm wm` runs the effects:
   - It loads each object's PAX (`obj/<name>.a.us`, BAR type 18) and
     `effect/texcommon.dpd`.
   - The map PAX elements follow the draw groups (FUN_1402c7010 /
     FUN_1402c65a0). `wm00` has none.
   - The VM is `effect`. The presenter follows the test room's object
     effects, under the map root, so the effects hide with the map during
     the character scenes.
5. `effect::Instance::heap_limit` (opt-in) refuses parts past a byte
   limit, as FUN_1404af240 does when the shared particle heap (0x80000
   bytes at start-up, FUN_1402bdb50 / FUN_1402c2480) is full. A part's
   block is FUN_1404b02f0's request. The world map shares that heap among
   its running effects.

## Checks

- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`,
  `cargo test --workspace --locked` (765 passed, 0 failed),
  `cargo build --workspace --locked` and the smoke run
  `cargo run -p main --locked -- --frames 180` passed on this branch.
- New unit tests on generated values: the symbol effect rules (held
  slots, pre-refresh, new episode) and the particle heap refusal.
- Effect census (`effect_census`): the world map's containers use no
  program this VM lacks.
- Installed unattended runs:
  - The default arrival starts Hollow Bastion's new-episode element 4, the
    cursor's idle element 7 and the closed symbols' element 5. The
    cursor's fast motion starts elements 0 and 1 and releases 7. The
    sparkles show around Hollow Bastion's symbol in the captures.
  - `--event 0x200`: op 0x110 puts the ship on Hollow Bastion's symbol,
    starts elements 9 and 10, and plays sound 16013.
- NOT RUN:
  - A comparison with the installed game.
  - A capture of the ride. The unattended run ends before it.

## Uncertainty

- The new-episode element 4 and the closed symbols' element 5 birth an
  immortal child part on every loop (VertexApLc, kind 177, whose restart
  hook is its create hook). Their parts grow until the particle heap is
  full. This happens in the exe as traced; whether the game reaches that
  state the same way is not compared.
  - The emitters' own pools (MegaBirth) also take heap in the exe and are
    not counted.
  - The area start may give the heap another size.
- Bound elements follow the object, not their bone. The object's turn for
  effects is the drawn model's (yaw − π) [I].
- Trigger 26 / 27's part mask is read as the model's mesh parts in order
  [I]. The installed motions fade over 0 frames, so parts switch at once.
- Trigger-8 motion sounds are not faded at a motion change
  (FUN_1403c7300's FUN_1401de500).
- Not carried out:
  - The symbols' hit reaction (BTLOBJ slot 28, elements 2 and 1, from the
    attack path FUN_1403d1730).
  - Range triggers.

## Verdict

PROGRESS (no gate). Next: the jump into the field (entrance 0x63
arrivals) and a comparison with the installed game.
