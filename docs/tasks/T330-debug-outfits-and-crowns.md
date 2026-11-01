# T330 — Debug menu: every Sora and Roxas model, and the crowns

Owner Claude, 2026-11-01, requested by burba in the project thread ("add all soras models and costumes, roxas, and crowns, add them
all to debug mode"). Gate: development aid in the test room; no final gate is inferred. Prerequisites: issue #39's debug menu, T108
Drive form kits. Build identity: local `main` after `efc3281` plus this change, package 0.1.2. Source: the selected Steam KH2FM build,
validated each run; the install was only read. Ghidra 12.1.4 headless on a private read-only project copy (plain Ghidra, no MCP).

Private evidence (outside the checkout): `%USERPROFILE%\kh2-outfit-research` (`tour6` captures and log, `survey.txt`, `faces.txt`,
`tour.ps1`), decompiles in `%LOCALAPPDATA%\KH2FinalRewrite\research\outfit-20261031`.

## What the original does

- **Models.** Every body is an entry of `00objentry.bin`. FUN_140405190 picks a member's object from the `memt` row of the place:
  the member's at +0x10 + slot × 2, a Drive form's at +0x16 + form × 2. Rows hold the world costumes and their forms.
- **Crowns.** FUN_1403a9e20 counts the Proofs held (DAT_1405c4c98: items 0x251, 0x252, 0x253) and passes DAT_1405c4ca0's byte for
  that count (0xa4, 0x9a, 0x9b, 0x9c) through FUN_1403cd8a0 to FUN_1401a87a0, which starts that TEXA animation on the object's model.
  So a crown is not an object or a pickup. It is a transparent mesh part on Sora's head whose texture page has a 64x64 patch with four
  images: copper, silver, gold and empty. FUN_1403a8e70 and FUN_1403a7040 call it after FUN_1403d6470, and FUN_1403a7f30 on its own.
- Which models have those animations comes from the install: `P_EX100`, its forms and scene bodies have them; the world costumes,
  Roxas and `P_EX120` do not. That matches the costume's own data, so a world costume shows no crown.

## Changes

- `test_room/outfit.rs`: the catalog (object table plus installed files, friendly names where known and the raw name otherwise), the
  crown (`crown`, FUN_1403a9e20's count unless the menu picks one) and the unattended `--outfit-tour`.
- `drive_room`: `set_outfit` wears an object in base Sora's place through the Keyblade change's background rebuild. `FormSources::build`
  takes the worn object; a Drive form uses the column of the worn object's `memt` row. An object with no move set of its own plays by
  base Sora's tables.
- `character_kit`: `lenient` kits. [Rewrite rule, not the game's] A motion neither set holds becomes the set's first motion, and a
  locomotion clip with no actionable range answers the stick.
- `motion_eval::evaluate_shared`: a cutscene body (`H_EX500`, `H_EX510`) shares only its body bones with the field skeleton (parents
  match to bone 143). Those follow the motion; its other hair and face bones stay in their bind pose. [Rewrite rule, not the game's:
  the original plays these models on event motions.]
- `debug_menu`: Sora tab rows Outfits and Crown. Crown offers the Proofs rule, each crown, and the Proof stocks.
- Base Sora's crown part used to draw the texture page as it is. It now shows the frame the Proofs pick, so it is empty with no Proofs.

## Checks

| Check | Result |
| --- | --- |
| `cargo fmt --all --check` | PASS |
| `cargo clippy --all-targets --locked -- -D warnings` | PASS |
| `cargo test --locked` | PASS |
| `--outfit-tour` on the Steam build: every catalog row worn in the room | PASS: 89 of 89 rows load, no failures, no panic |
| Crown frames on base Sora, by eye from captures | PASS: none, copper, silver, gold on his head |
| Outfits and Crown pages, including a search | PASS, by eye |
| Crown following a Proof gained in play | NOT RUN: the counting rule is unit tested, the menu's Proof rows were not driven |
| Controlled comparison with the original game | NOT RUN |

## Uncertainty

- Friendly names come from the object names and from looking at captures, except where stated in the code. `P_EX120` is named from its
  look (red shorts, yellow shoes); what the game calls it was not traced. The `P_EX120_NPC_*` tints are named from their look.
- Bodies made for a scene or a ride play whatever their set holds, so many of their motions are stand-ins. They load and answer the
  stick; their motions are not meant to fit field play.
- Lion and Mermaid Sora have other skeletons. T331 corrects how a worn body's Keyblade joint is found; the Keyblade's place on them
  was not compared with the game.
- What marks the camp's own model is unchanged: the camp keeps showing its base model while an outfit is worn.

## Verdict

PASS for the development aid. No gameplay gate moves.

## Next

T331: HUD faces that follow the form and costume, and Donald's and Goofy's outfits (burba, same thread).
