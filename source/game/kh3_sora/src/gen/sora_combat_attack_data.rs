// The row type of the reference's sheet sora-combat-attack-data; rows come from the player's files at run time (crate::sheets).

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AttackDataRow {
    pub id: &'static str,
    pub power: f32,
    pub damage_kind: ETresDamageKind,
    pub attribute: ETresDamageAttribute,
    pub power_scale: f32,
    pub parabola_angle: f32,
    pub move_length: f32,
    pub brake: f32,
    pub knockback: &'static str,
    pub exec_rate: f32,
    pub effect_time: f32,
    pub bad_status: &'static str,
    pub bad_gen_rate: f32,
    pub bad_time: f32,
    pub bad_param: f32,
    pub team_check: &'static str,
    pub reaction_power: i32,
    pub scrum: bool,
    pub armor_attack: i32,
    pub form_point: i32,
    pub magic: bool,
    pub invalid_guard: bool,
    pub rapid_fire: bool,
    pub killer: bool,
    pub attraction: bool,
    pub counter: bool,
    pub weak_guard: bool,
    pub reflect_level: i32,
    pub reflect_reaction: &'static str,
    pub revenge_point: f32,
    pub phys_force: &'static str,
}

pub static ATTACK_DATA: std::sync::LazyLock<Vec<AttackDataRow>> = std::sync::LazyLock::new(|| {
    crate::sheets::Table::named("sora-combat-attack-data").rows().map(|r| AttackDataRow {
        id: r.text("id"),
        power: r.num("power"),
        damage_kind: ETresDamageKind::from_game_name(r.text("damage_kind")).unwrap_or_else(|| panic!("sora-combat-attack-data: damage_kind: {}", r.text("damage_kind"))),
        attribute: ETresDamageAttribute::from_game_name(r.text("attribute")).unwrap_or_else(|| panic!("sora-combat-attack-data: attribute: {}", r.text("attribute"))),
        power_scale: r.num("power_scale"),
        parabola_angle: r.num("parabola_angle"),
        move_length: r.num("move_length"),
        brake: r.num("brake"),
        knockback: r.text("knockback"),
        exec_rate: r.num("exec_rate"),
        effect_time: r.num("effect_time"),
        bad_status: r.text("bad_status"),
        bad_gen_rate: r.num("bad_gen_rate"),
        bad_time: r.num("bad_time"),
        bad_param: r.num("bad_param"),
        team_check: r.text("team_check"),
        reaction_power: r.int("reaction_power"),
        scrum: r.yesno("scrum"),
        armor_attack: r.int("armor_attack"),
        form_point: r.int("form_point"),
        magic: r.yesno("magic"),
        invalid_guard: r.yesno("invalid_guard"),
        rapid_fire: r.yesno("rapid_fire"),
        killer: r.yesno("killer"),
        attraction: r.yesno("attraction"),
        counter: r.yesno("counter"),
        weak_guard: r.yesno("weak_guard"),
        reflect_level: r.int("reflect_level"),
        reflect_reaction: r.text("reflect_reaction"),
        revenge_point: r.num("revenge_point"),
        phys_force: r.text("phys_force"),
    }).collect()
});

