# T178 — World map: the landing jump opens the field

Owner Claude (thread "World map rewrite"), 2026-10-17. Issue #87 / #89
(part of #81). Gate: none passed; PROGRESS toward the world map work.
Prerequisites: T173 (the landing menu and its targets), T177. Branch
`claude/project-thread-9zxl2w-jump`, stacked on
`claude/project-thread-9zxl2w-effects` (T177). Selected KH2FM Steam app
2552430 build 15194255 English remains a quarantined candidate.

Private evidence: the run logs, the generated configs and the captures
below are in a private folder outside the checkout. The installed source
stayed read-only.

## Steps and implementation

1. The landing's decided target (FUN_1403946d0 → FUN_140152990: world,
   area and entrance 0x63 for a world point) now opens the field.
   - The game loads the area in place.
   - Here `kh2fm wm --field <test-room config>` writes a field config
     from that config's actor, HUD, menu and sound sources with the target
     area and the world map's story ranges (`test-room-wm-landing.json`
     beside it).
   - It then starts `kh2fm room` there and the world map closes.
   - Unattended runs write the config and log it without starting the
     field.
2. The launcher's field config writer is shared between the Twilight Town
   and World That Never Was tests and the landing
   (`launcher::landing_room_config`). The launcher's World map test passes
   `--field` with the arena config it already uses for the room tests.
3. A target that is a spawn id (FUN_14039fbc0) or outside the world table
   is refused and logged. The area loader's own entrance selection
   (spawn type 1, argument 0x63) places Sora.

## Checks

- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`,
  `cargo test --workspace --locked` (766 passed, 0 failed),
  `cargo build --workspace --locked` and the smoke run
  `cargo run -p main --locked -- --frames 180` passed on this branch.
- New unit test on generated files: the landing's field config names the
  target area, entrance and story and drops the arena's enemy.
- Installed unattended runs:
  - `--touch 47,48,50,52 --autoplay … --decide` decides Twilight Town's
    world point and writes the field config for tt09 entrance 0x63 with
    story 0x800..0x895.
  - `kh2fm room` on that config opens Central Station with Sora beside the
    train, as the arrival entrance places him.
  - At Hollow Bastion with the default story, the landing's default target
    is "none" (no `wlds` record for that state). The jump is refused and
    logged.
- NOT RUN:
  - A hand-driven run that starts the field process from the world map
    window.
  - A landing in a world other than Twilight Town at a story point that
    opens it.

## Uncertainty

- The game's arrival presentation (its fade and any arrival event) is
  whatever the field program runs at that entrance. The world map's own
  exit fade before FUN_140152990 is not modelled.
- The field starts from the story ranges given to the world map. The
  world map state changed in the run (touched points, events played) is
  not carried over.

## Verdict

PROGRESS (no gate). Next: a side-by-side comparison of the world map with
the installed game.
