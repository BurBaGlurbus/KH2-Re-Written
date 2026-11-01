# Audio output

`src/lib.rs` mixes sounds. `src/winmm.rs` adapts output to Windows, `src/cpal_device.rs` to other systems.
Sound-file readers live in `data/`; scene sound requests live in `visual/`.

See the [folder guide](../../docs/folder-guide.md) and [architecture](../../ARCHITECTURE.md).
