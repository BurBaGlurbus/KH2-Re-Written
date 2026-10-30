# T282 — KH3 mode: KH3's combat from the player's own KH3 files, in the LAN duel

Owner Claude, 2026-10-30, requested by burba in the project thread: port KH3's combat into the KH2 multiplayer as a
mod, with the magic system, effects, sound effects, all attacks, abilities, flow motion, shotlock and animations
pulled from KH3's executable or pak files; the option requires the KH3 game files, and KH3 is selected from the
launcher's quick start. Gate: none of the roadmap's. An addition outside the original game, in the LAN duel only; no
fidelity claim is made and single player and the KH2 ruleset are unchanged. Prerequisites: T279-T281. Build identity:
local `main` after `244e1c3`, package 0.1.2, Rust 1.99.0 Windows MSVC. Sources: the selected Steam KH2FM build as
before; the player's KH3 install (Steam, `KINGDOM HEARTS III\KINGDOM HEARTS III`), only read.

**What replaces what.** T279's rule was that KH3 is never needed at run time and KH2 motions and effects stand in. This
task replaces that rule for the KH3 option only: it requires a KH3 install and reads KH3's real assets and data.

**Nothing from KH3 is in the repository.** No pak key, assets, extracted tables or decompiled pseudocode. Everything is
read at run time from the player's own install. The key is found in the player's executable and kept in memory; its
file offset (not the key) is remembered in the launcher's settings. Routines translated from the executable note their
addresses in comments, as the KH2 work does.

