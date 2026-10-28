# T279 — LAN duel: a KH3 ruleset (movement, flowmotion, combo homing, focus, shotlock), multiplayer only

Owner Claude, 2026-10-28, requested by burba in the project thread: take only Kingdom Hearts III's combat,
physics and movement abilities, effects, game flow and gravity from StrawHatPro's `kingdom-hearts-x-elden-ring`
(flowmotion, shotlock and the rest) into this rewrite as a multiplayer-only game mode, with a multiplayer test
option. Gate: none of the roadmap's. This is an addition outside the original game, in the LAN duel only; no
fidelity claim is made and single player is unchanged. Prerequisites: the LAN duel (T240-T249, merged into main
at 7405db8). Build identity: local `main` after `1f7e809`, package 0.1.2, Rust 1.99.0 Windows MSVC, release
profile for the runs. Source: the selected Steam KH2FM build (and, for the Mirage Arena, its BBS package),
validated at each run; the install was only read.

**Reference and licence.** The behaviour reference is the MIT repository `StrawHatPro/kingdom-hearts-x-elden-ring`
(Copyright (c) 2026 StrawHatPro). It was cloned read-only outside the checkout, read through its specs, port
sheets and Rust, and deleted afterwards. Its crates could not be used as they are: they include tables generated
from a KH3 install, and their code, comments and documents are full of KH3 executable addresses and
decompilation. The moves were written anew in `player::kh3`. Only behaviour descriptions and plain tuning
numbers (speeds, gravity, timings, gauge costs, lock counts) were taken. No KH3 code, data, addresses or research
notes are in the repository. Credit and the MIT notice: `CREDITS.md`, `NOTICE`. A private digest of the
reference's behaviour and numbers is kept outside the checkout (`%LOCALAPPDATA%\KH2FinalRewrite\research\kh3-combat\`).

