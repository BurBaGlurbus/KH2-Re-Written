# T246 — Sora's own victim hit sound (issue #100)

Owner Claude, 2026-10-26, requested by burba in the project chat ("tackle some issues in the
github"). Gate: development of the test room only; no final gameplay acceptance is inferred.
Prerequisites: T182 (the victim hit sound for enemies), T190. Build identity: local `main` at
`5d7a960` plus this change, package 0.1.2, Rust 1.99.0 Windows MSVC. Source: the selected Steam
KH2FM build, validated each run; the install was only read.

Private evidence: `%LOCALAPPDATA%/KH2FinalRewrite/research/combat-feel-20261017/g/d5/140410d60_FUN_140410d60.c`;
unattended run log `bank.err` in the private `kh2-issues-out/`.

## What the executable does

`FUN_140410d60` (the damage routine, victim side) plays
`victim bank * 1000 + (counter & 1) + class` only when the hit record's flags bit 2 is clear, the
victim's bank record (`obj+0xbb0`) exists and its bank number is not negative. The bank is the
header number of the sound bank the victim's object archive was loaded with (T182).

## What the room did

Only enemies played a victim sound. When an enemy hit Sora (the room's own enemy or a gallery
enemy), no victim sound was requested for him.

## Rewrite

Both paths where an enemy's counted hit lands on Sora request his victim sound under the PLAYER
objentry id (0x236) with his own hit counter. His object archive (`obj/<player object>.a.us`) is
registered like an enemy's, so its bank plays if it has one.

## Checks

| Check | Result |
| --- | --- |
| `cargo fmt --all --check`, `cargo clippy --all-targets --locked -- -D warnings`, `cargo test --locked`, `cargo build --locked`, smoke | PASS (the same runs as T245) |
| Release room start with the installed source (private `bank.err`) | `obj/P_EX100.a.us` is found and carries no `se/obj` bank, so no bank is registered for Sora |

## Uncertainty

- [I] Base Sora's object carries no sound bank, so his bank record is taken to be absent and the
  executable plays nothing for him; that the record is null (rather than holding some other
  bank) is inferred from the load path, not observed in a running game.
- A Drive form's object is a different archive; only the base player object's bank is
  registered.
- Not compared with the installed game.

Verdict: PARTIAL. Sora's hits run the victim-sound step of `FUN_140410d60`; with the installed
data it plays nothing for base Sora.
Next: a capture of the original to confirm Sora makes no victim sound; Drive form objects' banks.
