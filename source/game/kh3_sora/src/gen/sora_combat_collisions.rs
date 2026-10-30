// The row type of the reference's sheet sora-combat-collisions; rows come from the player's files at run time (crate::sheets).

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AtkCollisionRow {
    pub id: &'static str,
    pub owner: &'static str,
    pub group: &'static str,
    pub shape: &'static str,
    pub size_x: f32,
    pub size_y: f32,
    pub size_z: f32,
    pub socket1: &'static str,
    pub socket2: &'static str,
    pub loc1: &'static [f32],
    pub loc2: &'static [f32],
    pub rot: &'static [f32],
    pub attach1: &'static str,
    pub attach2: &'static str,
    pub map_hit: &'static str,
    pub sweep: bool,
    pub default_row: &'static str,
    pub phys: bool,
}

pub static ATK_COLLISIONS: std::sync::LazyLock<Vec<AtkCollisionRow>> = std::sync::LazyLock::new(|| {
    crate::sheets::Table::named("sora-combat-collisions").rows().map(|r| AtkCollisionRow {
        id: r.text("id"),
        owner: r.text("owner"),
        group: r.text("group"),
        shape: r.text("shape"),
        size_x: r.num("size_x"),
        size_y: r.num("size_y"),
        size_z: r.num("size_z"),
        socket1: r.text("socket1"),
        socket2: r.text("socket2"),
        loc1: r.nums("loc1"),
        loc2: r.nums("loc2"),
        rot: r.nums("rot"),
        attach1: r.text("attach1"),
        attach2: r.text("attach2"),
        map_hit: r.text("map_hit"),
        sweep: r.yesno("sweep"),
        default_row: r.text("default_row"),
        phys: r.yesno("phys"),
    }).collect()
});