Evidence (private, outside the checkout): `%USERPROFILE%\kh2-kh3-out\` (local runs `run01`-`run0N`: logs,
captures), `%USERPROFILE%\kh2-lan-out\kh3lan*` (loopback LAN runs).

## What was built

- **Ruleset row.** `duel_rules::Ruleset` (KH2 default, KH3): the lobby's Match page row *Ruleset*, the
  host's choice, sent to the joiner with the rules (one more byte in the rules message), kept in
  `duel-prefs.json`; `--duel-ruleset kh2|kh3`.
- **The layer, `source/game/player/src/kh3/`.** One `Kh3` state per Sora (in `Fighter`). Its `tick` stands in
  for `Player::tick` in the KH3 ruleset. It owns locomotion and its own moves and drives the player's action and
  frame on KH2's motions. Attacks, spells, items, Drive and damage stay with the KH2 controller, with KH3's rules
  laid over them. `tuning.rs` holds every number with its source, `surface.rs` the wall probes and pole finder.
- **Room side, `test_room/duel_kh3.rs`.** Both tick sites (the room Sora in `simulate`, rivals in `rival::tick`)
  call the layer when the duel's rules say KH3. Shotlock bolts ride Wisdom Form's bullets: their effect (MAGF
  element 20), hit volume (ATKP 859) and snapshot entry. A grand magic is the room's own cast with its MP kept.
  Poles are found among the arena's wall triangles. The lean (wall run, pole, airstep) turns the model and its
  HIT spheres. The wall camera stands back while the local Sora is on a wall. The HUD shows a plain focus bar,
  the lock count and the grand magic offer.
- **Bounds.** KH3's moves climb higher than the arenas' walls are closed, so a Sora is kept inside the walls'
  extent: the Mirage Arena's ring as a circle, the test room's walls as a square. Before this, a wall run to
  the top, then a flow jump and a Superslide, left the Mirage Arena (`kh3lan03`/`04`, radius 3790). The layer
  also moves in steps of at most half the body radius, so a Superslide or an airstep cannot step through a thin
  wall.
- **Wisdom's effects.** `DriveRt::wisdom_pax` takes Wisdom's effect container from its preloaded kit without a
  form change. The bolts use it, and so do the joiner's copies of the other Sora's bullets: in the KH2 ruleset
  too, a joiner now sees Wisdom's shots.
- **Network.** The pad gained two bits (R2 held, Triangle). Snapshots carry an optional KH3 block per fighter:
  focus, style, mode, locks, aiming, offer, two lean angles (10 bytes; 1 byte when the ruleset is KH2). Bolts go
  in the existing bullet list, now up to 32 a fighter, 32 bytes each, only while a volley is in flight.
- **Test option.** The launcher's Multiplayer screen has *Test KH3 mode* (`--kh3-test`): a local duel in the KH3
  ruleset against the scripted rival, with Sora alone as in a session. `rival::kh3_pad` scripts every move in a
  1600-tick cycle. With `--autoplay` and `--rival-script` the room Sora runs it too, 800 ticks apart, and in a
  LAN session `--rival-script` on either side scripts that side.

## Taken from the reference (converted: lengths x160/180, per 60 Hz tick)

| Move | Behaviour and numbers kept |
| --- | --- |
| Gravity and jump | -1470 cm/s² falling, x2.8 rising with the button held, x5.6 after release, x0.1 for 8 ticks after the apex, 4000 terminal; jump 1480 on the 5th tick; High Jump 1950 (with KH2's High Jump growth); landing control after 0.0991 s, a new jump from the 2nd tick |
| Doubleflight | 2300 on the 4th tick, once per airtime, from vertical speed ≤ 500, no short-hop cut |
| Ground | walk 140, run 680, sprint 980 after 1 s, braking 16/s + 2048, facing 10 % a tick |
| Air control | 680 max, 3488 accel, 2048 brake, facing 10 % a tick |
| Dodge Roll | the root curve's keys (600 cm in 0.7 s), invulnerable and uncancellable to 0.4659 s |
| Air Slide | level 1: 819 cm over 0.7333 s, gravity off, invulnerable throughout, cancel at 0.5 s, 0.6 s cooldown after ending in the air |
| Glide | jump held while a jump falls; 1500 cm/s along the facing, facing 3 % a tick, gravity x0.001, ends on release |
| Superslide | 4000 cm/s losing 1000 a second for 1 s, then the 426 cm end; steering 2.5 % a tick; wall kick / pole after 0.1 s; Jump: flow jump; Attack: Sonic Slash |
| Wall run | Run or a fall into a wall with the stick: up within 35°, along at 50-130°; 1251 cm/s, 980 sideways drift; ends 0.2 s after the stick is let go, at the wall's end, or over the top (495 cm roll); Jump: flow jump (2500 + 175 push); Guard: wall kick into Superslide; Attack: wall-run attack |
| Wall kick | 0.4 s on the wall, 80 cm out and turned, Jump from 0.1 s, Guard latches a Superslide |
| Pole spin | grab within 100 cm of a point 120 ahead / 90 up from Air Slide, Superslide, airstep; 0.5 s attach, four 0.3 s turns; Jump: flow jump; Guard / time up: Superslide aimed at a target within 2500 cm and 50°; Attack: Buzz Saw |
| Airstep | Guard while aiming; legs at max(2d/1 s, min(60d, 6000)) cm/s, 3000 inside 600 cm, arrival inside 200 cm, invulnerable; hop back 150 / 96 over 0.6333 s; Attack then: Sonic Slash; Guard: Superslide |
| Homing | 800 cm/s toward the target inside 800 cm, stopping 160 short, easing 20 % a tick (the first air swing 1200, 30 %) |
| Air hover | air swings and casts hover; height closes at 3x the difference (≤ 500 cm) per second, then 0.1 of gravity |
| Launch | Jump in a ground chain that landed a hit, not a finisher, once a chain; rises 1000 cm/s from 0.1 to 0.6 s; the combo count restarts |
| Hit-confirm | a chain goes on only once one of its hits landed |
| Counter | 1 s window after a successful guard |
| Armour | light reactions do not flinch during Superslide, its end, the kick, the pole, flow attacks and a flow jump's rise |
| Focus | 100; aiming burns 20 a second until 24 locks |
| Shotlock | hold 0.2667 s, 0.1 s delay, a lock every 2.24/24 s within 5000 cm; Attack fires, release cancels; volleys lift 500 cm, invulnerable, ring of bolts 200 cm wide over 0.5 s, 7 spawned a tick; King of Hearts (1-23 locks: launch 3.07 s, end 4.0 s, 1500 cm/s, homing 180 → 360°/s from 0.0333 s) and Ragnarok (24 locks: launch 2.9 s, end 4.1333 s, 1000 → 2000 cm/s, 720°/s); homing off inside 100 cm |
| Grand magic | the style gauge (Fire 15, Blizzard 12, others 6 a cast; 1-8 per swing), decay 5/s after 20 s idle, offer for 20 s, no MP, double power (4.4 / 2.2) |

## Duel choices (not in the reference, or adapted)

- **Units.** KH3 lengths are scaled by 160/180, so each move keeps its size against Sora's body.
- **Motions.** KH2 stand-ins for KH3's motions:
  - growth motions: Dodge Roll 209, Air Slide = Quick Run 203, Doubleflight and the kicks = Aerial Dodge 202,
    Glide and Superslide 205, the poles 206 / 207, the volley 208;
  - the run loop on a wall, pitched or rolled;
  - attack records: the air finishers 7 / 8 for Sonic Slash and the wall-run attack, record 36 for Buzz Saw,
    Upper Slash (0x0c) for the launch, Retaliating Slash (0x28) for the counter. Their damage and reactions are
    KH2's ATKP rows.
- **Bolts.** Bolts are Wisdom's bullets (ATKP 859, subs 0 and 3); a volley deals at most 35 % of max HP.
- **Focus refill.** The reference has no refill without prizes. Here each hit dealt gives +3, each hit taken
  +5, and the gauge refills 1 a second.
- **Risk Dodge.** In the reference it is enemy-offered. Here a hit that meets a Dodge Roll or Air Slide opens
  the counter window.
- **Smaller choices:**
  - The cancel window of a swing is its first KH2 combo window.
  - The traction window is 0.1-0.33 s.
  - Air Slide's distance curve is fitted to its first 1/30 s.
  - The glide also ends on touching ground.
  - Wall-run tops roll back off the wall, so an arena wall never puts a Sora outside.
  - An up-run longer than 3 s kicks off.
  - Every wall is wall-runnable.
  - The volley's lift takes 8 ticks.
  - No slow motion during aim, since it is a two-player game.
  - The grand offer is the element with the most points (not a roll).
  - Style counts at the cast, not the hit.

## Left out

- Attractions, Keyblade transformations and Formchanges, Links, team attacks.
- Rail slides: the arenas have no rails.
- Final Blow and the chase.
- Risk Dodge's enemy-offered slides and warps.
- Pole swing (bars), corner rolls on walls, ledge grabs.
- The other Keyblades' shotlocks (only kinds 1 and 2 have behaviour in the reference).
- Focus Syphon.
- Magic combos beyond KH2's own: KH3's Fire → Fira → Firaga are separate menu tiers, not a chain, so the duel
  keeps KH2's spells, their levels and KH2's spell-in-combo rule.
- KH3's lock-on camera (KH2's lock stays).
- Slow motion.
- The pole spin's own damage (its rows' power is not in the reference).

## Checks

| Check | Result |
| --- | --- |
| Unit tests `player::kh3` (19: jump arc, short hop, Doubleflight once, roll curve and invulnerability, air slide, wall classes, wall run and flow jump, shotlock locks / focus / Ragnarok volley homing, King of Hearts and cancel, airstep speeds and arrival, glide, pole spin into Superslide, style offer, snapshot fields, armour, aim after a roll) | PASS |
| Duel wire, rules, prefs, sync, KH3 key tests | PASS |
| Local unattended KH3 test `run02` (both Soras scripted, test room arena) | PASS: dodge roll, air slide, launch, Doubleflight, glide, wall run (up and side), wall top, wall kick, flow jump, Superslide and its end, airstep, flow attacks that hit (9 damage), ground chains |
| Shotlock volley in game (`run01`) | PASS: aim, 24 locks, Ragnarok, 11 bolt hits of 2 |
| Loopback LAN, host and joiner both scripted, KH3 ruleset, Mirage Arena (`kh3lan05`) | PASS: the lobby and match ran; wall runs on the ring, wall top, kick, Doubleflight, glide, air slide, airstep and flow attacks, a Ragnarok from the joiner's Sora (24 bolt hits); the joiner shows its focus, lean and the bolts (Wisdom orbs); farthest radius 2185 against the 2160 ring; joiner receive about 10 KB/s, 37 KB/s while bolts fly |
| Test KH3 mode's arguments on the launcher build (`--kh3-test --input auto`, `run04`) | PASS: roll, launch, air slide, airstep, flow attack, wall run, King of Hearts, Doubleflight, glide, wall kick, flow jump, Superslide; Donald, Goofy and the Shadow are off |
| Poles in game | NOT RUN: the finder found one pillar in the Mirage Arena (radius 69, 1365 to 2142 up, overhead) and none in the test room; no grab happened in the runs; covered by the unit test only |
| Two PCs | NOT RUN |
| `cargo fmt --all --check`, `cargo clippy --all-targets --locked -- -D warnings`, `cargo test --locked`, `cargo build --locked`, smoke `--frames 180` | PASS |
| Release launcher (`tools\Build-Launcher.ps1 -Skate3`) | PASS |

## Uncertainty

- No KH3 reference run exists here: every number comes from the reference's own reading of KH3, not from the
  game.
- The motions are KH2's: timings that KH3 ties to its own clips (cancel windows, hit windows) follow the KH2
  motions instead.
- Two PCs were not tried; the LAN runs are loopback on one PC.
- The cameras: the wall camera frames a Sora on a wall from the arena's side. Elsewhere KH2's follow camera
  trails the fast moves, and some captures show only the wall or a distant Sora. KH3's own camera was not
  ported.
- Damage and reactions of the flow attacks and the launch are the KH2 records', ATKP's, not KH3's powers.

## Verdict

PARTIAL (in progress).

## Next

Tuning in play with two players; poles on an arena that has pillars; bolt visuals checked side by side; the
joiner's prediction (none yet, as for the KH2 ruleset).
