// The row type of the reference's sheet sora-levels; rows come from the player's files at run time (crate::sheets).

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LevelRow {
    pub level_name: &'static str,
    pub exp_total: i32,
    pub attack: i32,
    pub magic: i32,
    pub defense: i32,
    pub ap: i32,
    pub ability_a: &'static str,
    pub ability_b: &'static str,
    pub ability_c: &'static str,
}

pub static LEVELS: std::sync::LazyLock<Vec<LevelRow>> = std::sync::LazyLock::new(|| {
    crate::sheets::Table::named("sora-levels").rows().map(|r| LevelRow {
        level_name: r.text("id"),
        exp_total: r.int("exp_total"),
        attack: r.int("attack"),
        magic: r.int("magic"),
        defense: r.int("defense"),
        ap: r.int("ap"),
        ability_a: r.text("ability_a"),
        ability_b: r.text("ability_b"),
        ability_c: r.text("ability_c"),
    }).collect()
});
