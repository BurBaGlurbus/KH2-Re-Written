// The row type of the reference's sheet sora-combat-hits; rows come from the player's files at run time (crate::sheets).

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HitWindowRow {
    pub id: &'static str,
    pub anim: &'static str,
    pub clip: &'static str,
    pub start: f32,
    pub end: f32,
    pub group: &'static str,
    pub attack_row: &'static str,
    pub hit_se: &'static str,
    pub interval: Option<f32>,
    pub extra: &'static str,
}

pub static HIT_WINDOWS: std::sync::LazyLock<Vec<HitWindowRow>> = std::sync::LazyLock::new(|| {
    crate::sheets::Table::named("sora-combat-hits").rows().map(|r| HitWindowRow {
        id: r.text("id"),
        anim: r.text("anim"),
        clip: r.text("clip"),
        start: r.num("start"),
        end: r.num("end"),
        group: r.text("group"),
        attack_row: r.text("attack_row"),
        hit_se: r.text("hit_se"),
        interval: r.opt_num("interval"),
        extra: r.text("extra"),
    }).collect()
});
