//! The KH2 Re:Written engine, as a library: the room, the launcher and the
//! other programs. The `KH2RW` binary is its entry point; outside crates build
//! on the room through `test_room`'s public items.
#![allow(clippy::new_without_default, clippy::len_without_is_empty)]

pub mod area;
pub mod cutscene;
pub mod early_renderer;
pub mod image_inspector;
pub mod keyblade_board;
pub mod launcher;
pub mod load_timer;
pub mod model_inspector;
pub mod npcs;
pub mod pc_controls;
#[cfg(feature = "perf-diag")]
pub mod perf;
pub mod save_files;
pub mod sephiroth;
pub mod skate;
#[cfg(feature = "skate3")]
pub mod skate_hud;
pub mod skate_rig;
pub mod test_room;
pub mod title;
pub mod world_map;
