# T314 — The L1 shortcut menu; Donald's and Goofy's weapons out only for combat

Owner Claude, 2026-10-31, requested by burba in the project thread: "make the shortcut menu for magic appear 1:1 and make donald and goofy not always having their weapons out and only taking them out when combat happenes just like the original". Gate: development of the test room only; no gameplay gate moves. Prerequisites: T062, T092, T201, T202, T212. Build: local `main` from `28eb6a1`, package 0.1.2, Rust 1.99.0 Windows MSVC. Source: the selected Steam KH2FM build, validated at each run; the install was only read.

"Shortcut menu" is taken as the menu L1 shows (menu 5), which is the menu that lists the Customize slots. It did not draw at all: L1 + a face button cast, with nothing on screen. The Magic list under the command menu (menu 1) was read against the same routines and needed no change.

Private evidence: decompiles and tables in a `shortcut-weapon-20261031` research folder, tour runs in an output folder outside the checkout (`deck1`, `valor1`, `limitf1`, `party1`, `hit1`, `hit2`, `limit1`), probes in a scratch project. Nothing from the game is in Git.

## The shortcut menu

What the executable does:

- `FUN_1403f29e0` fills menu 5 every frame: one row per Customize slot, record +6 = slot + 1. An item names its own command (colour 6 when no slot holds it), in Limit Form any other slot is that form's limit (`0x1405c9678`), and `FUN_1403b68b0` turns a spell's level-1 command into the tier its level has reached. `FUN_140401d60` gives the colour; `FUN_1403b6cb0` greys what a Drive form masks.
- `FUN_1403f2470`: with L1 held (and no override deck, `FUN_1403b27e0`) `FUN_1403b1f30` leaves only menu 5 on the stack, so an open list closes silently. On release the root deck comes back with its cursor at 0 (`FUN_1403b1b70`).
- `FUN_14015df90` draws it: the header at (0x1c, 0x184 - 22n), rows at (0x1c, 0x19a - 22(n - i)). Groups from `0x140718330`: header 0x41, selectors 1-4 0x43, 0x45, 0x44, 0x42. Colours from `0x140717d50`. Each label is `FUN_14015aab0`'s: the world pen, the glyph (0xdb, 0xdf, 0xde, 0xda), then the message.
- `FUN_14015b6d0` on the first frame back: with the last kind 5 the panel plays `T_EF0` (its loop) forced; the cursor plays its refresh (`COMMAND_DRAW+0x197`, for any list deck).
- `FUN_1403b11e0`, `FUN_1403b21d0`, `FUN_1403f22e0`: the first selector pressed decides. An empty slot leaves the press to jump (bit 8) and guard (bit 9). Colour 5 or 6 plays sound 5. A shortcut's Cure heals Sora with no party list.

Built: `test_room/shortcut_menu.rs` (the rows, the stack), `field_command_menu` (`SHORTCUT_DECK`, the header and rows, the return), the `read_pad` fall-through and the colour gate on the cast. Command 0's name is loaded for an empty slot. `--command-deck-tour` opens the Magic list, holds L1 over it, casts the first slot, and holds L1 again over the second deck.

## The weapons

What the executable does:

- A weapon object's constructor hides it (`FUN_1403ef750`, class `+0x80` = `FUN_1403ef610`: `+0x124 |= 0x700`).
- `FUN_1403efa60` each tick: with no motion (`FUN_1403c7f90`, `ctl+0x18` empty) the weapon hides at once. With a motion, hidden, and the owner on variant 0 or 3 (`+0x188`), it starts element 0 of its own effects (`FUN_1403b6670`), sets its alpha from 0 to 1 over 16 ticks (`FUN_1403cd550`, `DAT_140623bc4`) and shows (`FUN_1403ef620`).
- `FUN_1403c8cd0` passes each slot the owner starts to the weapon; a slot its own set lacks leaves it no motion.
- `FUN_1403c7c50` picks the slot by stance and by whether a weapon shows (`FUN_1403d7640`): battle 0, 1, 3, 2; battle, hidden 1, 0, 2, 3; field 3, 2, 0, 1; field, hidden 2, 3, 1, 0.
- In the install (Donald, Goofy, Sora and the guests read): body and weapon both have idle 0 and 2, walk 4 and 7, run 8 and 11, switch 24 and 27. Only the body has walk 6 and run 10, and the dead and revive motions at 218 and 222 (Donald and Goofy).

