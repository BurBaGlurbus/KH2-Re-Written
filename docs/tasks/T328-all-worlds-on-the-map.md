# T328 — Every world open on the map; the doors between worlds; each world's party

Owner Claude, 2026-11-01, requested by burba in the project thread ("add all worlds to world map and working going into those worlds,
use ghidra to make it 1:1"). Gate: development of the world map and the field; no final gate is inferred. Prerequisites: T171-T179,
T212, T214, T323, T324, T325. Build identity: local `main` after `180a3af` plus this change, package 0.1.2, release launcher with
the `skate3` feature. Source: the selected Steam KH2FM build, validated each run; the install was only read. Ghidra 12.1.4 headless on
a private read-only project copy (plain Ghidra, no MCP).

Private evidence (outside the checkout): decompiles under `%LOCALAPPDATA%\KH2FinalRewrite\research\worlds-20261101\g`, runs and scan
tools under `C:\Users\ANIMA\kh2-wmall-out` (`links.txt`, `wm0`, `l1`, `g2`, `s2`, `tools`).

## What the executable does

- The map has 12 symbols, not 19 (worldmap::Symbol's table at 0x1405c07c0): Twilight Town, Hollow Bastion, The Land of Dragons,
  Beast's Castle, Olympus Coliseum, Disney Castle, Atlantica, Port Royal, Halloween Town, Agrabah, Pride Lands and The World That
  Never Was. `land` and `00worldpoint.bin` list world points for those 12 only.
- The other worlds are reached from inside these, by a signal the area's ARD turns into a jump (FUN_1403aabe0 → FUN_1403f60d0 →
  FUN_1403a46e0, FUN_1403a4bd0):
  - Timeless River: F_DC000 in dc04 / dc05 (command 0x6b, execute 0x1e). Its talk script is one libretto op 3 raising signal 11, and
    the entry jumps to wi00 entrance 7. The same object in wi00 goes back.
  - 100 Acre Wood: F_HB000 in hb13 (0x6c), signal 11, po00 entrance 6. F_PO000 in po00 (0x6d) goes back to hb13.
  - Space Paranoids: F_EX000 in hb05 with the placement's own command 0xc2 (entity +0x2c, FUN_140407de0), execute 0x7f, argument 0x58.
    F_TR010 in tr00 / tr05 / tr08 has 0xf6, argument 0x57. Those signals open the computers' menus (FUN_1402e4570, table 0x1405ba140:
    `hb_co.2ld`, `tr_co.2ld`), whose rows raise 60, 61 and 62.
  - Destiny Islands and The End of the Sea: only an event's end jumps there, in the scan of every ARD (`links.txt`).
- `wldf`'s priorities follow the story; the last script that changes anything (event 0x4c, priority 149) opens all 12 symbols and the
  routes. [U] What marks 0x4b and 0x4c was not traced: no `link` row or `wldf` trigger names them, so it is a script native
  (FUN_140436460) or nothing.
- A slot of a flag table with offset 0 has no script, and the exe does not test for it: it would read the table as a script. Its story
  never sets those flags, but setting a whole world's flags does. 35 of Hollow Bastion's are like that.
