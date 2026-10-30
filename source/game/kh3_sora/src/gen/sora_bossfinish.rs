// The types of the reference's sheet sora-bossfinish; values come from the player's files and the executable's
// constants at run time (crate::sheets).

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BossFinishParams {
    /// 0x14222C9D0
    pub speed_channel: f32,
    /// 0x144119CE0
    pub speed_rate: f32,
    /// 0x14411A230
    pub speed_interp: f32,
    /// 0x144119CE0
    pub speed_min: f32,
    /// 0x14222A250
    pub speed_reset_interp: f32,
    /// 0x14223D9D0
    pub cycle: f32,
    pub cut_count: i32,
    pub cut_blend: f32,
    pub cut_priority: f32,
    /// 0x14060A640
    pub cut_move_time: f32,
    /// 0x14060A640
    pub cut_move_curve: f32,
    pub cut_bone: &'static str,
    pub cut_1_subject: &'static str,
    pub cut_1_delay: f32,
    /// 0x14060B9C0
    pub cut_1_distance: [f32; 2],
    /// 0x14060B9C0
    pub cut_1_rotation: [f32; 3],
    /// 0x14060A640
    pub cut_1_rotation_end: [f32; 3],
    /// 0x14060B9C0
    pub cut_1_local_offset: [f32; 3],
    pub cut_1_collision: f32,
    pub cut_2_subject: &'static str,
    /// 0x140E70A70
    pub cut_2_delay: f32,
    pub cut_2_distance: [f32; 2],
    pub cut_2_rotation: [f32; 3],
    pub cut_2_rotation_end: [f32; 3],
    pub cut_2_local_offset: [f32; 3],
    pub cut_2_collision: f32,
    pub cut_3_subject: &'static str,
    pub cut_3_delay: f32,
    pub cut_3_distance: [f32; 2],
    pub cut_3_rotation: [f32; 3],
    pub cut_3_rotation_end: [f32; 3],
    pub cut_3_local_offset: [f32; 3],
    pub cut_3_collision: f32,
    pub shake_scale: f32,
    pub shake_real_time: f32,
    pub shake_duration: f32,
    pub shake_blend_in: f32,
    pub shake_blend_out: f32,
    pub shake_pitch: [f32; 2],
    pub shake_yaw: [f32; 2],
    /// 0x142A266D0
    pub blur_transition: f32,
    pub blur_intensity: f32,
    pub blur_centre: [f32; 2],
    pub blur_offset: [f32; 2],
    pub blur_range: f32,
    pub blur_hole: f32,
    pub blur_attenuation_inside: f32,
    pub blur_end_transition: f32,
    /// 0x144A1C040
    pub white_delay: f32,
    /// 0x140E81E30
    pub white_time: f32,
    /// 0x140E81E30
    pub white_colour: [f32; 3],
    pub after_hold: f32,
    /// 0x14221F260
    pub after_fade_in: f32,
    /// 0x14221E440
    pub after_prizes: f32,
    pub sound_static_se: f32,
    pub sound_static_se_alt: f32,
    /// 0x141515D60
    pub sound_mode: f32,
    pub sound_bgm_fade: f32,
    /// 0x14222F940
    pub hud_hide: f32,
    pub input_lock: f32,
    pub port_watchdog: f32,
    pub port_camera_collision: f32,
    pub port_probe_radius: f32,
    pub battle_rate: f32,
    pub battle_interp: f32,
    pub battle_hold: f32,
    pub battle_out: f32,
}

pub static BOSS_FINISH_PARAMS: std::sync::LazyLock<BossFinishParams> = std::sync::LazyLock::new(|| {
    let s = crate::sheets::Sheet::named("sora-bossfinish");
    BossFinishParams {
        speed_channel: s.num("speed.channel"),
        speed_rate: s.num("speed.rate"),
        speed_interp: s.num("speed.interp"),
        speed_min: s.num("speed.min"),
        speed_reset_interp: s.num("speed.reset_interp"),
        cycle: s.num("cycle"),
        cut_count: s.int("cut.count"),
        cut_blend: s.num("cut.blend"),
        cut_priority: s.num("cut.priority"),
        cut_move_time: s.num("cut.move_time"),
        cut_move_curve: s.num("cut.move_curve"),
        cut_bone: s.text("cut.bone"),
        cut_1_subject: s.text("cut.1.subject"),
        cut_1_delay: s.num("cut.1.delay"),
        cut_1_distance: s.arr("cut.1.distance"),
        cut_1_rotation: s.arr("cut.1.rotation"),
        cut_1_rotation_end: s.arr("cut.1.rotation_end"),
        cut_1_local_offset: s.arr("cut.1.local_offset"),
        cut_1_collision: s.num("cut.1.collision"),
        cut_2_subject: s.text("cut.2.subject"),
        cut_2_delay: s.num("cut.2.delay"),
        cut_2_distance: s.arr("cut.2.distance"),
        cut_2_rotation: s.arr("cut.2.rotation"),
        cut_2_rotation_end: s.arr("cut.2.rotation_end"),
        cut_2_local_offset: s.arr("cut.2.local_offset"),
        cut_2_collision: s.num("cut.2.collision"),
        cut_3_subject: s.text("cut.3.subject"),
        cut_3_delay: s.num("cut.3.delay"),
        cut_3_distance: s.arr("cut.3.distance"),
        cut_3_rotation: s.arr("cut.3.rotation"),
        cut_3_rotation_end: s.arr("cut.3.rotation_end"),
        cut_3_local_offset: s.arr("cut.3.local_offset"),
        cut_3_collision: s.num("cut.3.collision"),
        shake_scale: s.num("shake.scale"),
        shake_real_time: s.num("shake.real_time"),
        shake_duration: s.num("shake.duration"),
        shake_blend_in: s.num("shake.blend_in"),
        shake_blend_out: s.num("shake.blend_out"),
        shake_pitch: s.arr("shake.pitch"),
        shake_yaw: s.arr("shake.yaw"),
        blur_transition: s.num("blur.transition"),
        blur_intensity: s.num("blur.intensity"),
        blur_centre: s.arr("blur.centre"),
        blur_offset: s.arr("blur.offset"),
        blur_range: s.num("blur.range"),
        blur_hole: s.num("blur.hole"),
        blur_attenuation_inside: s.num("blur.attenuation_inside"),
        blur_end_transition: s.num("blur.end_transition"),
        white_delay: s.num("white.delay"),
        white_time: s.num("white.time"),
        white_colour: s.arr("white.colour"),
        after_hold: s.num("after.hold"),
        after_fade_in: s.num("after.fade_in"),
        after_prizes: s.num("after.prizes"),
        sound_static_se: s.num("sound.static_se"),
        sound_static_se_alt: s.num("sound.static_se_alt"),
        sound_mode: s.num("sound.mode"),
        sound_bgm_fade: s.num("sound.bgm_fade"),
        hud_hide: s.num("hud.hide"),
        input_lock: s.num("input.lock"),
        port_watchdog: s.num("port.watchdog"),
        port_camera_collision: s.num("port.camera_collision"),
        port_probe_radius: s.num("port.probe_radius"),
        battle_rate: s.num("battle.rate"),
        battle_interp: s.num("battle.interp"),
        battle_hold: s.num("battle.hold"),
        battle_out: s.num("battle.out"),
    }
});
