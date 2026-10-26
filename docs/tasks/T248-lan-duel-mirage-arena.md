# T248 — LAN duel: Birth by Sleep's Mirage Arena as the duel's arena, battle music in the arena

Owner Claude, 2026-10-26, requested by burba: "use ghidra and extract the world mirage arena from birth by
sleep exe and just have that world's arena as the new arena; have random battle music play of your choosing
at the ready up screen", then "battle music should be playing only in the arena". Gate: none of the roadmap's
(an addition outside the original game, in the LAN duel worktree only; no fidelity claim). This reads a second
game of the same Steam collection from the user's own install at the owner's direct request; the worktree
is never merged into main. Prerequisites: T245, T246. Build identity: worktree `KH2FinalRewrite-lan`, branch
`feature/lan-duel`; package 0.1.2, Rust 1.99.0 Windows MSVC, dev profile. Sources: the selected Steam
collection, validated at each run (KH2FM as before; Birth by Sleep FM's `bbs_third` package by its own
manifest, below); the install was only read.

Private evidence: `%USERPROFILE%\kh2-lan-out\bbs-research\` (the research decoder and renders, the Ghidra
project of the BBS FM executable and its `magic\` decompilations), run logs and captures `mir1-*` … `mir6-*`,
`mus1-*`; review shots in the worktree's ignored `.local\lan-review\mirage\`.

## Finding the arena

- The BBS packages (`Image/dt/bbs_first` … `bbs_fourth`) use the same index and package format as KH2's; the
  existing reader decodes them. Names are MD5 digests of paths; the executable's strings give `arc/map/`, and
  trying `arc/map/<two letters><two digits>.arc` found every area: world `vs` (versus) is the Mirage Arena,
  areas vs01 to vs20, all in `bbs_third`.
- An area archive (ARC) holds the map model (PMP: placed PMO models with their TIM2 textures) and the
  collision (BCD). A research decoder drew every vs area from above and in perspective: vs01 is the arena's
  building, **vs03 its battle stage** (a round floor, a wall ring 21.6 m out, rings of shards overhead, the
  arena's own sky and clouds, which vs01 and vs03 share).
- The formats are in `source/data/src/bbs_map.rs`. The PMP's layout matches the executable's own fix-up
  routine (headless Ghidra on the installed executable: FUN_1401017e0 for the PMP, FUN_140107a10 for each
  PMO, FUN_1401ac3c0 for TIM2). The collision's 294 faces all lie on their planes (n·p = d).

## Scale

BBS maps are in metres: of the NPC models in `arc/npc/`, an adult is 1.706 tall and the three identical
nephews 0.843. KH2 Sora's body spheres sit 47–136 above his feet, so he stands about 150–160 in the room's
units, about centimetres. [Duel choice] ×100: the stage's floor is at y 10, its wall ring at a radius of about
2160, and the starts, 600 apart, are well inside. In the duel both Soras stand at y 10 and the farthest either
got was a radius of 2168, against the wall (`mir4`). Handedness is not checked (the stage is radially symmetric
and has no lettering); it is mapped without a mirror, as the room maps its own scene.

## What changed

1. `data::bbs_map`: ARC files, TIM2 pictures (4- and 8-bit palettes, direct colour), PMP models (placed PMO
   meshes in PSP vertex formats, triangle lists and strips, opaque and translucent lists, the sky flag) and BCD
   collision faces; tests on synthetic bytes.
2. `data::bbs_source`: the BBS package validated as the KH2 source is. A full scan (whole-file and chunk
   digests) writes a private manifest beside the KH2 one (`<name>.bbs.json`); each session rehashes the index
   whole and checks the package's size and time; every byte read is checked by its chunk. The first scan
   takes about 20 s (on a thread, as soon as a duel session exists); later sessions are admitted at once.
3. `test_room/mirage_arena.rs`: the arena's meshes (unlit, alpha-tested, the translucent list blended), its sky
   (drawn around the camera, behind the stage) and its collision (for the Soras and the camera), shown in
   place of the test room's arena while a duel's rules choose it; the room's arena and collision come back
   after.
4. The arena is a rule: Mirage Arena (default) or Test room on the lobby's Match page (host-editable, the
   joiner's read-only, sent with the rules), `--duel-arena mirage|room`. The joiner tells the host whether its
   install can show the Mirage Arena; if either cannot, the host keeps the test room's arena and the lobby says
   why (`--duel-no-mirage` tries it).
5. `test_room/duel_music.rs`: while the players are in the arena (lobby, countdown, fight) one of the game's
   battle themes plays: the host picks one at random each match and sends it, so both play the same; it fades
   out at the result and when the session ends, and the room's own music comes back after. The pool is every
   room's battle track in 03system `arif` that differs from the room's field track: 17 themes (51, 53, 55, 65,
   69, 102, 104, 112, 119, 128, 134, 136, 149, 153, 155, 187, 190).
6. Test support: `bbs_probe` (names, find, dump, hd, mirage); `--duel-shots` captures now fire once per moment.

Not done: the PC remaster's HD textures for the arena (the package carries 22 DDS images, `JP_vs03_arcN`,
whose order does not follow the 19 TIM2 pictures one to one), the clouds' UV scroll, and BBS's own battle music.

## Checks

| Check | Result |
| --- | --- |
| `cargo fmt --check`, `clippy -D warnings`, `cargo test` (1,097 passed), `cargo build` | PASS |
| Arena through the validated source (`bbs_probe mirage`): first scan 19.2 s, then admitted at once; 432 meshes, 6340 triangles, 19 textures, 294 collision faces | PASS |
| Loopback duels in the arena (`mir1`–`mir6`): both screens show it in the lobby, the countdown and the fight; the Soras stand on its floor and the wall holds them | PASS |
| Fallback: joiner with `--duel-no-mirage` (`mir5`): the host switches to the test room's arena and the lobby says why | PASS |
| Music (`mus1`): the pool read, the same track on both PCs, a new one after the rematch, off at the result | PASS |
| Single-player tours against a build of 4d33db8 (`pre-*` / `mir-*`): talk, magic, deck identical; party-hit identical but for the exit tick (known) | PASS |
| Two PCs; a player at the keys | NOT RUN |

## Verdict / next

PASS on one PC (loopback, unattended). Two PCs and a player at the keys NOT RUN. Next: the owner's test on two
PCs; the arena's HD textures if wanted.
