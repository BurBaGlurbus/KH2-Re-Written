# T249 — LAN duel: polish (speed, arena looks, lobby comfort, animation)

Owner Claude, 2026-10-26, requested by burba ("keep working on more polish for the multi-player maybe make it
more optimized and more qol better menus animations that stuff"), with the coordinator's list of items. Gate:
none of the roadmap's (an addition outside the original game; no fidelity claim). Prerequisites: T248. Build
identity: worktree `KH2FinalRewrite-lan`, branch `feature/lan-duel`; package 0.1.2, Rust 1.99.0 Windows MSVC;
measurements on the release profile (`tools/Build-Launcher.ps1 -Skate3`). Source: the selected Steam
collection, validated at each run; the install was only read.

Private evidence: `%USERPROFILE%\kh2-lan-out\` (`perf0-*` … run logs, `perfsum.py`); before and after shots in
the worktree's ignored `.local\lan-review\polish\`.

## How it is measured

Two release instances on one PC over loopback, both `--uncapped`: the host's bench Sora fights the joiner's idle
one in the Mirage Arena (`duo.ps1 … --duel-bench --uncapped / --duel-idle --uncapped`). The log now carries,
every 600 frames, `frame_stats` with the mean FPS, the 1% low (the slowest 1% of frames, as FPS) and the 99th
percentile frame time, and `net_stats` with the round trip, loss and each way's KB/s. "Lobby" is the first
1200 ticks (the lobby, the vote and the countdown), "fight" the rest. Two instances share one GPU and CPU, so
the numbers are each instance's share, not a one-player figure.

## 1. The arena in fewer draws

The Mirage Arena was 432 meshes, one per PMO section, each with its own material. Sections now merge by
texture and blend: 18 draws (the sky's 8 layers stay apart, in their order).

| Run | Side | Fight FPS | Fight 1% low | Fight p99 | Lobby worst frame | Net send / recv |
| --- | --- | --- | --- | --- | --- | --- |
| `perf0` (before) | host | 170.7 | 69.5 | 14.1 ms | 353 ms | 11.3 / 6.7 KB/s |
| `perf0` (before) | joiner | 181.8 | 77.7 | 11.3 ms | 1125 ms | 6.7 / 11.3 KB/s |
| `perf1` (merged) | host | 203.6 | 90.9 | 10.0 ms | 278 ms | — |
| `perf1` (merged) | joiner | 202.8 | 94.8 | 9.9 ms | 1407 ms | — |

The lobby's stalls (about 2.7 s in all, 50-170 ms each, both instances) did not change: they come from
elsewhere (below).

## 2. The arena's HD textures and moving sky

- The PC remaster's 22 images for vs03 (`JP_vs03_arcN`, BC3 with mip chains) stand in for the 19 PSP
  pictures. Their numbering is not the texture table's (three images of the archive's other files come first),
  so each picture takes the image whose mip level of its own size looks most like it (mean RGBA difference),
  each image once, best pairs first (`bbs_map::match_hd`, tested on synthetic pictures). In this install all
  19 match: picture N takes `arcN+3` (clouds 21-25, sky 0.4, the stage's own 0.5-12.5; the ring's cut-out
  82.7, the loosest). Matched images upload with their whole mip chain, sampled trilinear and repeating.
- The sky's six layers scroll as the texture table says (U per frame: the clouds -0.00075 to 0.003, the sky
  0.0003, the star mask both ways), by the tick, so the same at any frame rate.
- Shots: `.local\lan-review\polish\arena-before.png`, `arena-after-hd.png`, `arena-after-hd-fight.png`.

Checks for items 1 and 2: fmt, clippy, `cargo test` (1,098 passed), single-player tours against a build of
4d33db8 (`p2-*`: talk, magic, deck identical; party-hit identical but for the exit tick), loopback duels
(`perf1`, `hd4`): PASS.

## The lobby's stalls

A `perf-trace` build (`trace1`: a host and a joiner over loopback) shows the lobby's long frames (50-180 ms)
spread over every system at once, on every worker thread, each with no slow child: systems that take
microseconds (`check_entities_needing_specialization`, `upload_light_probes`) take 30-50 ms together. That is
the process starved, not one system's work. The same build alone (`trace2`: a host with no joiner, the same
lobby and arena) has three long frames in 500 ticks, all at startup or at the capture. So the stalls are the
two instances on one PC competing while the joiner loads and checks Birth by Sleep (its first 10-20 s); two
players on two PCs do not share them. Removing the first run's full BBS scan (below) shortens that window.
The two traces are private (`kh2-lan-out\trace1`, `trace2`, with `stalls.py`). Verdict: explained, nothing in
the lobby's own code to fix.

## 3. Lobby and match comfort

- **Leave** on the lobby's top level (the launcher's Multiplayer screen comes back; the other player sees
  "left"), and **Start** toggles Ready while the builds are made (the camp hands a kept-open Start back to its
  caller, `DuelPages::start`, and leaves its sound to it).
- **Ping**: the other player's row reads "Ready, 15 ms" (the session's round trip, to 5 ms, refreshed every
  two seconds).
- **Remembered rules and builds** (`duel_prefs.rs`): the host's rules when changed and this player's build when
  Ready is set go to `duel-prefs.json` beside the source manifest (the player's settings; no game data). Read
  back, each value is checked against the tables (mode by name, damage in range, the caps and limits in their
  lists, the arena by key); a kept build is fitted to the duel (`duel_prefs::fit`: a Keyblade it has, its
  abilities as far as the duel has them, items it allows) before the first builds. Unattended runs neither
  read nor write it. Tests: round trip, bad values keep defaults, a build fitted.
- **Forfeit**: Start during a match opens *Keep fighting* / *Forfeit* (the fight goes on: nothing pauses a
  duel); Forfeit asks once more on its help line, then the match goes to the other player
  (`Match::forfeit`, `Why::Forfeit` code 4; the joiner sends `FORFEIT`). Test: a forfeit ends the match once
  and round-trips.
- **Quick rematch** on the result: both quick starts the next match at once with the same builds and bans (the
  lobby's match number moves on, so the music changes and the builds go back on); either plain Rematch goes to
  the lobby as before. Wire: the rematch flags carry each seat's quick bit.

Loopback (`qol2`, `qol3`; flags `--duel-forfeit`, `--duel-quick`): the joiner readied with Start
(`duel_start_ready`), forfeited through the menu (host `duel_forfeit` seat 1, "JoinSora forfeited"), and both
quick rematches went straight to match 1 (`duel_rematch` quick, stage Start with the same bans). Shots:
`.local\lan-review\polish\forfeit-confirm.png`, `result-quick.png`.

## 4. No full scan of Birth by Sleep

The first session used to hash the whole 2.6 GB `bbs_third.pkg` (whole file and every 1 MiB chunk) before the
arena could load. Now the BBS manifest holds the index's digests and the package's size, time and the digests
of the chunks the duel reads; a chunk with no record is recorded on its first read (`SourceReader::recording`)
and kept (`BbsSource::keep`), and every later read of it is checked. Trust is as before (the full scan also
recorded what was on disk on first use); a changed package size or time starts a new record; a recorded chunk
that differs is refused. Test: `a_recording_reader_records_chunks_once_and_the_next_one_checks_them`
(synthetic bytes).

| Run (lone host, no BBS manifest yet) | Arena ready at | BBS manifest |
| --- | --- | --- |
| `bbs-old` (before, full scan) | tick 1602 (about 27 s) | 2,534 chunks |
| `bbs-fresh` (after) | tick 0 | 4 chunks, 8 KB |
| `bbs-again` (after, second session) | tick 0 | the 4 checked |

## Speed after items 1-4

Back to back (`perfA`: the item 1 build, PSP textures; `perfB`: this build, with item 2's HD textures), two
instances, uncapped: host fight 188.8 → 169.5 FPS (1% low 80.6 → 71.1), joiner 190.1 → 183.5 (84.1 → 75.8);
lobby stall totals 2.36 / 1.92 s → 2.26 / 1.92 s. The fight's cost is the HD textures' (larger images on a GPU
the two instances share); still above the PSP-texture baseline `perf0` (170.7 / 181.8).

Checks for items 3 and 4: fmt, clippy, `cargo test` (all passed), single-player tours with the release build
against 4d33db8 (`q3-*`: talk, deck exit tick ±1, magic one float at 1e-5 as before, party-hit identical but
the exit; `auto` the same up to tick 7700, where the debug-built baseline itself panics on an integer
overflow in `enemy/src/script.rs:215`, an existing bug outside this work), loopback duels: PASS.

## 5. The other Sora's trail and Wisdom shots

- **Trails**: the swing trail (weapon_trail.rs) followed `game.sora` only, so neither instance drew the other
  Sora's. Each rival now keeps his own trail state (`Own::trail_live`, `trail_length`, swapped with his kit)
  and runs the same window rule with his state swapped in, under owners of his own
  (`weapon_trail::RIVAL_OWNERS`, 0xf0-0xf3), anchored `RivalWeapon(slot, weapon)`; drive_fx takes his trail's
  points with him swapped in. A rival whose Keyblade kit never changed has no weapon PAX of his own: he uses the
  room Keyblade's as loaded (`Effects::room_weapon`). Trails are each instance's own (nothing on the wire).
- **Wisdom shots**: bullets lived only in the host's simulation and their effect looked only in the room Sora's
  Drive state, so the host lost the joiner's and the joiner saw none. A rival's bullet keys now carry his slot
  (`offset_bullet_keys`); the effect finds a bullet on either Sora (`bullet_at`) and the rivals' bullet ends are
  drained; each snapshot fighter carries his bullets (key, position, direction; at most 16), and the joiner puts
  them on that Sora (`show_bullets`: a new one starts its element, one gone fades).
- Test flag `--duel-wisdom` (joiner): Wisdom Form every ten seconds and the bench's swings.

Loopback `fx6` (host `--duel-bench`, joiner `--duel-wisdom`): the host starts the joiner's trail (owner 0xf2,
`weapon_trail_length` rival 1), the joiner the host's (0xf0, rival 0), and both start the Wisdom shots' element
20. Shot: `.local\lan-review\polish\joiner-wisdom-shot-and-host-trail.png`. Snapshot test with a bullet.
Checks: fmt, clippy, `cargo test` (all passed), tours (`f6-*`: talk identical, deck exit ±1, magic the 1e-5
float, party-hit identical but the exit): PASS.
