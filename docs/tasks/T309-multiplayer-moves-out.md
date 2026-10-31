# T309 — Multiplayer and KH3 move out to Kingdom Hearts: Connected

Owner Claude, 2026-10-30, requested by burba in the project thread. Gate: none of the roadmap's; single player must not change. Prerequisite: T308, and Kingdom Hearts: Connected working alone on these hooks (its record KHC-002). Source: Steam KH2FM build as before, only read.

## What left this repository

- The room's duel and KH3 modules (`test_room/duel_*`, `kh3_*`, `rival_play.rs`, `mirage_arena.rs`, `online.rs`), and their options (`--host`, `--join`, `--kh3-test`, `--duel-*` and the rest).
- `source/net`, `source/game/kh3_sora`, `source/game/player/src/kh3`, `source/data/src/kh3` with its recipes and `kh3_probe`, `source/data/src/bbs_*` with `bbs_probe`, `source/visual/effect/src/cascade`.
- The launcher's Multiplayer screen and KH3 folder, and their settings. A settings file that still names them loads; the names are dropped on the next save.
- The `aes`, `sha1` and `renet` dependencies, and `player`'s link to `kh3_sora`.
- The README's duel sections, the StrawHatPro notices, and these records, by file name: T240-T244, the `lan-duel` records of T245, T246, T248 and T249, T258, T279-T282, T300. They live in Kingdom Hearts: Connected now. T256 stays: single player uses the PC controls.

## What stays

The room's extension points (`extension.rs`, T308), a neutral list of other Soras (`rival.rs`), the se999 sound bank, the PC controls with `--input`, and the pause menu's generic pages (`DuelPages`, which keeps its name).

`NOTICE` no longer says what it did about KH3 data; it now says this tree holds none and that earlier local commits did. Those commits are still in the local history: this change does not remove them.

## Checks

| Check | Verdict |
|---|---|
| `cargo fmt --all --check` | PASS |
| `cargo clippy --all-targets --locked -- -D warnings` | PASS |
| `cargo test --locked` | PASS |
| `cargo run -p main --locked -- --frames 180` | PASS |
| clippy and test with `--workspace --features main/skate3` | PASS |
| `tools\Build-Launcher.ps1 -Skate3` (what `build.bat` runs) | PASS |
| Unattended runs, on the Skate 3 launcher build, against a build of 87dc536: `auto`, `talk`, `gameover`, `deck`, `magic`, `partyhit` | PASS: the same by event kind and content (`magic` 6361 records), leaving out wall-clock records; `auto`, `talk` and `partyhit` differ only by the exit tick, by up to 3, as two baselines do |
| Launcher opened on this PC | PASS by its own screenshot: the test list opens on the remembered folder |

The run logs are private and stay outside the repository.

## Verdict

PASS for this change: single player showed no difference in the runs above. No claim about the rest of the game, and no gameplay gate moves.

## Next

The owner decides what to do with the KH3 content in the unpushed local history before main is pushed.
