// The row type of the reference's sheet sora-combat-attacks; rows come from the player's files at run time (crate::sheets).

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AttackKindRow {
    pub kind: i32,
    pub label: &'static str,
    pub anim: &'static str,
    pub clip: &'static str,
    pub phase: &'static str,
    pub role: &'static str,
    pub requires: &'static str,
    pub root_len: Option<f32>,
    pub stop_dist: f32,
    pub z_gain: f32,
    pub air_mode: bool,
    pub keep_face: bool,
    pub length: f32,
    pub frames: i32,
    pub root_motion: bool,
    pub root_fwd: f32,
    pub root_up: f32,
    pub enable_input: Option<f32>,
    pub enable_change: Option<f32>,
    pub attack_rows: &'static [&'static str],
}

pub static ATTACK_KINDS: std::sync::LazyLock<Vec<AttackKindRow>> = std::sync::LazyLock::new(|| {
    crate::sheets::Table::named("sora-combat-attacks").rows().map(|r| AttackKindRow {
        kind: r.int("kind"),
        label: r.text("label"),
        anim: r.text("anim"),
        clip: r.text("clip"),
        phase: r.text("phase"),
        role: r.text("role"),
        requires: r.text("requires"),
        root_len: r.opt_num("root_len"),
        stop_dist: r.num("stop_dist"),
        z_gain: r.num("z_gain"),
        air_mode: r.yesno("air_mode"),
        keep_face: r.yesno("keep_face"),
        length: r.num("length"),
        frames: r.int("frames"),
        root_motion: r.yesno("root_motion"),
        root_fwd: r.num("root_fwd"),
        root_up: r.num("root_up"),
        enable_input: r.opt_num("enable_input"),
        enable_change: r.opt_num("enable_change"),
        attack_rows: r.texts("attack_rows"),
    }).collect()
});

pub const KIND_01: usize = 0;
pub const KIND_02: usize = 1;
pub const KIND_03: usize = 2;
pub const KIND_04: usize = 3;
pub const KIND_05: usize = 4;
pub const KIND_06: usize = 5;
pub const KIND_07: usize = 6;
pub const KIND_08: usize = 7;
pub const KIND_09: usize = 8;
pub const KIND_0A: usize = 9;
pub const KIND_0B: usize = 10;
pub const KIND_0C: usize = 11;
pub const KIND_0D: usize = 12;
pub const KIND_0E: usize = 13;
pub const KIND_0F: usize = 14;
pub const KIND_10: usize = 15;
pub const KIND_11: usize = 16;
pub const KIND_12: usize = 17;
pub const KIND_13: usize = 18;
pub const KIND_14: usize = 19;
pub const KIND_15: usize = 20;
pub const KIND_16: usize = 21;
pub const KIND_17: usize = 22;
pub const KIND_18: usize = 23;
pub const KIND_19: usize = 24;
pub const KIND_1A: usize = 25;
pub const KIND_1B: usize = 26;
pub const KIND_1C: usize = 27;
pub const KIND_1D: usize = 28;
pub const KIND_1F: usize = 29;
pub const KIND_20: usize = 30;
pub const KIND_21: usize = 31;
pub const KIND_22: usize = 32;
pub const KIND_23: usize = 33;
pub const KIND_24: usize = 34;
pub const KIND_27: usize = 35;
pub const KIND_28: usize = 36;
pub const KIND_29: usize = 37;
pub const KIND_2A: usize = 38;
pub const KIND_2B: usize = 39;
pub const KIND_35: usize = 40;
pub const KIND_31: usize = 41;
pub const KIND_32: usize = 42;
pub const KIND_33: usize = 43;
pub const KIND_34: usize = 44;
pub const KIND_2F: usize = 45;
pub const KIND_39: usize = 46;
pub const KIND_36: usize = 47;
pub const KIND_37: usize = 48;
pub const KIND_38: usize = 49;
pub const KIND_7A: usize = 50;
pub const KIND_7B: usize = 51;
pub const KIND_7C: usize = 52;
pub const KIND_7D: usize = 53;
pub const KIND_7E: usize = 54;
pub const KIND_7F: usize = 55;
pub const KIND_E0: usize = 56;
pub const KIND_E1: usize = 57;
pub const KIND_E2: usize = 58;
pub const KIND_E3: usize = 59;
pub const KIND_E4: usize = 60;
pub const KIND_E5: usize = 61;
pub const KIND_E6: usize = 62;
pub const KIND_E7: usize = 63;
pub const KIND_E8: usize = 64;
pub const KIND_E9: usize = 65;
pub const KIND_EA: usize = 66;
pub const KIND_EB: usize = 67;
pub const KIND_EC: usize = 68;
pub const KIND_ED: usize = 69;
