# T242 — LAN duel, phase 3: the lobby

Owner Claude, 2026-10-26, requested by burba in the project chat (phase 3 of the LAN duel; the owner's answers
of 2026-10-26: host-chosen rules with modes, a ban phase of up to two abilities each at the start of every
match, an AP limit of level 99 with every AP Boost, a different Keyblade each, a reusable vote with a host coin
flip; the coordinator relayed that no party vote is shown while Donald and Goofy cannot duel). Gate: none of the
roadmap's (an addition outside the original game; no fidelity claim). Prerequisites: T241. Build identity:
worktree `KH2FinalRewrite-lan`, branch `feature/lan-duel`; package 0.1.2, Rust 1.99.0 Windows MSVC, dev
profile. Source: the selected Steam KH2FM build, validated at each run; the install was only read.

Private evidence: `C:\Users\ANIMA\kh2-lan-out\` (run logs `lobby3-*.jsonl`, their captures) and the traces
`%LOCALAPPDATA%/KH2FinalRewrite/research/playable-room-20261008/ap-max-01/` (AP) and `drive-recovery-01/`.

## What changed

1. Match rules (`test_room/duel_rules.rs`): a mode table (One KO, Best of 3, Time limit), the damage multiplier
   and the time limit (60–300 s); host-authoritative, on the reliable channel, checked on arrival.
2. The lobby's state (`test_room/duel_lobby.rs`): stages Bans → Loadout → Start; each player bans up to two
   abilities (or skips), bans apply to both; a build (Keyblade, ability slots, item slots) is checked by the
   host against the duel's limits (abilities Sora has, as many times as he has them, none banned; equipped AP
   within the limit; a real Keyblade; stocked items in at most Sora's slots); both ready → the match. A rule
   change un-readies both.
3. The lobby in the room (`test_room/duel_lobby_room.rs`): a plain list in the system font with the menus'
   se000 sounds; the host's rule rows change with Left/Right, the joiner sees them read-only; a ban picker;
   Ready with the other player's status; refusals shown in orange. The builds are made in the game's own camp
   (Start: Abilities, Items, Equip) over the duel's inventory: the match's banned abilities removed, one of each
   of the 24 Keyblades, the duel's consumables, the spell levels; the camp's own state comes back when the
   session ends. Both Soras stand still while the lobby is up.
4. The AP limit: 166 = LVUP's 66 at level 99 + the save's AP bonus at its cap of 100 (Standard difficulty;
   FUN_1403c1d90, FUN_1403eda00, cap in FUN_1403c0950). The game hands out 61 AP Boosts once (52 chests, 3
   event rewards, 1 puzzle, 5 synthesis milestones), which give 129; the cap takes the repeatable Skill
   Recipe [duel choice: the game's own maximum].
5. Keyblades: the 24 of the item table's type 2 that the game gives Sora (not the Struggle weapons, Detection
   Saber, Frontier of Ultima, Antiform Dummy or the unnamed 0x51, which has no loadable model). A build's new
   Keyblade rebuilds that Sora's weapon kit on both instances (`drive_room::rebuild_weapons`, also run on the
   joiner).
6. The vote (`test_room/duel_vote.rs`): a question and choices, one ballot each, the same choice wins, a tie
   goes to the host's coin flip (its seed sent with the outcome, so both play the same flip: the coin turns
   over every 6 ticks for 1.5 s with a cursor sound and lands with a decide). F3 → Sora → Duel → Test vote; no
   real vote uses it yet (the party vote waits for party members in a duel).
7. Messages: the reliable channel's messages go to an inbox each tick and each system takes its own kinds.

8. Phase 4, the arena (`test_room/duel_arena.rs`): a table of arenas (the test room's, the room the launcher's
   duel opens); at the match start the host puts both Soras on a line through the room's own start, 300 units
   either side, facing each other (`Player::place` starts the motion over and keeps the build); the camera
   turns behind each player's Sora. No talk and no area exits in a duel (enemies were already off).

## Checks

| Check | Result |
| --- | --- |
| `cargo fmt --all --check`, `cargo clippy --all-targets --locked -- -D warnings`, `cargo test --locked` (1,081 passed) | PASS |
| Single-player tours (talk, magic, party-hit) against a build of local main `5d7a960` | PASS (identical) |
| Rules, lobby and vote logic and wire round trips (`duel_rules`, `duel_lobby`, `duel_vote` tests) | PASS |
| Loopback duel, unattended lobby script (`lobby3`): Bans → Loadout → Start on both; the joiner's ban (Guard) shown by name on both and taken out of the builds; the joiner's Keyblade change built on both (`keyblade_kit`); the test vote tied, flipped and landed on the same choice on both | PASS |
| The camp's builds under Start (Abilities with 166 AP and no banned ability, Equip with the 24 Keyblades, Items) | NOT RUN (needs a player; unattended runs do not open the camp in the lobby) |
| Rule changes by the host in the lobby, the joiner's read-only view | NOT RUN unattended (logic tested) |
| Arena starts (`arena2`): both Soras placed mirrored through the room's start facing each other, the camera behind the host's Sora; the start positions' symmetry and facing (`duel_arena` test) | PASS |
| Two PCs on a real LAN | NOT RUN |

## Uncertainty

- The ability pool is the room's own Sora catalog after the duel build; both players need the same room
  configuration, or the host refuses the joiner's build (the launcher gives both the same).
- The camp's Abilities page shows the AP limit as its maximum, but its own rules for equipping (not the host's
  check) apply while it is open.

## Verdict / next

Phase 3 PASS on one PC for what the unattended runs cover; the camp-made builds and the host's rule rows are
NOT RUN with a player. Phase 4 PASS on one PC. Next: phases 5 and 6, the countdown and the match flow.
