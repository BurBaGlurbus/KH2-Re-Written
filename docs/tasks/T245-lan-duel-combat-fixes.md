# T245 — LAN duel: combos that connect, magic checked spell by spell

Owner Claude, 2026-10-26, requested by burba after testing the duel ("comboing doesn't work on another player
because they have normal iframes when getting hit so its only 1 hit basically and magic doesn't work at all").
Gate: none of the roadmap's (an addition outside the original game; no fidelity claim). Prerequisites: T244.
Build identity: worktree `KH2FinalRewrite-lan`, branch `feature/lan-duel`; package 0.1.2, Rust 1.99.0 Windows
MSVC, dev profile. Source: the selected Steam KH2FM build, validated at each run; the install was only read.

Private evidence: `%USERPROFILE%\kh2-lan-out\` (bench logs `combo1-*` … `combo7j-*`, `combo7h-*`,
`magic1-*` … `magic12b-*`, `mag1*`; `magic_table.py` tabulates them).

## Combos

What stopped them, found with a bench that has one Sora combo an idle one (`--duel-bench` / `--duel-idle`)
and records every refused contact (`duel_hit_refused`):

1. The victim's hit record was the player's: a player-type record locks out every other attack group for 30
   ticks after a hit, and the lock does not run while he is in a damage motion (`HitRecord`, FUN_1403cf4b0).
   Each combo step is its own group, so only the first step landed.
2. Once that lock was gone, a swing hit on every tick of contact: the duel marked a swing's instance as seen
   only when it landed, and with the room's clock (`simulate` has advanced it when the duel's hits run) one
   tick ahead of the retire's (enemy_battle), so every instance's attack id was freed each tick.

The change (`test_room/duel_hits.rs`, `duel_loadout.rs`, `rival.rs`):

- [Duel rule] Each Sora takes the other's hits on a target's record (`HitRecord::new(false)`), as an enemy
  takes Sora's: no group lock, so each step that connects counts; the attack rows' own re-hit intervals still
  space multi-hits (each rival's record now advances once a tick, as the room Sora's does).
- [Duel rule] A Sora's flinches (hurt motions 0–5, ground and air) have no invulnerable frames in a duel; a
  blow (a finisher's launch or a knock-down), its landing, the recovery and the knock-out keep the frames the
  game's motions give them, as do dodges and attacks with frames of their own, so a launched Sora gets up
  before he can be hit again. This is the closest model to KH2's own: the player's damage protection is the
  group lock and those motion frames; a target (an enemy) has neither, and KH2 has no player-against-player
  rule to copy.
- Every live swing and spell volume marks its instance on the simulated tick, so a swing hits a Sora once per
  contact window.
- The bench presses inside a ground combo's reach (a press from farther starts the running lunge, which is what
  made the scripted rival look as if only step 1 landed).

Results at ×3 (an idle target; full Kingdom Key string: three steps, attack rows 126, 128, 147, then the
finisher 129):

| Attacker → victim | Keyblade | Steps 1–3 | Finisher | Full string |
| --- | --- | --- | --- | --- |
| Host → joiner | Kingdom Key | 15 | 12 | 27 |
| Joiner → host | Winner's Proof (the lobby script's pick) | 27 | 24 | 51 |

170 HP is about six to seven Kingdom Key strings, about 15 s of uninterrupted strings on a target that never
moves or guards; with players missing, guarding and moving that is in the 30–60 s range, so the default stays
×3 [duel choice].

## Magic

Found with a bench that casts each spell in turn from one side or both (`--duel-magic`, `--duel-idle`; each cast
is logged with the spell levels and MP before and after, `duel_cast`):

1. **No spell could be cast after the lobby.** The duel's item set (`duel_loadout.rs`) replaced the battle stock
   with the consumables, which dropped the spell items (the element items, whose counts are the spell levels).
   The camp shows the battle stock and, when it closes, writes the spell levels back from it (`camp_tick`).
   Once the lobby's build menu closed, every spell was at level 0 and every cast was refused without a sound.
   This is the "magic doesn't work at all". Fix: the duel's stock keeps the element items at the duel's
   levels, and the lobby's builds put the duel's stock in the battle stock as well.
2. **A Magnet hid every other hit.** The duel lets one attack land on a Sora per tick. A Magnet's volume
   (radius 800, about 200 ticks) touches its victim every tick, so it took that landing each tick even though
   the victim's record refuses it again. Fire, Blizzard, Thunder and swings that met him meanwhile were dropped.
   Fix: an attack whose id the victim's record already holds gives the landing up.
3. **Magnet only damaged.** The player's reactions have no case for the Magnet's hold (reaction 5,
   FUN_1403d2fe0). [Duel rule] A Sora a Magnet hits flinches and slides toward it, stopping at its hold radius
   (the middle of KB1 and KB2). The Magnet forms on its target, so in practice he is already inside and only
   flinches.
4. Bench only: the unattended script's own casts took the bench's turns, and the room's two test blows (ticks
   1500 and 1700) knocked the caster down. Neither applies in a duel now.

Results at ×3: each side on Kingdom Key, the duel's spell levels (level 3: Firaga … Reflega; the duel gives the
top level only, so Fire, Fira and the like cannot be cast in a match). Runs `mag1h` (host casts, joiner idle),
`mag1j` (the reverse) and `mag1b` (both cast, then an item and Valor), with `magic9h` … `magic12b` before them:

| Spell | Host → joiner | Joiner → host | MP | Damage a cast (×3) |
| --- | --- | --- | --- | --- |
| Firaga | fixed | fixed | 12 | 24 (5 hits) |
| Blizzaga | fixed | fixed | 15 | 24 |
| Thundaga | fixed | fixed | 18 | 48 (2 bolts) |
| Magnega | fixed (flinch; [duel rule] pull) | fixed | 30 | 6 |
| Reflega | fixed: blocks the other's spell and counters (33–48) | fixed | 10 | the counter only |
| Curaga | fixed: full heal (host 116 → 170, joiner 34 → 170) | fixed | all that is left | — |
| Item (Potion) | works (+68 HP) | works | — | — |
| Valor Form | works (3 bars), shown on the joiner | works, shown on the joiner | — | — |

The spells' effects reach the joiner (44–72 `spell_effect` records a run). A cast is refused while the caster
is in a hurt motion, as in the game. Curaga uses all the MP left and starts the MP charge (3000 ticks at the
base rate, the game's), so the bench casts it last.

Balance: the formula is power × (stat − defence). At level 99 Sora's Defence is 53 and his Magic 55 with Kingdom
Key, a difference of 2; the Keyblade with the most Magic (item 544, Magic +7) makes it 8, four times the damage.
At ×3 its Thundaga was 90 a bolt, 180 a cast: a knock-out from full HP.

## Magic damage rule and hit cap

The owner's decision (2026-10-26, 13:33Z): keep the game's formula and add a separate magic rule. [Duel rules]

- `MatchRules::magic` multiplies hits whose power is the attacker's Magic (the attack row's stat: spells,
  Reflect's counter, form magic); `damage` now multiplies the hits on Strength only.
- `MatchRules::hit_cap`: no single hit, attack or magic, takes more than this percent of the target's max HP
  (20, 25, 30, 35, 40, 50, 60, 75, or 100 for none).
- Both are rows on the lobby's Match page (host-editable, the joiner's read-only), go to the joiner with the
  rules over the reliable channel, and have flags: `--duel-magic-damage X`, `--duel-hit-cap PCT`.

The default, from the benchmark (`mag3h`: the host on Kingdom Key casting at an idle joiner; `mag3j`: the joiner
on item 544 casting at an idle host). Each hit's game formula, then the damage at magic ×1.5 and a 40 % cap:

| Keyblade (Magic bonus) | Firaga (5 hits) | Blizzaga | Thundaga (2 bolts) | Magnega |
| --- | --- | --- | --- | --- |
| Kingdom Key (+1) | 1, 1, 1, 1, 4 → 14 a cast | 8 → 12 | 8 + 8 → 24 | 2 → 3 |
| Item 544 (+7, the most Magic) | 4, 4, 4, 4, 14 → 45 | 15 → 23 | 30 + 30 → 90 | 8 → 12 |

[Duel choice] magic ×1.5, cap 40 %: a Kingdom Key Thundaga (24) is about a Kingdom Key combo string at ×3 (27),
and the strongest Keyblade's Thundaga takes about half of 170 HP, never a full bar. The cap (68 of 170) holds
even if the host raises magic: at ×3 the same Keyblade's Blizzaga (formula 30, 90) stopped at 68 (`mag3c`).

## Checks

| Check | Result |
| --- | --- |
| `cargo fmt`, `clippy -D warnings`, `cargo test` (1,089 passed) | PASS |
| Single-player tours (talk, magic, party-hit) against a build of `5d7a960` | PASS (identical) |
| Bench, joiner → idle host and host → idle joiner (`combo7j`, `combo7h`): full strings land, no refused contact, one hit per step | PASS |
| Magic bench, host → idle joiner, joiner → idle host, both casting (`mag1h`, `mag1j`, `mag1b`): every spell accepted, MP spent, hits, Reflect, Cure, items, Valor on both screens | PASS |
| Tours after the magic fix (`mag1-*` against `main5d-*`): talk, magic identical; party-hit identical but for the exit tick (6840/6841, known) | PASS |
| Magic rule and cap: rules round-trip and damage tests; `mag3h`, `mag3j` (numbers above), `mag3c` (×3 capped at 68; the joiner's Match page shows the host's Magic damage and Hit cap read-only) | PASS |
| Two PCs, players at the keys | NOT RUN |

## Verdict / next

Combos PASS, magic PASS, and the magic rule and hit cap PASS on one PC (loopback, unattended). Two PCs NOT
RUN. The menus are T246.
