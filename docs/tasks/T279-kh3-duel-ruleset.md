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
captures; deleted after the follow-up, whose kept runs are in the research folder below),
`%USERPROFILE%\kh2-lan-out\kh3lan*` (loopback LAN runs).

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
    Upper Slash (0x0c) for the launch, Retaliating Slash (0x28) for the counter (the ground chain's first swing,
    0x05, since the follow-up). Their damage is KH2's ATKP rows; their reactions the reference's (follow-up).
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

## Follow-up, 2026-10-28: effects, sounds, reactions, guard, the left-out moves

Requested by burba in the project thread: every KH3 move and event with its effects and sounds on the host and
the joiner; the other Sora reacting to each attack as KH3 does (stun, launch, knock-down and getting up, air
reactions); blocking, which did not work in either ruleset; KH2 assets in place of anything that needs KH3's;
and the left-out list again. Evidence (private): `%LOCALAPPDATA%\KH2FinalRewrite\research\kh3-combat\` —
`kh3-cues-reactions-table.md` (move → effect → sound → host → joiner, hit → reaction), `kh2-standin-catalog.md`
(the KH2 sound and effect ids), `runs-20261028\` (`run24` Test KH3 mode with captures, `kh3lan19` loopback LAN,
`guard-kh2` / `guard-kh3b` real key presses, the audit scripts).

- **Cues, `player::kh3::cue`.** A table from each move and moment (a lock, the 24th lock, the bolts' ring and
  launch, a bolt's impact, a wall-run step, a dash's afterimage, a pole turn, the grand magic's offer) to KH2
  stand-ins: Quick Run's afterimage (Sora's element 0x6e) for the dashes, Wisdom's muzzle, shot spark and Quick
  Run MAX for the bolts and the airstep, the clash flash (system 13) for the counter and the warps, the lock-on,
  lock-off, cursor and Drive-gauge-full sounds for the shotlock, the jump, roll, run-cloth and swing sounds for
  the flowmotion. Moves whose KH2 motion already sounds keep the motion's own sounds and voices. Every cue is a
  one-shot; a lasting move repeats one now and then while it runs, so nothing loops past its move. The room
  plays them on the Sora's own containers (`duel_kh3::cues`, logged as `kh3_cue`), and the duel's existing event
  path takes them to the joiner (sound cause `kh3`; Wisdom's container is made ready there). A bolt's spark
  plays at most once in 4 ticks on one Sora, and a body already knocked into the air keeps its motion (and
  voice) when another bolt hits it.
- **Resident bank 999.** The room never loaded `se/se999`, the second bank the game keeps resident from boot
  (fieldsfx-01), so every 999xxx sound was silent, in single player too: Aerial Dodge, Quick Run, Wisdom's shots,
  Aerial Recovery, the forms' and limits' effect sounds. It is loaded with Sora's voices now.
- **Reactions, `player::kh3::hurt`.** Each hit takes the reference's damage kind and scale by what it came from
  (a swing, a finisher, a flow attack's hit, the launch, the counter, Final Blow, a bolt, a spell or its
  finisher, a grand magic): flinch 550 cm/s × scale, ×0.9 a frame; blow 2000 and 300 up, ×0.95, gravity under
  700; toss 1500 and launch 2500 straight up at ×3 gravity with the 0.1 s hang; parabola at 45°; beat
  straight down; spins as blows and tosses [duel choice]. The victim cannot act until the reference's change
  point (the hit stun: 0.4667 s for a flinch, 0.6 s after a knock-down's landing); heavy kinds land him into
  KH2's knock-down and get-up (`BLOW_LAND`); Aerial Recovery (the KH2 ability) from 0.2 s into a blow, toss,
  launch or parabola. The armour rules: the flow states and a command's first 0.1 s shrug off light kinds. No
  invulnerability while flinching or in the air (juggles go on); [duel choice] KH2's while down, and 0.25 s
  after getting up. No hitstop (the reference's shipped game has none). Logged as `kh3_react` (attacker,
  victim, source, kind, scale, shrug, the victim's mode and motion).
- **Guard, both rulesets (`duel_hits`).** The duel's guard-against-swing test took the vector from the guard's
  owner to the attacker where the enemy port (and FUN_1403d1f40's callers) take the other way, so a guard only
  met swings from behind; and the guard recoil pushed the guarder toward the attacker, facing away. Both now
  follow the battle-enemy code: the guard flash (11) with the recoil and its invulnerability, or the clash
  flash (13) in the air or when a row says so (the joiner gets the flash id); a swing whose own row repels it
  plays the Keyblade's recoil in the KH2 ruleset [I: the map recoil stands for the reflect motion]. In the KH3
  ruleset a guarded hit pushes back at the reference's 300 or 1000 cm/s, opens the counter window, and a
  heavy one offers the Risk Dodge. Single player was already right (same test, the right way round; logs show
  guards against the Shadow).
- **Built from the left-out list, with KH2 stand-ins:**
  - Final Blow: a counter that blows the other Sora away offers the chase for 1 s [duel choice]; 0.2 s of
    Quick Run, then he is put 150 cm short and 100 cm under the target and the second air finisher record plays
    (0.3 small, then 0.6 sidespin).
  - Risk Dodge [duel choice, the reference only offers it from an enemy's marked attack]: guarding a heavy hit
    offers it for 0.5 s; Jump slides under (the Dodge Roll motion) to just behind the attacker (300-1000 cm,
    invulnerable, turned round) or warps behind one in the air, and offers the counter for 2 s.
  - Focus Syphon: once the focus gauge is emptied (until it is full again) the shotlock takes up to 18 locks, each
    after 0.0002 s a cm, and gives trunc(1.4 × locks) focus, no damage.
  - KH3's lock-on camera while locked on: from behind Sora toward the target, 20° down (none when the target is
    well below), back far enough to frame both (at least 400 cm, at most 1600 [duel choice]), eased at 8 a
    second, half a second in and out.
- **Script and logs.** The KH3 test script runs 6400 ticks: one Sora acts for half the cycle while the other
  takes it (guards as a swing comes, Aerial Recovery during the Ragnarok, the Risk Dodge, the counter and Final
  Blow while offered). [Test] The scripted lobby's ban skips Guard (it banned the first of the pool, which is
  Guard, so a scripted joiner could never block).
- **Fixed on the way.** The layer's own states never ran the KH2 command lockout down, so after some swings
  every attack and cast was refused; records the layer started (launch, counter, flow attacks) did not report a
  new action, so their hit windows kept the previous action's attack id and the launch and the counter never
  hit. The counter plays the chain's first swing (0x05) now (Retaliating Slash is an air record cut short on the
  ground; Counterguard's swing only clashes); the counter and the launch home to 60 cm [duel choice]; Upper
  Slash rises from 0.35 s, after its hit [duel choice]; a record the layer plays snaps to the target first.

## Left out

- Attractions, Links, team attacks: no counterpart in a 1v1 duel of two Soras (rides, partners, party).
- Keyblade transformations and Formchanges: each is a Keyblade's own motion set and model change; KH2's Drive
  forms are the duel's counterpart and stay. Too big for stand-ins.
- Rail slides, pole swing (bars), corner rolls, ledge grabs: the arenas have no rails, bars or ledges.
- The other Keyblades' shotlocks: the reference has behaviour only for kinds 1, 2 (built) and 36 (Focus
  Syphon, built); the rest are rows of lock counts with no behaviour to stand in for.
- Risk Dodge's slow motion and the shotlock's: one clock for two players.
- Magic combos beyond KH2's own: KH3's Fire → Fira → Firaga are separate menu tiers, not a chain, so the duel
  keeps KH2's spells, their levels and KH2's spell-in-combo rule.
- The pole spin's own damage (its rows' power is not in the reference).
- Payback Strike after Aerial Recovery, Critical Counter's just guard: their timers are not located in the
  reference.

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
| Follow-up unit tests `player::kh3` (32 now: every attack's damage kind, the flinch's slide and stun, the toss's peak and the knock-down and get-up, the launch's peak and fall, Aerial Recovery, armour and the command's tenth, a re-hit in the air, Final Blow, the Risk Dodge, Focus Syphon, every move's stand-in in a loaded bank and the repeats, the layer's cue names) and `duel_wire` (the flags) | PASS |
| Follow-up Test KH3 run `run24` (6400 ticks, both scripted) | PASS: every cue played (none unplayed), no sound more than twice in a tick; reactions: swing flinch ×11, air flinch, finisher and air finisher blows into down and up, launch toss ×2, counter vertical roll ×3, Final Blow ×3, King of Hearts sidespin, Ragnarok toss ×10, spell finisher parabola, Aerial Recovery ×2, guard, Risk Dodge, grand magic offer and cast |
| Real key presses (Right Shift posted to the window, `guard-kh2`, `guard-kh3b`) | PASS: Guard from the keyboard path; guarded hits (flash 11 and 13) in both rulesets |
| Follow-up loopback LAN `kh3lan19` (both scripted, KH3) | PASS: the joiner guards the host's swing and the host guards, counters and Final Blows the joiner's; the joiner plays every effect the host starts (by element 1:1 but one afterimage at the very end) and the Soras' sounds; joiner receive 10.8 KB/s |
| Pole spin, side wall run, slide end, Focus Syphon and the Risk Dodge in a LAN run | NOT RUN: the scripts did not reach them; unit tests only |

## Uncertainty

- No KH3 reference run exists here: every number comes from the reference's own reading of KH3, not from the
  game.
- The motions are KH2's: timings that KH3 ties to its own clips (cancel windows, hit windows) follow the KH2
  motions instead.
- Two PCs were not tried; the LAN runs are loopback on one PC.
- The cameras: the wall camera frames a Sora on a wall from the arena's side. Elsewhere KH2's follow camera
  trails the fast moves, and some captures show only the wall or a distant Sora. KH3's own camera was not
  ported.
- Damage of the flow attacks and the launch is the KH2 records' ATKP, not KH3's powers; their reactions are the
  reference's kinds and scales now.
- The stand-in sounds and effects were chosen from what the ids are known to be (the catalog), not by listening
  side by side with KH3.
- On the joiner, the flow attack's whoosh (500046) played 6-7 fewer times than on the host in the LAN runs; not
  explained.

## Verdict

PARTIAL (in progress).

## Next

Tuning in play with two players; poles on an arena that has pillars; bolt visuals checked side by side; the
joiner's prediction (none yet, as for the KH2 ruleset).
