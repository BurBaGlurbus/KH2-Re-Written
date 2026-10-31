# Credits

KH2FinalRewrite is an unofficial fan project. Kingdom Hearts is a trademark of Square Enix and Disney. This repository contains no game files, textures, models, sounds or saves. The program reads them from the user's own copy of the game.

The project's own code and documents are Apache-2.0 (see [LICENSE](LICENSE)). The legal notices and pinned source details are in [NOTICE](NOTICE). This file is the readable summary.

## Projects whose code or formats we follow

| Project | Licence | How we use it |
|---|---|---|
| [OpenKH](https://git.openkh.dev/) (Open Kingdom Hearts Research and Development) | Apache-2.0 | Adapted into our package, model, message, motion and font readers. Pinned at commit `7a3b945c538d32c6a285128c98aefba093f52ceb`. Changes are listed in NOTICE. |
| [kh2-combat-kit](https://github.com/steferro78-HIV/kh2-combat-kit) by steferro78 | MIT | Reviewed for the combat rules. We used it only as a cross-check for the variable-height jump and the Square-button moves (T105). No code or data was copied. |
| .NET runtime | MIT | Math ordering for the bind and coordinate profiles (NOTICE; licence text in `docs/release/dotnet-runtime-LICENSE.txt`). |
| bevy_mikktspace | Zlib, and MIT or Apache-2.0 | Tangent generation for native mesh inspection (licence texts in `docs/release/`). |
| zlib-rs | Zlib | Pulled in through the image codec feature (licence text in `docs/release/zlib-rs-LICENSE.txt`). |
| DirectXTex (Microsoft) | MIT | Used only as a private reference for checking pixel formats. Nothing from it is in this repository or the distributed program. |
| MapStudio, EgsTools, msetDoc (kenjiuno) | Public documentation and rules | Used as references for file layouts and naming rules. No code copied. |

## Skate 3 (optional feature, GPL-3.0)

The `extras/skateboard/engine` folder holds two crates from [SK8-ENGINE/skate-3-rust-engine](https://github.com/SK8-ENGINE/skate-3-rust-engine) at commit `2e16697`. They are licensed **GPL-3.0-only**, unlike the rest of this repository. A `KH2RW Launcher.exe` built with `--features skate3` is therefore a GPL-3.0 combined work. Without that feature the program does not include them. See [extras/skateboard/engine/README.md](source/extras/skateboard/engine/README.md).

## Rust libraries

The program is built on many Rust crates, including Bevy, wgpu, image, png, flate2, lewton, fontdue, serde and sha2. Each one keeps its own licence, mostly MIT or Apache-2.0. The complete list, with versions and licence labels, is in [docs/release/dependency-inventory.csv](docs/release/dependency-inventory.csv). The labels there are metadata, not full licence texts. Release packaging still needs the full licence texts for every dependency.
