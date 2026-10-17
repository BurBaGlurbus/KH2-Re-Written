# T171 — Debug menu: enemy animation preview tabs, search and skeleton

Owner Claude, 2026-10-17, requested by burba in the project chat. Gate:
development of the test room only; no gameplay acceptance is inferred.
Prerequisites: the enemy gallery (T110-era preview) and the F3 debug menu. Build
identity: `main` at `554d3bf`, package 0.1.2, Rust 1.99.0 Windows MSVC. Source:
the selected Steam KH2FM build, validated each run; the install was only read.

Private evidence: `%LOCALAPPDATA%/KH2FinalRewrite/research/debugmenu-20261017/`
(unattended `--enemy-tour` captures `tour-01`..`tour-05`; the configuration copy
`cfg/`). No game data is in the repository.

## What changed

- The preview (F3, Summon enemies, Summoned enemies) has five tabs: Playback,
  Motions, Actions, View and Enemy. Tab / Shift+Tab, L1 / R1, or Left / Right on a
  row with no value to change switch tabs. Two lines stay above the rows: the
  enemy and motion, then frame / last frame, source fps, speed and play state.
- Playback: play / pause, step one whole frame forward and back, go to frame
  (Left / Right), jump to start / middle / end, replay, loop, speed presets
  (0.1x to 4x), length in frames and seconds. The preview keeps playing behind the
  menu, and F3 reopens on the same tab.
- Typed letters narrow any long list (items, enemies, summoned enemies, motions,
  actions, worlds, rooms); Backspace removes a letter, Esc clears the search
  before it leaves the page.
- View: skeleton lines (a line mesh from each bone's joint to its parent), hide
  model, collision overlay (the F1 toggle, now also driven from the menu) and root
  motion.
- Names: the motion set names its entries with bare codes ("02A"), so motions are
  named by the enemy-script actions that request them ("appear, idle"), with the
  code and slot beside it; action rows read "name - slot N" (no "[candidate]"
  tag); enemies with no known name read "Unidentified enemy" with the model code
  in the value column; object ids and the `#` prefixes are gone.
- Code names: `Window`/`repeat` are `Timeline`/`loop_start`, `frozen` is `paused`,
  `basic_fk` in the gallery is `forward_kinematics_only`, `rt`/`bons`/`d`/`a` in
  the menu are `battle`/`bonus_table`/`drive`/`actor`, and the actions are named
  for what they do (`PlayPause`, `StepFrame`, `ToggleSkeleton`, ...). Routine
  addresses stay in comments.

## Checks

- Unit tests: tab wrapping, header lines vs rows, search text round trip through
  the menu font, frame stepping and wrapping, jumps, speed presets, motion naming
  from actions. fmt, clippy (warnings denied), tests and build: see the PR.
- `--enemy-tour` against the installed build: all pages rendered (summoned list,
  each tab, a typed search "move" showing 6 of the actions, the skeleton over and
  without the model); the tour verdict is unchanged (pass).

## Not checked / uncertain

- Keyboard and pad input (Tab, Left/Right on rows, typing, Backspace/Esc, F3
  reopening on the tab, live playback behind the menu) was not driven in a live
  session; the unattended tour sets the page directly.
- The skeleton is drawn with depth testing, so it is hidden inside the model
  unless the model is hidden; raw-profile motions (no evaluated joints) draw
  their bone matrices' origins.
- Most motions in a set have no action naming them ("Unnamed motion"); no better
  name is known.

## Verdict

PARTIAL: rendering, layout, search filtering, naming and the skeleton PASS (unit
tests and the tour); the interactive controls are NOT RUN. Next task: confirm
the controls in a live session and name motions further if more action tables
are decoded.
