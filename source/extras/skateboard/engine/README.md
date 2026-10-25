# Skate 3 skateboard (optional)

These two crates are **GPL-3.0-only** (see `LICENSE`), unlike the rest of this repository (Apache-2.0). They come from [SK8-ENGINE/skate-3-rust-engine](https://github.com/SK8-ENGINE/skate-3-rust-engine) at `2e16697`:

- `skate-sim`: skate-game's skater simulation (physics, animation graphs, input, gameplay camera numbers, the scoring HUD player and the skater's sound path) without rendering, UI, replay, modding, networking or SDL3, on Bevy 0.19.1. Adds host collision worlds and the `Sim` entry point (`src/host.rs`). Upstream unit tests need skate-game's private captures and were not carried over; the 52 that remain are `#[ignore]`d and not built (`test = false`).
- `skate-disc`: the asset setup in Rust (EB BIG v3, chunkref, RefPack, AttribSys VLT, ABIN physics skeletons, the trick-display HUD, the skater's sounds), reading the user's own disc. Sounds are decoded by vgmstream r2117, downloaded on first use and checked against its pinned SHA-256.

`skate-core`, `skate-data`, `skate-dynamics`, `skate-audio` and `skate-audio-fma` are pinned git dependencies of the same commit.

They are built only with the `skate3` feature:

```
build.bat
```

(or `cargo build --release -p main --bin KH2RW --features skate3`). A launcher built with it is a GPL-3.0 combined work. Without it, J says skateboarding is not installed. No Skate 3 data is in the repository; riding needs the user's own Skate 3 Xbox 360 ISO, set in the launcher. See `docs/tasks/T101-skate3-board.md` and `docs/tasks/T103-skate3-riding.md`.
