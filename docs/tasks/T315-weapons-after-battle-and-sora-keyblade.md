# T315 — Party weapons put away after battle; Sora's Keyblade on the executable's rule

Owner Claude, 2026-10-31, requested by burba through the project thread's decision card after T314 ("Hide them after battle"), relayed by the coordinator with the Keyblade work. Gate: development of the test room only; no gameplay gate moves. Prerequisites: T057, T201, T314. Build: local `main` from `2773038`, package 0.1.2, Rust 1.99.0 Windows MSVC. Source: the selected Steam KH2FM build, validated at each run; the install was only read.

Private evidence: `shortcut-weapon-20261031` in the research folder (decompiles `b7` added), tour runs in an output folder outside the checkout (`party2`, `valor2`, `final2`, `hit3`, `limit2`, `lform1`).

## Party weapons: a user-requested departure

The executable keeps a member's weapon in hand after a battle (T314). burba chose otherwise. Once the battle flag and the stance are down and motion 6 is over, on a field variant, the weapon fades out over 16 ticks, the fade-in's length, and the member goes back to the empty-handed walk and run (slots 6, 10). `party_motion::WeaponDraw::tick`'s `put_away` and its caller in `party_room` are marked `[User request, not the executable]`. It applies to every party member. A member still walking when the stance ends has no motion 6 to wait for (the executable plays it only for idle), so its weapon fades as it walks.

## Sora's Keyblade: the executable's rule

T057's 6-second timer was a guess. The Keyblade is the same class as the members' weapons (`FUN_1403ef750`, vtable `0x1405c9408`) and the rest of the Keyblade's rule is shared:

- The stance: on while the battle flag is set (`FUN_1403d4cc0`), held in a Drive form (`FUN_1403ba460`), off at a loop's end once the flag is down (`FUN_1403ba350`).
- Motion 6 when idle crosses stances (`FUN_1403c88c0`): the draw, slot 24, or the put-away, slot 27. Landing or a reaction (variant 2) into battle idle plays the draw too. A reaction no longer sets the stance: those motions are variant 2.
- The Keyblade (`FUN_1403efa60`): hidden from the constructor, out with element 0 and the fade on variant 0 or 3, hidden on a slot its set lacks. A form change makes a new player object, so it comes out again.
- Field walk and run are 6, 10 hidden and 7, 11 in hand (`FUN_1403d7640`, `Player::pose_key`).

**Sora keeps his Keyblade after a battle, lowered.** Every path that hides a weapon was read (`FUN_1403efa60`, `FUN_1403eff20`, the script traps `FUN_14042b640`, `FUN_14053f180`) and none is for Sora or for the flag falling. burba's choice was for the party, conditional on the executable for Sora, so he stays on the executable: in hand until a slot its set lacks (the KO motion) or a new room.

Also found and fixed: the kit read Keyblade motions only for the battle slots, so hurt, jump and field motions drew a stale pose; it now reads every slot. The Drive state's battle flag counted a disabled enemy (`rt.enabled`).

## Checks

| Check | Verdict |
|---|---|
| `cargo fmt --all --check` | PASS on these files; another session's uncommitted `effect_mesh.rs` differs |
| `cargo clippy --all-targets --locked -- -D warnings` | PASS |
| `cargo test --locked` | PASS, with new tests for the fade-out, the stance and its motions, and the lowered clips. These ran over the other session's edits as well |
| The commit's own tree, exported apart from those edits | PASS: `cargo fmt --all --check`, `cargo clippy -p main -p player -p visual --all-targets -- -D warnings`, `cargo check -p main --features skate3`, `cargo test -p player` |
| Release build with `skate3` | PASS |
| `--party-anim-tour` (`party2`) | PASS: the flag at 764; Donald's and Goofy's weapons out at 764, fading out to hidden at 2251 and 2282 on slots 7 and 11, then empty hands; Sora empty-handed to 765, the draw, walking lowered from 2123, no put-away to hidden |
| `--magic-form 1` and `5` with `--command-deck-tour` (`valor2`, `final2`) | PASS: the Keyblades show in the form and after its arrival |
| `--party-hit-tour`, `--party-limit-tour`, `--limit-form-tour` (`hit3`, `limit2`, `lform1`) | PASS: exit 0, no blank captures, no panics |

## Uncertainty

- Nothing here is compared with the running game, and the party's fade-out is not the game's by choice.
- Sora's element 0 starts at his hand, unbound: effects have no weapon object. Position by eye only.
- Slot 1's Keyblade shows and hides with slot 0 and does not fade.
- A limit's or summon's scene, and a Sora another instance moves, follow the stance they set, with no fade.
- Kingdom Hearts: Connected: `Params::disarm_ticks` is gone, poses take `Player::pose_key`, `Clips` has new fields: needed when its pinned engine moves. Its own Sora draws through its hooks and was not run.
- Open from T314: the area flag for rooms that start armed, owner parts of `+0x48` bit 7, the owner-hidden case, the item count icon.

## Verdict

PARTIAL. The tours pass and both rules are in; neither is compared with the installed game. No gameplay gate moves.

## Next

A capture of the installed game after a battle would show what Sora's Keyblade does, and what the party's departure replaces.
