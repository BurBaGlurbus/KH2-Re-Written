# T331 — HUD faces follow the body; Donald's and Goofy's outfits

Owner Claude, 2026-11-01, requested by burba in the project thread ("changing forms or customes should update soras hud icon, make
sure the different outfits for donald and goofy are also made"). Gate: development aid in the test room, plus the HUD face on a real
Drive; no final gate is inferred. Prerequisites: T330. Build identity: local `main` after `7b0fdd2` plus this change, package 0.1.2.
Source: the selected Steam KH2FM build, validated each run; the install was only read. Ghidra 12.1.4 headless on a private read-only
project copy (plain Ghidra, no MCP).

Private evidence (outside the checkout): `%USERPROFILE%\kh2-outfit-research` (`tour9` captures and log, `faces.txt`), decompiles in
`%LOCALAPPDATA%\KH2FinalRewrite\research\outfit-20261031`.

## What the original does

- A gauge's face comes from its object: FUN_14017e7d0 → FUN_14017e980 → FUN_1403b57b0, which takes the type 0x18 image named `face`
  from the archive held at the object's +0x928. So the icon belongs to the object. The install has a `face` in the `.a.us` of nearly
  every playable body: base Sora, all forms, all world costumes and their forms, Lion, Mermaid, Roxas, and Donald's and Goofy's world
  bodies (`faces.txt`). Atlantica's `N_LM010_PLY` and `N_LM020_PLY`, and scene or NPC bodies, have none.
- FUN_14016c3a0 builds the three gauges. When the original rebuilds them around a form change was not traced; here the face changes when
  the kit changes.
- FUN_1403d7620 finds a body's SKLT row by its object entry's byte +7. T330 used that byte as a bone by mistake; this record corrects it.
- `memt` columns +0x12 and +0x14 are a place's Donald and Goofy, which lists their world bodies.

## Changes

- `outfit::hud_faces`: each gauge's face follows the body now shown, on a thread. Sora's follows `DriveRt::shown_model`, so a real Drive,
  a revert and a worn outfit all change it. A body with no `face` keeps the last one. `Gauge::face_changed` restarts the element.
- `party_room::wear`: rebuilds a member's kind on another body through the weapon rebuild; `swap_weapons` then spawns the body again. The
  member's records and level stay. `load` takes a scene body with no usable set on its member's set. Atlantica's bodies have no walk, so
  [rewrite rule, not the game's] a required motion they lack becomes their set's first, and they hold nothing (SKLT has no row).
- Outfits: sections Donald and Goofy, from character ids 2 and 3 and the members' scene bodies. A member's row needs that member in the
  room.
- `drive_room`: a worn body's SKLT row is found by objentry +7, then by character.
- `--outfit-tour` also wears the party's rows and runs a real Drive into Valor.

## Checks

| Check | Result |
| --- | --- |
| `cargo fmt --all --check` | PASS |
| `cargo clippy --all-targets --locked -- -D warnings` | PASS |
| `cargo test --locked` | PASS: 1186 passed, 0 failed |
| Sora's face on forms and costumes, by eye from captures | PASS: Halloween Town, Christmas Town, Space Paranoids, Timeless River, Lion, Antiform, Roxas |
| Face on a real Drive into Valor | PASS by the form change event and the captures |
| `--outfit-tour`: all 106 rows worn, 17 of them Donald's and Goofy's | PASS: no failed kit, no failed member rebuild, no panic |
| Controlled comparison with the original game | NOT RUN |

## Uncertainty

- `N_LM010_PLY` and `N_LM020_PLY` keep their raw names: what they are called in the game was not traced.
- Worn party bodies are not drawn on the camp's own screens, which keep the bodies the room started with.
- A world costume's Keyblade or weapon on a worn body was checked only by eye.
- A Drive absorbs members as before; nothing checks that a costume's member matches the form's rule.

## Verdict

PASS for the development aid. No gameplay gate moves.

## Next

World rooms can call `drive_room::set_outfit` and `party_room::wear` for per-world bodies (T328's open item).
