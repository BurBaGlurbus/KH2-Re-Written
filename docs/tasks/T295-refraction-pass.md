# T295 — The refraction pass: programs 145 and 160, and the ribbon's frame-buffer strip

Owner Claude, 2026-10-30, requested by burba in the project thread ("start working on the github issues and getting them
out of the way"). Issues #148 and #63. Gate: development of the effect renderer; no final gate is inferred. Prerequisites: T226
(the effect VM's side, draft PR #141, brought over here), T229 (the scene blends in gamma space), T291 (the render pass and the
scene's alpha), T293 (the ribbon's pair mode). Build identity: local `main` after `d663197`, package 0.1.2. Source: the selected
Steam KH2FM build, validated each run; the install was only read.

Private evidence (outside the checkout): `research\party-1to1-20261023\refraction-spec.md`, `research\issues-20261030\g\rib\`
(FUN_1404b7530).

## What the executable does

- 160 (pppKeShpDtt) draws a disc and 145 (pppKeMdlDtt) a model's untextured triangles, each vertex sampling a picture of the
  frame at the screen place of a point moved by a strength. See T226.
- FUN_1404b7530 is the same idea for a ribbon (241 with +8 set, which every pair-mode key in the install has): a corner at `t`
  across the strip samples the frame at `(t - 0.5) · k + 0.5`, `k = strength + 1`, or `strength · fade + 1` under the row fade.
  The strength integrates in the u3 block (+0x18..+0x20, key adds at +0x20..+0x28); +0x14 turns the depth test off.

## Change

- T226's `effect::refraction`, brought over from `6eb57dd` (the branch itself stays as it was); its record comes with it.
- `effect::ribbon`: the strip as `Refraction` records. Blends other than the plain one (+0x15) stay flagged unsupported.
- `visual::refraction`: hosts publish each tick's records (the room and the event presenter). After the scene's transparent
  pass and before the glow, the frame is copied and the triangles sample the copy: texture × vertex colour, alpha blend, no
  depth write, depth test unless the effect turns it off, texture coordinates linear on the screen.

## Checks

| Check | Result |
| --- | --- |
| Unit tests: T226's disc; a fan becomes triangles in the scene's space; a corner samples its point's screen place | PASS |
| `effect_census` against the install | PASS: the 26 pair-mode ribbons are no longer listed. Left: 14 ribbons in space 2, 229, 239, 242, 82 |
| Release `--magic-form 6 --form-moves --magic-tour` (Antiform), with a test-only count: refractions drawn from tick 960, up to 96 triangles a tick, no validation errors | PASS |
| The same captures beside the build from before this change: the floor's lines and the dark wisps warp around Sora only with the pass | PASS |
| Quick Run's haze seen in a capture | NOT RUN: `--magic-form 2 --form-moves` drew no refraction; why that tour's run doesn't reach it wasn't traced |
| Side by side with the installed game | NOT RUN |

## Uncertainty

- 145 samples the same copy as 160, not the exe's previous output frame with the HUD: a frame's difference.
- All refractions draw after the other effects (`RefractionOrder` is carried and not yet used). The exe sorts 145 and the ribbon among them and puts screen-mode 160 in layer 4.
- 145's v axis is divided by 448 in the exe (a known vertical mis-scale, refraction-spec §3.3). Here it samples its true place.
- Effects' stepped ticks: frames between ticks redraw the last tick's triangles.
- Instances with an untranslated program draw no refraction, in every host.

## Verdict

PASS for the pass in the room and the event presenter; not compared with the installed game. Next: Quick Run's haze in a run
that has the ability, and the order among the other effects.
