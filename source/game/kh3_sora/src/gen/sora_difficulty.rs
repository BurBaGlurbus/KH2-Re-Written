// The row type of the reference's sheet sora-difficulty; rows come from the player's files at run time (crate::sheets).

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DifficultyRow {
    pub id: &'static str,
    pub game_level: i32,
    pub base_hp: i32,
    pub base_mp: i32,
    pub hp_up: &'static [i32],
    pub mp_up: &'static [i32],
    pub player_damage: f32,
    pub enemy_damage: f32,
    pub enemy_attack_add: i32,
    pub ap_add: i32,
    pub mp_charge_time: f32,
    pub start_abilities: &'static [&'static str],
}

pub static DIFFICULTY: std::sync::LazyLock<Vec<DifficultyRow>> = std::sync::LazyLock::new(|| {
    crate::sheets::Table::named("sora-difficulty").rows().map(|r| DifficultyRow {
        id: r.text("id"),
        game_level: r.int("game_level"),
        base_hp: r.int("base_hp"),
        base_mp: r.int("base_mp"),
        hp_up: r.ints("hp_up"),
        mp_up: r.ints("mp_up"),
        player_damage: r.num("player_damage"),
        enemy_damage: r.num("enemy_damage"),
        enemy_attack_add: r.int("enemy_attack_add"),
        ap_add: r.int("ap_add"),
        mp_charge_time: r.num("mp_charge_time"),
        start_abilities: r.texts("start_abilities"),
    }).collect()
});

pub const DIFFICULTY_BEGINNER: usize = 0;
pub const DIFFICULTY_STANDARD: usize = 1;
pub const DIFFICULTY_PROUD: usize = 2;
pub const DIFFICULTY_CRITICAL: usize = 3;

pub fn difficulty_by_id(key: &str) -> Option<&'static DifficultyRow> {
    DIFFICULTY.iter().find(|row| row.id == key)
}
