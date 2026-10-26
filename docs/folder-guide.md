# Finding and adding files

Start with the part of the game you want to change. These folders name what the
code does, so you do not need the task history to find it. This guide covers
editing the rewrite; a stable public mod API and mod package format are not yet
established.

## Choose a folder

| I want to change... | Start here |
|---|---|
| Movement, jumping or attacks | [source/game/player/src/lib.rs](../source/game/player/src/lib.rs), [combat.rs](../source/game/player/src/combat.rs) |
| Abilities, growth, magic or forms | `source/game/player/src/abilities.rs`, `growth.rs`, `magic.rs`, `drive.rs`, `forms.rs` |
| Shadow, Dusk or Soldier behavior | `source/game/player/src/shadow.rs`, `dusk.rs`, `soldier.rs` |
| Room setup, exits or progress | [source/game/area/src/](../source/game/area/src/) |
| Walls and floors | [source/game/collision/src/lib.rs](../source/game/collision/src/lib.rs) |
| Cutscene timing and camera state | [source/game/event/src/](../source/game/event/src/) |
| The launcher window | [source/main/src/launcher.rs](../source/main/src/launcher.rs), [launcher/](../source/main/src/launcher/) |
| The test room or debug menu | [source/main/src/test_room.rs](../source/main/src/test_room.rs), [debug_menu.rs](../source/main/src/test_room/debug_menu.rs) |
| Pause menu, equipment or journal | [source/visual/src/pause_menu/](../source/visual/src/pause_menu/), [pause_menu.rs](../source/visual/src/pause_menu.rs) |
| HUD, gauges or level-up panels | `source/visual/src/hud.rs`, `gauge.rs`, `level_up.rs` |
| Rooms, scenery or 2D drawing | `source/visual/src/room.rs`, `scenery.rs`, `overlay.rs` |
| Keyboard/controller mapping | [source/visual/src/input.rs](../source/visual/src/input.rs), [controller.rs](../source/visual/src/controller.rs) |
| Particle/model effects | [source/visual/effect/src/](../source/visual/effect/src/) |
| Spell effects and the magic tour | `source/main/src/test_room/spell_fx.rs`, `magic_room.rs`, `magic_tour.rs` |
| Mixing and Windows sound output | [source/audio/src/](../source/audio/src/) |
| LAN play: UDP session, discovery, link simulator | [source/net/src/](../source/net/src/); the duel in the room: `source/main/src/test_room/duel_*.rs`, `rival.rs` |
| Which sounds a scene requests | `source/visual/src/sound.rs`, `footsteps.rs` |
| Reading a game file format | [source/data/src/](../source/data/src/); descriptive model, motion, room and table prefixes |
| Supported-source checks | [validation/src/lib.rs](../validation/src/lib.rs) |
| Model/image viewers | [source/main/src/model_inspector.rs](../source/main/src/model_inspector.rs), [image_inspector.rs](../source/main/src/image_inspector.rs) (run inside the launcher) |
| Motion probes | [source/main/src/bin/](../source/main/src/bin/) |
| Build/research scripts | [tools/README.md](../tools/README.md) |
| What has been verified | [tasks/README.md](tasks/README.md), [progress/README.md](progress/README.md) |

## A few Rust words

- A **package** is a buildable unit. Its `Cargo.toml` lists its name and dependencies.
- `src/` means source code. Rust files end in `.rs`; `.wgsl` files are graphics shaders.
- `lib.rs` lists shared modules. A **module** is a named part of the code.
- `main.rs` starts a program. Additional programs live in `src/bin/`.
- Tests usually live beside the code they check or in a nearby `tests.rs`.
- **HUD** means the display over the game, such as health and command menus.

Readers sometimes keep the original format name for research: BAR is a container,
ARD describes areas, ANB contains motion, PTYA contains player action parameters,
SCD holds sound and IMGD holds images. These are formats, not bundled game assets.

## Names for future work

Use familiar nouns: `player`, `enemy`, `camera`, `menu`, `effect`, `audio`.
Use lowercase folders and lowercase filenames with underscores between words:
`pause_menu`, `new_game.rs`, `level_up.rs`. Name the feature or purpose rather
than its implementation technique. Avoid owned package suffixes `_sim`, `_vm`,
`_out`, unexplained initials and catch-all `misc` or `utils` files. Established
format names and upstream package names may stay for research and attribution.

