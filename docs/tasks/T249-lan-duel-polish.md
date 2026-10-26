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
