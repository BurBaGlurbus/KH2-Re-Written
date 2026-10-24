# Research tools index

Scripts and helper sources used for the research recorded in `docs/`. The files stay in this folder; this index only groups them. "Cited by" lists the task record or note that names the file. A script with no citation is marked as such and needs review before anyone relies on it.

These tools read only what they are given and write only private output. Source-derived results stay outside the repository (see [CONTRIBUTING.md](../CONTRIBUTING.md)).

## Reference builders and oracles

| File | Purpose | Cited by |
|---|---|---|
| `Bc3Reference.cpp` | Owned adapter for a privately extracted, hash-pinned DirectXTex BC3 decoder | T024 |
| `Build-Bc3Reference.ps1` | Builds `Bc3Reference.cpp` against the pinned source | T024, [model-inspection.md](../docs/research/model-inspection.md) |
| `BgraReference.cpp` | Owned DDS admission and file adapter for pinned scanline branches | none |
| `Build-BgraReference.ps1` | Builds `BgraReference.cpp` | T038, [model-inspection.md](../docs/research/model-inspection.md) |
| `HdImageOracle.cpp` | Windows-only independent PNG and DDS pixel inspection helper | T023 |

## Managed code

| File | Purpose | Cited by |
|---|---|---|
| `ManagedReader.ps1` | Shared static-reader helpers for managed research | [managed-inspection.md](../docs/research/managed-inspection.md) |
| `Inspect-ManagedExecutable.ps1` | Execution-free inspection of the bound managed candidate | [managed-inspection.md](../docs/research/managed-inspection.md) |
| `Verify-ManagedReader.ps1` | Runs the managed reader against `fixtures/ManagedConformance.cs` | [managed-inspection.md](../docs/research/managed-inspection.md) |

## Native (static and live) research

| File | Purpose | Cited by |
|---|---|---|
| `NativeReader.ps1` | Authored helpers for source-bound static native inspection | [bridge-inspection.md](../docs/research/bridge-inspection.md), [native-inspection.md](../docs/research/native-inspection.md) |
| `Invoke-NativeReader.ps1` | Private, execution-free native research entry point | [bridge-inspection.md](../docs/research/bridge-inspection.md), [native-inspection.md](../docs/research/native-inspection.md) |
| `NativeRangeEngine.ps1` | Low-level isolated raw inspection (never gameplay admission) | T047, [native-inspection.md](../docs/research/native-inspection.md) |
| `Invoke-NativeMemoryReader.ps1` | Source and process bound admission for live memory reads | T047, [native-inspection.md](../docs/research/native-inspection.md) |
| `NativeMemorySelected.java` | Bounded raw-snapshot inspection | T047, [native-inspection.md](../docs/research/native-inspection.md) |
| `NativeSelected.java` | Authored research script for selected source-derived output | T014 |
| `NativeFunctionTable.cs` | AMD64 runtime-function metadata selection | T050, [native-inspection.md](../docs/research/native-inspection.md) |
| `ReadOnlyRangeProbe.cs` | Windows x64 read and query probe, bound to a source, process and span | T046, [native-inspection.md](../docs/research/native-inspection.md) |
| `NativeIndirectChain.ps1` | Operand and readable-span admission for indirect chains | T049, [native-inspection.md](../docs/research/native-inspection.md) |
| `NativeRipReads.ps1` | Byte-derived AMD64 RIP-read profile | T048, [native-inspection.md](../docs/research/native-inspection.md) |
| `Verify-NativeReader.ps1` | Oracle for `NativeReader.ps1` using authored fixtures | [native-inspection.md](../docs/research/native-inspection.md) |
| `Verify-NativeMemoryReader.ps1` | Oracle for the live memory reader using authored fixture ranges | T047, [native-inspection.md](../docs/research/native-inspection.md) |
| `Verify-NativeBranches.ps1` | Synthetic-byte branch checks | [native-inspection.md](../docs/research/native-inspection.md) |
| `Verify-NativeRuntimeFunctions.ps1` | Generated AMD64 metadata checks | [native-inspection.md](../docs/research/native-inspection.md) |
| `Verify-NativeSharedModules.ps1` | Synthetic shared-module record checks | [native-inspection.md](../docs/research/native-inspection.md) |
| `Verify-ReadOnlyRangeProbe.ps1` | Own allocated bytes only; checks `ReadOnlyRangeProbe.cs` | [native-inspection.md](../docs/research/native-inspection.md) |
| `Verify-NativeFunctionTable.ps1` | Generated-metadata check for the function table | none (repo-reorganization-plan.md only) |
| `Verify-NativeIndirectChain.ps1` | Synthetic-operand check for the indirect chain | none (repo-reorganization-plan.md only) |
| `Verify-NativeRipReads.ps1` | Synthetic-encoding check for RIP reads | none (repo-reorganization-plan.md only) |
| `Verify-PlainProbe.ps1` | Independent scalar reader check (requires PowerShell 7) | T003 |

