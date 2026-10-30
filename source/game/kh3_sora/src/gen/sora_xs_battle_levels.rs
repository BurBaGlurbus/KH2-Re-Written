// The types of the reference's sheet sora-xs-battle-levels; values come from the player's files and the executable's
// constants at run time (crate::sheets).

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct XsBattleLevels {
    pub depth_0: f32,
    pub depth_1: f32,
    pub depth_2: f32,
    pub depth_3: f32,
    pub depth_4: f32,
    pub default: f32,
}

pub static XS_BATTLE_LEVELS_PARAMS: std::sync::LazyLock<XsBattleLevels> = std::sync::LazyLock::new(|| {
    let s = crate::sheets::Sheet::named("sora-xs-battle-levels");
    XsBattleLevels {
        depth_0: s.num("depth.0"),
        depth_1: s.num("depth.1"),
        depth_2: s.num("depth.2"),
        depth_3: s.num("depth.3"),
        depth_4: s.num("depth.4"),
        default: s.num("default"),
    }
});