Keep rules in `source/game/`, presentation in `source/visual/`, effect execution in `source/visual/effect/`,
application wiring in `source/main/`, decoding in `source/data/`, validation in `validation/`, and
mixing/output in `source/audio/`. An effect-file reader belongs in `source/data/`, running that
effect belongs in `source/visual/effect/`, and drawing its output belongs in `source/visual/`.

Add a subfolder when several files form a clear feature. Add a package only when
working code needs an independently checked boundary. Do not scaffold empty
folders for future worlds or systems. Existing unfinished boundaries are listed
in [ARCHITECTURE.md](../ARCHITECTURE.md).

Move module declarations, imports, Cargo paths, asset/shader references, scripts,
ignore rules and current guides together. Run [the checks](../CONTRIBUTING.md#checks)
and record nontrivial work in `docs/tasks/`. Keep game data, manifests, saves and
captures in private storage outside the checkout.

## Names used by older records

Records through T128 retain the paths and commands actually used at the time.
Use these current names after `cargo ... -p`:

| Earlier name/path | Current package/path |
|---|---|
| `crates/windows_shell` | Programs: `source/main/` (`main`); shared display code: `source/visual/` (`visual`) |
| `crates/windows_shell/src/bin/kh2fm` | `source/main/src/`; launcher binary remains `KH2RW` |
| `windows_shell` smoke binary | `smoke` in package `main` (default run target) |
| `crates/player_sim` | `source/game/player` (`player`) |
| `crates/area_sim` | `source/game/area` (`area`) |
| `crates/field_collision` | `source/game/collision` (`collision`) |
| `crates/event_sim` | `source/game/event` (`event`) |
| `crates/effect_vm` | `effect` (`effect`) |
| `crates/audio_out` | `audio` (`audio`) |
| `crates/package_reader` | `data` (`data`) |
| `crates/source_identity` | `validation` (`validation`), scanner executable `validation.exe` |
| `main/`, `visual/`, `game/` (before T130) | `source/main/`, `source/visual/`, `source/game/` |
| `source/` validator folder (before T130) | `validation/` (package and executable `validation`) |
| `crates/skate_source` | `source/extras/skateboard/source` (package name unchanged) |
| `crates/skate3` | `source/extras/skateboard/engine` (upstream package names unchanged) |

File renames (T130, same pattern as the display renames): `data/src/effect.rs` to
`object_effect.rs`, `named.rs` to `path_lookup.rs`, `original.rs` to
`original_payload.rs`, `plain.rs` to `unencrypted_payload.rs`, `replacement.rs` to
`replacement_payload.rs`, `sequence.rs` to `sprite_sequence.rs`, `layout.rs` to
`menu_layout.rs`, `event.rs` to `event_script.rs`, `preferences.rs` to `party_records.rs`,
`probe.rs` to `probe_source.rs`; `visual` `command.rs` to `field_command_menu.rs`,
`layout.rs` to `menu_screen.rs`, `sprite.rs` to `sprite_player.rs`, `fog.rs` to
`area_fog.rs`; `main` `test_room/kit.rs` to `character_kit.rs`; player `physics.rs` to
`actor_physics.rs`, `party.rs` to `party_members.rs`, `prize.rs` to `enemy_prizes.rs`,
`wisdom.rs` to `wisdom_form.rs`, `limitform.rs` to `limit_form.rs`, `camera.rs` to
`follow_camera.rs`; event `camera.rs` to `event_camera.rs`. Format and hardware names
(BAR, ARD, PTYA, SCD, OGG, VIF, VPU, TEXA, OBJENTRY) are kept as file names.

Display module renames: `camp` to `pause_menu`, `pad` to `controller`, `bobs` to
`scenery`, `footstep` to `footsteps`, `newgame` to `new_game`, `levelup` to `level_up`,
`title_edits` to `title_banner`, `layout2d` to `layout`, `sprite2d` to `sprite`,
and `ui2d` to `overlay` (including its shader).

The old `cargo run -p windows_shell --locked -- --frames 180` is now
`cargo run -p main --locked -- --frames 180`. Launch with
`cargo run -p main --bin KH2RW --locked`. Inspector and probe binary names remain
the same. Source manifests, private launch configurations and saves are unchanged.