## Images and textures

| File | Purpose | Cited by |
|---|---|---|
| `Verify-ImageStructure.ps1` | Independent histogram and IMGD scalar checks | [image-inspection.md](../docs/research/image-inspection.md) |
| `Verify-ImgdPixels.ps1` | Independent indexed-pixel expansion | [image-inspection.md](../docs/research/image-inspection.md) |
| `Verify-InspectorCaptures.ps1` | Compares captured screen pixels against decoded images | [image-inspection.md](../docs/research/image-inspection.md) |
| `Verify-BgraImages.ps1` | Replacement-storage check for BGRA images | T038, [model-inspection.md](../docs/research/model-inspection.md) |
| `Verify-Bc3MipImages.ps1` | BC3 mip-chain check against the pinned decoder | T039, [model-inspection.md](../docs/research/model-inspection.md) |
| `Verify-OriginalSamples.ps1` | Compares samples against pinned public OpenKH code and .NET `DeflateStream` | T005, T038, [model-inspection.md](../docs/research/model-inspection.md) |
| `Compare-Rgba.ps1` | First-divergence diagnostics for RGBA evidence | none (repo-reorganization-plan.md only) |
| `Verify-FontCells.ps1` | Font cell draw check against pinned draw methods | none (repo-reorganization-plan.md only) |
| `Make-BannerText.ps1` | Renders the "Final Re:WRITE" banner text from a system font | T087, `source/visual/src/title_banner.rs` |

## Models and motion

| File | Purpose | Cited by |
|---|---|---|
| `Verify-ModelInspector.ps1` | Mesh input and screen comparison for the finite axis tour | [model-inspection.md](../docs/research/model-inspection.md) |
| `Verify-ModelCaptureRepeat.ps1` | Repeat-tour capture comparison | T041, [model-inspection.md](../docs/research/model-inspection.md) |
| `Verify-ModelPackets.ps1` | Packet reconstruction with pinned VIF execution | [model-inspection.md](../docs/research/model-inspection.md) |
| `Verify-ModelSkeleton.ps1` | Scalar reads guided by the pinned legacy layout | [model-inspection.md](../docs/research/model-inspection.md) |
| `Verify-ModelTextures.ps1` | Texture scalar and layout reads with pinned register and CLUT methods | [model-inspection.md](../docs/research/model-inspection.md) |
| `Verify-ModelTexturePixels.ps1` | Texture pixel reconstruction | T036, [model-inspection.md](../docs/research/model-inspection.md) |
| `Verify-ModelTextureRouting.ps1` | Texture routing against storage and topology | T037, [model-inspection.md](../docs/research/model-inspection.md) |
| `Verify-ModelTextureAnimations.ps1` | TEXA constructor and getters checked against storage | T042, [model-inspection.md](../docs/research/model-inspection.md) |
| `Verify-TexturedModelInputs.ps1` | Texture, UV and upload input comparison | [model-inspection.md](../docs/research/model-inspection.md) |
| `Inspect-ModelBindRoutes.ps1` | Comparison of public bind routes (research only) | none |
| `Verify-ModelGeometry.ps1` | Read-only topology oracle | none |
| `Verify-ModelPose.ps1` | Finite bit comparison of model pose | none (repo-reorganization-plan.md only) |

## Other

The `KH2RW room` developer program supports `--enemy-audit <new-private-report.json>`
for sequential source-bound catalog/model/motion inspection and
`--enemy-tour <new-empty-private-capture-directory>` for a bounded renderer check.
Supply the usual selected Steam root, manifest and room configuration; every
launch validates that source. Reports/captures stay outside Git and the install.
These checks do not execute or qualify complete enemy combat scripts.
The current enemy tour requires exact Shadow, Dusk, Mushroom IV and the retained
raw-motion candidate; it fails instead of substituting a different model.
For Dusk it selects source candidates from the claw, kick and walking-attack
actions. It previews their animations without executing the combat callbacks.

| File | Purpose | Cited by |
|---|---|---|
| `Compare-CandidateNames.ps1` | Candidate-only name metadata, following pinned OpenKH name generation | T006 |
| `Build-Launcher.ps1` | Builds `KH2RW Launcher.exe` (viewers inside) and removes other top-level executables; `-Debug`, `-Skate3`, `-Timings` | T126 |
| `New-TaskIndex.ps1` | Regenerates `docs/tasks/README.md` from the task records | none (documentation tool) |

## Fixtures

`AbilityNativeAudit.java` is T120's source-bound static ability/growth/combo research helper. It writes pseudocode/instruction receipts only to a new private destination outside the installation and Git. Use a read-only Ghidra project; reported slices are static analysis, not gameplay or complete-function equivalence.

`fixtures/ManagedConformance.cs` and `fixtures/NativeConformance.c` are authored conformance inputs. Only the oracles above compile and run them.
