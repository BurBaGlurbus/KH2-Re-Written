# T233 — Movement physics matched bit for bit against the executable's own routines

Owner Claude, 2026-10-25, requested by burba in the project chat (the byte-match feature, "Both, round-trip first":
after BAR and saves, match movement-physics outputs against the original game). Gate: P18 (movement) development;
no gate acceptance is inferred. Prerequisites: T231, the movement port in `player::actor_physics` (2026-10-09/10
movement work). Build identity: branch `feature/byte-match-bar` from local `main` at `4b4ded6`, package 0.1.2, Rust
1.99.0 Windows MSVC. Source: the selected Steam KH2FM executable, read through a private copy of the analysed Ghidra
project; the game was not run and nothing was written to the install.

Private evidence root: `%LOCALAPPDATA%/KH2FinalRewrite/research/byte-match-20261025/` (`scripts/MoveOracle.java`,
`crtmath.exe`, `g/d1` decompiles, `g/a1`–`g/a3` disassembly, `movement-oracle-01.txt` the oracle,
`movement-mismatch-01.txt` before the fix, `movement-mismatch-02.txt` after it, empty).

## How the reference is made

No recording of the original game's movement existed (the private autoplay and tour logs are all of the rewrite).
Instead, the executable's own routines run in Ghidra 12.1.4's p-code emulator on a grid of inputs:

| Routine | What it does | Cases |
| --- | --- | ---: |
| FUN_1403b73f0 | jump rise time √(2h/g) | 24 |
| FUN_1403cbc30 | jump launch: t, rise, base, span | 24 |
| FUN_1403cb500 | jump/fall step: double-sine rise, parabola, fall cap (whole arcs for 8 heights × 2 fall caps × time steps 1 and 2, 160 ticks each) | 5,120 |
| FUN_1403b7ad0 | air steering toward the stick, drag when released | 316 |
| FUN_1403d4960 | knock-slide start | 30 |
| FUN_1403d4570 | knock-slide step until it stops | 655 |
| FUN_1404228b0 | blow flight start (speed floor) | 8 |
| FUN_140422780 | blow flight step (brakes, gravity, fall cap) | 1,920 |

The routines call `sinf`, `powf` and `sqrtf` through the import table of `api-ms-win-crt-math-l1-1-0.dll`. Those calls
are answered by a small native helper whose Rust `f32::sin`/`powf` import the same functions from the same DLL
(checked in its import table); `sqrt` is IEEE-exact either way. The routine that returns the position
(FUN_1403b5b40) is stubbed to return a test buffer, and FUN_1403ba190 to return true; the frame time is 1.0.

`movement_match` (player crate) reads the oracle and runs each case through `player::actor_physics`
(`rise_time`, `Arc::launch`, `Arc::step`, `steer`, `Knock::new`, `Knock::step`, `Blow::new`, `Blow::step`), comparing
every f32 by its bits.

## What differed, and the fix

The first comparison matched 7,841 of 8,097. Every difference was one unit in the last place, in two places:

- FUN_140142080, the normalise both the knock start and the blow step use, multiplies by `1 / length`; the port
  divided by the length.
- The blow routines take the horizontal length as `sqrtf(x² + z²)`; the port used `hypot`.

`Knock::new`, `Blow::new` and `Blow::step` now do the same (a zero length leaves the blow vector as it is, as the
original does). The blow speed floor uses the original's `<=` test.

## Checks

| Check | Result |
| --- | --- |
| `movement_match` on the oracle | PASS: 8,097 of 8,097 bit-identical (arc 5,120, blow step 1,920, knock step 655, steer 316, knock start 30, launch 24, rise 24, blow start 8) |
| New unit test (synthetic input): knock and blow directions use the reciprocal length | PASS |
| Workspace checks (fmt, clippy and tests with and without `main/skate3`, build, smoke) | PASS |

## Uncertainty

- [I] The reference is the executable's code in Ghidra's emulator, not the running game. The emulator's single-precision
  arithmetic is trusted; that the only differences found were the ones the disassembly predicts supports it.
- Only the paths the grid reaches are compared: the frozen (no-gravity) flag, the knock bonus from obj+0x5c0, a
  zero-length knock direction (the port ends the slide, the original keeps a zero-direction slide running) and the
  status bits the routines set are not. FUN_1403b7ad0's vertical lane is not compared (the port steers in X/Z only).
- Ground movement (FUN_140422350), the sampling every second frame (FUN_1403b89a0), the jump cut and whole-frame
  integration in the test room are not part of this check.
- These functions are not complete translations, so the public ledger's function count is unchanged; this is not a
  1:1 behaviour scenario either.

## Verdict

PASS for the compared routines: on 8,097 cases the port gives the executable's results bit for bit.

## Next

Extend the grid to the frozen flag, knock bonus and ground movement; then a recorded in-game comparison.
