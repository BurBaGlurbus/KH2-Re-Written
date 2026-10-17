# T173 — World map: landing reaction, info panel and the World Menu

Owner Claude (thread "World map rewrite"), 2026-10-16. Issue #87 (part of
#81). Gate: none passed; PROGRESS toward the world map work. Prerequisites:
T171 (world state, landing grid, default landing, episodes), T172 (World
Map program), the 2LD layout, SEQD sprite and system font ports. Branch
`claude/project-thread-9zxl2w-landing`, stacked on
`claude/project-thread-9zxl2w-screen` (T172). Selected KH2FM Steam app
2552430 build 15194255 English remains a quarantined candidate.

Private evidence root: `%LOCALAPPDATA%/KH2FinalRewrite/research/worldmap-20261016/`
(`worldmap-spec.md`; decompiles under `out/all`, `out/land1`..`out/land15`,
`out/vt-lay`; table dumps `out/data-06.txt`..`data-09.txt`). Captures of the
runs below are in a private folder outside the checkout. The installed
source stayed read-only.

## Steps and implementation

1. `world_map::landing` (game rules):
   - the reaction a symbol offers: its kind-5 sphere (the shape argument
     names a 03system 'cmd' entry whose execute is 0x9f) against the
     cursor's kind-1 sphere (FUN_140407de0, FUN_1403c9c00, FUN_1403c94c0),
     nearest first;
   - dk::WM_LANDING's rules: the two-row menu when no world point of the
     world is touched (FUN_1403919e0, FUN_1403931f0), else the two-column
     world-point grid (FUN_140390ff0) with the grid list's moves
     (FUN_1402f8240 for 2 columns, wrapping, no scroll), the step over
     empty cells (FUN_1403928a0), the blocked notice (0x4c0d), decide and
     cancel, the episode list on R1 and back on L1 / cancel (pad mode 3,
     0x1405c3880), scrolling three rows;
   - the info panel's values: the world name (sys 0x8211 + world, 0x8ae9 for
     Hollow Bastion after flag 0x1077), play time digits (FUN_140157430,
     FUN_140392620).
2. `world_map::camera::close_up`: worldmap::CloseUpCamera (FUN_140396b80)
   from the symbol's kind-7 shape with the per-mode offsets (0x140748718..)
   and fov (65°, 105° for The World That Never Was).
3. `visual::world_map_menu`: dk::WM_LANDING_INFO (FUN_140395970 /
   FUN_1403966c0) and the menu's presentation on `file/us/wm_ws.2ld`: the
   layout scenes per mode (0x1405c0f00), Tz::LaySeq cells and Tz::LayMsg
   labels following the scene's props (FUN_140194f20, FUN_14034b710,
   FUN_1402f9d30) with the flags FUN_140390d00 leaves them, the rows and
   their selected groups (FUN_1402f8ab0 / FUN_1402f8bd0 / FUN_1402f90b0),
   the two marks on the selected cell (FUN_1402f6610), the Tz::LightCur
   cursor, the world-point pictures (70landing.bar 'ws_i' on each IMGD),
   the episode panel and rows, the notice.
4. `kh2fm wm`: the reaction each tick, the info panel on the reacting
   world, Triangle (field logical 0x400) opening the menu, the close-up
   camera, the menu in pad mode 3, ObjCamera blending back on close, and a
   decided world point ending the program with its target (the jump,
   FUN_1403946d0). Messages use FUN_1403ad9a0's search over wm.bar, sys.bar
   and jm.bar with the sys 0xadc fallback. The battle level is 00battle.bin
   'btlv' summed over the records whose story flag is set (FUN_1403ee4a0,
   FUN_1403ee1d0). Options `--touch` (world points touched) and
   `--decide` (unattended runs land instead of cancelling).

## Checks

- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`,
  `cargo test --workspace --locked`, `cargo build --workspace --locked`: see
  the PR for the run on this branch.
- New unit tests on generated values: the grid moves, the cell skip and
  landing, the blocked notice, the two-row menu, the episode scroll, the
  info values, the reaction spheres (7) and the close-up camera (1).
- Installed unattended runs (story 0x800–0x895): at Twilight Town with
  points 47, 48, 50 and 52 touched the arrival sits in the symbol's
  reaction sphere, the info panel shows Twilight Town with battle level 06,
  Triangle opens the World Menu grid with the four pictures, the place name
  of the selected cell, "Past events" / "Next objective" from the journal,
  the Episode list and Jiminy's Journal buttons; R1 slides to the episode
  panel and L1 back; cancel closes and the camera blends back; decide
  lands at (world 2, area 9, entrance 0x63). At Hollow Bastion with no
  point touched the two-row menu shows "Land in Hollow Bastion" /
  "Maybe later...". Battle levels match the first-visit values (Land of
  Dragons 10, Beast's Castle 13, Olympus Coliseum 16, Disney Castle 18,
  The World That Never Was 50).
- NOT RUN: any side-by-side comparison with the installed game's menu
  (positions were checked only against the layout's own frames); sounds
  (logged, not played); the jump into the field.

## Uncertainty

- The model-space turn of reaction shapes (−R(yaw)) follows the drawing
  convention and the arrival points [I]; the bone matrix routine itself
  was not traced.
- The projection distance used by the close-up (320 / tan(fov/2)) and the
  third message table being jm.bar on the world map are inferred.
- The episode rows' help line position (FUN_140392680) is approximated.
- TT and The World That Never Was play an event instead of the menu at
  some story points (0x72d / 0x72c, #89); not handled.
- With this program's synthetic story state some worlds' landing sets have
  no 'wlds' row, so the two-row menu's default target is empty (the exe
  would reuse its last result).
- Triangle opening Jiminy's Journal (FUN_14038e050) and the reaction
  command's prompt in the command menu are not ported.

## Verdict

PROGRESS (no gate). Next: the Gummi route menu (#88), then the world map
events, effects and sounds (#89).
