# Contributing

This repository rewrites Kingdom Hearts II Final Mix for Windows, reading the owner's own Steam installation at runtime. Read [AGENTS.md](AGENTS.md) for the rules, [ARCHITECTURE.md](ARCHITECTURE.md) for where things are, and [docs/fidelity-contract.md](docs/fidelity-contract.md) for what counts as established.

## Setup

- Windows. The Rust toolchain is pinned in `rust-toolchain.toml` (Rust 1.99.0); rustup installs it on first use.
- Put the Cargo bin directory (`%USERPROFILE%\.cargo\bin`) on `PATH`.
- Running the game needs your own legitimate Steam install of KH2 Final Mix. Nothing from it goes into the repository.

## Local builds and disk use

Use `build-release.bat` (or `powershell -NoProfile -File tools/Build-Launcher.ps1`)
for an optimized KH2FM launcher with its model/image viewers. The batch scripts
are `build-release`, `build-release-skate3`, `build-debug` and
`build-debug-skate3`; the `-skate3` ones enable the optional `skate3` feature,
as does `-Skate3` on the PowerShell helper. This selects three binaries instead
of every research probe. The `build-debug` scripts use the
faster development profile; add `-LauncherOnly` when only the launcher changed
and the viewers are already available. The helper prints the launcher path;
add `-Timings` to write a Cargo report under `target/cargo-timings/`.

Development and test builds keep workspace file/line backtraces but omit full
type/variable symbols and dependency symbols. Incremental compilation stays on
for quick edits. For workspace variable/type debugging, use
`cargo build -p main --bin KH2RW --profile dev-symbols --locked`;
its larger cache lives in `target/dev-symbols/`. Third-party debugging can opt
in with `--config 'profile.dev-symbols.package."*".debug=2'` as well.

Windows builds use Rust's bundled `rust-lld.exe`; the Windows SDK/MSVC libraries
are still required. No native-CPU flags or floating-point rule changes are used.
Release optimization settings are unchanged. Build dependencies once, then keep
the cache: deleting it before each build makes the next build cold again.
Use one build invocation at a time when measuring speed; simultaneous release
and development builds compete for CPU and memory even with separate caches.

Test-free CLI adapters omit empty unit-test/benchmark harnesses. Their code is
still compiled by workspace builds and all-target Clippy. If you add tests to
one of these adapters, remove its `test = false` entry in the crate manifest.
The libraries, launcher and model inspector retain their actual tests.

The generated-data CPU benchmark for animated mesh uploads is
`cargo run -p main --example mesh_update_bench --release --features skate3 --locked`.
Use the same feature set as your regular build to reuse dependency artifacts.
It compares vertex-buffer replacement with allocation reuse and checks exact
packed vertex bytes. It does not measure whole-game FPS.

To reclaim obsolete **development/test build outputs only**, close any running
debug programs and run `cargo clean --profile dev --locked`. This removes
`target/debug/`, including its executables; it preserves `target/release/`.
Do not use a full `cargo clean` when you want to keep the release launcher.
Symbols, compiled libraries and incremental data are disposable compiler output;
gameplay data, source manifests and saves belong outside the checkout.

A complete `cargo clean --locked` also removes release outputs, all dependency
caches and timing reports. For a minimal runnable checkout, first copy the
launcher and its two viewers outside the checkout and verify the copies, close
running programs, clean, then restore those three executables to `target/release/`.
This frees the most space but the next compilation must rebuild dependencies.
Keep `.local` settings and private data separate from compiler cleanup.

## Checks

Run these before each commit. Each must pass.

```powershell
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo build --workspace --locked
cargo run -p main --locked -- --frames 180
```

The last command is the smoke run. Report a check you did not run as NOT RUN, not as passing.

## Finding and naming files

Use the [folder guide](docs/folder-guide.md) to choose a home for an edit and the
[architecture](ARCHITECTURE.md) to check boundaries. Prefer familiar nouns and
descriptive snake_case filenames. Update all references and current guides with
moves. Older records retain the names used at the time; the guide maps these.

## Making a change

- A nontrivial change gets a task record in `docs/tasks/`. The required fields are listed in AGENTS.md: ID, gate, owner, prerequisites, build and source identity, steps, evidence locations, checks, uncertainty, verdict and next task.
- Verdicts are PASS, FAIL, NOT RUN or BLOCKED. An unknown or skipped check does not pass a gate.
- Add a crate only when a tested behaviour needs the boundary. Do not add empty subsystems.
- Keep reading and decoding (`data`) separate from simulation (`player`, `effect`) and from presentation (`visual`). Renderer, window and input must not change progression.
- Translated game routines note the original function address in a comment or in the task record.

## Keeping private data out of Git

Never commit game data: assets, executable extracts, captures, saves, decompiled output, personal paths or secrets. Tests use generated synthetic bytes.

Private output goes in `.local/` (ignored) or outside the checkout. The default-deny `.gitignore` allows only listed paths. A new allowlist entry needs a provenance review before it is added. Check `git status` before staging, and check the file contents if anything looks like it could reveal a secret.
