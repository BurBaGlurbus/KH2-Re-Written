# T245 — Donald's and Goofy's models in the camp (issue #145)

Owner Claude, 2026-10-26, requested by burba in the project chat ("tackle some issues in the
github"). Gate: development of the test room only; no final gameplay acceptance is inferred.
Prerequisites: T220 (the members in the camp lists), the camp-03 MenuObj work. Build identity:
local `main` at `5d7a960` plus this change, package 0.1.2, Rust 1.99.0 Windows MSVC. Source: the
selected Steam KH2FM build, validated each run; the install was only read.

Private evidence: `%LOCALAPPDATA%/KH2FinalRewrite/research/playable-room-20261008/camp-03/menuobj-spec.md`
(Tz::MenuObj), `camp-08/party-spec.md` §4 (Party cards), `camp-01/camp-spec.md` §3.5 and
`dialogue-01/objentry.txt` (the MENU_FRIEND entries); unattended run `camp1` in the private
`kh2-issues-out/`.

## What the executable does

Tz::MenuObj adds one model per party slot with `add(i, obj | 0x20000000, motion 0x8b, variant 2,
x, y)`. The object comes from table `0x1405babf0` by the slot's position in the world party table:
PLAYER 0x236, MENU_FRIEND_1 0x319 (model MENU_FRIEND_1, set P_EX020_MEMO.mset), MENU_FRIEND_2
0x31a (P_EX030.mset), MENU_FRIEND_3 0x3ee. `FUN_1403e6230` loads variant 2 of motion 0x8b and
falls back to 3, 2, 1, 0. Each slot is cleared and drawn into render target 1 on its own, then
pasted at its offset (`FUN_1402ffbb0`); `FUN_1403001b0` advances each record's walk by one tick
per frame and wraps it at that motion's own length.

## What the room did

Only member 0 (Sora) was rendered; the paste list already held the members' slots, offsets and
alphas, but `Sheet::Model(i)` for `i > 0` drew nothing.

## Rewrite

- `test_room/menu_friend.rs`: reads the MENU_FRIEND object of party slot 1 and 2 from
  00objentry, its model and set, picks motion 0x8b with the field, no-weapon chooser order
  (2, 3, 1, 0, the same order as `FUN_1403e6230`) and samples every tick of it with the root kept.
- Each member gets its own render layer, camera and target with Sora's menu camera; the paste of
  `Sheet::Model(i)` samples member `i`'s target.
- Each member walks on its own length: the presenter counts camp updates from Sora's walk time
  (which wraps at 64) and maps them onto the member's motion (`menu_friend::time`).
- The members load on the party thread next to the party kinds and stay with the member they
  were read for.
- `--party-camp-tour` captures the top level and the Party cards as well.

## Checks

| Check | Result |
| --- | --- |
| `cargo fmt --all --check` | PASS |
| `cargo clippy --all-targets --locked -- -D warnings` | PASS |
| `cargo test --locked` | PASS (new: `the_walk_runs_one_to_its_length`, `steps_follow_sora_walk_across_its_wrap`) |
| `cargo build --locked` | PASS |
| Smoke `cargo run -p main --locked -- --frames 180` | PASS |
| Unattended release `--party-camp-tour` with the installed source (private `camp1`) | PASS: exit 0, `party_camp_verdict` pass; capture 00 shows Sora, Donald and Goofy over the party frame, capture 25 shows all three on their Party cards |

## Uncertainty

- [I] A world guest's slot (Party member other than Donald in slot 1 or Goofy in slot 2) is left
  empty: what object `FUN_1403dfeb0` resolves MENU_FRIEND_n to in another world is not traced.
- [I] The members' lighting and shading follow Sora's menu model, which is itself unverified
  (camp-03 Unresolved).
- Not built: the late-load fade (group 125) for a member whose object loads after the camp opens;
  the hidden parts 20..31 for party entry +10 == 8.
- Not compared: any frame against the installed game.

Verdict: PASS for issue #145 in the development room (the party frame and Party cards show the
members, posed on their own walk and faded with their hosts); PARTIAL against the installed game.
Next: an owner check of the camp against the original.
