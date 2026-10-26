# T255 — NEW! badges for abilities, Customize's magic and the Equip lists (issue #31, part)

Owner Claude, 2026-10-26, requested by burba in the project chat ("continue working"). Gate:
development of the test room only; no final gameplay acceptance is inferred. Prerequisites: T162
(Items badges), T220 (party pages). Build identity: local `main` after `cd6d1fd`, package 0.1.2,
Rust 1.99.0 Windows MSVC. Source: the selected Steam KH2FM build, validated each run; the
install was only read.

Private evidence: `%LOCALAPPDATA%/KH2FinalRewrite/research/issues-20261026/g/` (`n1.txt` references
to DAT_140bf0dc0, `n2`..`n4` decompiles of FUN_140369fa0, FUN_140369520, FUN_140369690,
FUN_140368ed0, FUN_140369f10, FUN_14036a0b0, FUN_14036a120, FUN_14036a2b0; `n5.txt` callers),
`playable-room-20261008/camp-06/abilities-spec.md` §1.5 and §2.3; unattended runs `camp7` and
`abil1` in the private `kh2-issues-out/`.

## What the executable does

The camp's start allocates the "new" bit block `DAT_140bf0dc0` (`FUN_140369fa0`) and fills it
(`FUN_140369520`): `FUN_140368ed0` appends each ability in a character's record slots (+0x54)
that the character's obtained-order list lacks (Sora save+0x4278, 80; the party save+0x4318 +
0x60·(t−2), 48), sets the new bit of every list position whose seen bit is clear (Sora
save+0x41f8, the party +0x4204), then sets the seen bit of every listed ability no longer in the
slots; the forms' lists work alike. A row's badge is G50 (pulse) while new and unseen, G49 (dim)
while new and seen; visiting a row in the list sets its seen bit (`FUN_140363c70` →
`FUN_14036a120`). The character rows and the top level show a badge when any ability is new.
(This record found the producer issue #31 named as untraced.)

## Rewrite

- `pause_menu::abilities`: `MemberAbilities` carries the order list with its new and seen bits;
  `new_marks` is FUN_140368ed0's pass; the list rows, the character rows (row + 100) and the top
  level's Abilities row draw G50 / G49; entering the list and each cursor move see the cursor
  ability.
- The room keeps each character's list and seen bits across camps (`CampRes::ability_marks`,
  written back when the camp closes).
- Customize (camp-07 §3.1, §4.2, §4.7: `FUN_14036aa70`, `FUN_14036a9a0`, `FUN_14036a220`): the
  new magic is the camp's new items of type 0x12 (the producer's first loop in FUN_140369520);
  the list rows (prio P+3 at the row group's (165, 0)), Sora's row in the character list and
  the top level's Customize row show G50 / G49, and resting on a shortcut or MP Charge row sees
  its magic (save+0x41a8, the items' seen list).
- Equip (camp-09 §4: `FUN_14036a8f0`, `FUN_14036a7f0`, `FUN_140358ba0`): each pick-list row
  shows G50 / G49 for a new item (the camp's new items), and every cursor change sees the item
  under the cursor.

## Checks

| Check | Result |
| --- | --- |
| `cargo fmt --all --check`, `cargo clippy --all-targets --locked -- -D warnings`, `cargo build --locked`, smoke | PASS |
| `cargo test --locked` | PASS (new `new_abilities_badge_until_seen_and_stay_seen`, `only_new_magic_carries_the_customize_badge`) |
| Release `--party-camp-tour` (private `camp7`) | verdict pass; the top level's Abilities row shows NEW!; Donald's list shows Cure pulsing and Thunder dim after the cursor visited it |
| Release `--ability-tour` (private `abil1`) | verdict pass; Sora's list shows the badges, dimming as the cursor passes |

## Uncertainty

- Not built: the Drive forms' and Lion's ability lists (save+0x4798, `FUN_14036a0b0`) and the
  Status screen's form and Summon rows (new bits block +0xc8, seen save+0x4274): the room's
  Abilities and Status screens have no form or Summon rows to carry them. The Journal's badges.
- No run showed an Equip badge: the staves, shields and Keyblades the room owns are starting
  stock, which is never new.
- No run showed a Customize badge: the room's magic is part of its starting stock, which is
  never new [test-room choice, T162]; the rule is checked by the unit test only.
- The room has no save: the lists start empty, so every ability of the development catalog is
  new at the first camp.
- The character-row badge was not caught in a capture (the tours pass the character list while
  it opens).
- Not compared with the installed game.

Verdict: PARTIAL for issue #31: the Abilities, Customize and Equip badges follow the traced
producer; the forms', Status and Journal badges remain.
Next: issue #161 (the Journal row badge, the forms' lists and the Status form and Summon rows).
