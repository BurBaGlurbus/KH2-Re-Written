# T185 — The Keyblade's trail ends with its window (issue #52)

Owner Claude, 2026-10-17, requested by burba in the project chat (issue #52: every
Keyblade shows an effect while walking that the original does not have). Gate:
development of the test room only; no final gameplay acceptance is inferred.
Prerequisites: T150 (effects), T180. Build identity: branch
`claude/combat-feel-walk-effect` on PR 104's branch (`main` at `554d3bf`), package
0.1.2, Rust 1.99.0 Windows MSVC. Source: the selected Steam KH2FM build, validated
each run; the install was only read.

Private evidence root: `%LOCALAPPDATA%/KH2FinalRewrite/research/combat-feel-20261017/`
(decompiles `g/d39`, `g/d40`, the earlier `playable-room-20261008/vfxsfx-01/hit/g/b2/`
`1403effa0`); unattended runs `wk1` to `wk9` and captures `cap10` to `cap14` in the
private `kh2-combat-out/`.

## What the executable does

The weapon object's per-frame routine `FUN_1403effa0` (vtable slot at 0x142bbb160)
owns the swing trail through one handle at `obj+0xc00`:

- No handle, `FUN_1403ba840(obj)` clear and the object's range-12 flag set
  (`obj+0x18c` bit 11, from the weapon motion's range trigger 12,
  `FUN_14040d810` case 0xc): start element 10 of the weapon's own effects
  (`FUN_1403b6670(obj, 10, 1, 0, 0)`) and keep the handle.
- A handle and the same conditions still true: keep it.
- A handle and the window closed (or blocked): end the effect
  (`FUN_1402be8e0` = `FUN_1402c2830` sets the instance's end flag, +0x91), then clear
  the handle. A looping effect ends this way once its parts have gone, so the
  strip fades out by its own life, and the next window starts a new one.

`FUN_1403ba840` blocks when `obj+0x124 & 0x180`, or in an event mode
(`DAT_142a24edc == 1` for an object without flags 0x44 in `+0x6c8`), or when
`obj+0x120` bit 10 or a menu slot is set.

## What the room did

`weapon_trail::tick` started element 10 when a window opened and never ended it. The
element is a looping program (PAX element 10: looping, fade-out 0), so the instance
lived for the rest of the room and kept drawing the strip along the blade, which is the
orange edge seen on every Keyblade while running or walking after a swing. A run
(`wk1`) kept two owner-0xfc instances alive from tick 51 on.

## Rewrite

`weapon_trail::tick` follows `FUN_1403effa0`: `Game::trail_live` is the handle
(recomputed by `drive_fx` from the running instances), a window opening spawns once, a
window closing queues `Game::fx_release` for the trail's owner and `drive_fx` calls
`Instance::release` (the new `is_released` reads it back). The old rescanning of
window starts is gone.

## Checks, uncertainty, verdict and next task

PASS unattended runs with the installed source: the armed run after a swing no longer
shows the strip (`cap14`, tick 170, against `cap10`), the two instances spawned at the
swing's windows (ticks 51 and 69) are released at the window ends and gone by tick 90,
and `weapon_trail_length` is no longer logged every tick while unarmed.
Engineering checks are recorded in the pull request.

Not verified: no installed side-by-side of the trail's fade length; `FUN_1403ba840`'s
blocking conditions (`obj+0x124 & 0x180`, event mode, menus) are taken as clear; the
window's last frame is treated as inside it (inclusive end). Separately,
`FUN_1403d9be0` keeps the eight most recent hit effects and fades the oldest in four
frames; that cap is not ported (issue filed).

Verdict: PASS for issue #52's symptom in the development room; PARTIAL against the
installed game until a capture exists.
Next: the hit-effect cap; owner checks the trail against the original.
