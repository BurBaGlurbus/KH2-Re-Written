# T351 — The world map's small details: the flag tail, faces on the widgets, the trigger-8 fade

Owner Claude, 2026-11-01, requested by burba in the project thread ("work through issues and close them"). Issue #162. Gate: development of the world map and the story
state; no final gate is inferred. Prerequisites: T324, T328, T335, T343, T344. Build identity: local `main` after T350 plus this change, package 0.1.2. Source: the selected
Steam KH2FM build, validated each run; the install was only read. Plain Ghidra 12.1.4 headless on a read-only project copy.

Private evidence (outside the checkout): `research\issues-20261102\wm162-spec.md`, with its decompiles and data scans; runs `wf502`, `wf1823`, `wmf_*` under
`%USERPROFILE%\kh2-issues-out\s162\`.

## What the executable does

- `dsa` ops 4 and 5 also forget the destination's world point (FUN_1403f0600, FUN_1403f0650, FUN_1403f2230). 124 of their 127 uses name a place with a point. The rewrite
  left those points touched, so this was a bug.
- FUN_14039afc0's tail, in order: the Torn Page rule (five flags take a page; a held page opens a chapter), one step of Atlantica's chain, the area-seen table, the 0x102b
  rule, the links, FUN_1403a5540, world events, battle bits, FUN_1403e66c0's level checks, flag 0x8d0, world points.
- FUN_1403a5540 and FUN_1403a5330 keep the Port Royal ship stage and set world 0x10's block flags on entering four areas.
- Range trigger 0x17 on a widget (FUN_140535b10 → FUN_14040e7f0): the test takes `dt`, not the clock, so a record fires for the whole motion or never. FUN_1401c0310: the
  tick after none fires, a worker goes back to its idle group. So widget models also blink.
- Trigger-8 sounds fade over 10 units when a map object's motion changes (FUN_1403c7300, FUN_1401de500).
- A widget's draw block fogs towards grey, its own colour (flag 2).

## Changes

- `Progress`: `Holdings`, `taken`, `area_seen`, `stage`, `check_torn_pages`, `check_atlantica`, `check_levels`, `see_area`, `enter_stage_area`, and `set` and `after_flag` in the
  executable's order. A room with no save holds nothing, so no item check fires.
- `chest::sync_holdings`, each tick: the stock and levels into `holdings`, a gain runs its check, `taken` leaves the stock, and each area entered is seen. `state::AREA_SEEN` is
  saved and loaded.
- `widgets::Clip::faces`, `scene::clip`, `world_map::widget_faces`, `outfit::texa_image_page`.
- `Actor::live_sounds`, `WorldMap::sound_fades`.
- The widget parts' fog colour.

## Nothing to build, with the reason

- The symbols' hit reaction: slot 28 means "my attack hit", and no `WM_*` motion has an attack range. So it can't happen.
- Op 0x105's pre-refresh was already there (`world_map.rs` `pre_refresh`). No script uses op 3.
- The journal flag table's other users are camp screens; the map's are done.
- The hidden map: map mesh, sky, BOBs and map effects hide, and the clear colour matches.

## Checks

| Check | Result |
| --- | --- |
| `cargo fmt --all --check`, `cargo clippy --all-targets --locked -- -D warnings`, `cargo test --locked` | PASS |
| Unit tests: Torn Page rule, empty holdings, Atlantica's steps, area table and 0x8d0, 0x102b rule, stages, level checks | PASS |
| Release `wm --event 502` (`wf502`) | PASS: Chip and Dale draw in the hangar with their faces moving; `widget_face` logs their images |
| Release `wm --event 1823` (`wf1823`) | PASS: `widget_face` with ids [15,33] on Sora's face, whose first frame draws; idle groups run where no record fires |
| dsa ops 4 and 5 on real data | NOT RUN: no run set flag 0x814; the rule is a few lines over tables the map already uses |
| The trigger-8 fade | NOT RUN: `sound_fade` did not log in these events. The only use is WM_CURSOR's motion 156 |
| The flag tail in a story run | NOT RUN beyond unit tests: it needs events setting flags (#25) with the stock on hand |
| Side by side with the installed game | NOT RUN |

## Uncertainty

- [U] save+0x352a: what it counts. It is read from the save, and a fresh room holds 0, so Atlantica's second step waits there.
- [I] Names of items 0x20, 0x57 and 0x17 (Torn Page, Magnet, Thunder).
- [I] The five records that never fire look authored against the clock, but the executable's code ignores them. This is the one point most worth a side-by-side.
- [U] The 80000.0 at the draw block's +0x20 has no reader I could find.
- [I] The face's material is unlit like the rest, with the parts' own blending.
- FUN_1403a54a0's stage redirect (signal 7 at program 0xe1) isn't built. It belongs with Port Royal's ship stages and #25's other signals.
- The same trigger-8 path on field actors isn't built.
- The executable does not hide the symbols and cursor behind a widget scene, while the rewrite does; scene models should cover them except through translucent parts ([I]).

## A trap for the next check

An unattended capture keeps the scene's alpha, the glow stencil. A contact sheet that composites a PNG shows those pixels as flat black or white, which looks like missing
textures. Drop the alpha before judging one.

## Verdict

PASS for #162's list in the room, apart from the NOT RUN rows. No claim about the installed game.

## Next

The rest of the batch.
