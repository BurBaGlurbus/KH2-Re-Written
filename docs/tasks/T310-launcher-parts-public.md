# T310 — The launcher's parts public for Kingdom Hearts: Connected

Owner Claude, 2026-10-30, requested by burba in the project thread. Gate: none of the roadmap's; single player must not change. Prerequisite: T309. Source: none read; this changes visibility only.

Kingdom Hearts: Connected now takes this repository's crates as a pinned library instead of holding copies (its record KHC-003). Its launcher had copies of the parts below, so they are public here and it uses them.

## What changed

Only `pub` was added, in `source/main/src/launcher*`; no logic, data or test changed:

- `launcher.rs`: `pub mod install`, `pub mod paint`; the palette and layout constants; `Job`, `Running` and their fields; `plain`, `wrap`, `start_check`, `spawn`, `failure_lines`, `triangle`, `check_screen`.
- `launcher/install.rs`: `SETTINGS`, `CONFIG_LIMIT`, `read_limited`, `steam_libraries`, `LaunchFile` and its fields, `launch_file`, `config_identity`, `made_config`.
- `launcher/paint.rs`: `quad`.

## Checks

| Check | Verdict |
|---|---|
| `cargo fmt --all --check` | PASS |
| `cargo clippy --all-targets --locked -- -D warnings` | PASS |
| `cargo test --locked` | PASS |
| `cargo run -p main --locked -- --frames 180` | PASS: exit 0 |
| The diff adds `pub` and nothing else | PASS: 46 lines changed in the three files, each the same line with `pub` added (`steam_libraries` went from `pub(super)` to `pub`) |
| Unattended room runs against a baseline | NOT RUN: the change adds `pub` only, so the compiled behaviour has nothing new to compare |

The folder is shared: clippy, the tests and the smoke run compiled the working tree, which also held another thread's uncommitted edits to `lens_probe` and `refraction` in `source/visual`. They are not in this commit. Kingdom Hearts: Connected's build from this commit (its record KHC-003) covers the crates it takes without those edits.

## Verdict

PASS for visibility only. No gameplay gate moves.

## Next

None here. Kingdom Hearts: Connected's launcher still has its own copy of the list and input code that takes its own `Launcher` type (`Item`, `browse_items`, `list`, `read_input`, `move_cursor`, `keep_visible`); sharing those would need this launcher made generic, which is a refactor, not a visibility change.
