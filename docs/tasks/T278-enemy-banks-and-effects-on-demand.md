# T278 — Every battle enemy's sound bank, voice bank and effects, read when it first appears

Owner Claude, 2026-10-28, requested by burba in the project thread ("you can keep working"). Gate: development of
enemy AI in the test room; no final gate is inferred. Prerequisites: T262 (every type 4 / 21 enemy on its script),
T277. Build identity: local `main` after `500029c`, package 0.1.2, release launcher with the `skate3` feature. Source:
the selected Steam KH2FM build, validated each run; the install was only read.

Evidence: run logs in a private run folder outside the checkout (`roster-09`, `roster-11`, `combat-13`).

## What was wrong

The room's start read the object sound bank, voice bank and effect container only for `enemy_battle::SUPPORTED`
(Shadow, Soldier, Dusk, Sephiroth). Since T262 every type 4 / 21 enemy fights on its own script, but the others'
sounds had nothing to resolve against:

- motion sounds (frame event 8): in `roster-09`, all 35 of M_EX570's and its neighbours' requests logged
  `"layers": null`;
- victim sounds;
- voices: in `roster-10`, the `bank: null` that T277 reported;
- frame event 1 effects.

## Changes

- `test_room::enemy_assets::service`: when a gallery battle actor's object has no bank and no effects yet, its
  archive `obj/<model>.a.us` is read on a worker thread with the same readers the start uses
  (`sound::object_bank`, `sound::object_voice_bank`, `drive_room::object_pax`). When the read finishes, the banks are
  registered in the room's sound banks and the three maps (`enemy_banks`, `enemy_voice_banks`, `enemy_paxes`) get
  them. Each object is asked for once and logged (`enemy_assets`).

## Checks

| Check | Result |
| --- | --- |
| Roster `roster-11` (B_CA020, M_EX570, M_EX680, M_EX050) | PASS: four archives read (banks 294, 210, 238, 202; B_CA020's voice bank 5205; all four with effects); no faults. Motion sounds resolve (27, none with `layers` null; before, 35 with `layers` null); 20 victim sounds, 10 of 10 voices taken, 49 effect sounds (none of these three kinds before) |
| Combat tour in the flat arena (`combat-13`) | PASS: 16 hits on the enemies, 2 on Sora |
| `rustfmt --check` on the changed files, `cargo clippy --all-targets --locked -- -D warnings`, `cargo test --locked` | PASS |
| Release launcher (`tools\Build-Launcher.ps1 -Skate3`) | PASS |
| Heard / seen side by side with the installed game | NOT RUN |

## Uncertainty

- The first sounds and effects of an enemy, those before its archive is in (well under a second), are lost.
- The executable loads an object's archive with its area, not on first appearance. The order banks are registered in
  is the room's, not the game's.

## Verdict

PARTIAL. Every battle enemy's own sounds, voices and effects now resolve; not compared with the installed game.

## Next

Range 0x17 (the effect stop), frames 26 / 27 (the colour fades), and range 0x33 (a sound slot of the object).
