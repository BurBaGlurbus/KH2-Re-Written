//! The Gummi Ship mode's rules: the mission records the save keeps, and
//! what a flight's result writes to them (ranks, medals, score, prizes,
//! treasure share, unlocks), from the exe's gumi battle code.
pub mod menu;
pub mod record;
pub mod rules;

pub use record::{
    Mission, RECORD_BYTES, RECORD_COUNT, RECORDS, obtained, read_records, record_index,
    route_missions, write_records,
};
