# T293 — pppEiDrawShipoly's pair mode, averaged edges, taper and view-inverse space

Owner Claude, 2026-10-30, requested by burba in the project thread ("start working on the github issues and getting them
out of the way"). Issue #63 (and the ribbon item of #28). Gate: development of the effect VM; no final gate is inferred.
Prerequisites: T150 (effect VM, single-mode ribbons). Build identity: local `main` after `e31a43c`, package 0.1.2. Source: the
selected Steam KH2FM build, validated each run; the install was only read.

Private evidence (outside the checkout): `research\issues-20261030\g\rib\` (FUN_1404b9c00 update, FUN_1404b9260 draw,
FUN_1404b8380 and FUN_1404b7530 strips, FUN_1402c31e0 the view's inverse, FUN_1402bebf0 the instance's object list).

## What the executable does

The issue said pair mode needed "two bone or object positions handed to the instance by whatever starts it". The update shows
it doesn't: the two points are the part's own matrix on (0, ±width / 2, 0).

- **Pair mode (+9 == 0):** two samples per tick in the ring. The draw transforms each rail to view space, fits one spline per
  rail, and the rails are the strip's edges.
- **+9 >= 2:** a single path whose edges use the sum of this step's and the last step's directions (no renormalising), the last
  row with its x direction cleared.
- **+0x10:** the edges drawn in to their middle: fully at the newest row, not at all at the oldest.
- **Spaces (+0x11):** 1 the world. 3 the view's inverse (ece0, FUN_1402c31e0: `FUN_140478ce0`), with the draw multiplying the
  view in twice, so the samples follow the camera's movement. 2 `DAT_140b60270 + 0x40`, the instance's object list, which only
  a script fills (FUN_1402bebf0: flag 0x80000; B_EX170_LAST's 239/242 read the same list).
- **+8 != 0** draws through FUN_1404b7530 (screen-grab textures, DAT_140b60250), the frame-buffer family of #148.

## Change

`effect::ribbon`: pair mode, the averaged edges, the taper and space 3; `effect::math::inverse`. Still flagged unsupported
where met: +8 (the frame-buffer strip, #148) and space 2.

## Checks

| Check | Result |
| --- | --- |
| `effect_census` against the install | The same 26 and 14 effects stay listed: every pair-mode key also has +8 != 0 (the frame-buffer strip, #148), and every space above 1 is space 2 (the object list). No installed effect uses space 3 (the count did not move when it left the flag); the census does not count +9 >= 2's users |
| Unit test: the inverse undoes a view; the existing single-mode test | PASS |
| Release run of the Axel and Roxas event | PASS: runs as before; it has no pair ribbon of its own in the frames captured |
| Side by side with the installed game | NOT RUN |

## Uncertainty

- The scale inside the taper's middle (`thunk_FUN_140140e00`'s lost argument) is taken as 0.5.
- `FUN_140478cb0` is taken as a plain lerp with its last argument as `t`.
- Space 2's draw matrix copies a matrix the decompile lost; with the list empty (every room today) it never runs.

## Verdict

PARTIAL. The geometry is translated; pair ribbons draw once the frame-buffer strip exists (#148). Space 2 needs the object list's writer.
