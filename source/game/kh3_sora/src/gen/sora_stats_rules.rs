// The types of the reference's sheet sora-stats-rules; values come from the player's files and the executable's
// constants at run time (crate::sheets).

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StatsRules {
    /// 0x140EA8620
    pub object_game_instance: &'static str,
    /// 0x140EA85E0
    pub object_game_level: &'static str,
    /// 0x140E864B0
    pub object_exp: &'static str,
    /// 0x140EA85D0
    pub object_battle_level: &'static str,
    /// 0x140E97490
    pub object_growth_type: &'static str,
    /// 0x1405832D0
    pub object_ability_growth_type: &'static str,
    /// 0x1414358B0
    pub object_action_flags: &'static str,
    /// 0x140E86C10
    pub object_save_record: &'static str,
    /// 0x140575C80
    pub object_base_param: &'static str,
    /// 0x140575C80
    pub base_max_hp: f32,
    /// 0x140575C80
    pub base_max_mp: f32,
    /// 0x1405838E0
    pub base_max_focus: f32,
    /// 0x140576E70
    pub base_row_stats_overridden: bool,
    /// 0x140582FD0
    pub level_from_exp: &'static str,
    /// 0x140576E70
    pub level_row_is_absolute: bool,
    /// 0x140576E70
    pub level_max: f32,
    /// 0x140576E70
    pub level_99_exp: f32,
    /// 0x1405832D0
    pub level_99_attack: f32,
    /// 0x1405832D0
    pub level_99_magic: f32,
    /// 0x1405832D0
    pub level_99_defense: f32,
    /// 0x1405832D0
    pub level_99_ap: f32,
    /// 0x140583990
    pub stat_max_hp: &'static str,
    /// 0x140583B40
    pub stat_max_mp: &'static str,
    /// 0x140580E70
    pub stat_attack: &'static str,
    /// 0x1405836E0
    pub stat_magic: &'static str,
    /// 0x140581830
    pub stat_defense: &'static str,
    /// 0x1405808B0
    pub stat_ap_max: &'static str,
    /// 0x1405850B0
    pub stat_resist: &'static str,
    /// 0x140632240
    pub stat_ceil_floor: &'static str,
    /// 0x1405864C0
    pub bonus_hp_up: [f32; 5],
    /// 0x1405864C0
    pub bonus_mp_up: [f32; 4],
    /// 0x140E90EE0
    pub bonus_recount: &'static str,
    /// 0x1405A06B0
    pub bonus_apply_now: &'static str,
    /// 0x140576130
    pub exp_enemy_reward: &'static str,
    /// 0x140632CB0
    pub exp_award: &'static str,
    /// 0x140E864B0
    pub exp_total_cap: f32,
    /// 0x140EBE400
    pub exp_level_up: &'static str,
    /// 0x140E9E830
    pub exp_hud_popup: &'static str,
    /// 0x1405B9700
    pub hp_set: &'static str,
    /// 0x1405A2A00
    pub hp_damage_apply: &'static str,
    /// 0x1405A2A00
    pub hp_combo_leave_timer: f32,
    /// 0x1405AD600
    pub hp_pinch: &'static str,
    /// 0x1405AB250
    pub mp_charge_time: f32,
    /// 0x140632240
    pub damage_base: &'static str,
    /// 0x140632240
    pub damage_min_rule: &'static str,
    /// 0x140632240
    pub damage_max_rule: &'static str,
    /// 0x140632240
    pub damage_cap_player: f32,
    /// 0x140632240
    pub damage_resist: &'static str,
    /// 0x140642820
    pub damage_difficulty: &'static str,
}

pub static STATS_RULES_PARAMS: std::sync::LazyLock<StatsRules> = std::sync::LazyLock::new(|| {
    let s = crate::sheets::Sheet::named("sora-stats-rules");
    StatsRules {
        object_game_instance: s.text("object.game_instance"),
        object_game_level: s.text("object.game_level"),
        object_exp: s.text("object.exp"),
        object_battle_level: s.text("object.battle_level"),
        object_growth_type: s.text("object.growth_type"),
        object_ability_growth_type: s.text("object.ability_growth_type"),
        object_action_flags: s.text("object.action_flags"),
        object_save_record: s.text("object.save_record"),
        object_base_param: s.text("object.base_param"),
        base_max_hp: s.num("base.max_hp"),
        base_max_mp: s.num("base.max_mp"),
        base_max_focus: s.num("base.max_focus"),
        base_row_stats_overridden: s.flag("base.row_stats_overridden"),
        level_from_exp: s.text("level.from_exp"),
        level_row_is_absolute: s.flag("level.row_is_absolute"),
        level_max: s.num("level.max"),
        level_99_exp: s.num("level.99.exp"),
        level_99_attack: s.num("level.99.attack"),
        level_99_magic: s.num("level.99.magic"),
        level_99_defense: s.num("level.99.defense"),
        level_99_ap: s.num("level.99.ap"),
        stat_max_hp: s.text("stat.max_hp"),
        stat_max_mp: s.text("stat.max_mp"),
        stat_attack: s.text("stat.attack"),
        stat_magic: s.text("stat.magic"),
        stat_defense: s.text("stat.defense"),
        stat_ap_max: s.text("stat.ap_max"),
        stat_resist: s.text("stat.resist"),
        stat_ceil_floor: s.text("stat.ceil_floor"),
        bonus_hp_up: s.arr("bonus.hp_up"),
        bonus_mp_up: s.arr("bonus.mp_up"),
        bonus_recount: s.text("bonus.recount"),
        bonus_apply_now: s.text("bonus.apply_now"),
        exp_enemy_reward: s.text("exp.enemy_reward"),
        exp_award: s.text("exp.award"),
        exp_total_cap: s.num("exp.total_cap"),
        exp_level_up: s.text("exp.level_up"),
        exp_hud_popup: s.text("exp.hud_popup"),
        hp_set: s.text("hp.set"),
        hp_damage_apply: s.text("hp.damage_apply"),
        hp_combo_leave_timer: s.num("hp.combo_leave_timer"),
        hp_pinch: s.text("hp.pinch"),
        mp_charge_time: s.num("mp.charge_time"),
        damage_base: s.text("damage.base"),
        damage_min_rule: s.text("damage.min_rule"),
        damage_max_rule: s.text("damage.max_rule"),
        damage_cap_player: s.num("damage.cap_player"),
        damage_resist: s.text("damage.resist"),
        damage_difficulty: s.text("damage.difficulty"),
    }
});
