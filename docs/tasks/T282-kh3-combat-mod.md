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

## Checks so far

| Check | Result |
| --- | --- |
| Reader unit tests on synthetic bytes (27 at phase 2) | PASS |
| `kh3_probe` on the installed game: key found in the executable, 140 paks, 479 822 files, each index's SHA-1 checked, mounted in 0.5-1.2 s | PASS |
| `kh3_probe mesh` on Sora: 274 bones, 46 709 vertices, 12 sections, material slots named | PASS |
| `kh3_probe anim` on clip 0001: 3 s, 91 frames, 100 tracks | PASS |

## Verdict

PARTIAL (in progress).
