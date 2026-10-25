# T235 — Effect pages keep each draw's address mode (Thunder's white boxes in play)

Owner Claude, 2026-10-25, requested by burba in the project chat after T228 and T229 were merged on local main ("I
want you to redo the fix you did for vfx because thunder and other various effects still have the white box effect").
Gate: development of the test room, world map and cutscene effect presenters; no final gameplay acceptance is
inferred. Prerequisites: T228, T229. Build identity: local `main` (T228 and T229 merged, at `423a5e2` when the fix
was built), package 0.1.2, Rust 1.99.0 Windows MSVC, `tools/Build-Launcher.ps1 -Skate3` release build. Source: the
selected Steam KH2FM build, validated each run; the install was only read.

Private evidence: runs, logs and captures in the private `kh2-vfx-out/t234` folder outside the checkout (`run1`–`run4`,
the probe runs `nomodels`, `noquads`, `ska`–`skc`, `k*`, `only*`, `dump*`, `bat`, `tag`, and the after runs
`after1`–`after4`, each with its `.jsonl` log, `.err` output and `cap/` frames); the probe build lived in a separate
private target folder and was never committed. Before / after window captures for the owner in the ignored
`.local/vfx-review/live/`.

## How it was reproduced

T228 and T229 were judged on `--magic-tour` captures. This time the game ran the way the owner plays: the release
launcher executable with the launcher's Play arguments (`room`, the combined test room config) and the owner's saved
settings (`kh2fm-settings.json` beside the config: 200% render scale, no MSAA, V-Sync). The test room was driven
through its keyboard bindings (the command menu: Magic, Thundaga, …), and the game window itself was captured.

- Thundaga cast from the menu: in the frames after the strike, several large hard-edged white sheets stand around
  Sora (the domes and shells of the strike). Donald's own Thunder, cast by his AI, shows a solid white square.
- 100% render scale shows the same sheets, so the Resolution setting is not the cause.
- The unattended magic tour on the same executable shows Thundaga as a soft dome with arcs, no sheets.
- Leaving out all effect models removes the sheets; leaving out the sprites does not. Per-draw dumps of the white
  frames name the draws that turn white: textured models on texcommon's page 5 (buffer 0x2c80, a 128×128 quarter
  starburst whose only white texels are at one corner), drawn with wrapping UVs 1..2.

## Root cause

The PC picks the texture address mode per draw: an OMD model clamps unless its header flag 4 is set, sprites wrap
(T228). The rewrite's four effect presenters (spells, Drive / system effects, world map, cutscenes) cached one image
per texture, and the sampler is part of the image, so the first draw of a page in a session fixed its mode for every
later draw. In the test room the first effect is Donald's AI Thunder, whose casting flash (a flag-3 model) clamps
texcommon page 5. From then on every wrapping draw of that page — Thundaga's storm dome and shells (flag 7, UVs
1..2), Thunder's strike shells, Magnet's and the Drive effects' models on the same page — clamps to the corner
texel and fills its whole surface with white. The magic tour casts Fire first, so the page is made wrapping and the
sheets never show there. The same holds for texcommon page 0x2c60 and any page drawn both ways.

## What the rewrite does now

`visual::effect_draw::PageCache` keeps one image per (texture, address mode), and all four presenters bind their
pages through it, so a clamped and a wrapped draw of one page each get their own sampler, as on the PC.

## Checks

| Check | Result |
| --- | --- |
| `cargo fmt --all --check` | PASS |
| `cargo clippy --all-targets --locked -- -D warnings` | PASS |
| `cargo test --locked` | PASS (new test: a page wrapped after it was clamped gets its own sampler) |
| `cargo clippy --workspace --all-targets --features main/skate3 --locked -- -D warnings` | PASS |
| `cargo test --workspace --features main/skate3 --locked` | PASS |
| `cargo build --locked` | PASS |
| Smoke `cargo run -p main --locked -- --frames 180` | PASS |
| Release `tools/Build-Launcher.ps1 -Skate3` | PASS |
| Live launcher-path runs before the fix (window captures) | FAIL as reported: white sheets in Thundaga's strike frames and Donald's Thunder |
| Live launcher-path runs after the fix, the same config and settings, Donald's AI Thunder first (window captures every 50–150 ms) | No white sheet in two Thundaga casts, four of Donald's Thunders, Firaga, Blizzaga, Magnega, Reflega, Curaga, the Valor and Final Form changes, or Final Form Thundaga |

## Uncertainty

- The live input was posted to the game window (key messages, not focus), because another application kept the
  foreground; the unfocused window runs at its lower frame cap. The effects advance per 60 Hz tick either way.
- Air casts did not register with posted input, so Thunder from the air was not captured after the fix.
- Magnega's vortex shows a bright rectangle with a cyan frame at its centre; the public footage shows a bright,
  roughly square core there too, but it was not compared frame for frame.
- A Final Form Thundaga strike next to the camera still floods that side of the screen and shows the storm dome's
  silhouette edge; the dome is now drawn with the starburst page instead of solid white, but it was not compared with
  the game. Hit sparks and prize effects were only seen in passing (party hits, no Sora combo or prize drops).
- World map and cutscene effects were not run; their presenters take the same change.

## Verdict

PARTIAL. The white sheets the owner reported come from a page cache that ignored the address mode; with one image
per mode they are gone from every effect captured live. Nothing is compared frame for frame with the installed game.

## Next

Capture air casts and Sora's combo hit sparks and prize drops live; compare Magnega's core and Final Form
Thundaga's dome with footage; run the world map and cutscene effects.
