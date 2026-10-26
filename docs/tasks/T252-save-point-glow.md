# T252 — The save point's glow and its pulse while Save heals (issue #139)

Owner Claude, 2026-10-26, requested by burba in the project chat ("continue working on more
issues"). Gate: development of the test room only; no final gameplay acceptance is inferred.
Prerequisites: T211 (save points), T251. Build identity: local `main` after `b52312b`, package
0.1.2, Rust 1.99.0 Windows MSVC. Source: the selected Steam KH2FM build, validated each run; the
install was only read.

Private evidence: `%LOCALAPPDATA%/KH2FinalRewrite/research/playable-room-20261008/fieldsfx-01/fieldsfx-spec.md`
(§2.6 and the TT object PAX table: SAVE_POINT 570 element 2 with se000 #29, flags 1),
`save-20261016/save-spec.md` §4-5; unattended run `save3` (Twilight Town area 9) in the private
`kh2-issues-out/`.

## What the executable does

The save point object (objentry 0x23a, type 7) starts element 1 of its own container when it
is created (`FUN_140418e30`: `FUN_1403b6670(obj, 1, 1, 0, 0)`). Its update (`FUN_140418a00`)
starts element 2 (se000 #29) when the pulse `+0xc08` is set and no instance runs, and ends it
(`FUN_1402be8e0`) when the pulse is gone; the heal `FUN_140418f30` sets the pulse while Save is
offered.

## Rewrite

`reaction_command::save_point_effects`: on an area with save points, element 1 of
`obj/SAVE_POINT.a.us` starts at each save point; element 2 starts at the offered save point
while the heal runs and is released (its sound stopped) when it stops; an area change releases
both. `LoadedArea::save_point_pax` reads the container with the area.

## Checks

| Check | Result |
| --- | --- |
| `cargo fmt --all --check`, `cargo clippy --all-targets --locked -- -D warnings`, `cargo test --locked`, `cargo build --locked`, smoke | PASS |
| Release `--save-point-tour` (private `save3`) | element 1 starts at tick 1 (owner 0xf7), the pulse turns on at 20 when Sora stands on the save point, element 2 starts at 21 with se000 #29; the capture at 120 shows the save point's light columns and swirl around Sora; the verdict passes |

## Uncertainty

- [I] The save point's yaw (the spawns' rotation) is not applied to its effects; the glow looks
  round.
- [I] The pulse stays on while the file menu is open and restarts when the menu closes (608 to
  611); whether the original pauses it is not traced.
- The save point's own model (if `SAVE_POINT.mdlx` has one) is not drawn; only its effects.
- Not compared with the installed game.

Verdict: PASS for issue #139's glow in the development room; PARTIAL against the installed game.
Next: compare a save point side by side with the original.
