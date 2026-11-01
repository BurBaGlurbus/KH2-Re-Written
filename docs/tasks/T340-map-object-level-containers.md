# T340 — Map objects that are level containers

Owner Claude, 2026-11-01, requested by burba in the project thread ("commit to local main and continue working on issues on guthub"). Issue
#22 (the last item). Gate: development of the room's map objects; no final gate is inferred. Prerequisites: T294. Build identity: local `main`
after `150c951` plus this change, package 0.1.2. Source: the selected Steam KH2FM build, validated each run; the install was only read. Plain
Ghidra 12.1.4 headless on read-only project copies.

Private evidence (outside the checkout): `research\issues-20261101\g\f2\`, `f3\`, `f4\` (FUN_1401c3330, FUN_1401c3200, FUN_1401a5180, FUN_1401af000,
FUN_140188120); runs `tr1` to `tr6` under `%USERPROFILE%\kh2-issues-out\s161\`.

## What the executable does

- A model's data starts 0x90 bytes in. When its first word is -1 the model is a level container: u32 offsets from the data's +0x10, ending at 0,
  each to a whole model (FUN_1401c3330).
- Its texture bank is one too: first i16 -1, u32 count at +4, u32 offsets at +8 (FUN_1401a5180). FUN_1401c3200 counts the textures of the earlier
  levels, so each level's own numbers start at 0.
- The draw takes the level from the object's byte (+0x139 for a map object, FUN_140188120).

## Change

`visual::scenery`: `level_payload` and `level_bank` give the first level's model and bank, and a rejected map object now logs why. `area_enter`
and `area_start` log `bob_models` and `bobs_unread`.

## Checks

| Check | Result |
| --- | --- |
| `cargo clippy --all-targets --locked -- -D warnings`, `cargo test --locked` | PASS |
| Unit tests: first level of a model and of a bank; plain payloads unchanged | PASS |
| Release run in tr area 2 (`tr6`) | PASS: `bob_models` 3, `bobs_unread` 0 (it was 1, "unqualified model type"). The tour's captures do not frame that object, so it has not been looked at |
| Twilight Town and the rest of the 278 maps | No change by construction: their first word is not -1 (T294's census) |
| Side by side with the installed game | NOT RUN |

## Uncertainty

- [I] The room always draws level 0. The writer of the level byte +0x139 was not traced, so a far level never shows.
- T294's census counted two such models in tr02. This run loads one area's program, which reads one of them; the other was not seen.
- The per-part fade only acts on these models (T294) and is not built.

## Verdict

PASS for reading them. #22's alpha order (T294) and these models are done, as far as the room can check. Level and part-fade selection by
distance stays unbuilt, and it only affects these models.

## Next

The owner looks at tr02 for the object.
