//! KH3's particle effects: a Cascade evaluator with the Square Enix node
//! modules, ported from StrawHatPro's `crates/kh-vfx` (MIT; see NOTICE). It runs
//! in KH3's conventions (cm, seconds, degrees, X forward, Y right, Z up,
//! left-handed) at a fixed 60 Hz step and hands the host draw lists. It knows
//! no engine and holds no KH3 data: effects come from the player's own paks
//! through `data::kh3::effect_json`.
//!
//! ```text
//! let mut world = World::new();
//! world.register(EffectDef::from_json(&value)?);          // once per effect
//! let id = world.spawn("vs_fir_hit0", Xform::at(point), SpawnOptions::default());
//! per fixed step:   world.set_transform(id, xform);  world.step(1.0 / 60.0);
//! per drawn frame:  world.draw(alpha, &camera, &mut draw_list);
//! ```

pub mod def;
pub mod dist;
pub mod draw;
pub mod math;
pub mod rng;
pub mod screen;
pub mod sim;
pub mod vfxr_modules;

pub use def::{EffectDef, EmitterDef, Kind, SortMode, base_name};
pub use dist::Dist;
pub use draw::{
    Blend, Camera, Decal, DrawGroup, DrawList, Light, MeshInstance, Quad, Ribbon, RibbonPoint,
    TintRange, group_order, sub_image_code,
};
pub use math::{Xform, euler, rotator};
pub use rng::Rng;
pub use screen::{CharParam, ScreenFx, ScreenKind};
pub use sim::{
    EffectId, EmitterProbe, ShakeRequest, SpawnOptions, World, lens_turns, perlin, recolor,
    set_depth_offset_toward_camera, set_lens_rule,
};
