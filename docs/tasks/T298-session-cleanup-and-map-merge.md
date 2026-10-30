# T298 — A long session's cleanup: effects, forms, sounds and caches; the map's chunks merged

Owner Claude, 2026-10-30, requested by burba in the project thread ("make a sophisticated garbage collection system for game
because effects get stacked up over time ... my brother drive forms effect kept lasting after rounds and certain rooms in worlds
are also not optimized and laggy"). Gate: P30 performance engineering; no gameplay gate is inferred. Prerequisites: T236, T237
(the frame and allocation tools), T279 and the LAN duel records (rounds, rival state), T294 (the map's alpha order). Build
identity: measured on `f69c1b4` plus these changes, then merged onto local `main` at `4254d93`; package 0.1.2, Rust 1.99.0 Windows
MSVC. Source: the selected Steam KH2FM build, validated at each run; the install was only read.

Private evidence (outside the checkout): the private output folder `kh2-gc-out` (run logs, frame records, captures, one folder per
run: `b-*` before, `a-*` and `a2-*` after).

## What was found

- **Form effects on the other player's Sora never ended.** A replaced object's effects fade through `DriveRt::fx_owner_left`, and
  `drive_fx::tick` read only the room Sora's. A rival's was never read, and a joiner's `show_form` never set one. Each revert starts
  twelve looping elements (15..27), so each form change on the other player's Sora left twelve running for good. Scripted duel over
  loopback, 9 rounds and 3 matches in five minutes: running effects 0 to 105 on the host, 0 to 121 on the joiner, never falling.
- **A round reset left a form half ended.** `refill` cleared the change and swapped the kit, but left the afterimage and the grey
  overlay running if a change was under way, the gauge in form mode, and the form's abilities and spells.
- **A room change wiped Sora's numbers.** `Player::new` in `area_transition` reset his abilities, growth abilities, MP and Drive
  gauge. It is also why forms stopped being available after a door in a soak.
- **Effect caches kept every container.** `dpds` kept an `Arc` on each DPD ever drawn, with its HD pages, pages and materials: the
  room-change soak went from 1 to 32 entries and the walk through every room from 1 to 212, with 182 pages. Nothing freed them.
- **The mixer's limit dropped the oldest voice**, which is the music in a long session. Looping effect sounds were stopped only for
  map elements and save points.
- **Worst rooms: one material per UV-scrolled chunk, rewritten every tick.** eh22, lm09, hb34 and others have about 2,000 scrolled
  chunks. Each tick wrote each material, and the run allocated 30 to 80 MiB a frame against 6 in Twilight Town. Every room also drew
  each chunk as its own mesh and entity (2,650 in Twilight Town), and map objects uploaded poses while hidden.
- **Each door logged 233 warnings** for commands on despawned children.
- **What did not grow:** with no duel and no doors the room's five-minute soak was already flat (effects peak 24, process memory
  settling at 1.93 GiB). Bevy's asset counts return after each room. The Windows log has 77 RADAR_PRE_LEAK reports for test
  builds, which only say memory was high, and one crash record (10/25, a fast-fail in a perf build) with no link to this.
- **One 12.6 s frame** in a first soak (tick 11641, at a Wisdom hit) did not come back in any later run; its cause was not found.

## Change

- `test_room/cleanup.rs` (new): `Scope` (`RoundEnd`, `RoundStart`, `MatchEnd`, `RoomChange`), a table of what each one does, and one
  pass, `cleanup::run`, each frame after the area change. A cleanup ends the forms and spells of the instance that simulates them,
  ends the Soras' effects (fading at a round's or match's end, at once at the next round or a door), stops looping sounds nothing
  holds, drops queued effects, forgets per-actor maps at a door, and lets the caches go of containers nothing else holds. One
  `cleanup` line in the log and one `info` line per cleanup, with counts and milliseconds.
- Ceilings, oldest first: 512 running elements per presenter (the map's last), 256 effect meshes per presenter, 1,024 requests per
  queue (sound, effect, duel events, net events, inputs, warnings), 4,096 entries for attack-position and victim maps, 256 MiB of
  decoded sounds (least recently asked for). The soaks peak near 30 elements, 15 meshes and 40 MiB in ordinary play; going over
  logs `effect_ceiling` or counts in the next `cleanup` line.
- `drive_room::end_form`: the form ends now with no motion or effect of its own: no change, afterimage, overlay or bullets, the base
  kit, Drive mode, the form's abilities gone. A duel round's end, a match's end and a room change call it on both Soras
  [rewrite rule, at the owner's request; not compared with the installed game, where a form may well stay through a door].
- `drive_fx`: every Sora's `fx_owner_left` is read and only that Sora's elements fade; `show_form` sets it, so the joiner fades too;
  elements of a Sora who left go with him. `duel_match_room::round_change` asks for the cleanups from the match's phase, so host and
  joiner both do.
- `area_transition` keeps Sora's abilities, growth abilities, MP and gauge across a door, and despawns with `try_despawn`.
- `net::Session`: a joiner's wait for the host starts with its first Hello. It ran from the session's creation, so a load longer
  than the timeout ended the join before anything was sent ("no answer"), with the host waiting.
- `audio::Mixer`: past the voice limit the oldest one-shot goes, then the oldest loop, the music last; `stop_loops`.
- `spawn_room`: chunks with the same texture, blend, background group and scroll record are one mesh. Opaque ones merge anywhere;
  alpha ones only with their neighbour in T294's batch order, each run's material holding the run's place. Scrolled chunks with one
  record share one material, so a scroll writes one material a tick. `spawn_model`: parts with one texture share a material.
  `bob_present`: map objects in a hidden background group upload no pose.
- Tools: `--soak <dir>` (`--soak-ticks`, `--soak-rooms w:a,...`, `--soak-rooms-every`), `--room-survey <dir>` (`--survey-rooms`,
  `--survey-ms`, `--survey-settle`), a `census` log line every five seconds, `heap_bytes` in `--perf`'s record, and
  `tools/census_report.py`, `tools/room_survey_report.py`.

## Measured

Same PC as T236. Other sessions' compilers ran through all of this, so frame times swing by about 2x between runs and single
numbers mean little; counts and allocations do not depend on that. Before/after here is `f69c1b4` with only the census added, against
the same tree with these changes. Each is one five-minute scripted run unless noted. `b-duo` and `a-duo` are two instances on one PC.

| Duel, 5 min, about 9 rounds | Host before | Host after | Joiner before | Joiner after |
| --- | --- | --- | --- | --- |
| Running effects: peak / at the end | 121 / 121 | 28 / 0 | 133 / 99 | 28 / 0 |
| Entities at the end | 714 | 705 | 604 | 602 |
| Materials (map and models) peak | 354 | 58 | 422 | 54 |
| Process memory at the end (GiB) | 2.05 | 2.05 | 1.75 | 1.75 |
| Frame median / p99 (ms; two windows, compilers) | 35.5 / 98 | 39.5 / 120 | 35.1 / 90 | 39.3 / 122 |

After each round's end the host's and the joiner's census show no running effects from a form; each form change leaves none.

| Room-change soak, 5 min, 15 doors | Before | After |
| --- | --- | --- |
| Effect containers kept at the end | 41 | 30, and 2 to 8 in the 278-room walk below |
| Entities peak | 3,167 | 1,196 |
| Meshes peak | 2,888 | 923 |
| Materials peak | 668 | 278 |
| Process memory at the end (GiB) | 1.95 | 2.04 |
| Forms taken / effects started | 9 / 364 | 13 / 581 |

The after run does more: forms keep working past a door, which also loads more form kits (its heap settles about 200 MiB higher). Its
frame times are not comparable with the before run's for that reason and are left out.

| Walk through all 278 rooms | Before | After |
| --- | --- | --- |
| Effect containers kept, first to last | 1 to 212 | 1 to 2 (peak 8) |
| Effect pages kept, first to last | 0 to 182 | 0 to 12 (peak 18) |
| Heap, first to last sixth of the run (MiB) | 1,407 to 1,485 | 1,386 to 1,397 |
| Process memory, first to last sixth (GiB) | 1.99 to 2.08 | 1.93 to 1.98 |
| Median frame, first to last sixth (ms) | 41.2 to 39.8 | 25.7 to 20.6 |
| Entities, mean of the sixths | 1,444 to 2,080 | 818 to 890 |

All installed rooms load and run in both builds. `es01` and `al14` have no spawn for entrance 0 (the same in both). Rooms over 50 ms
median: 81 before, 21 after; over 33 ms: 202 before, 85 after. The median room: 39.7 ms to 26.4 ms, 1,285 to 303 meshes drawn.

Slowest rooms, three alternating runs each, median of medians (ms, compilers running; about 30 ms is the machine's floor then):

| Room | Before | After | | Room | Before | After |
| --- | --- | --- | --- | --- | --- | --- |
| lm09 | 195.7 | 31.0 | | ca25 | 79.7 | 30.1 |
| hb34 | 156.7 | 32.4 | | al05 | 77.3 | 19.6 |
| eh22 | 145.4 | 31.1 | | es00 | 75.1 | 36.6 |
| eh20 | 119.5 | 32.3 | | he09 | 65.3 | 31.7 |
| wm00 | 114.6 | 32.1 | | di02 | 63.6 | 21.5 |
| eh25 | 107.2 | 28.3 | | tt35 | 54.5 | 23.6 |
| hb14 | 84.1 | 31.1 | | he14 | 52.6 | 36.9 |
| eh23 | 81.9 | 32.6 | | tt05 | 37.9 | 32.6 |
| eh29 | 79.8 | 26.3 | | tt06 | 31.8 | 23.5 |

Rooms still over 50 ms in the one all-room run (ca01, hb26, he01, ca14, nm00, tt37, hb24, hb27, tt18, hb33 and others) are mostly noise
in that run or high vertex counts (hb27: 215,000); they were not rerun and no cause was found. Characters' CPU skinning still uploads
about 190 meshes a frame (T236's deferred list).

## Checks

| Check | Result |
| --- | --- |
| Unit tests: a form's effects end on the Sora who left it only; a round's end ends every Sora's and keeps the map's; the ceilings take the oldest; a container nothing holds leaves the caches; a round's every end and start asks for a cleanup; the mixer spares the music and stops loops; the clip budget; a slow load keeps the join's wait | PASS |
| `cargo fmt --all --check`, `clippy --all-targets --locked -D warnings`, `test --locked` on the merged code | PASS |
| Scripted duel on loopback, host and joiner, final code: 8 rounds and 3 matches in five minutes; effects 0 after each round on both, forms ended, no panic | PASS |
| Scripted duel, the same before these changes: the count climbs to 121 and 133 and does not fall | Reproduced |
| Walk through all 278 rooms, before and after: all load and run, heap flat after | PASS |
| Captures of ten rooms against a build of clean main (`784a043`), still camera: within about 3% in rooms with no scroll, the party's random places included; scroll rooms differ by the scroll's tick; the same objects by eye | PASS |
| eh22's flying map object: an off-screen skip of mine lost it (its bounds come from its pose); fixed, and seen at the same tick | PASS |
| Character missing in one capture at tick 41 on an exe's very first run: its shader was still compiling; two reruns show it | Noted, not a change of this task |
| Two PCs, real keys and pad, hours of real play | NOT RUN |
| Side by side with the installed game | NOT RUN |

## Uncertainty

- Frame times come from a PC with other builds running. The per-room table's large gains are safe; a few milliseconds are not.
- A real LAN duel on two PCs, real keys and pad, and hours of real play were not run. The 12-element count comes from the log of
  the scripted Wisdom script, with Antiform by the roll; other forms were run only in the single-player soak.
- Ending a form at a door is the owner's rule for this rewrite. Whether the installed game keeps a form through a door was not checked.
- Enemy archives read on demand (T278) are not evicted: standing enemies are off unless `KH2RW_AREA_ENEMIES` is set, so they did not
  grow in these runs. With that on, a long session keeps each type's bank and effects.
- NPC kinds stay loaded across doors (`AreaSource::kinds`): 0 in these runs, so not exercised.
- Any crash report the owner saw has not been reproduced: no run here crashed, before or after.

**Verdict:** PARTIAL. PASS for scripted duels and scripted room walks: nothing grows without bound and form effects end with the round on both instances. Two PCs and real long play NOT RUN.

**Next task:** a two-PC duel of several matches with `--log`, reading its `cleanup` and `census` lines; enemy archive eviction if
`KH2RW_AREA_ENEMIES` becomes the default.
