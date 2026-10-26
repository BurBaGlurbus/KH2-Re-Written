# T254 — The leash flag's readers and Party Attack's re-pick after an action (issue #146)

Owner Claude, 2026-10-26, requested by burba in the project chat ("work on more issues"). Gate:
development of the test room only; no final gameplay acceptance is inferred. Prerequisites: T221
(battle styles). Build identity: local `main` after `b372f37`, package 0.1.2, Rust 1.99.0 Windows
MSVC. Source: the selected Steam KH2FM build, validated each run; the install was only read.

Private evidence: `%LOCALAPPDATA%/KH2FinalRewrite/research/issues-20261026/g/` (`l1.txt` every
instruction on obj+0xe18, `l2`..`l5` decompiles, `l3.txt` references, `l6.txt` obj+0xee8),
`command-deck-20261020/summon/friend/spec/friend-spec.md` §2.5 and
`playable-room-20261008/party-01/{donald,goofy}-spec.md` (action tables); unattended run `style2`
in the private `kh2-issues-out/`.

## What the executable does

1. **The leash flag (obj+0xe18 bit 6) is read in two places.** `FUN_1401b4d20` ("a target to act
   on") is false while the flag stands, else true when the target (+0xbf8) is valid and alive;
   it gates the action entries that need a target (`FUN_1401b2880`, entry flag bit 1), the hold
   and Goofy's run acts (`FUN_1401ccba0`, `FUN_1401ce8f0`) and `FUN_1401b2c90`. The style think
   `FUN_1401b3340` (vtable slot at 0x142b976d0) returns 1 at once while the flag stands, before
   its battle-state dispatch (0x140723a90 / vt+0x158). So while the leash flag stands the member
   acts as if it had no target and follows Sora; the room's assumption holds.
2. **Party Attack's keep test** `FUN_1401b33f0` first drops the target (returns 1) when the
   current action entry (obj+0xee8) exists and its flag bit 0 is clear. In Donald's and Goofy's
   tables bit 0 is set only on the wait commands (0x201: hold, follow; Goofy's run); attacks and
   spells (2), Cure (0x10) and the items (0x18, 0x28) have it clear. The entry is cleared when
   nothing is chosen.

## Rewrite

- `party_style::Targeting` and `Field` document the readers instead of the inference.
- `Follower::acted`: true after an action or item entry is chosen, false after a wait command or
  with no target; `Field::acted` feeds it to Party Attack's keep test, which drops the target
  then (the pick runs again the same tick).

## Checks

| Check | Result |
| --- | --- |
| `cargo fmt --all --check`, `cargo clippy --all-targets --locked -- -D warnings` | PASS |
| `cargo test --locked` | PASS (new `party_attack_picks_again_after_an_action_entry`) |
| Release `--autoplay --party-styles 3:3` (private `style2`) | exit 0; both members take the Shadow in battle and leave it out of battle (44 target events); 31 party hits land |
| Release `--enemy-combat-tour` with Party styles | PASS (the tour's own verdict) |

## Uncertainty

- With one enemy in reach the re-pick takes the same enemy, so the change shows only with
  several enemies; no run checked which enemy is taken then.
- The exact ticks the entry pointer is cleared (the rest rule, `FUN_1401b1d40` step 3) follow the
  port's wait / no-target paths, not every executable branch.
- Not compared with the installed game.

Verdict: PASS for issue #146's two items (both traced and in the room); PARTIAL against the
installed game.
Next: compare Party Attack with several enemies against the original.
