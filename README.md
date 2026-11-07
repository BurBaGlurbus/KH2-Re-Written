# Kingdom Hearts II Re:Written

<p align="center">
  <img src="source/visual/assets/kingdom-hearts-final-rewrite-logo.png" alt="Kingdom Hearts Final Re:WRITE logo" width="850">
</p>

A Rust and [Bevy](https://bevy.org) rewrite of Kingdom Hearts II Final Mix that runs on your own installed copy of the game.

This is an unofficial fan project.
It is not affiliated with or endorsed by Square Enix or Disney.
**No game files are in this repository, only code.**
To play it you need your own Steam copy of Kingdom Hearts HD 1.5 + 2.5 ReMIX, which the program reads at runtime.

> **Status: very early.** The worlds can be explored and it works as a sandbox, but most of the game is not finished yet.

## Getting started

1. Install [Rust](https://rustup.rs) and, on Windows, the Visual Studio C++ build tools.
   The toolchain version is pinned in `rust-toolchain.toml`, and rustup installs it on first use.
2. Run `build.bat` from this folder (on Linux, `./build.sh`; Linux is untested so far, see [docs/LINUX.md](docs/LINUX.md)).
3. Start the launcher:
   ```powershell
   & '.\target\release\KH2RW Launcher.exe'
   ```
   On first launch it finds your Steam install (or lets you browse to it), checks that it is a supported version, and then offers the title screen and the test room.

The keyboard, mouse and controller follow the PC version. The full list is in [docs/controls.md](docs/controls.md).

## More

- [Building, checks and contributing](CONTRIBUTING.md)
- [Folder guide](docs/folder-guide.md) and [architecture](ARCHITECTURE.md)
- [Progress report](docs/progress/README.md)

## Credits

Made by TheGlurbus ([@BurBaGlurbus](https://github.com/BurBaGlurbus)).
The game's file formats are read following [OpenKH](https://git.openkh.dev/).
The optional skateboard mode uses [SK8-ENGINE/skate-3-rust-engine](https://github.com/SK8-ENGINE/skate-3-rust-engine).
Everyone else whose work helped is in [CREDITS.md](CREDITS.md).

This is an AI-written project: the code is written with Claude Code under the owner's direction, and playtested by the owner.

## Disclaimer

Kingdom Hearts II Re:Written is an unofficial, non-commercial fan project.
Kingdom Hearts and all of its content, including characters, models, textures, animations, audio and trademarks, belong to Square Enix and Disney.
No game data is included in or distributed with this repository.

- You need your own legitimately purchased copy of the game.
- Do not commit, upload or share anything taken from your game files.
- No warranty is given; use it at your own risk.

If you are a rights holder and have a concern about this project, please open an issue.

## License

Apache-2.0, see [LICENSE](LICENSE). Third-party notices are in [NOTICE](NOTICE).
The skateboard crates in `source/extras/skateboard/engine/` are GPL-3.0, so a build with the `skate3` feature (the default `build.bat` build) is GPL-3.0 as a whole.
