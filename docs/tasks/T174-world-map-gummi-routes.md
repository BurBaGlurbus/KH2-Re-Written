# T174 — World map: Gummi route reaction, route menu and route panel

Owner Claude (thread "World map rewrite"), 2026-10-16. Issue #88 (part of
#81). Gate: none passed; PROGRESS toward the world map work. Prerequisites:
T173 (landing menu presentation and rules). Branch
`claude/project-thread-9zxl2w-gummi`, stacked on
`claude/project-thread-9zxl2w-landing` (T173). Selected KH2FM Steam app
2552430 build 15194255 English remains a quarantined candidate.

Private evidence root: `%LOCALAPPDATA%/KH2FinalRewrite/research/worldmap-20261016/`
(decompiles `out/all/1403982*..140399f*`, `out/gumi1`; table dump
`out/data-10.txt`). Captures of the runs below are in a private folder
outside the checkout. The installed source stayed read-only.

## Steps and implementation

1. Reaction: a drawn route object (symbol slot of its route in state 1 or
   2, FUN_140397490) offers its kind-5 sphere's command when the 'cmd'
   entry executes 0x206 (FUN_140394fc0), with the symbols' landing in the
   same nearest-first choice.
2. `world_map::landing::Landing::route_menu`: dk::WM_GLANDING (FUN_140398bd0
   / FUN_140398270 / FUN_140398b10 / FUN_140398da0) is the two-row menu on
   the landing's scenes and groups (0x1405c2b98..); its first row ends with
   `Outcome::Route`.
3. `world_map::gummi`: the route names (FUN_140397400, sys 0x8224 + route −
   1), the menu's first rows ('wm' 0x4a4d + route − 1, table 0x1405c2bce),
   the mission record fields and the panel state, and the name / rank
   group tables (0x1405c2c48 / c50 / c58 / c60).
4. `visual::world_map_menu::RouteInfo`: dk::WM_GLANDING_INFO
   (FUN_140398ef0, closed by FUN_140399cd0): the name frame and star, and
   per cleared mission row the rank, the EX rank and the score digits.
5. `kh2fm wm`: the route panel follows the reacting or chosen route
   (FUN_140394810 / FUN_140399db0), Triangle opens the route menu under the
   close-up ({0, 0, 1}, FUN_140394fc0) and its first row ends the program
   (FUN_1403946f0 queues the Gummi mission, which this program does not
   run). Test options `--slot <n>:<state>` and `--at <x>,<z>`.

## Checks

- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`,
  `cargo test --workspace --locked`, `cargo build --workspace --locked`: see
  the PR for the run on this branch.
- New unit tests on generated values: the route texts and panel states,
  the route menu's outcome (2).
- Installed unattended run (story 0x800–0x895, slot 1 state 1, cursor at
  5458, −6500): route 1's object and its first route line show, the panel
  shows "Asteroid Sweep" with its unlit star, Triangle opens "Proceed
  through Asteroid Sweep" / "Maybe later...", and deciding ends with route
  1's mission.
- NOT RUN: a comparison with the installed game; a run with cleared
  missions (this program keeps no Gummi records, so the rank and score rows
  were not drawn from the install); the Gummi missions themselves.

## Uncertainty

- The mission record layout (+0x1b rank, +0x1e cleared, +0x20 score) is
  read from FUN_140398ef0's uses only.
- The EX rank's x offset (0x5a) and the score digits' order follow the
  decompile's loop; not seen drawn.

## Verdict

PROGRESS (no gate). Next: the world map events, effects and sounds (#89).