**Reference and licence.** The formats and the behaviour follow `StrawHatPro/kingdom-hearts-x-elden-ring` (MIT, credited
in `CREDITS.md`, `NOTICE`), cloned outside the checkout. Code is reimplemented or ported from it under MIT; its generated
tables, `sheets/port/*` and anything embedding KH3 data are not. Private digests and probes: `%USERPROFILE%\kh2-kh3-ref\`
(`spec\`, `probe\`, `scripts\`).

## Phase 1: the launcher

- `Settings::kh3_folder`, `kh3_key_offset`. At boot the set folder is checked (`data::kh3::admit`), or a Steam library's or
  Epic's KH3 is found and remembered.
- A *KINGDOM HEARTS III* row on the quick start guide, the test list and the Multiplayer screen opens a folder browser
  that marks the game's folder; a refused folder says why.
- *Test KH3 mode* is disabled until KH3 is set.
- The room admits the install itself each session (`Game::kh3_install`, `--kh3-folder` for tests).
- The lobby: the joiner reports its install's identity (`Intent::Kh3`); the host can pick the KH3 ruleset only when both
  have the same one, is told why not, and a saved KH3 rule falls back to KH2.
- No cache or conversion was needed: the paks mount in about a second (below).

## Phase 2: the readers (`source/data/src/kh3/`)

`pak` (footer, AES-256-ECB index checked by SHA-1, entries, zlib, mount order, key search), `package` (header tables),
`props` (tagged properties, DataTables), `skeletal` (reference skeleton, LOD 0), `anim` (compressed tracks and KH3's
sampling), `texture` (mips left compressed). `kh3_probe` is the research tool.

## Phase 3: Sora (`sora.rs`, `material.rs`, `kh3_model.rs`)

KH3's Sora and Kingdom Key from `p_ex001`: reference skeleton (274 bones, 194 weighted joints in the skin), LOD 0, all of
`p_ex001_AnimSet`'s rows (428) and clips (499) with blend spaces, each asset's blend-in time, KH3's saved-pose blending and
root motion. Both Soras of a duel use it; a joiner plays each from the host's row, serial and clock. Materials use one
generic shading path (base colour and tint, not KH3's material graphs).

## Phase 4: data (`kh3_book.rs`, `player::kh3::{attack, combo, ability}`)

Read at load: 71 attack clips' windows, traction, face-at, input and change notifies, root tracks and blade points; 756
`p_ex001_AttackData` rows; 18 hit shapes of `w_so_KeybladeBase`; level 99's stats (strength 66, magic 67, defense 59, HP 90
with the Kingdom Key). `start_attack`/`attack_tick` run KH3's kinds: flowmotion attacks, wall-run attacks, Risk Dodge
counters, Payback Strike and Final Blow included. The chain knows 18 more abilities; AP costs were compared with the
player's `TresAbilityData` (one was wrong and is fixed) and the build's AP is level 99's (180). Damage: KH3's formula with
Combo, Air Combo, Magic Combo, Counter and element boosts and Critical Counter, then scaled from KH3's HP to the duel's gauge
and through the duel's own damage rules. The shotlock camera uses `TresCameraShootFlow`'s constants.

## Phase 5: magic (`spell.rs`)

Fire, Blizzard, Thunder, Water, Aero and Cure with their tiers from `TresCmdData`, `p_ex001_MagicSet` and the projectile
Blueprints: stages, shapes, homing, delays, respawns; magic combos; Grand magic offered by `m_FormPoint`s, cast with Triangle.
KH2's command menu picks the spell: Magnet's slot is Aero, Reflect's slot is Water.

## Phase 6: effects and sounds (`effect_json.rs`, `cascade/`, `kh3_fx.rs`, `sead.rs`, `hca.rs`)

`crates/kh-vfx` is ported (MIT notice kept) and fed from the tagged properties: 151 systems, 385 materials, 149 meshes, 106
textures. Sounds are SEAD banks decoded with HCA tables found in the player's executable: 165 of 165 used. Effects and sounds
replay from the clips' notifies on each PC, so a swing's trail and sounds follow KH3's own timing. KH2's composite looks do not
play with the data present.

KH3 runs its GPU sprite emitters on the graphics card; this evaluator is on the CPU, and a shot's bolts reached 100 000
particles and a million mesh triangles, about 7 FPS. GPU emitters keep an even share up to 250 particles an emitter, and mesh
particles are bounded at 150 000 triangles a frame. Those emitters are therefore thinner than in KH3.

## Phase 7: network

The wire carries ids and numbers only: row, serial, clock, blend, actors by command, effect and sound indices. The lobby
compares both PCs' KH3 identity before KH3 can be picked; a saved KH3 rule falls back to KH2 with a note.

## Phase 8: the dummy

Test KH3 mode's dummy stays KH2's Shadow.

## From the executable (Ghidra, the analysed private project)

Checked by decompiling the installed build: `FUN_1405fd440` (spring 200, pitch limits +-70, start pitch -10),
`FUN_1414165e0` (the 50 cm offset), `FUN_1413325f0` (flow attack choice and its drop to an air attack), `FUN_141433a60`
(finisher count), the guard counter's kinds at `0x141437E28`. Other routines (the kind table, traction, the chain's rules,
damage) follow the reference's reading of the executable and were not all re-read. Output stays private
(`research\kh3-exe\out\t282-check`).

## What is 1:1 and what is not

Read from the player's KH3 files: model, skeleton, animations, blends, root motion, notifies; attack windows, shapes and rows;
level 99 stats; AP costs; spell data; particle systems; sounds.

Approximated or left out:

- Materials and lighting: one generic path, not KH3's material graphs.
- Effects: Cascade class defaults that no asset stores are partly assumed; GPU emitters thinned, mesh particles bounded;
  `ead_*` attach data not parsed; no `LoopEndSound`; no reverb or bus effects.
- `KineDriver` corrective bones are not evaluated (clips carry their baked helper tracks; skirt and sleeve roots stay at the
  reference pose).
- Footsteps: KH3's bank and bones, a simplified detection, one surface.
- Traversal moves (glide, wall run, poles, superslide) keep T279's layer with KH3's rows on top; their movement was not
  re-derived. Risk Dodge has the under-slide and warp only, not the over-slide or slide-turn.
- Falling Slash takes a target 250 cm below as on the ground. Copies allowed per ability are the duel's choice.
- Status effects (freeze, discharge), Aero's residue, the survival, MP and system abilities, Attractions, Formchanges, Links.
- Damage is scaled to the duel's gauge and rules, so a duel's numbers are not KH3's.

No claim of equivalence with KH3 is made; no controlled comparison against the running game was done.

## Checks

Build identity: local `main` at f69c1b4, f80c4c5, d252e7e, 69b1772, 1c1b559, d41f994 and this record, in a private tree of HEAD plus this task's files, since other threads'
uncommitted edits broke the shared checkout at times. KH3: Steam, 140 paks, identity `94939e202559...`.

| Check | Result |
| --- | --- |
| `cargo clippy --all-targets --locked -- -D warnings` (default members) | PASS |
| `cargo test --locked` (default members): 0 failed, 1 ignored | PASS |
| `cargo fmt --all --check` | FAIL: `source/visual/src/room.rs` only, from another thread's commit; this task's files pass |
| `kh3_probe`: key found in the executable, each index's SHA-1 checked, mounted in 0.5-1.2 s | PASS |
| Live room, Test KH3 mode, scripted: KH3 Sora, combos, flow attacks, spells, both shots, dummy damage from KH3's rows; contact sheets in `%USERPROFILE%\kh2-kh3-ref\runs\r06`, `r13` | PASS |
| Loopback LAN, release, two instances, scripted: lobby picks KH3, match, hits, KO, rematch; joiner shows KH3 Sora and effects (`runs\d02`) | PASS |
| Loopback LAN, joiner without KH3: host told, falls back to KH2 (`runs\d03`) | PASS |
| FPS, single instance, release, machine under load from other builds: 60 at rest, about 40-50 during a shot (was 6-8 before the budget); 1% lows 14-23 (`runs\r13`) | PASS with load noted |
| `tools\\Build-Launcher.ps1 -Skate3`, release: builds; only the launcher kept as the result to run | PASS |
| That launcher, Test KH3 mode and loopback LAN, scripted: same results, no panics (`runs\\r14`, `runs\\d04`); 35-46 FPS with the CPU at 100% from other builds, 1% low 17-20 | PASS with load noted |
| Launcher's Multiplayer screen, keys posted: KINGDOM HEARTS III row found and tagged, Test KH3 mode available (`runs\\launcher-b.png`) | PASS |
| The first-run quick start guide's KH3 row and the folder browser, on screen (this PC goes straight to the title; the row is in the code path) | NOT RUN |
| Two PCs | NOT RUN |
| Real pad or keyboard (scripted input only) | NOT RUN |
| Hearing the sounds (decoded with no bad frames; no one has listened) | NOT RUN |
| Visual comparison against KH3 itself | NOT RUN |
| `--workspace --features main/skate3` clippy and test (`source/extras/skateboard` untouched) | NOT RUN |

`docs/progress/coverage.json` is unchanged: this is an addition outside the original game.

## Uncertainty

Sound levels and pitches are unheard. Effects are only compared by eye with memory of the game. FPS was measured with other
builds running; a free machine should do better, a weaker GPU may not. A dev build's first frame (11-13 s) can pass the 5 s join
timeout; a release build did not.

## Verdict

PASS for what was run: the KH3 option works from the player's own files in Test KH3 mode and over loopback. Not a claim of
1:1 with KH3 as a whole; the list above stands. Next task: the owner's play test on two PCs with a pad, then the items above
in the order the owner finds them.
