// The row type of the reference's sheet sora-abilities; rows come from the player's files at run time (crate::sheets).

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AbilityRow {
    pub id: &'static str,
    pub kind: ETresAbilityKind,
    pub name: &'static str,
    pub category: &'static str,
    pub ap: Option<i32>,
    pub max_equip: Option<i32>,
    pub owner: &'static str,
    pub copies: i32,
    pub system: &'static str,
    pub status: &'static str,
    pub port: &'static str,
    pub help_text: &'static str,
}

pub static ABILITIES: std::sync::LazyLock<Vec<AbilityRow>> = std::sync::LazyLock::new(|| {
    crate::sheets::Table::named("sora-abilities").rows().map(|r| AbilityRow {
        id: r.text("id"),
        kind: ETresAbilityKind::from_game_name(r.text("enum_value")).unwrap_or_else(|| panic!("sora-abilities: enum_value: {}", r.text("enum_value"))),
        name: r.text("name"),
        category: r.text("category"),
        ap: r.opt_int("ap"),
        max_equip: r.opt_int("max_equip"),
        owner: r.text("owner"),
        copies: r.int("copies"),
        system: r.text("system"),
        status: r.text("status"),
        port: r.text("port"),
        help_text: r.text("help_text"),
    }).collect()
});

pub const ABILITY_AIR_RECOVERY: usize = 0;
pub const ABILITY_BLOW_COUNTER: usize = 1;
pub const ABILITY_REFLECT_GUARD: usize = 2;
pub const ABILITY_GUARD_COUNTER: usize = 3;
pub const ABILITY_REVENGEIMPACT: usize = 4;
pub const ABILITY_REVENGEDIVE: usize = 5;
pub const ABILITY_REVENGE_EX: usize = 6;
pub const ABILITY_RISKDODGE: usize = 7;
pub const ABILITY_RD_COUNTER: usize = 8;
pub const ABILITY_SLASH_UPPER: usize = 9;
pub const ABILITY_AIR_ROLL_BEAT: usize = 10;
pub const ABILITY_AIR_DOWN: usize = 11;
pub const ABILITY_TRIPPLE_SLASH: usize = 12;
pub const ABILITY_CHARGE_THRUST: usize = 13;
pub const ABILITY_MAGICFLUSH: usize = 14;
pub const ABILITY_HIGHJUMP: usize = 15;
pub const ABILITY_DOUBLEFLIGHT: usize = 16;
pub const ABILITY_SUPERJUMP: usize = 17;
pub const ABILITY_SUPERSLIDE: usize = 18;
pub const ABILITY_GLIDE: usize = 19;
pub const ABILITY_LIBRA: usize = 20;
pub const ABILITY_DODGE: usize = 21;
pub const ABILITY_AIRSLIDE: usize = 22;
pub const ABILITY_AIRDODGE: usize = 23;
pub const ABILITY_MP_SAFETY: usize = 24;
pub const ABILITY_EXPZERO: usize = 25;
pub const ABILITY_FRIEND_AID: usize = 26;
pub const ABILITY_COMBO_PLUS: usize = 27;
pub const ABILITY_AIRCOMBO_PLUS: usize = 28;
pub const ABILITY_COMBO_MASTER: usize = 29;
pub const ABILITY_COMBO_UP: usize = 30;
pub const ABILITY_AIRCOMBO_UP: usize = 31;
pub const ABILITY_FIRE_UP: usize = 32;
pub const ABILITY_BLIZZARD_UP: usize = 33;
pub const ABILITY_THUNDER_UP: usize = 34;
pub const ABILITY_WATER_UP: usize = 35;
pub const ABILITY_AERO_UP: usize = 36;
pub const ABILITY_WIZZARD_STAR: usize = 37;
pub const ABILITY_LUCK_UP: usize = 38;
pub const ABILITY_ITEM_UP: usize = 39;
pub const ABILITY_PRIZE_DRAW: usize = 40;
pub const ABILITY_FOCUS_BOOST: usize = 41;
pub const ABILITY_LEAF_VEIL: usize = 42;
pub const ABILITY_LAST_LEAVE: usize = 43;
pub const ABILITY_COMBO_LEAVE: usize = 44;
pub const ABILITY_FOCUS_ASPIR: usize = 45;
pub const ABILITY_ATTRACTION_TIME: usize = 46;
pub const ABILITY_LINK_BOOST: usize = 47;
pub const ABILITY_FORM_TIME: usize = 48;
pub const ABILITY_DEFENDER: usize = 49;
pub const ABILITY_CRITICAL_HALF: usize = 50;
pub const ABILITY_DAMAGE_ASPIR: usize = 51;
pub const ABILITY_MP_HASTE: usize = 52;
pub const ABILITY_MP_HASTERA: usize = 53;
pub const ABILITY_MP_HASTEGA: usize = 54;
pub const ABILITY_MAGIC_COMBO_SAVE: usize = 55;
pub const ABILITY_MAGIC_COMBO_UP: usize = 56;
pub const ABILITY_WALK_REGENE: usize = 57;
pub const ABILITY_WALK_HEALING: usize = 58;
pub const ABILITY_MAGIC_DRAW: usize = 59;
pub const ABILITY_MASTER_DRAW: usize = 60;
pub const ABILITY_ATTRACTION_UP: usize = 61;
pub const ABILITY_BURN_GUARD: usize = 62;
pub const ABILITY_CLOUD_GUARD: usize = 63;
pub const ABILITY_SNEEZE_GUARD: usize = 64;
pub const ABILITY_FREEZE_GUARD: usize = 65;
pub const ABILITY_DISCHARGE_GUARD: usize = 66;
pub const ABILITY_STUN_GUARD: usize = 67;
pub const ABILITY_COUNTER_UP: usize = 68;
pub const ABILITY_AUTO_FINISH: usize = 69;
pub const ABILITY_FORM_UP: usize = 70;
pub const ABILITY_MAGIC_TIME: usize = 71;
pub const ABILITY_AUTO_LOCK_MAGIC: usize = 72;
pub const ABILITY_GUARD_REGENE: usize = 73;
pub const ABILITY_MP_SAVE: usize = 74;
pub const ABILITY_MP_LEAVE: usize = 75;
pub const ABILITY_FULLMP_BURST: usize = 76;
pub const ABILITY_HARVEST: usize = 77;
pub const ABILITY_HP_CONVERTER: usize = 78;
pub const ABILITY_MP_CONVERTER: usize = 79;
pub const ABILITY_MUNNY_CONVERTER: usize = 80;
pub const ABILITY_ENDLESS_MAGIC: usize = 81;
pub const ABILITY_FP_CONVERTER: usize = 82;
pub const ABILITY_FIRE_ASPIR: usize = 83;
pub const ABILITY_BLIZZARD_ASPIR: usize = 84;
pub const ABILITY_THUNDER_ASPIR: usize = 85;
pub const ABILITY_WATER_ASPIR: usize = 86;
pub const ABILITY_AERO_ASPIR: usize = 87;
pub const ABILITY_DARK_ASPIR: usize = 88;
pub const ABILITY_BEST_COMBI: usize = 89;
pub const ABILITY_SONIC_SLASH: usize = 90;
pub const ABILITY_SONIC_DOWN: usize = 91;
pub const ABILITY_TURN_CUTTER: usize = 92;
pub const ABILITY_SUMMERSALT: usize = 93;
pub const ABILITY_POLE_SPIN: usize = 94;
pub const ABILITY_POLE_SWING: usize = 95;
pub const ABILITY_WALL_KICK: usize = 96;
pub const ABILITY_BATTLE_GRAPHER: usize = 97;
pub const ABILITY_CHARISMA_CHEF: usize = 98;
pub const ABILITY_JOIN_FORCE: usize = 99;
pub const ABILITY_HEARTLESS_BUSTER: usize = 100;
pub const ABILITY_NOBODY_BUSTER: usize = 101;
pub const ABILITY_POWER_CURE: usize = 102;
pub const ABILITY_CRITICAL_COUNTER: usize = 103;
pub const ABILITY_CRITICAL_CHARGE: usize = 104;
pub const ABILITY_CRITICAL_CONVERTER: usize = 105;
pub const ABILITY_QUICK_SHAFT: usize = 106;
pub const ABILITY_FLASH_STEP: usize = 107;
pub const ABILITY_RADIAL_CUT: usize = 108;
pub const ABILITY_FINAL_HEAVEN: usize = 109;
pub const ABILITY_AERIAL_SWEEP: usize = 110;
pub const ABILITY_AERIAL_DIVE: usize = 111;
pub const ABILITY_LUNCH_TIME: usize = 112;
pub const ABILITY_POWER_LUNCH: usize = 113;
pub const ABILITY_OVER_TIME: usize = 114;
pub const ABILITY_BEST_CONDITION: usize = 115;
pub const ABILITY_EXP_BARGAIN: usize = 116;
pub const ABILITY_PRIZE_FEEVER: usize = 117;
pub const ABILITY_MILLIONAIRE: usize = 118;
pub const ABILITY_CURAGAN: usize = 119;
pub const ABILITY_CHARGE_BERSERK: usize = 120;
pub const ABILITY_OVERCOME: usize = 121;
pub const ABILITY_GRAND_MAGIC: usize = 122;
pub const ABILITY_FIRAGAN: usize = 123;
pub const ABILITY_BLIZZAGAN: usize = 124;
pub const ABILITY_THUNDAGAN: usize = 125;
pub const ABILITY_WATAGAN: usize = 126;
pub const ABILITY_AEROGAN: usize = 127;
pub const ABILITY_MAGIC_ROULETTE: usize = 128;
pub const ABILITY_UNISON_FIRE: usize = 129;
pub const ABILITY_UNISON_BLIZZARD: usize = 130;
pub const ABILITY_UNISON_THUNDER: usize = 131;
pub const ABILITY_FUSION_SPIN: usize = 132;
pub const ABILITY_FUSION_ROCKET: usize = 133;
pub const ABILITY_HYPER_HEALING: usize = 134;
pub const ABILITY_COMBI_UP: usize = 135;
pub const ABILITY_COMBI_TIME: usize = 136;
pub const ABILITY_SHARE_PRIZE: usize = 137;
pub const ABILITY_MICKEY_CURE: usize = 138;
pub const ABILITY_MICKEY_HOLY: usize = 139;
pub const ABILITY_DONALD_FIRE: usize = 140;
pub const ABILITY_DONALD_BLIZZARD: usize = 141;
pub const ABILITY_DONALD_THUNDER: usize = 142;
pub const ABILITY_DONALD_AERO: usize = 143;
pub const ABILITY_DONALD_CURE: usize = 144;
pub const ABILITY_GOOFY_TORNADE: usize = 145;
pub const ABILITY_GOOFY_TURBO: usize = 146;
pub const ABILITY_GOOFY_FLIP: usize = 147;
pub const ABILITY_HERCULES_ROCKTHROW: usize = 148;
pub const ABILITY_HERCULES_ENERGYBURST: usize = 149;
pub const ABILITY_WOODY_SHOULDERTACKLE: usize = 150;
pub const ABILITY_WOODY_ROPEATTACK: usize = 151;
pub const ABILITY_BUZZ_BEAM: usize = 152;
pub const ABILITY_BUZZ_DOUBLELARIAT: usize = 153;
pub const ABILITY_RAPUNZEL_HAIRSWING: usize = 154;
pub const ABILITY_RAPUNZEL_HAIRROTATION: usize = 155;
pub const ABILITY_FLYNN_THROWFRYINGPAN: usize = 156;
pub const ABILITY_FLYNN_BARRELBASH: usize = 157;
pub const ABILITY_SULLEY_ROAR: usize = 158;
pub const ABILITY_SULLEY_SHOULDERTACKLE: usize = 159;
pub const ABILITY_MIKE_MIKEAPPEAL: usize = 160;
pub const ABILITY_MIKE_SUPPORT: usize = 161;
pub const ABILITY_SPARROW_BONECRASH: usize = 162;
pub const ABILITY_SPARROW_TWISTROLLING: usize = 163;
pub const ABILITY_MARSHMALLOW_JUMPSTRIKE: usize = 164;
pub const ABILITY_MARSHMALLOW_COLDBREATH: usize = 165;
pub const ABILITY_BAYMAX_ROCKETPUNCH: usize = 166;
pub const ABILITY_BAYMAX_CURESPRAY: usize = 167;
pub const ABILITY_RIKU_DARKFIRE: usize = 168;
pub const ABILITY_RIKU_SHADOWBREAKER: usize = 169;
pub const ABILITY_VENTUS_WINDRAID: usize = 170;
pub const ABILITY_AQUA_BLIZZAGA: usize = 171;
pub const ABILITY_AQUA_THUNDAGA: usize = 172;
pub const ABILITY_AQUA_CURAGA: usize = 173;
pub const ABILITY_ROXAS_CROSSSHOT: usize = 174;
pub const ABILITY_ROXAS_CROSSBRADE: usize = 175;
pub const ABILITY_XION_CURAGA: usize = 176;
pub const ABILITY_XION_LASERRAIN: usize = 177;
pub const ABILITY_LEA_FIRAGARAID: usize = 178;
pub const ABILITY_LEA_FIRAGASLICER: usize = 179;
pub const ABILITY_KAIRI_CURE: usize = 180;
pub const ABILITY_SORA_FIRAGA: usize = 181;
pub const ABILITY_SORA_CURAGA: usize = 182;
pub const ABILITY_DIVING_STRIKE: usize = 183;
pub const ABILITY_SONIC_DASH: usize = 184;
pub const ABILITY_SPIRAL_RUSH: usize = 185;
pub const ABILITY_ZANTETSUKEN: usize = 186;
pub const ABILITY_EVENT_HORIZON: usize = 187;
pub const ABILITY_GIANT_STAMP: usize = 188;
pub const ABILITY_CHANDELIER: usize = 189;
pub const ABILITY_SHOTLOCK_SPECTRAL_RAYS: usize = 190;
pub const ABILITY_LIGHT_WAVE: usize = 191;
pub const ABILITY_RAGE_RAID: usize = 192;
pub const ABILITY_RISKCHARGE: usize = 193;
pub const ABILITY_ANCIENT_LIGHT: usize = 194;
pub const ABILITY_MYRIAD: usize = 195;
pub const ABILITY_BREAK_BOOST: usize = 196;

pub fn abilities_by_id(key: &str) -> Option<&'static AbilityRow> {
    ABILITIES.iter().find(|row| row.id == key)
}
