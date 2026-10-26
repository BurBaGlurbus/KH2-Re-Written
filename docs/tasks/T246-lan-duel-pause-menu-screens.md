# T246 — LAN duel: the lobby, vote and result in the game's pause menu

Owner Claude, 2026-10-26, requested by burba after testing the duel ("make the menus look nicer, use the ui
thats used in the pause menu, and just make it look more polished overall"). Gate: none of the roadmap's (an
addition outside the original game; no fidelity claim). Prerequisites: T245. Build identity: worktree
`KH2FinalRewrite-lan`, branch `feature/lan-duel`; package 0.1.2, Rust 1.99.0 Windows MSVC, dev profile.
Source: the selected Steam KH2FM build, validated at each run; the install was only read.

Private evidence: `%USERPROFILE%\kh2-lan-out\` (run logs and captures `camp1-*` … `camp8-*`, `before1-*`, the
launcher shots `launcher-mp.png` and `launcher-mp2.png`; tour logs `pre-*` and `menus-*`). Before and after
screenshots: the worktree's ignored `.local\lan-review\before\` and `.local\lan-review\after\`.

## What changed

1. **The lobby is the game's camp (the pause menu).** It opens on its Config screen with the duel's pages in
   place of the settings: Match (mode, damage, time limit, where the other player is), Bans (two bans, Confirm,
   the other player's bans) and Ready (Keyblade, the AP counter, Ready, the other player). L2/R2 turn the pages
   with the camp's "Switch" banner; Left/Right step a row; rows this player may not change (the joiner's rules)
   refuse with the camp's invalid sound. While the builds are made the top level adds the camp's own Abilities
   and Items, so the build is made in the game's own screens. Header, frames, cursor, help line, status panel
   (now at level 99), transitions and sounds are the camp's (`visual::pause_menu`, `DuelPages`).
2. **One camp, three screens** (`test_room/duel_camp.rs`). The lobby, a vote and a match's result each ask for
   the camp with their entries and pages; one holds it at a time, an older version closes before a new one
   opens, and a vote goes before the others. The camp hands back Left/Right steps and picks of the caller's
   own top-level entries (`CALLER_ENTRY`), and its top-level help line follows changes the caller makes.
3. **The vote** is the camp with the choices as its entries; the help line says where the vote stands and shows
   the coin's face as it spins.
4. **The result** is the camp with Rematch, Leave and Results (the match's numbers on a Config page); VICTORY,
   DEFEAT or DRAW and why the match ended stand over the field to the right of the entries. Rematch's help
   says when the other player wants a rematch or is being waited for. The name of a player who left stays for
   the result ("JoinSora left the duel").
5. **The countdown, GO and round text** keep the game's font over the field; a menu over the field hides them.
   Help lines wrap between words (the text's new-line code) to fit the help box; the game's font has no
   brackets or semicolons, so the duel's texts use none.
6. **The launcher's Multiplayer screen** is drawn after the camp's layout with the launcher's own painter: the
   header pill, the entries as boxes (red under the cursor, a pointer beside it), the help box with the entry's
   line, and a panel with the player's name and the hosts found. [Launcher drawing; the game's menu art is not
   loaded in the launcher, so its sprites and font are not used there.]

Gone with the old lobby (T244): the status panel in the system font (its contents are rows now), and the
Keyblade preview shot (the camp covers the field; the Equip page shows a Keyblade's numbers).

Test support: `--duel-shots` lets the duel's screens add captures of themselves and slows the lobby script to
match; the unattended test vote now comes during the builds (Ready waits for it) instead of over the
countdown; the unattended script turns the lobby's pages for the captures.

## Checks

| Check | Result |
| --- | --- |
| `cargo fmt --check`, `clippy -D warnings`, `cargo test` (1,090 passed), `cargo build` | PASS |
| Loopback duel `camp8` with `--duel-shots`: lobby pages, vote, countdown, GO, knock-out, VICTORY/DEFEAT, Results page, rematch, the second lobby | PASS |
| Magic after the change (`mag2b`, both casting through the new lobby): spells cast and hit both ways, Reflect blocks, Cure heals, Potion and Valor on both; the result says "JoinSora left the duel" | PASS |
| Single-player tours against a build of 4d33db8 (`pre-*` / `menus-*`): magic identical; talk, deck, party-hit identical but for the exit tick (±1, known); auto: the same 750 records, one background form-kit load logged at another place, and both builds end in the same existing panic (`enemy/src/script.rs:215`, also in the `base-*` runs from before the LAN work) | PASS (no change from this work) |
| Launcher Multiplayer screen with a loopback host found (`launcher-mp2.png`) | PASS (looked at) |
| A player at the keys in the lobby (pad and keyboard), two PCs | NOT RUN |

## Verdict / next

PASS on one PC (loopback, unattended). A player at the keys and two PCs NOT RUN. Next: the owner's test on two
PCs.