pub const ATK_DEFAULT: usize = 0;
pub const ATK_DIVEFALL_ATTACK: usize = 1;
pub const ATK_DIVEFALL_ATTACK_END: usize = 2;
pub const ATK_RUN_SLOPE: usize = 3;
pub const ATK_RUN_WALL_RISE: usize = 4;
pub const ATK_RUN_WALL_DOWN: usize = 5;
pub const ATK_RUN_WALL_SIDE: usize = 6;
pub const ATK_FF_SONIC_1: usize = 7;
pub const ATK_FF_SONIC_2: usize = 8;
pub const ATK_FF_SONIC_DOWN: usize = 9;
pub const ATK_FF_SONIC_BOMB: usize = 10;
pub const ATK_FF_SONIC_TURN: usize = 11;
pub const ATK_FF_SONIC_TURN_END: usize = 12;
pub const ATK_FF_SONIC_SS: usize = 13;
pub const ATK_FF_SONIC_SS_END: usize = 14;
pub const ATK_ATK_BARRIERCRACKER: usize = 15;
pub const ATK_BLOW_COUNTER_START: usize = 16;
pub const ATK_BLOW_COUNTER: usize = 17;
pub const ATK_REVENGE_SLASH: usize = 18;
pub const ATK_REVENGE_EX: usize = 19;
pub const ATK_REVENGE_EX_END: usize = 20;
pub const ATK_REVENGE_IMPACT: usize = 21;
pub const ATK_REVENGE_DIVE: usize = 22;
pub const ATK_REVENGE_DIVE_END: usize = 23;
pub const ATK_RDC_HOOKUPPER: usize = 24;
pub const ATK_RDC_HOOKUPPER_END: usize = 25;
pub const ATK_RDC_BACKRAID: usize = 26;
pub const ATK_RDC_BACKRUSH: usize = 27;
pub const ATK_RDC_BACKRUSH_END: usize = 28;
pub const ATK_RDC_WARPSLASH: usize = 29;
pub const ATK_RDC_WARPSLASH_END: usize = 30;
pub const ATK_KB_VERTICAL: usize = 31;
pub const ATK_KB_STAB: usize = 32;
pub const ATK_KB_TURN: usize = 33;
pub const ATK_KB_BREAK: usize = 34;
pub const ATK_KB_TRIPLE: usize = 35;
pub const ATK_KB_TRIPLE_END: usize = 36;
pub const ATK_KB_THRUST: usize = 37;
pub const ATK_KB_THRUST_END: usize = 38;
pub const ATK_KB_FLASH: usize = 39;
pub const ATK_KB_RISE: usize = 40;
pub const ATK_KB_ROLLDOWN: usize = 41;
pub const ATK_KB_ROLLING: usize = 42;
pub const ATK_KB_ROLLING_END: usize = 43;
pub const ATK_KB_DOWN: usize = 44;
pub const ATK_KB_DOWN_FIN: usize = 45;
pub const ATK_KB_DOWN_BOMB: usize = 46;
pub const ATK_KB_LAUNCHSPIN: usize = 47;
pub const ATK_KB_ROLLBEAT: usize = 48;
pub const ATK_UNISON_FIRE: usize = 49;
pub const ATK_UNISON_BLIZZARD: usize = 50;
pub const ATK_UNISON_BLIZZARD_END: usize = 51;
pub const ATK_UNISON_THUNDER: usize = 52;
pub const ATK_FUSION_SPIN: usize = 53;
pub const ATK_FUSION_ROCKET: usize = 54;
pub const ATK_LM_CHANGE: usize = 55;
pub const ATK_LM_STRIDEBREAK: usize = 56;
pub const ATK_LM_STRIDEBREAK2: usize = 57;
pub const ATK_LM_EXPLOSION: usize = 58;
pub const ATK_LM_EXPLOSION2: usize = 59;
pub const ATK_LM_RIPPLEDRIVE: usize = 60;
pub const ATK_LM_HURRICANEPERIOD: usize = 61;
pub const ATK_LM_HURRICANEPERIOD2: usize = 62;
pub const ATK_LM_AERIALFINISH: usize = 63;
pub const ATK_LM_AERIALFINISH2: usize = 64;
pub const ATK_LM_MAGNESPLASH: usize = 65;
pub const ATK_LM_MAGNESPLASH2: usize = 66;
pub const ATK_LM_STUNIMPACT: usize = 67;
pub const ATK_LM_SONICRAVE: usize = 68;
pub const ATK_LM_SONICRAVE_END: usize = 69;
pub const ATK_LM_LASTARCANUM: usize = 70;
pub const ATK_LM_LASTARCANUM2: usize = 71;
pub const ATK_LM_LASTARCANUM_BREAK: usize = 72;
pub const ATK_LM_LASTARCANUM_BREAK2: usize = 73;
pub const ATK_LM_LASTARCANUM_BREAK3: usize = 74;
pub const ATK_LM_LASTARCANUM_BREAK4: usize = 75;
pub const ATK_LM_LASTARCANUM_BREAK5: usize = 76;
pub const ATK_LM_LASTARCANUM_END: usize = 77;
pub const ATK_LM_LASTARCANUM_END2: usize = 78;
pub const ATK_LM_UNIONDISCODE: usize = 79;
pub const ATK_LM_UNIONDISCODE2: usize = 80;
pub const ATK_LM_UNIONDISCODE3: usize = 81;
pub const ATK_LM_UNIONDISCODE4: usize = 82;
pub const ATK_FR_SHIELDGUARDIAN_SHIELDBASH: usize = 83;
pub const ATK_FR_SHIELDGUARDIAN_SHIELDBASH2: usize = 84;
pub const ATK_FR_SHIELDGUARDIAN_HEAVYBASH: usize = 85;
pub const ATK_FR_MOUNTCURLING_TACKLE: usize = 86;
pub const ATK_FR_GOOFYSHOOT_SORA: usize = 87;
pub const ATK_FR_GOOFYSHOOT_EXP: usize = 88;
pub const ATK_FR_JACKSPIN_SORA_R: usize = 89;
pub const ATK_FR_JACKSPIN_SORA_L: usize = 90;
pub const ATK_FR_JACKSPIN_SORA_F: usize = 91;
pub const ATK_FR_JACKSPIN_BOMB_EXP: usize = 92;
pub const ATK_FR_MIKEBOWLING_HOWLING: usize = 93;
pub const ATK_FR_WBROCKET_FLIGHT_LOOP: usize = 94;
pub const ATK_FR_WBROCKET_ATTACK: usize = 95;
pub const ATK_FR_WBROCKET_FINISH: usize = 96;
pub const ATK_FR_WBROCKET_FINISH_EXP: usize = 97;
pub const ATK_FR_WBROCKET_START: usize = 98;
pub const ATK_FR_POWERSTRIKE_SPIN: usize = 99;
pub const ATK_FR_POWERSTRIKE_FINISH: usize = 100;
pub const ATK_FR_POWERSTRIKE_FINISH_EXP: usize = 101;
pub const ATK_FR_GIANTSODIER_ATTACK_L: usize = 102;
pub const ATK_FR_GIANTSODIER_ATTACK_R: usize = 103;
pub const ATK_FR_GIANTSODIER_FINISH: usize = 104;
pub const ATK_FR_SPINNINGHOLD_SPIN: usize = 105;
pub const ATK_FR_SPINNINGHOLD_FINISH_EXP: usize = 106;
pub const ATK_FR_SPINNINGHOLD_FINISH: usize = 107;
pub const ATK_FR_BAYMAXRIDE_SHOT: usize = 108;
pub const ATK_FR_BAYMAXRIDE_COMBO1: usize = 109;
pub const ATK_FR_BAYMAXRIDE_COMBO2: usize = 110;
pub const ATK_FR_BAYMAXRIDE_COMBO3: usize = 111;
pub const ATK_FR_BAYMAXRIDE_COMBOFINISH: usize = 112;
pub const ATK_FR_BAYMAXRIDE_COMBORAIL: usize = 113;
pub const ATK_FR_BAYMAXRIDE_FINISH: usize = 114;
pub const ATK_FR_SNOWCOVERSWING_ATTACK: usize = 115;
pub const ATK_FR_CALLMETEOR_FINISH: usize = 116;
pub const ATK_FR_CALLMETEOR_FINISH_EXP: usize = 117;
pub const ATK_FR_FLAREFORCE_FINISH01: usize = 118;
pub const ATK_FR_FLAREFORCE_FINISH02: usize = 119;
pub const ATK_FR_FLAREFORCE_FINISH03: usize = 120;
pub const ATK_FR_FLAREFORCE_FINISH_EXP: usize = 121;
pub const ATK_FR_ICEREVENGER_ATTACK: usize = 122;
pub const ATK_FR_GIANTSODIER_FINISH_EXP: usize = 123;
pub const ATK_FR_GIANTSODIER_ATTACK_JUMP: usize = 124;
pub const ATK_KB_QUICKSHAFT: usize = 125;
pub const ATK_KB_QUICKSHAFT_END: usize = 126;
pub const ATK_KB_FLASHSTEP: usize = 127;
pub const ATK_KB_FLASHSTEP_END: usize = 128;
pub const ATK_KB_RADIALCUT: usize = 129;
pub const ATK_KB_FINALHEAVEN: usize = 130;
pub const ATK_KB_FINALHEAVEN_END: usize = 131;
pub const ATK_KB_AERIALSWEEP: usize = 132;
pub const ATK_KB_AERIALSWEEP_END: usize = 133;
pub const ATK_KB_AERIALDIVE: usize = 134;
pub const ATK_FR_POPPINGHOLY_BULLET: usize = 135;
pub const ATK_FR_POPPINGHOLY_PILLAR: usize = 136;
pub const ATK_FR_POPPINGHOLY_FINISH: usize = 137;
pub const ATK_FR_POPPINGHOLY_FINISH_SPIN: usize = 138;
pub const ATK_FR_ULTIMATE_END_EXPLOSION: usize = 139;
pub const ATK_FR_PROMISEWINGS_TACKLE: usize = 140;
pub const ATK_FR_PROMISEWINGS_FINISH: usize = 141;
pub const ATK_FR_PROMISEWINGS_FINISH_END: usize = 142;
pub const ATK_AN_CRIMSONBLAST: usize = 143;
pub const ATK_AN_CRIMSONBLAST_END: usize = 144;
pub const ATK_AN_CRIMSONBLAST_IMPACT: usize = 145;
pub const ATK_AN_CRIMSONBLAST_RC1: usize = 146;
pub const ATK_AN_CRIMSONBLAST_RC2: usize = 147;
pub const ATK_AN_CRIMSONBLAST_RC3: usize = 148;
pub const ATK_AN_RISKCHARGE: usize = 149;
pub const ATK_AN_COUNTER: usize = 150;
pub const ATK_AN_COUNTER_END: usize = 151;
pub const ATK_SCHM_GIANTSTAMP: usize = 152;
pub const ATK_SCSK_CHANDELIER: usize = 153;
pub const ATK_SCSK_CHANDELIER_END: usize = 154;
pub const ATK_SCSW_MYRIAD: usize = 155;
pub const ATK_SCSW_MYRIAD_END: usize = 156;
pub const ATK_SF_REFLECTIVELASER: usize = 157;
pub const ATK_SF_REFLECTIVELASER_RANGE: usize = 158;
pub const ATK_SCLF_ZANTETSUKEN: usize = 159;
pub const ATK_SCLF_FINISHRISE: usize = 160;
pub const ATK_SCLF_EVENTHORIZON: usize = 161;
pub const ATK_SCLF_EVENTHORIZON_END: usize = 162;
pub const ATK_SCLF_TRIPLE: usize = 163;
pub const ATK_SCLF_FOURTH: usize = 164;
pub const ATK_SCLF_FOURTH_END: usize = 165;
pub const ATK_SCLF_SPIRALRUSH_START: usize = 166;
pub const ATK_SCLF_SPIRALRUSH: usize = 167;
pub const ATK_SCLF_SPIRALRUSH_END: usize = 168;
pub const ATK_SF_LFSHOOT: usize = 169;

pub fn attack_data_by_id(key: &str) -> Option<&'static AttackDataRow> {
    ATTACK_DATA.iter().find(|row| row.id == key)
}
