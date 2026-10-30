//! The engine's integer rounding, exactly as the game's code does it (docs/sora-stats-spec.md
//! header, sheet row `stat.ceil_floor`; instructions e.g. 0x1406326EF..0x140632705):
//! `ceil(x) = -(cvtss2si(-0.5 - 2x) >> 1)`, `floor(x) = cvtss2si(2x - 0.5) >> 1`, where
//! `cvtss2si` converts with the default rounding mode (to nearest, ties to even). Every formula of
//! the status side goes through these two, in `f32`, so that the results are the game's bit for bit.

/// `cvtss2si` under the default MXCSR: round to nearest, ties to even. (Out of the `i32` range
/// the instruction gives 0x80000000; Rust saturates. No status value comes near that range.)
#[inline]
fn cvtss2si(x: f32) -> i32 {
    x.round_ties_even() as i32
}

/// The engine's `ceil`: `-(cvtss2si(-0.5 - 2x) >> 1)`.
#[inline]
pub fn ue_ceil(x: f32) -> i32 {
    -(cvtss2si(-0.5 - (x + x)) >> 1)
}

/// The engine's `floor`: `cvtss2si(2x - 0.5) >> 1`.
#[inline]
pub fn ue_floor(x: f32) -> i32 {
    cvtss2si((x + x) - 0.5) >> 1
}

/// Round half up, as the item code does (docs/sora-magic-spec.md I5: "`round(x)` = round half up").
#[inline]
pub fn round_half_up(x: f32) -> i32 {
    (x + 0.5).floor() as i32
}
