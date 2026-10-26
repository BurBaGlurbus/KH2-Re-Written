# T248 — An equipped staff or shield is the one Donald or Goofy holds (issue #51, part)

Owner Claude, 2026-10-26, requested by burba in the project chat ("continue with more
issues"). Gate: development of the test room only; no final gameplay acceptance is inferred.
Prerequisites: T247. Build identity: local `main` after `b7482f7`, package 0.1.2, Rust 1.99.0
Windows MSVC. Source: the selected Steam KH2FM build, validated each run; the install was only
read.

Evidence: `data::weapon_entry` (the WENT reader, T132's trace of `FUN_1403ef8b0`); unattended run
`camp6` in the private `kh2-issues-out/`.

## What the executable does

`FUN_1403ef8b0(set, item)` gives the weapon object of an item: entry `item.amount` (item record
+4) of the WENT set the member object's neo-moveset (objentry +0x4e) selects. The member holds
that object.

## What the room did

Each member was built once with the set's first object (the base staff or shield), whatever its
record held.

## Rewrite

- `party_room::load` takes an optional weapon object; a weapon object with no motion set of its
  own plays the base weapon's ([I]: the weapon objects' entries name no set).
- When the camp closes with a new weapon in a member's record, `start_reweapon` resolves the
  object (`weapon_object`, FUN_1403ef8b0) and rebuilds the member's kind on a thread from the
  load-time sources (`ReloadSource`); `swap_weapons` puts the new kind in place, keeps the voices
  and spawns the new weapon meshes. A record with no weapon goes back to the base object.
- `--party-camp-tour` gives Donald the second staff in his list and captures the room after the
  camp.

## Checks

| Check | Result |
| --- | --- |
| `cargo fmt --all --check` | PASS |
| `cargo clippy --all-targets --locked -- -D warnings` | PASS |
| `cargo test --locked` | PASS |
| `cargo build --locked`, smoke `--frames 180` | PASS |
| Unattended release `--party-camp-tour` with the installed source (private `camp6`) | PASS: Donald leaves the camp with item 148, which resolves to `W_EX020_10`; `party_weapon` logs the swap 26 ticks after the camp closes (6 weapon parts); the capture after the camp shows him holding the hammer-headed staff instead of the hat-topped base staff |

## Uncertainty

- [I] The staves and shields borrow the base weapon's motion set; the weapon objects' entries
  name none, and how the executable picks one was not traced.
- The swap happens a few ticks after the camp closes (the rebuild runs on a thread); the
  executable's timing is not traced.
- Not built: the weapon's ability (Thunder Boost and others) in battle.
- Not compared with the installed game.

Verdict: PARTIAL for issue #51: the model and the battle numbers follow the equipment; the
weapon ability remains.
Next: the equipped weapon's ability in the member's ability list.
