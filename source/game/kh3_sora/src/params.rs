//! `SoraParams`: every tunable, generated from `sheets/port/sora-movement.json` by
//! `node tools/gen_port.js` into `build/gen/rust/sora_params.rs` (one field per row; the doc
//! comment is the row's source, evidence and address). `build.rs` passes the file's path.
//! Never edit the generated file: change the sheet and regenerate.

include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/src/gen/sora_params.rs"
));
