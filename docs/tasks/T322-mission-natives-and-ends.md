# T322 — The missions' other natives and ends; a tour that casts magic

Owner Claude, 2026-10-31, requested by burba in the project thread (issue #163). Gate: development of enemy AI in the test
room; no final gate is inferred. Prerequisites: T321. Build identity: local `main` after `24348f4` plus this change, package
0.1.2, release launcher with the `skate3` feature. Source: the selected Steam KH2FM build, validated each run; the install was
only read. Ghidra 12.1.4 headless on private read-only project copies (plain Ghidra, no MCP).

Private evidence (outside the checkout): `%LOCALAPPDATA%\KH2FinalRewrite\research\enemy-1to1-20261027\runs-20261031` (`f8`, `t13`,
mission scripts in `msn2`), decompiles in `g\st`.

## What was wrong

The 66-program boss sweep `f8` on T320 reached 23 clears, with three natives missing from mission scripts: 1:142 (HE05, HE06,
HE19, MU07), 4:20 (LK02, HE17), 1:332 (TT05). `mission_complete`, `mission_exit` and `mission_failed` did nothing, so a mission that
ends on one of them never ended the room.

## What the data shows

Of the 44 missions dumped, 19 share `ms_boss`. The others show what a fight's end really rests on:

- `he17_ms105`: code 75 lowers a count, code 14 completes the mission, code 74 fails it.
- `ms_attackend` (HE06, HE19): its trigger 10 reads codes 123, 20 and 74, and it has an OBJ_DAMAGE of its own.
- `ms_luxde` (EH14): the fight is a gauge fed by damage, through `mission_add_gauge`.

FUN_1403ab150 (`signal_reserve_min_hp`) puts a signal on the list at DAT_142a112b8. FUN_1403aaf30 walks that list every frame: an
entry of kind 1 fires when the object's pool 0 is at or below its floor, and kind 0 when its rate is at or below a value.
FUN_1403aabe0 drops every entry of a code when that code is signalled.

## Changes

- `traps::mission`: 1:142 and 1:332. `traps::service`: 4:20 reports itself.
- `boss::reserved_signals`: FUN_1403aaf30's kind 1.
- `mission_complete` plays the clear as `mission_dead_boss` does, and `mission_exit` ends the room. `mission_failed` ends a boss
  fight's room. [I] The first two have overlays and jumps of their own that are not carried out.
- `KH2RW_TOUR_CAST=<ticks>`: the area tour casts Thunder, Blizzard and Fire in turn, for fights that turn on magic.

## Checks

| Check | Result |
| --- | --- |
| `cargo fmt --all --check`, `cargo clippy --all-targets --locked -- -D warnings`, `cargo test --locked` | PASS |
| Synthetic test of the reservation | PASS |
| Grim Reaper tour with magic (`t13`) | The magic reactions and `medal_vary` run. He stays at full HP and the fight does not end in the tour; medal flow not confirmed by a count |
| Boss sweep on this build (`g1`), ordinary sweep on this build (`g2`) | Running at the time of this commit |
| Side by side with the installed game | NOT RUN |

## Uncertainty

- OBJ_DAMAGE of a mission script (key 6: CA01, EH14, HE06, HE19) is not fired, so Luxord's gauge does not fill and those fights
  cannot end by their own rule.
- The HUD's counts, gauges and timers still only report themselves.
- FUN_1403aabe0 also passes a signal to FUN_1403f4980, FUN_1403b4270 and FUN_1403f60d0; only the mission gets it here.
- Kind 0 reservations (FUN_1403ab0c0) have no script caller found yet and are not ported.

## Verdict

PARTIAL.

## Next

The two sweeps' results, then a mission's OBJ_DAMAGE.