So in order:

1. A room starts with hidden weapons: field idle 2, walk 6, run 10.
2. The battle flag sets the stance. Idle plays the draw (24) or walk plays 4, variant 0, and the weapon fades in.
3. When the flag falls, idle plays the put-away (27), which ends on the field idle's weapon pose, and then idle 2. **Nothing hides the weapon here.** Walk and run are now 7 and 11, weapon in hand and lowered.
4. It hides again only on a motion its set lacks (the KO motion, for one) or with a new room.

Built: `party_motion::WeaponDraw`, `PartyKind::bare` (the hidden-weapon field clips), `KindClips` choosing by weapon state, `weapon_slot` on each clip in place of the bind pose, `party_room::present` hiding and fading the weapon parts, `room_loaded` at an area change. `MemberRt.stored` is untouched. The weapon stops showing the idle pose during motions it has no slot for. Every member with a model uses this; before, the weapon showed in all of these.

## Checks

| Check | Verdict |
|---|---|
| `cargo fmt --all --check` | PASS |
| `cargo clippy --all-targets --locked -- -D warnings` | PASS |
| `cargo test --locked` | PASS, with new tests for the weapon's states and the shortcut's rows and input |
| `cargo run -p main --locked -- --frames 180` | PASS |
| Release build with `skate3` | PASS |
| `--command-deck-tour` (`deck1`) | PASS: the Magic list, L1 closes it; "SHORTCUTS" header; Firaga, Blizzaga, Thundaga, Curaga with their button glyphs; Curaga in the low-MP colour; the first slot casts; back to the deck with the cursor at 0 |
| The same with `--magic-form 1` (`valor1`) | PASS: all four rows greyed |
| The same with `--magic-form 3` (`limitf1`) | PASS: Sonic Rave, Last Arcanum, Strike Raid, Infinity |
| `--party-anim-tour` (`party1`) | PASS: empty hands and slots 2, 6, 10 up to tick 764; weapons fade in at tick 764 as the flag rises (its delay after `enemy_on`, tick 720); shown through the fight; lowered in hand and walking on 7 after it |
| `--party-hit-tour` (`hit2`) | PASS: exit 0, ends at tick 6713. `hit1` ended the same but exited 1: some of its captures were 1x1, the window minimized during the run |
| `--party-limit-tour` (`limit1`) | PASS: exit 0; both weapons show from the first battle motion |

## Uncertainty

- Nothing here is compared with the running game. The rules are read from the executable, the art comes from the install's own layouts.
- **After a fight the weapons stay in hand, lowered.** That is the code as read, and the data agrees (motion 27 ends on slot 2's weapon pose, motion 24 starts from it, and walk and run have weapon variants). The request reads "only when combat happens"; a capture of the installed game after a battle should confirm this before anyone changes it.
- Rooms with the area flag `(DAT_142a0ff68+8)->+4` bit 4 start armed (`FUN_1403a3c70`, `FUN_1403d6470`); that flag is not read, so they start hidden here.
- Element 0 binds to the member, not the weapon: effects have no weapon object yet. Donald's and Goofy's containers have no element 0. Auron's and Riku's do.
- Weapon entries with `+0x48` bit 7 (Mulan, Sparrow, Tron) also show and hide three parts of the owner (`FUN_1403d7690`, `FUN_1403d7d70`): not ported.
- The owner's hidden case in `FUN_1403efa60` (`FUN_1403ba890`) is not ported.
- A row's item count icon (`FUN_14015b4a0`, record +4 of 2 or more) is not drawn, here or in the Items list.
- `FUN_140406390` and `FUN_14039a830`, which can stop the stack changing, are not ported; the shortcut is refused during a talk and over a limit's or summon's deck.
- Sora's Keyblade keeps its 6-second timer (T057). It is the same class in the executable, so the same rule applies to it: not changed here.
- Kingdom Hearts: Connected: `KindClips` takes a second field and `MemberRt` a new one, in case its pinned engine moves.

## Verdict

PARTIAL. The menu and the weapons follow the executable and pass the tours above; neither is compared with the installed game. No gameplay gate moves.

## Next

Capture the installed game's L1 menu and Donald and Goofy after a battle, and compare. Then Sora's Keyblade on the same rule.
