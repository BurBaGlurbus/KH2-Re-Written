# Application and launcher

`src/main.rs` starts KH2RW. `src/launcher/` checks the game folder and prepares
configurations. `src/test_room/`, `area.rs` and `cutscene/` assemble scenes.
`src/bin/` contains the smoke window and developer probes. The model and image
viewers (`model_inspector.rs`, `image_inspector.rs`) run inside the launcher.

Launch: `cargo run -p main --bin KH2RW --locked`. `build.bat` names the
built launcher `KH2RW Launcher.exe`; it is the only executable the build leaves.
Smoke: `cargo run -p main --locked -- --frames 180`.

See the [folder guide](../../docs/folder-guide.md) and [architecture](../../ARCHITECTURE.md).
