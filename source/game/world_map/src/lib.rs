//! The world map (field area `wm00`, world 15): its objects' looks from the
//! world state and the camera that follows the cursor, translated from the
//! exe's `worldmap::*` and `dk::WM_*` classes. Positions are engine
//! coordinates (Y down).
pub mod camera;
pub mod landing;
pub mod objects;

pub use camera::ObjCamera;
pub use objects::{GumiLook, SymbolLook, symbol, symbol_look};
