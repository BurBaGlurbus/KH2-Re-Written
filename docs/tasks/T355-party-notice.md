# T355 — The event camp's party notice (state 0x15) and the row an event changes

Owner Claude, 2026-11-02, requested by burba in the project thread ("work through issues and close them"). Issue #34, its last part. Gate: development of the camp and party in the test room; no
final gate is inferred. Prerequisites: T344, T347, T354. Build identity: local `main` after T354 plus this change, package 0.1.2. Source: the selected Steam KH2FM build, validated each run; the
install was only read. Plain Ghidra 12.1.4 headless on a read-only project copy.

Private evidence (outside the checkout): `research\issues-20261102\party34-spec.md` section F, `research\playable-room-20261008\camp-08\party-spec.md` section 8; runs `n1`, `n2`, `n3`, `bb04c`,
`bb04r` under `%USERPROFILE%\kh2-issues-out\s162\`; private configuration `area-bb04-n.json`.

## What the executable does

- An entry's jump record can name a member preset and a mode (FUN_1403a46e0). The row of the jump's world takes the preset. Mode 0 stops there, mode 1 opens the camp on the notice, mode 2 also strips whoever left (FUN_1403fd8d0) and adds a line saying so.
- The camp is the whole camp, opened on Party with state 0x15 over it (FUN_140305440, FUN_140320fc0). FUN_140320680 lists who came and who went, a locked name after icon 10. The third line is 0xcc69 for mode 2, else the last locked member's message.
- OK or cancel closes the box and the Party screen is live. The second pick's keys stay swapped after it (FUN_140320fc0).
- Closing that camp writes the row's bytes only: the jump that follows rebuilds the field.

## Changes

- `pause_menu/party.rs`: `PartyNotice`, `enter_party_notice`, `party_notice`; `Camp::open_party_notice`; `State::PartyNotice`; `items::open_dialog_in`.
- `story_events.rs`: an entry's preset goes on the row before the save image is taken, silently for mode 0, and the notice travels with the jump.
- `game_save::{party_changes, strip_departing}`; `party_room::{notice_texts, parse_notice, notice_arg}`; `Game::party_notice`; `--party-notice <row>:<mode>` on the room and the event, which hands it on.
- The twelve new texts in `CampSource`.

## Checks

| Check | Result |
| --- | --- |
| `cargo fmt --all --check`, `cargo clippy --all-targets --locked -- -D warnings`, `cargo test --locked` | PASS |
| Unit test: who came and who went, a lock bit kept | PASS |
| bb area 2 program 1, old row given, mode 1 (`n2`) | PASS: box with Party In "Beast", Party Out empty, OK; after OK the Party screen with Beast, Goofy and the reserve's Donald |
| The same, mode 2 (`n3`) | PASS: the third line "The departing member has surrendered their equipment." The stripping itself ran on no real member here |
| The real chain at bb area 4 program 10, switch on, `KH2RW_CHAIN=1` (`bb04c`, `bb04r`) | FAIL first: the notice opened at tick 1 while the field still followed the row, so the camp held two Goofys and its close swapped the row. The camp now waits for a replacement to finish |
| The same after the fix (`bb04c2`, `bb04r2`) | PASS: `story_signal` flag 5149, event 205 played to its end, the room started at program 74, `party_replace` then the notice at tick 21: Party In "Beast", cards Beast, Donald, Goofy; close `party_row` [0,3,1,2] unchanged, field Beast and Donald, the next top level showing the same |
| A jump with no event (the transition arm) | NOT RUN: mu area 0 and bb area 4 both play an event first |
| A locked name's icon, and a lock message | NOT RUN: no entry was run that locks a member |
| Side by side with the installed game | NOT RUN |

## Uncertainty

- The camp won't open for Start while a member is being replaced, a second at most. The executable holds its own camp open through that instead.
- The event's jump always rebuilds in the next program, so the notice shows over the new room. The executable shows it before the jump, over the event's last frame: a visible difference.
- `FUN_1403e6c40`'s 0xcc9f, Ping's other message, is left out: that check isn't read.
- Story events stay behind `KH2RW_STORY_EVENTS=1`, so a player sees none of this until they're on by default.
- A name appears only if the place's `memt` holds that member's object. `FUN_1403c5dc0`, called before the camp, isn't read.

## Verdict

PASS in the room for the notice and the row. #34's list is done; what's left there is written in its closing comment.

## Next

The next open issue.
