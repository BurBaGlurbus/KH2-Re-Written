# Credits

KH2FinalRewrite is an unofficial fan project. Kingdom Hearts is a trademark of Square Enix and Disney. This repository contains no game files, textures, models, sounds or saves. The program reads them from the user's own copy of the game.

The project's own code and documents are Apache-2.0 (see [LICENSE](LICENSE)). The legal notices and pinned source details are in [NOTICE](NOTICE). This file is the readable summary.

## Projects whose code or formats we follow

| Project | Licence | How we use it |
|---|---|---|
| [OpenKH](https://git.openkh.dev/) (Open Kingdom Hearts Research and Development) | Apache-2.0 | Adapted into our package, model, message, motion and font readers. Pinned at commit `7a3b945c538d32c6a285128c98aefba093f52ceb`. Changes are listed in NOTICE. |
| [kh2-combat-kit](https://github.com/steferro78-HIV/kh2-combat-kit) by steferro78 | MIT | Reviewed for the combat rules. We used it only as a cross-check for the variable-height jump and the Square-button moves (T105). No code or data was copied. |
| [kingdom-hearts-x-elden-ring](https://github.com/StrawHatPro/kingdom-hearts-x-elden-ring) by StrawHatPro | MIT (notice below) | Used for the LAN duel's KH3 mode (T279, T282). The file formats (paks, packages, meshes, animations, textures, SEAD banks, HCA), the behaviour of KH3's Sora and the Cascade effect evaluator (`source/visual/effect/src/cascade`, ported from its `crates/kh-vfx`), KH3's Sora (`source/game/kh3_sora`, from `crates/kh-core`), the KineDriver and Bonamik solvers (`source/game/kh3_rig`, from `crates/kh-anim`) and the auto SE detector (`kh3_autose.rs`, from `crates/kh-audio`) come from it, and its JavaScript readers were reimplemented in Rust (`source/data/src/kh3`). Its generated tables and anything embedding KH3 data were not copied. Nothing from KH3 is in this repository: the KH3 mode reads the player's own install, and needs it. |
| .NET runtime | MIT | Math ordering for the bind and coordinate profiles (NOTICE; licence text in `docs/release/dotnet-runtime-LICENSE.txt`). |
| bevy_mikktspace | Zlib, and MIT or Apache-2.0 | Tangent generation for native mesh inspection (licence texts in `docs/release/`). |
| zlib-rs | Zlib | Pulled in through the image codec feature (licence text in `docs/release/zlib-rs-LICENSE.txt`). |
| DirectXTex (Microsoft) | MIT | Used only as a private reference for checking pixel formats. Nothing from it is in this repository or the distributed program. |
| MapStudio, EgsTools, msetDoc (kenjiuno) | Public documentation and rules | Used as references for file layouts and naming rules. No code copied. |

### kingdom-hearts-x-elden-ring licence

```text
MIT License

Copyright (c) 2026 StrawHatPro

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

## Skate 3 (optional feature, GPL-3.0)

The `extras/skateboard/engine` folder holds two crates from [SK8-ENGINE/skate-3-rust-engine](https://github.com/SK8-ENGINE/skate-3-rust-engine) at commit `2e16697`. They are licensed **GPL-3.0-only**, unlike the rest of this repository. A `KH2RW Launcher.exe` built with `--features skate3` is therefore a GPL-3.0 combined work. Without that feature the program does not include them. See [extras/skateboard/engine/README.md](source/extras/skateboard/engine/README.md).

## Rust libraries

The program is built on many Rust crates, including Bevy, wgpu, image, png, flate2, lewton, fontdue, serde and sha2. Each one keeps its own licence, mostly MIT or Apache-2.0. The complete list, with versions and licence labels, is in [docs/release/dependency-inventory.csv](docs/release/dependency-inventory.csv). The labels there are metadata, not full licence texts. Release packaging still needs the full licence texts for every dependency.
