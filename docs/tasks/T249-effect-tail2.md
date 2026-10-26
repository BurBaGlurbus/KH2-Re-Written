# T249 — Effect program pppKeShpTail2 (154) (issue #65, part)

Owner Claude, 2026-10-26, requested by burba in the project chat ("continue with more
issues"). Gate: development of the effect VM only; no final gameplay acceptance is inferred.
Prerequisites: T150 (Tail2X), the effect census. Build identity: local `main` after `93c306e`,
package 0.1.2, Rust 1.99.0 Windows MSVC. Source: the selected Steam KH2FM build, validated each
run; the install was only read.

Private evidence: `%LOCALAPPDATA%/KH2FinalRewrite/research/playable-room-20261008/vfxsfx-01/g/d03/`
(FUN_1404cce70, FUN_1404cd080), `ghidra-kh2-a4/fx-19/` (FUN_1404cd550, FUN_1404cd760),
`issues-20261026/g/t1/` (FUN_1404cd050 and the UserCtrl programs); census outputs
`census-before.tsv` / `census-after.tsv` in the private `kh2-issues-out/`.

## What the executable does

Program 154's update, draw and create are Tail2X's (155) with these differences, read line by line
against the 155 decompiles: the world-transform flag is key +0x23 (155: +0x26) in both update and
draw; the draw has no +0x23 / +0x24 / +0x25 draw-flag and colour-pointer options, takes its
blend bits from +0x24 / +0x25 (155: +0x27 / +0x28), scales two axes (155: three) and draws each
sprite through `FUN_1404ae220` instead of batching through `FUN_1404ae550`. The create
(`FUN_1404cd050`) is byte for byte TailX's (`FUN_1404cc930`).

## Rewrite

`tail2.rs` handles 154 with Tail2X's key layout and its own world flag; the programs table runs
its create, update and draw.

## Checks

| Check | Result |
| --- | --- |
| `cargo fmt --all --check`, `cargo clippy --all-targets --locked -- -D warnings`, `cargo build --locked` | PASS |
| `cargo test --locked` | PASS (new `each_tail_reads_its_own_world_flag`) |
| `effect_census` against the install | before: 11 unsupported kinds, 154 hiding M_EX950 element 14, B_EX150 and B_EX150_LV99 element 10; after: 10 unsupported kinds, 154 absent |

## Uncertainty

- The blend bits (+0x24 / +0x25) are not applied, as for Tail2X (`emit_sprites` takes none).
- None of the three effects was drawn in a run; the draw path is Tail2X's, which was.
- Not done in issue #65: pppRyjMegaBirthUserCtrl / UserCtrl3 (239/242) draw a particle list that
  game code fills (`DAT_140b60268`, count `DAT_140b60264`); its writer is not traced.
  ModelFilter (229) is not started.

Verdict: PARTIAL for issue #65: program 154 runs; 229, 239 and 242 remain.
Next: trace the writer of the UserCtrl particle list.
