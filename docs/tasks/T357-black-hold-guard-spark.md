# T357 — The black hold after the iris, measured; the attacker's own guard spark

Owner Claude, 2026-11-02, requested by burba in the project thread ("work through issues and close them"). Issues #36 and #67 (part). Gate: development of area changes and combat presentation in the
test room; no final gate is inferred. Prerequisites: T110, T237, T336. Build identity: local `main` after T356 plus this change, package 0.1.2. Source: the selected Steam KH2FM build, validated each run; the
install was only read. Plain Ghidra 12.1.4 headless on a read-only project copy.

Private evidence (outside the checkout): `research\issues-20261101\ko\ko-spec.md` section 3, `research\issues-20261102\g\h4`; runs `surv2`, `cold1`, `ecb1` under `%USERPROFILE%\kh2-issues-out\s162\`; the census at
`%USERPROFILE%\kh2-issues-out\census.tsv`.

## The black hold

The next area decodes on a worker from the moment the iris starts to close, which takes 71 frames, 1183 ms. T110 measured the hold at 1 to 1.2 s; T237 and later work cut the decode. An area's black hold is now its decode past that time plus the main thread's part.

| Measured, `area_load` from `--room-survey` over 16 areas, then `mu00`, `bb04` | ms |
| --- | --- |
| Worker: 16 areas of 11 worlds | 242 to 635, median about 380 |
| Worker: the first area change after launch (`mu00`) | 656 |
| Main thread | 4 to 31 |

So no area measured outlasts the iris, and the hold is the two-frame fade plus the main thread's part: nothing is left here to shorten. No change was made for this.

## The guard spark

FUN_1403d10c0 starts the attacker's own element on a clash when its ATKP hit effect names a row with category `(1 << (32 + eff - 1)) | 0x10` in its own container, and marks the pair so FUN_1403d0f00 starts no system element, unless the attack's RefactSelf is 2. The census now lists these rows: 70 of them, in enemy, boss and form containers.

- `EffectRequest::ClashSpark`, on Sora's two guard paths (the room's enemy and the battle enemies).
- `effect_census` prints the rows.

## Checks

| Check | Result |
| --- | --- |
| `cargo fmt --all --check`, `cargo clippy --all-targets --locked -- -D warnings`, `cargo test --locked` | PASS |
| `--enemy-combat-tour` (`ecb1`) | PASS for what it reaches: three enemies' hits land on Sora as before. It runs no guard, so it doesn't test the spark |
| A guard against an attack with such a row | NOT RUN: no tour reaches it |
| The hold with a cold file cache | NOT RUN: this PC's cache couldn't be emptied. The first change after launch is the nearest measure |
| The hold against the installed game | NOT RUN |

## Uncertainty

- The direction of the guard spark isn't read: it starts with no turn.
- A party member's guard still plays its system element only, as the claw's attacker isn't kept there, and FUN_1403d10c0's other direction, the guard's own row, isn't built. P_EX100 has no such row; P_EX100_MAGF has one.
- The once-per-pair gate is met by the hit record, which stops the same attack clashing twice; it isn't a separate flag.
- Positional sounds are not built. FUN_1401de150 hands a position to the audio middleware (FUN_1401374e0, FUN_1400825b0), and how that turns into volume and pan wasn't read.

## Verdict

PASS for #36's measuring: no hold is left to shorten when the disk cache is warm. PASS to compile and regress for the spark, which no run has shown.

## Next

Tinker Bell's and Mickey's revives of Sora, and positional sounds, stay in #67.
