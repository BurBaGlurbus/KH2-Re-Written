# T298 — An event's depth of field (SeqFocus)

Owner Claude, 2026-10-30, requested by burba in the project thread ("start working on the github issues", then "continue").
Issue #151; a correction to T291. Gate: development of the event player; no final gate is inferred. Prerequisites: T099 (event
player), T291 (the render passes). Build identity: local `main` after `01716cb`, package 0.1.2. Source: the selected Steam KH2FM
build, validated each run; the install was only read.

Private evidence (outside the checkout): `research\issues-20261030\g\dof\` (FUN_14018a9e0, FUN_14018a5a0, FUN_140189e50,
FUN_14018a060, FUN_1402da870, FUN_14019dc50, FUN_140142ba0, FUN_1401bb350, FUN_1401bc480); `research\sephiroth-01\dof-01\`.

## What the executable does

- SeqFocus (op 0x18, FUN_1402da870): frame, on (byte +2 is 0), mode (byte +3), GS depth (+4, at least 15000). Event 601 turns it
  on at frame 1 with mode 0 and depth 20000, and off at frame 1153.
- **The open question from #151, the depth mapping:** the projection (near 1, far 4000000, FUN_14019dc50) stores about `1 - 1 / w`
  and FUN_140142ba0 turns a GS depth into `1 - z / 16777215`, so a GS depth `z` stands for the view distance `16777215 / z`.
  Event 601's 20000 is about 839 units.
- The pass (FUN_14018a9e0, record mode 0): the frame shrunk to a quarter, then five quads with a depth test at that depth, only
  where the scene is beyond it: four shifted 3 pixels (up, left, down, right) at alpha 0x40, the last in place at 0x60.

## Change

- `visual::focus` (the pass, before the glow) and `cutscene::focus_switch`.
- T291's correction: `FUN_1401bb350` writes the glow's stencil for models too. Opaque models now write class 0 (`fog.wgsl`), so
  a character in front of a bright map pixel no longer blooms.

## Checks

| Check | Result |
| --- | --- |
| Unit tests: the depth as a view distance; the five quads' shifts and alphas | PASS |
| Release run of event 601 (`--event hb:1:601`), captures beside the build without it: the far sky and horizon soften, Sephiroth stays sharp, no validation errors | PASS |
| Side by side with the installed game | NOT RUN |

## Uncertainty

- The blend is taken as the usual 0x44: the static `DAT_140abedb8`'s initial value was not read [I].
- The record's other mode (FUN_14018a2d0's prepass) is not built; no record in event 601 uses it, and other events weren't scanned.
- The pass's place relative to the glow and the effects is [I].
- The far override `DAT_14071c65c` (FUN_140142b20) is 0 outside some areas and isn't applied.

## Verdict

PASS for event 601's depth of field. Next: nothing here unless a comparison shows otherwise.
