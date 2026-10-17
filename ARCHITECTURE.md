# Architecture

This is a Windows Rust/Bevy rewrite of Kingdom Hearts II Final Mix. It reads the
selected Steam installation without modifying it. Game files, manifests, saves,
captures and research output stay outside Git. Scope and acceptance remain in the
[fidelity contract](docs/fidelity-contract.md), [milestones](docs/milestones.md)
and [unknowns](docs/unknowns.md).

## Folder map

Start with the [folder guide](docs/folder-guide.md) for examples of common edits,
a glossary and the old-to-new name table.

Game rules, presentation, the launcher and the data readers that feed them sit
together under `source/`. `validation/` stays separate: it checks the selected
installation, and every gameplay session runs it first. The effect crate lives
inside the visual folder because it draws what the game produces.

| Folder | What belongs here | Cargo package |
|---|---|---|
| `source/main/` | Launcher, application startup, playable scene assembly and developer programs | `main` |
| `source/game/player/` | Player movement, abilities, combat, forms, targeting and current enemy controllers | `player` |
| `source/game/area/` | Area setup, spawn activation, exits and progress | `area` |
| `source/game/collision/` | Movement against room geometry | `collision` |
| `source/game/event/` | Cutscene timeline and camera state | `event` |
| `source/visual/` | Rendering, menus, HUD, controller input and scene presentation | `visual` |
| `source/visual/effect/` | Particle and model effect execution | `effect` |
| `source/audio/` | Mixing and Windows sound output | `audio` |
| `source/data/` | Bounded readers for game packages, models, motion, images, text and sound | `data` |
| `validation/` | Selected-install validation, hashing and manifest tools | `validation` |
| `source/extras/` | Existing optional skateboard extension and its license boundary | See its README |
| `tools/` | Build helpers and research/verification scripts | None |
| `docs/` | Guides, evidence, scope, roadmap and acceptance records | None |

`Cargo.toml` at the root lists the workspace members. Each package has its own
`Cargo.toml` and Rust source under `src/`. These Rust conventions are explained in
the folder guide. New folders are added when working code needs them.

## Boundaries

Data moves from selected-source validation (`validation`) through decoding (`data`)
into game/effect state and presentation. Presentation reads the state; game rules
must not depend on Bevy or the window. `audio` supplies output rather than game
rules. Every gameplay and installed-data inspection session still validates the
selected files. Folder names do not qualify a supported build.

`source/main/src/main.rs` starts the KH2RW launcher. `source/main/src/launcher/` checks the game
folder and prepares the launch configurations. `source/main/src/test_room.rs` assembles
the development room; `source/main/src/area.rs` and `cutscene/` connect state to scenes.
`source/main/src/bin/` holds the smoke window and research viewers/probes. The default
`cargo run -p main` program is the source-free smoke window; the player launcher
is selected with `--bin KH2RW` or built with `build-release.bat`.

`source/visual/src/lib.rs` lists the shared presentation modules. The pause menu is in
`source/visual/src/pause_menu.rs` and `pause_menu/`. The title state and title banner are
`source/visual/src/title.rs` and `title_banner.rs`; `source/main/src/title.rs` assembles the title
application. `source/visual/src/overlay.rs` and `overlay.wgsl` compose the 2D display.

The optional Skate 3 code retains its upstream package names and GPL notices in
`source/extras/skateboard/engine/`. It is existing extension code, not a roadmap for
rewriting another collection game. The `skate3` feature remains opt-in for Cargo;
the `build-*-skate3.bat` scripts enable it.

## Current limitations

This layout preserves behavior and existing unfinished boundaries. The visual
package still contains menu/title state and input adapters. The main package
still assembles gameplay and rendering in the development room, and current enemy
controllers still share the player package. Future behavior work should extract
state where it needs independent verification. These are not newly completed
simulation or gameplay gates. A source-bound transactional cache is still planned.

## Private local files

`target/` contains disposable compiler outputs. `.local/` is ignored development
storage; historical local test-room files are retained during this refactor.
Source data, manifests, saves, captures, disassembly, logs and backups belong in
private storage outside the checkout. Do not create tracked asset or mod-data
folders containing installed game content. The default-deny `.gitignore` admits
only maintained code, documentation, authored fixtures and explicitly reviewed
project images.

For build commands and required checks, see [CONTRIBUTING.md](CONTRIBUTING.md).
For naming and placement rules, see [the folder guide](docs/folder-guide.md).
