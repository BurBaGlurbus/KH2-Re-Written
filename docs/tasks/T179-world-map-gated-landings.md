# T179 — World map: gated land reactions and program-id jumps

Owner Claude (thread "World map rewrite"), 2026-10-18. Issues #87 / #89
(part of #81). Gate: none passed; PROGRESS toward the world map work.
Prerequisites: T178. Branch `claude/project-thread-9zxl2w-gates`, stacked
on `claude/project-thread-9zxl2w-jump` (T178). Selected KH2FM Steam app
2552430 build 15194255 English remains a quarantined candidate.

Private evidence root: `%LOCALAPPDATA%/KH2FinalRewrite/research/worldmap-20261016/`
(decompiles `out/all/140394fc0`, `out/warp1`, `out/fbc0`, `out/fc50`;
disassembly `out/dis-fc50`). The run logs, generated configs and captures
are in a private folder outside the checkout. The installed source stayed
read-only.

## Steps and implementation

1. FUN_140394fc0's land reaction (command 0x9f) on Twilight Town's symbol
   plays world event 0x72d instead of opening the landing menu while
   world-event bit 1 is set and bit 2 clear (FUN_1403deab0). On The World
   That Never Was's symbol it plays 0x72c while bit 0x35 is set and 0x37
   clear.
   - The event runs as a new events task (FUN_14038de90).
   - The panels close (FUN_1403966c0 / FUN_140399cd0).
   - `world_map::objects::land_event` holds the rule.
   - The program preloads both events' archives and widget models.
   - `--seen <bits>` marks world-event bits as a save would hold them.
2. Jumps by program id (FUN_14039fbc0) resolve through `07localset.bin`
   (FUN_14039fc50: per world, BAR type-2 entry n, `{u16 program, u16
   area}` sorted by program).
   - The target is that area at entrance 0, with the id as its map,
     battle and event programs.
   - This covers a default landing whose `wlds` area is 0x33 or more, and
     op 0x10f.
   - `data::world_map::read_program_areas` reads the table, and
     `area::world_state::resolve_program` does the lookup.
   - The field config carries the program overrides (`programs`), which
     the field's area config already accepts.
3. Not ported: the cursor's commands 0x246 (ActionWarp toward the
   targeted object, FUN_1403978c0) and 0x247 (motion 0x3b). Which object
   offers them (FUN_1403947a0, set from several reaction vtables) was not
   traced.

## Checks

- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`,
  `cargo test --workspace --locked` (769 passed, 0 failed),
  `cargo build --workspace --locked` and the smoke run
  `cargo run -p main --locked -- --frames 180` passed on this branch.
- New unit tests on generated values: the land reaction's gates, the
  program table reader, and the program-id lookup. The landing config
  test now also covers program overrides.
- Installed unattended runs:
  - `--seen 1 --autoplay … --decide` at Twilight Town plays event 0x72d
    (Chip and Dale: "Huh? You want to go back to Twilight Town?") on each
    reaction, with its voices, instead of the landing menu.
  - `--event 0x71f` ends with op 0x10f (Hollow Bastion, program 0x5c),
    which resolves to hb01 entrance 0 with programs 0x5c. The field loads
    it with Sora placed (logged position, no room change in a 400-frame
    run).
- NOT RUN:
  - The game's own response to these reactions, side by side.
  - A capture of hb01 standing still (the unattended field run walks to
    an exit).

## Uncertainty

- The panels' close animation is skipped: they go at once when the gated
  event starts [I].
- A program id missing from the world's table leaves the target as it was
  (the exe would read a null record).

## Verdict

PROGRESS (no gate). Next: the warp commands' source, and a side-by-side
comparison with the installed game.
