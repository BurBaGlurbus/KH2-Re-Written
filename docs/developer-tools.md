# Developer tools

These need a selected Steam install and a private output directory outside Steam and Git. Nothing they write belongs in the repository.

## Source identity

Write a candidate manifest to an **existing private directory**:

```powershell
cargo run -p validation --release --locked -- '<selected-install-root>' '<private-directory>/source-manifest.json'
```

The scanner hashes only the observed KH2 candidate set, recording each file's SHA-256 and the SHA-256 of every 1 MiB chunk. It never labels an unknown build supported and refuses existing output files. Full shared/language dependency coverage and actual gameplay remain pending. Every session still checks the installation itself: a manifest only records what the bytes must be. Use the optimized scanner for multi-gigabyte packages.

## Room preview

```powershell
cargo run -p main --bin room_scene --locked -- '<selected-install-root>' '<private-directory>/source-manifest.json' '<private-directory>/room-selection.json' '<private-directory>/actor-selection.json' '<private-directory>/new-scene.jsonl' '<private-directory>/new-scene.png'
```

- J/L orbit the camera, +/- zoom, Escape exits. The preview captures a private PNG after settling.
- `--frames 900` gives a bounded capture run.
- `--motion '<private-directory>/motion-selection.json'` opens the basic FK motion preview (Space pause, N/B step, M next slot; `--tour motion` captures two frames per slot).

## Package probes

Unencrypted originals and BAR table metadata only:

```powershell
cargo run -p data --bin plain_probe --locked -- '<selected-install-root>' '<private-directory>/source-manifest.json' '<private-directory>/new-plain-report.json'
```

This verifies the bound HED hashes and package metadata, retains unknown formats, and exports no original asset files.

BAR byte match: every installed BAR, and every BAR nested in one, is read, written again with `bar::write_bar`, and compared by SHA-256 with the original:

```powershell
cargo run -p data --release --bin bar_roundtrip --locked -- '<selected-install-root>' '<private-directory>/source-manifest.json' '<private-directory>/new-bar-roundtrip.json'
```

It prints totals only. The private report lists each record's index, ordinal, name hash, both digests and, where the plain 16-byte packing rule does not reproduce the file, its layout facts. See [T231](tasks/T231-bar-byte-match.md).

Save byte match, on a container the original game wrote (read only):

```powershell
cargo run -p save --release --bin save_roundtrip --locked -- '<selected-install-root>' '<My Games>/KINGDOM HEARTS HD 1.5+2.5 ReMIX/Steam/<id>/KHIIFM_WW.png' '<private-directory>/new-save-roundtrip.json'
```

It checks the header mask, writes the container back, and for each save image checks its checksum and that the
rewrite's save fields read and write back to the same bytes. See [T232](tasks/T232-save-byte-match.md).

Bounded storage samples and their first two HD replacements:

```powershell
cargo run -p data --bin original_probe --locked -- '<selected-install-root>' '<private-directory>/source-manifest.json' '<private-directory>/new-storage-report.json'
```

The storage reader is checked against pinned OpenKH and .NET on finite samples. Source names, full dependency closure and semantic/gameplay decoding remain under research. Both commands retain only private metadata and hashes, and they require a full source rescan to establish mutation assurance. See [T005 evidence](tasks/T005-storage-decoding.md).

Selected message and font structure:

```powershell
cargo run -p data --bin message_probe --locked -- '<selected-install-root>' '<private-directory>/source-manifest.json' '<private-directory>/selection.json' '<private-directory>/new-message-report.json'
```

The bounded selection names source identity and each index/ordinal/name hash plus a `message` or `font` research purpose. Message records retain encoded spans and hashes. Recognized localized RawBitmap records decode to indexed pixels, palettes and straight RGBA fingerprints. Glyph/Unicode mapping and in-game rendering remain unverified. An optional fifth argument names an existing private directory for new checkerboard font BMP previews, outside Steam and Git, with a 32 MiB aggregate cap. These are research assets and must not be distributed. See [T007 structure evidence](tasks/T007-message-font-structure.md), [T008 pixel evidence](tasks/T008-font-planes.md), and the [requested-tool prerequisite audit](research/tool-prerequisites.md).

## Magic tour

```powershell
& '.\KH2RW Launcher.exe' room '<selected-install-root>' '<manifest>' '<test-room-config>' --log '<new-log.jsonl>' --magic-tour '<new-empty-capture-dir>' [--magic-form 5]
```

An unattended run: each spell on the ground and in the air, cast presses at several distances after an attack, a cast after two attacks (the finisher) and an attack after a cast, with Sora invincible and an enemy that stays in front. `--magic-form` casts in a Drive form (2 Wisdom, 4 Master, 5 Final). The log has `cast`, `cast_refused`, `spell_release`, `spell_effect` and `spell_effect_unsupported` events; captures follow each cast. After Sora's casts the tour orders Donald's four spells (`party_cast_order`, `party_cast`, `party_release`, `party_hit`, `party_heal`), lets the party AI cast on its own, and probes Magic Lock-On with the enemy 1400 ahead (`magic_lock_on`). Both outputs stay outside Git. See [T136](tasks/T136-magic-effects-and-combos.md).

## Research notes

- [Static managed research](research/managed-inspection.md): the pinned reader must pass an authored oracle before source-bound selections are inspected. Vendor bytes are parsed without execution. Findings and pseudocode stay private. This does not qualify native methods or enable gameplay.
- [Bounded static native research](research/native-inspection.md): a separately pinned Ghidra engine must pass the current oracle and profile and preserve native PE mappings. Full initialization, runtime behavior and gameplay remain unimplemented.
- [Pinned MCP bridge research](research/bridge-inspection.md): finite fixture and source qualification with a private prepared-project workflow. The project database and all source findings stay external. Opening a bridge does not qualify gameplay.