- World point 0x18 (eh11) is in the tables, but no program of that area places a WORLD_POINT or an entrance 0x63.
- `memt` (FUN_1403e2e90, FUN_1403e2eb0): rows that hold (world, flag set, flag clear, the room's arif byte +0x34) overwrite 18 member
  objects; the world's member bytes index them, and ARD op 0x0f reorders those by a preset (FUN_1403e31f0). FUN_140405190(0) is Sora,
  FUN_1403c32e0 the friends. This is what makes him P_EX100_NM or Donald P_LK020.

## Changes

- `Progress::finish_story`: every world's flags in order, every world event in priority order with its script, every world point
  touched, nothing queued. `kh2fm wm --all-worlds` starts from it, and the launcher's World map test passes it. A landing's save
  (T324) carries it on. A map opened from a save (T324) still shows what that save holds.
- `Progress::story_ranges`; a flag slot of offset 0 runs nothing.
- `area::Gate`, `AreaSource::gates`, `signal_jump`: type-8 objects whose reaction raises a signal the area answers with a jump. They are
  offered through the reaction search (`reaction_command::GATE`) and fade out over n = 15 like any mode 1 jump.
- `data::member_table` (`MemberTable`, `apply_preset`), `AreaSource::members`: a field room's party comes from the place, and op 0x0f
  writes the world's table (`game.save.party_table`, T325).
- `KH2RW_TOUR_GATE=<tick>` stands Sora at the area's first gate and presses Triangle.
- `game_save::jump_to_field`: a door to another world fades out, writes the save and starts the field on it. `party_room::load` plays
  the object's own motion set (entry +0x28), which the worlds' bodies share with the member.

## Checks

| Check | Result |
| --- | --- |
| `cargo clippy --all-targets --locked -- -D warnings`, `cargo test --locked` (1,180 passed at the first commit), `cargo build --locked`, smoke `--frames 180` | PASS |
| `cargo fmt --all --check` | PASS on these files. Fails on two older lines outside this change, `save/src/state.rs` and `visual/src/pause_menu.rs`, which another session had open and uncommitted |
| Synthetic tests: `finish_story`, `story_ranges`, `member_table`, `signal_jump` | PASS |
| `wm --all-worlds` (`wm0`): 12 symbols drawn and reacting, routes 1-9 open, no event queued | PASS |
| Landing through the map's own menu on each of the 12 symbols, then the field on the save it hands over (`l1`) | PASS: 12 of 12 fields load with Sora placed |
| 73 places in the field: all 54 world points, each world's default landing, wi00 / po00 / tr00 at their door entrances (`s2`) | PASS except eh11, which has no such entrance (see above) |
| Doors (`g2`, in place): dc04 and dc05 to wi00, hb13 to po00, hb05 to tr00, tr08 to hb05 | PASS with the press path, new area entered |
| Doors (`d1`, second commit): the same jumps write the save and config, and the field opens on them with that world's party | PASS, with the second room started by the run script, not by the door |
| Party per world (`s3`): nm08 P_EX020_XM / P_EX030_XM, nm01 Jack in Santa's body, tr01 Tron and P_EX030_TR, ca10 Sparrow, lk04 P_LK020 / P_LK030 | PASS: loaded and following, faces in the HUD |
| Doors po00, wi00, tr00, tr05 | NOT RUN to the jump: the tour's teleport leaves Sora unable to take the command there. The offers show (po00, wi00) or the entry is absent from that program (tr00, tr05: see below) |
| Side by side with the installed game | NOT RUN: no save on this PC reaches the map |
| A hand-played flight, landing and return | NOT RUN |

## Uncertainty

- [Not built] The computers' menus are not drawn: the reaction jumps at once with signal 60, the first row. Signals 61 and 62 (Space
  Paranoids' other terminals, and back to Merlin's House) cannot be chosen.
- [I] The finished story's flags are set world by world in number order. No save from the installed game has been compared, so an
  area's map, battle and event programs at that point are not confirmed; many battle programs end at 0, so those areas have no
  enemies. tr00 and tr05's last event programs have no signal 60, so F_TR010 offers nothing there.
- [I] With no op 0x0f in an area's programs and no save, the party takes preset 1 (Donald and Goofy). The exe keeps the last one per
  world in the save.
- A room's party is chosen when the process starts, so a door to another world (the second commit) ends the room and starts a new one,
  as a landing does: the window closes and opens. Exits within a world keep the room. tt40's and eh01's exits to each other still load in
  place and keep the first world's party.
- N_LM010_PLY and N_LM020_PLY (Atlantica's members) refuse to load: SKLT has no entry for them, so Sora is alone there.
- [I] A world's weapon object with no motion set plays the member's (W_EX020_TR plays W_EX020).
- A signal entry's event (FUN_1403ac350) and flag (FUN_1403f61a0) are not carried out; the jump is.
- F_EX000's other placements only show a message (a book, a sign): they are not offered yet, since their window needs the talk camera's
  owner.
- Atlantica's swimming and the Pride Lands' four legs are other players; they are not part of this.
- Gummi routes still end at the route menu.

## Verdict

PARTIAL.

## Next

Sora's world objects through `drive_room::set_outfit`, the computers' menus, messages on F_EX000.
