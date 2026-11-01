# T321 — The Port Royal medals; a room of 32 actors

Owner Claude, 2026-10-31, requested by burba in the project thread (issue #163). Gate: development of enemy AI in the test
room; no final gate is inferred. Prerequisites: T320. Build identity: local `main` after `43408e9` plus this change, package
0.1.2, release launcher with the `skate3` feature. Source: the selected Steam KH2FM build, validated each run; the install was
only read. Ghidra 12.1.4 headless on private read-only project copies (plain Ghidra, no MCP).

Private evidence (outside the checkout): `%LOCALAPPDATA%\KH2FinalRewrite\research\enemy-1to1-20261027\g\medal`, and the run
`t12` in `runs-20261031`.

## What was wrong

- The Grim Reaper's and his chest's medal natives only reported themselves, so no medal ever left an object.
- The room held 16 actors. The Lords' fight, with its eight pillars, refused the Lords' fire globes (`enemy_summon_rejected`).

## What the executable does

- FUN_14040b5a0 (`obj_scatter_medal`): up to `n` of the object's +0xd1c leave it as prizes on the list at DAT_142ae9e80 +0xbf8,
  falling with FUN_14040b8b0.
- FUN_14040bd40 (`prize_vacuum_ca`): up to `n` of `from`'s leave it homing on `to` with FUN_14040b9e0; it answers true when `from`
  ran out. FUN_14040bde0 (`prize_vacuum_range_ca`): the free ones within range horizontally home on `to`.
- FUN_14040b9e0: speed 20 toward the object, keeping 0.1 of the velocity; within 100 it adds 1 to the object's +0xd1c.
- FUN_14040b6a0 (`prize_num_ca`): the list's count.

## Changes

- `traps::medal`, new: 1:265, 1:266, 1:267, 1:339, 1:340, 1:341. The list and Sora's count are in the store fighters share.
- `boss::medal_tick`: falling medals use the ordinary prize's fall and Sora collects them; flying medals home and count.
- `drive_fx` draws medals with the prize billboards, PRIZE group 11.
- `enemy_gallery::MAX_ACTORS` 32.

## Checks

| Check | Result |
| --- | --- |
| `cargo fmt --all --check`, `cargo clippy --all-targets --locked -- -D warnings`, `cargo test --locked` | PASS |
| Synthetic tests: scatter, vacuum and vacuum range | PASS |
| Grim Reaper tour (`t12`) | Runs with no fault. **No medal was seen moving:** the Reaper scatters on magic hits and the tour only attacks, so he stays at full HP. The medal flow is NOT RUN in a fight |
| Medal billboards on screen | NOT RUN |
| Side by side with the installed game | NOT RUN |

## Uncertainty

- [I] Group 11 comes from the third word of DAT_1405ce188, read by analogy with the ordinary prizes' table.
- [I] Sora collects medals by the ordinary prizes' rule. FUN_14040b8b0 hands a landed medal to the player's vt+0x160, which is
  not traced.
- CA01_MS204's OBJ_DAMAGE, which scatters a quarter of Sora's medals on attack row 1073, is not fired. FUN_14040bb00 and
  FUN_14040b6c0, the other two ways medals appear, are not ported.
- The fight stays unwinnable in a tour until something casts magic at him.

## Verdict

PARTIAL. The natives are translated; the fight is not shown to work.

## Next

A tour that casts magic in the Reaper's fight, the mission's OBJ_DAMAGE, and the boss sweep's results.
