# T247 — Donald's and Goofy's equipment counts in battle (issue #51, part)

Owner Claude, 2026-10-26, requested by burba in the project chat ("continue with more
issues"). Gate: development of the test room only; no final gameplay acceptance is inferred.
Prerequisites: T132 (equipment stat rows), T220 (member equipment in the camp). Build identity:
local `main` after `1c92617`, package 0.1.2, Rust 1.99.0 Windows MSVC. Source: the selected
Steam KH2FM build, validated each run; the install was only read.

Evidence: T132's private traces of `FUN_1403c1d90` / `FUN_1403c1190` (`research/gear-20261016/`);
unattended runs `camp3` and `camp4` in the private `kh2-issues-out/`.

## What the executable does

A member's status totals are its level row's numbers plus the record's bonuses plus the stat
row of each equipped weapon, armor and accessory (`FUN_1403c1d90` through `FUN_1403c1190`),
rebuilt when the equipment changes (`FUN_1402fc760` → `FUN_1403c2100`); damage reads those
totals.

## What the room did

The camp showed the members' totals with their equipment, but their attacks and the hits they
took used the level row only, and the room owned no staves or shields to equip.

## Rewrite

- `MemberRt::gear` holds the equipment's share of the totals (the same `gear::totals` pass as
  Sora's), rebuilt from the record when the camp closes.
- The members' attacks (weapon swings, spells, the Bash, the party limits' volumes) add its
  Strength and Magic; hits on a member use its Defense and resists (`MemberRt::target`).
- The room's development stock owns every staff and shield with a picture, like the Keyblades.
- `--party-camp-tour` equips Donald's first staff and its verdict requires a weapon in his
  record; the log's `camp_party` lists each member's gear.

## Checks

| Check | Result |
| --- | --- |
| `cargo fmt --all --check` | PASS |
| `cargo clippy --all-targets --locked -- -D warnings` | PASS |
| `cargo test --locked` | PASS |
| Unattended release `--party-camp-tour` with the installed source (private `camp4`) | PASS: verdict pass; Donald leaves the camp with item 75 (Mage's Staff) and gear Strength +1, Magic +1, Defense 0, the numbers the Equip preview shows |

## Uncertainty

- Not built: the equipped staff or shield's model in the room (the member keeps its base weapon
  object), and the weapon's ability (Thunder Boost and others) in battle.
- The staves and shields in stock are a test-room choice, not campaign progression.
- Not compared with the installed game.

Verdict: PARTIAL for issue #51: the battle numbers follow the equipment; the weapon model and
ability remain.
Next: rebuild the member's weapon object from the WENT set on equip.
