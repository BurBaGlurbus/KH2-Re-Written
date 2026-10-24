# T228 — Effect textures, sprite coordinates, blends, depth-only draws, the heal effect's end and anchors

Owner Claude, 2026-10-24, requested by burba in the project chat ("a lot of vfx textures are incorrect or
incomplete, e.g. thunder shows a large white square, fire doesn't look like the original, a lot of vfx has
square shapes, particles also have a lot of square shapes. Some effects like healing orbs last forever, and
those effects aren't in the middle of Sora/the player character, they appear below him").
Gate: development of the test room, world map and cutscene effect presenters; no final gameplay acceptance is
inferred. Prerequisites: T132, T150, T225. Build identity: branch `claude/project-thread-pk09n9` from `main` at
`33ce82e`, package 0.1.2, Rust 1.99.0 Windows MSVC. Source: the selected Steam KH2FM build, validated each run;
the install was only read.

Private evidence root: `%LOCALAPPDATA%/KH2FinalRewrite/research/vfx-fix-20261024/`: `vfx-fix-spec.md` with its
headless Ghidra outputs `g/`; runs, captures and census outputs in a private folder outside the checkout
(`kh2-vfx-out`: `base-cap`, `final-cap`, `prize-*`, `f5-*`, `census*.txt`, `sheet-*.png`).

## What the executable does

- Sprite S and T (FUN_1404ad7b0 / FUN_1404ada10) are 12-bit fractions of the TEX0 page (4096 = its whole width
  or height), whatever the page size. The census of every spell, system and Sora sprite reaches 4096 on
  128×64, 128×128 and 256×512 pages alike.
- The PC does not emulate GS memory for effects. FUN_1404aecd0 makes one texture per DPD texture. Textures that
  share a buffer pointer form a group; the remastered page of the group is keyed by the first member's data
  offset in the file, and member r asks for its own page at that offset + r + 1 (FUN_14012aa20). FUN_1402c2ae0
  registers, for each effect it draws, the textures its PDATA header (+0xb0) marks, by buffer pointer with the
  palette pointer 0; a later one replaces an earlier one. FUN_14012c0f0 then finds a draw's texture by buffer
  pointer alone, so the CLUT pointer (and a sprite's CLUT byte) does not change it.
- Every spell file and `effect/texcommon.dpd` carries remastered texture pages (DDS stored after the original
  bytes), 2× for the spells, 1× for the common atlas. Their alpha equals the decoded PS2 alpha pixel for pixel.
- FUN_140472110 maps the GS ALPHA byte to a D3D blend: 0x48 add, 0x44 alpha, 0x42 reverse subtract, 0x88
  Cs·As with the frame buffer dropped. A model draws with the draw's mode word when it is not 0, else its
  header's (FUN_1404b6860).
- pppDrawMdl3's key byte +0xd marks a depth-only draw: FUN_1404dc760 sets GS TEST 0x72001 (alpha test NEVER,
  fail = Z only). Thunder's large untextured planes are such draws.
- A picked HP/MP/Drive prize starts system element 6 (looping, bone 0x4001) once and stores its time
  (FUN_1403d7bd0). FUN_1403d4cc0 counts the time down every frame and releases the effect at zero
  (FUN_1402be8f0).
- Bone codes 0x4001 / 0x4009 are the centres of the object's first type-1 / type-9 collision entries
  (FUN_1403b50f0), not its origin.

## What the rewrite does now

- `effect`: sprite quads carry page texels from S·2^TW/4096, T·2^TH/4096 (the prize sprites' own correction is
  gone, as it is now the general rule); `ModelDraw::depth_only` from pppDrawMdl3's +0xd; `Instance::use_texture`.
- `data::object_effect`: each DPD texture records its data offset and may hold its remastered page
  (`Dpd::attach_hd`, `Pax::attach_hd`, `hd_pages`); `read_file_pax`, `read_pax_file` and `read_dpd_file` read an
  installed file with its pages. `Library::read_replacements` reads a file's stored replacements.
- `visual::effect_draw` (new): the texture a TEX0 binds (`TexKey::pick` by the effect's texture mask, own
  textures before the common ones), the page image (remastered page, else the PS2 texture alone at its place
  through its own palette), the blend of an ALPHA byte and a model's ALPHA. The spell, Drive/system, world map
  and cutscene presenters use it. Depth-only model draws and subtractive draws are not drawn.
- The prize heal effect runs under its own owner and is released when its time runs out.
- `drive_fx` places bone codes 0x4001 / 0x4009 at Sora's body centre and heal point.
- `effect_draw_census` (example, private output) lists each effect file's textures, remastered pages, and what
  each element draws (blend, TEX0, texture found, texel range, lifetime).

## Checks

| Check | Result |
| --- | --- |
| `cargo fmt --all --check` | PASS |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | PASS |
| `cargo test --workspace --locked` | PASS (new tests: page coordinates, remaster page groups, texture pick, blend map) |
| `cargo build --workspace --locked` | PASS |
| Smoke `cargo run -p main --locked -- --frames 180` | PASS |
| Release `tools/Build-Launcher.ps1 -Skate3` (private target dir) | PASS; it leaves `KH2RW Launcher.exe` and the two viewers |
| Unattended `--magic-tour` before / after (private `base-cap`, `final-cap`, release skate3 launcher) | exit 0 both; Thunder's white planes gone, Fire, Blizzard, Thunder, Magnet and Reflect sprites no longer show tiled squares or stripes; Cure's flower still drawn |
| Unattended `--magic-form 5 --magic-tour` before / after (private `f5-base`, `f5-fix2`) | exit 0 both; the same changes in Final Form |
| Unattended `--prize-tour` before / after (private `prize-base`, `prize-fix2`) | exit 0 both; the heal sparkles sit at Sora's body and end after the pickup time (before: at his feet and never ending) |
| `python tools/progress.py`, `python tools/test_progress.py` | PASS |
| Census of remastered pages against the PS2 textures | every page's mean alpha and colour match the PS2 texture's to within 1 |

## Uncertainty

- [I] A subtractive draw (0x42, two models of Sora's and the Keyblade's own effects) is left out: a standard
  material has no reverse-subtract blend.
- [I] 0x88 draws (Cure's flower) are drawn as alpha blends; the GS alpha test of these draws is not traced.
- [I] The rewrite blends effects in linear light (sRGB target); the PC blends in gamma space. Not changed.
- [I] A battle actor's bound elements on 0x4001 / 0x4009 still sit on its root (actor bones are another open
  branch); the shape centres are heights above the root, not posed with their bones.
- Not compared with the installed game: Fire's grey smoke (alpha-blended atlas sprites), every spell's colour
  balance, form-specific spell variants other than Final Form, world map and cutscene effects (presenter changed,
  no tour run).

## Verdict

PARTIAL. Texture selection, page coordinates, blends, depth-only draws, the heal effect's end and Sora's
anchors follow the executable; nothing is compared with the original game.

## Next

A reverse-subtract effect material; trace the GS TEST state of 0x88 model draws; a world map and cutscene
effect tour against this presenter change.
