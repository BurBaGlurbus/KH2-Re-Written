# T191 — The eight most recent hit effects stay, the displaced one fades (issue #107)

Owner Claude, 2026-10-18, requested by burba in the project chat (issue #107, found
while tracing T185). Gate: development of the test room only; no final gameplay
acceptance is inferred. Prerequisites: T150, T180, T185. Build identity: branch
`claude/combat-feel-hit-spark-cap` on PR 112's branch (`main` at `554d3bf`), package
0.1.2, Rust 1.99.0 Windows MSVC. Source: the selected Steam KH2FM build, validated each
run; the install was only read.

Private evidence root: `%LOCALAPPDATA%/KH2FinalRewrite/research/combat-feel-20261017/`
(decompiles `g/d22`, `g/d47`, references `g/r19.txt`, data `g/dd11.txt`); unattended run
`sp1` in the private `kh2-combat-out/`.

## What the executable does

`FUN_1403d9be0(handle)` keeps a ring of eight effect handles (`DAT_142a24cd0`, index
`DAT_142a24d10`, advancing modulo 8). Each hit spark registers in the next slot; if the
slot still holds an effect, `FUN_1402be6a0(old, 4.0)` ends it with a fade over four event
frames (`DAT_140623b90`). `FUN_1403d9c40` sweeps dead handles out. The callers are the
hit-effect creators: the attacker's hit spark (`FUN_1403d1b40`, `FUN_1403d10c0`), the
clash spark (`FUN_1403d0f00`), the map spark (`FUN_1403d1960`) and `FUN_1401b82f0`.

## What the room did

Only the legacy list's `hit` label was capped, by deleting the oldest hit effect outright
(no fade) when a ninth started; clash (`system`) sparks and the enemy's hit sparks (the
`drive_fx` list) were not counted.

## Rewrite

`SparkRing` (a resource): each Keyblade hit, map spark, clash spark and enemy hit spark
registers; the effect displaced from the slot gets `Instance::fade_out(4)` in whichever
list holds it (`effects_tick` or `drive_fx`), and nothing is deleted. Enemy hit sparks now
use their own owner id (`ENEMY_HIT_OWNER`, 0xf9) so the enemy's death effect (0xfe) is not
counted. The log gains `spark_fade` events.

## Checks, uncertainty, verdict and next task

PASS unit test `the_ninth_hit_effect_fades_the_first_and_so_on_round_the_ring`. PASS the
combat tour with the installed source and the ring temporarily shortened to two slots
(private `sp1`): displaced live sparks logged `spark_fade` and were faded rather than
dropped. Engineering checks are recorded in the pull request.

Not verified: no run reached nine live sparks at once with the real ring size (the tour's
hits are far apart), so the live behaviour at eight is by the shortened run and the unit
test; `FUN_1401b82f0`'s effects are not identified in the room; the fade length (four
event frames, eight game frames) follows `Instance::fade_out`'s definition.

Verdict: PARTIAL. The ring follows `FUN_1403d9be0` in the development room.
Next: none for this item.
