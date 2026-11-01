# T327 — Members' ability records; a form's arrival on an area load; slot 1's sparks; area loads with townspeople

Owner Claude, 2026-11-01, requested by burba in the project thread ("work on issues on the github"). Issues #51 and #150. Gate:
development of the party and Drive forms in the test room; no final gate is inferred. Prerequisites: T220, T225, T247, T248, T296,
T323. Build identity: local `main` after `1972788` plus this change, package 0.1.2. Source: the selected Steam KH2FM build, validated
each run; the install was only read.

Private evidence (outside the checkout): `research\issues-20261031\g\b1\` (FUN_1403c1190, FUN_1404014a0, FUN_1404016b0),
`research\party-1to1-20261023\form-effects-spec.md` §1.2, §1.4; runs `c3`, `c4`, `f1`, `f2` under `%USERPROFILE%\kh2-issues-out\s79\`.

## What the executable does

- FUN_1403c1d90 builds every member's status the same way: FUN_1403c1190 adds each piece of equipment's numbers and passes its stat
  row's ability to FUN_140401730, and FUN_1404014a0 adds the equipped slots. So a staff's Fire Boost lands in stats +0x218 like
  Sora's.
- FUN_140405630: an object an area load builds (spawn reason 0) while Sora is in a form plays motion 0x35 and its frame-trigger
  elements, every time.
- A window of the form's second Keyblade is owned by that weapon object, so its spark comes from that object's container.
- FUN_1403efa60's reappear (element 0 and the fade from 0 to 1) is already in since T314 and T315.

## Change

- `party_room::member_abilities`: each member keeps an ability record, built from its weapon, armor, accessories and equipped
  slots, rebuilt with its gear. Its attacks use the record's element percents; item use takes its Item Boost from it.
- The area change plays Sora's arrival when a form is on.
- `EffectRequest::Hit` knows a slot 1 window and uses slot 1's PAX.
- **Bug found on the way:** `AreaSource::fork` starts with no kinds, but `townspeople` indexed `self.kinds` by the global kind
  index, so entering an area whose townspeople were loaded earlier panicked the load thread ("failed to load"). The programs are
  now kept by global index in `kind_programs`.

## Checks

| Check | Result |
| --- | --- |
| Release `--party-camp-tour` on a loaded save with an element staff (`c4`) | PASS: Donald, holding item 154 (stat ability 0x198), logs `element` [120, 100, 100]; Goofy stays at 100; verdict pass |
| Release `--party-camp-tour` (`c3`) | PASS: item 148 has no ability, `element` stays at 100 |
| Release `--soak` hopping Twilight Town areas 8 and 9 in Valor (`f2`) | PASS: `form_arrival` at each area entered in the form, no load failure; before the fix (`f1`) every hop failed with the panic |
| A slot 1 hit in a run | NOT RUN (the log doesn't name the container; `effect_missing` stayed absent) |
| Side by side with the installed game | NOT RUN |

## Uncertainty

- Members' other abilities (Second Chance, Once More, Lucky Lucky and the rest) sit in the record but nothing in the party path reads
  them yet.
- [I] A weapon object whose entry names no motion set still borrows the base weapon's; `FUN_1403ef8b0`'s choice was not traced.
- The airborne branch of the arrival is not there because spawn reason 0 has none.

## Verdict

PARTIAL for #51 (the abilities count; the motion set question stays). PASS for the three items left in #150 in the test room.

## Next

The camp's Status form rows (#161).
