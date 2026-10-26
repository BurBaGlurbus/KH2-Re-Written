# T259 — Drive forms' physics and combos, and how magic joins the combo

Owner Claude, 2026-10-26, requested by burba in the project chat ("fix all the drive forms physics, effects,
combos, and feel exactly 1:1 with the original. also fix magic combo system as some magics dont contribute to the
combo system"). Gate: development of the test room's combat; no final gate is inferred. Prerequisites: T108 and
T113 (Drive forms), T136 (magic and the combo gate), T121 (combo abilities), the movement work of
`docs/research/movement-feel-1to1-plan.md`. Build identity: local `main` after `fe2a81f`, package 0.1.2, release
launcher with the `skate3` feature. Source: the selected Steam KH2FM build, validated each run; the install was only
read. Ghidra 12.1.4 headless on private read-only project copies.

Private evidence (outside the checkout): `%LOCALAPPDATA%\KH2FinalRewrite\research\drive-magic-20261026\` (the
census of every exe use of obj+0xde0, `g\disp-de0.txt`; decompiles `g\form1`, `g\stat`, `g\gate`, `g\exec`,
`g\exec2`), and this session's scratch folder (tour logs `mc*-f*.jsonl`, `mv*-f*.jsonl`, captures, the summary
scripts `combo_summ.py` / `moves_summ.py`).

## Method

1. Census of the form id: every instruction reading obj+0xde0 (83 sites, ~50 in gameplay code) was decompiled and
   compared with the rewrite. Everything a form changes beyond its own data (PTYA, PRTY, motion set, BDX programs,
   ability slots, fmab rows) is in those branches.
2. Two unattended probes added to the magic tour (`--magic-tour`, with `--magic-form n` for a form):
   - per spell (Fire, Blizzard, Thunder, Cure, Magnet, Reflect) three combo cases: Attack, Attack, spell (the
     finisher); Attack then spell; jump, air Attack then spell; Attack is mashed after each cast;
   - `--form-moves` (alone, or after the rest): a full-stick run, a half-stick walk, a standing jump, a mashed
     ground combo and air combo on the Shadow, Square while running, then Jump and Square in the air, with a
     `move_probe` event every tick. The form's time is held full during a form tour; Antiform comes from the roll
     (Light & Darkness on a Valor request in battle).
3. Runs for base Sora and all six forms, before and after the fixes.

## Findings and fixes

| Item | Original (evidence) | Rewrite before | Now |
| --- | --- | --- | --- |
| Ground attacks that hop (PTYA +0xc > 0; Master Form's whole ground combo, base Sora's ability leaps) | FUN_1403d78d0 enters mode 1 through the physics jump slot; the mode's land test runs only once the arc has stepped | The take-off tick collided with the floor and landed: Master replayed its landing motion every 12 ticks and never attacked on the ground | The take-off tick does not land (test `a_ground_attack_with_a_hop_takes_off`). Master: 181 → 185 → 181 → 185 → finisher 184 |
| A hop after a hovering swing (range 3 ending with the motion change) | Mode-1 enter FUN_1403cbd30 clears obj+0x120 bits 23/24, so FUN_1403b4780 does not re-seed the arc as a fall | The new hop was overwritten by a plain fall the same tick (Master's second swing dropped to the floor) | Every arc launch clears the hover memory (test `a_hop_after_a_hovering_swing_rises_instead_of_falling`) |
| Damage taken in Antiform | FUN_1403c12f0 @0x1403c15e0: the lead player in form 6 takes × SSTM +0x94 (1.5), truncated, after the difficulty term | Not applied | Applied for Sora in the room, the battle room and the duel (`TargetState::antiform`) |
| HP prizes in Antiform | FUN_1403dbea0: the lead player in form 6 skips prize kinds 0 and 1 | Collected (sound, effect) without the heal | They stay on the ground |

Checked and already matching (no change): Antiform's voices (FUN_140404d00), its dimmed reaction command
(FUN_140408520), its refill-as-drain and bars (FUN_1403c0b50, FUN_1403c17c0), the Drive payment of the rolled
form's command cost (FUN_140405440: Antiform pays 9), Wisdom's Attack → Shoot (FUN_1403f2600), Limit Form's Limit
row (FUN_1403f29e0), Lion form's exclusions (form 7), the form keyblade in the stat totals (FUN_1403c26c0 →
FUN_1403c1d90), the form ability pass, and the range triggers that open combo windows (FUN_14040d810: only 11, and
35 when its parameter is the current motion).

Per form, measured in the room after the fixes (PRTY from the form's own objentry; growth from its slots):

| Form | Run / walk | Jump | Ground combo (mashed, Shadow in front) | Air combo | Square / air Jump |
| --- | --- | --- | --- | --- | --- |
| Base | 8 / 2 | 185 | 151, 153, finisher 155 | 181, 182, 183 | Guard |
| Valor | 12 / 2 | 235 (High Jump LV1) | 151, 153, 151, finisher 155 (Combo Plus) | 181, 182 (the second misses from above) | none in the probe (no guard record in its PTYA) |
| Wisdom | 12 / 2, inertia 0.85 / 0.97 | 185 | Shoot 151 repeated, finisher 154 | one shot (181 has no combo window) | Quick Run 203 → 204 |
| Limit | 8 / 2 | 285 | 162, 151, finisher 154 | 181 | Dodge Roll 209 |
| Master | 10 / 2 | 285 | hops: 181, 185, 181, 185, finisher 184 (air limit 3 + 2) | 185, 181 | Aerial Dodge 202 on the air Jump |
| Final | 16 / 2, inertia 0.94 / 0.99 | 585 | 151, 152, finisher 153 | 182, 181 | Glide 208 |
| Anti | 16 / 2 | 185 | 151, 153, finisher 155 (each with several windows) | 181, 182 (144-tick hover) | none |

With `--form-moves-chain` (Combo Master during the moves, so a miss does not end the string) every form reaches
its finishers from the data: base air 181, 182, 183; Valor air 181, 182, 181, finisher 183 (Air Combo Plus);
Limit air 181, 182, 183; Master 181, 185, 181, 185, 184; Final air 182, 181, finisher 183. Final's 183 starts
its elements 42–50 at the PAX's frames, 44/45 on the right Keyblade's grip (0x4005) and 46/47 on the left's
(0x4006) at frames 36/40 — the elements issue #147 had never seen (read from `P_EX100_ULTF.mset`'s frame
trigger 1 records).

## Magic and the combo

Exe (FUN_1403d6fc0 case 2 → FUN_1403d6ca0(obj, 1), FUN_1403a9dd0, FUN_1403d7780, FUN_1403c5170): every spell is
accepted inside a combo window (range 11 / 35), a finisher window (range 50) or when the motion takes commands,
counts one combo step whether or not it hits, clears the "connected" bit, and is the ground finisher when the counted
step reaches the ground limit (never in Lion form, never in the air). The next Attack chains only inside the cast
motion's own windows and only if a spell hit set the bit again (FUN_1403d12c0 on the spell's owner).

Room results (base form, same in Wisdom; Master's combo is airborne, so casts land between its windows):

| Spell | 3rd hit after Attack, Attack | Attack after the spell (spell hit) | Why (installed data) |
| --- | --- | --- | --- |
| Fire | finisher motion 57, row 5 | chains (step 3) | 56 has a combo window [60,80] |
| Blizzard | finisher 60, row 5 | chains | 59 window [36,72] |
| Thunder | finisher 63, row 5 | chains | 62 window [42,68] |
| Cure | ground motion 65 (no finisher motion in the magic table) | new combo | 65 shares 62's animation; its range 35 opens only for 62 |
| Magnet | ground motion 68 (no finisher motion) | new combo | 68 has only a finisher window (range 50) |
| Reflect | finisher 72, row 5 | new combo unless something was reflected | the barrier only hits when it reflects |
| any spell in the air | air motion, no finisher | no air chain | the air cast motions carry no combo window |

So in the rewrite every spell already counts toward the combo, and the spells that end it do so because of the
game's own motion data. No change was made to the magic combo rules.

## Checks

| Check | Result |
| --- | --- |
| Form probes (`--magic-tour --form-moves`, forms 0–6), before and after | PASS after: table above; before, Master's ground combo never started and its second swing fell |
| Magic combo probes (base; Wisdom, Master, Final with the form held) | PASS: as in the magic table |
| New unit tests: ground hop, hop after hover, Antiform damage | PASS |
| `cargo fmt --all --check` | PASS |
| `cargo clippy --all-targets --locked -- -D warnings` | PASS |
| `cargo test --locked` | PASS |
| `cargo build --locked` | PASS |
| Smoke `cargo run -p main --locked -- --frames 180` | PASS |
| Release launcher with `skate3` | PASS |
| Side-by-side with the installed game | NOT RUN: no save with Drive forms exists on this PC (T232), and the install is not modified |

## Uncertainty

- Combos were mashed against one Shadow standing 110 ahead; a swing that misses (Valor's and Final's air combos
  from above) ends the chain as the exe's connected bit would, but enemy height and size change which records fit.
- Wisdom's air shot has no combo window in the data; whether the original lets Wisdom shoot again in the air before
  landing was not observed.
- The refraction draw (#148) and the items of #150 (the Keyblade's reappear flash, arrival replay, slot-1 sparks)
  are not part of this record. Element 51 of Final's 183 (frame 144) did not start: the motion ended first in the
  room; not compared with the game.

## Verdict

PASS for the traced items in the room: the form-specific rules found in the executable are applied, Master's and
other hop attacks take off and chain, and magic counts toward combos by the original rules; no final gate, nothing
compared side by side with the installed game.

## Next

The remaining form effects (#150, #148); a side-by-side check once a save with Drive forms exists.
