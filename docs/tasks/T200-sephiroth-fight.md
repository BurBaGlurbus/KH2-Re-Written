# T200 — The Sephiroth fight: meeting, fight and aftermath

Owner Claude, 2026-10-18, requested by burba in the project chat ("a fully 1:1
recreation of Sephiroth's boss fight ... all animations, effects, VFX, SFX, intro scene,
reaction commands"). Gate: enemy AI and events (not passed; nothing here is inferred as a
gate pass). Prerequisites: T099 (event player), T125/T133/T190 (enemy object scripts and
services), T170 (lock-on), T140 (game over). Build identity: branch
`claude/project-thread-qq8tbs` cut from the local `main` at `3645961`, which holds merges of
the open PR branches listed in the pull request; package 0.1.2, Rust 1.99.0 Windows MSVC.
Source identity: the selected Steam build 15194255 (content identity `a9ca1e03...`), read
only, validated at every run.

## What the install says (decoded, no bytes kept in Git)

- There is one Sephiroth fight. Boss object: objentry 2230, model `N_HB630` (type 3,
  character 5044), script `obj/N_HB630.mdlx` BAR entry 2 (acts `atk_spin_slash`, `atk_upper`,
  `atk_air_slash`, `atk_split_flash`, `atk_hless_angel`, `atk_shadow_flare_s/_l`,
  `atk_meteor`, `atk_faiga_wall`, `power_up`, `rc_success`, `mode_*_boss`, `dead`). His attack
  handler starts `rc_success` when the player's attack with parameter id 1353 reaches him.
- Area `ard/us/hb01.ard`. Spawn set `b_60` places Sora, Donald and Goofy near
  (-324, 0, 783) and the boss at (-324, 0, 1533) (engine coordinates). Its `btl` record
  names program 0x4b (75) with the mission string `HB01_MS601`. Running the area scripts with
  programs (map 261, btl 75, evt 261) gives the entry for event `601` (mode 66, then a jump to
  program 262); with (262, 75, 262) it gives event `602` (jump to 263). Both give background
  group mask 0x3 and battle level 80.
- Event 601 is the meeting (the actor `n_hb590`, objentry 1290 `N_HB590_RTN`, is Sephiroth's
  event model; voices `hb602xxsf`, music 188 starting at frame 1172 and continuing into the
  fight); event 602 is the aftermath. Subtitles are msg/us/hb.bar 12861-12872.
  Other hb01 events (109, 417, 420, 605s, 607) belong to the story around it and were not
  played.
- Text encoding used by the research tools: capitals from 0x2e, lower case from 0x9a, space
  0x01.

## What was built

1. `source/data/examples/`: `boss_placements`, `ard_events`, `ard_sets`, `message_dump`,
   `message_find`, `object_dump`, `enmp_dump`: private research tools that print names,
   ids and decoded events of the install (nothing is written).
2. Event player (`source/main/src/cutscene.rs`): the map is now built by the field's own room
   spawner (texture regions, fog, UV scroll, TEXA, clear colour) instead of a private copy;
   op 0x16 `BgGrupe` is decoded (`event_script::Command::BgGroup`,
   `Timeline::bg_mask`, unit tests) and applied to the group mask given with `--bg-mask`.
3. Fight host: `N_HB630` joins the supported battle enemies with the script brain
   (`enemy_battle.rs`); `--sephiroth` in the room (`test_room/sephiroth.rs`) turns the room's
   own Shadow off, summons him at the area's `b_60` placement and enables the brain. The
   gallery battle request is allowed in an area for that mode.
   `--sephiroth-result <file>` writes `won` and closes the room 240 ticks after he dies.
4. `kh2fm sephiroth` (`source/main/src/sephiroth.rs`): event `hb:1:601`, the fight, and, on a
   win, event `hb:1:602`, each as its own run of the executable. The launcher lists
   "Sephiroth (Hollow Bastion)" and builds its config (`sephiroth_room_config`).

## Checks

Recorded in the pull request (fmt, clippy, tests and build of the whole workspace; the
Sephiroth chain's load-only check). Unattended runs, in the private `kh2-seph-out/`:
`ev601b`/`ev601c` (meeting frames), `ev602`, `tour1` (the combat tour with the boss on the
flat arena), `f1`/`f2` (the fight in hb01). Observed: the meeting and aftermath play with
voices, subtitles, music and no bars; the boss walks, hops, cuts with Spin Slash, uses Faiga
Wall and his effects play; Sora and the party can hit him.

## Not done, not compared (read this before trusting anything above)

- No frame, sound or number has been compared with a capture of the installed game.
- Sephiroth's HP, level, damage cap and prize come from a placeholder ENMP row (id 34, the
  Shadow's); `KH2RW_SEPH_ENMP=<id>` picks another row for development. The exe's
  object-to-ENMP lookup is not traced.
- Reaction commands: nothing offered or executed. The script only shows that attack id 1353
  starts `rc_success`.
- The motion event "revenge" (revenge gauge +0xd48/+0xd4c) is not modelled, so his
  revenge/battle mode switching is not original.
- No boss HP gauge, no boss camera, no end-of-fight effect; his SFX/voice banks play through
  the generic enemy path and were not audited.
- Event 601 depth of field (op 0x18 `SeqFocus`, state at `0x140abdab8`, focus value 20000) is
  not drawn. Event 602 and 601 SeqBlur, face animation, effects were not audited.
- Actor textures use nearest sampling here; whether the original filters Sephiroth's wing is
  unknown (it looks blocky).
- Event-driven start: the area script's event entries are not run in the room; the chain
  hard-wires 601 -> fight -> 602 and a game over restarts nothing.
- Donald and Goofy's behaviour against him is the generic party AI.

## Verdict and next task

PARTIAL. The meeting, the fight and the aftermath run in order from the install; the fight is
not yet the original fight (stats, reaction commands, revenge, gauge, death).

Next: (1) trace and apply the ENMP row, level and difficulty scaling; (2) reaction commands
(who offers them, command ids and labels, what Sora does, attack 1353); (3) the revenge gauge
and boss damage/stagger rules; (4) the boss gauge, boss camera and the death sequence;
(5) audit his effects and sounds against the object's PAX and sound bank; (6) the meeting's
depth of field; (7) a comparison of the three parts against the installed game.
