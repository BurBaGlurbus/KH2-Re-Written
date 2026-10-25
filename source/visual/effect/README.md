# Effects

Executes particle and model effect programs. `src/lib.rs` defines shared state
and output. `programs.rs`, `ribbon.rs`, `thunder.rs` and related files execute
effect types. File decoding lives in `data/src/object_effect.rs`; drawing lives in
`source/visual/src/effects.rs` and `effect_draw.rs` (texture, blend). No installed effect assets belong here.

See the [folder guide](../../../docs/folder-guide.md) and [architecture](../../../ARCHITECTURE.md).
