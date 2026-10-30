// The types of the reference's sheet sora-combat-params; values come from the player's files and the executable's
// constants at run time (crate::sheets).

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SoraCombatParams {
    /// 0x141433570
    pub combo_base_max: i32,
    /// 0x141433A60
    pub combo_finishers_base: i32,
    /// 0x141433570
    pub combo_second_form_max: i32,
    /// 0x141433A60
    pub combo_second_form_finishers: i32,
    /// 0x141341340
    pub swim_exit_delay: f32,
    /// 0x14134ACA0
    pub decide_far_dist: f32,
    /// 0x14134ACA0
    pub decide_crowd_count: i32,
    /// 0x141404E50
    pub decide_crowd_radius: f32,
    /// 0x141330D30
    pub decide_dist_offset: f32,
    /// 0x14134ACA0
    pub decide_air_below_dz: f32,
    /// 0x14134ACA0
    pub decide_air_down_dz: f32,
    /// 0x141330D30
    pub decide_rise_dz: f32,
    /// 0x14134ACA0
    pub decide_rise_speed: f32,
    /// 0x14134C420
    pub decide_ground_probe_range: f32,
    /// 0x14134C420
    pub decide_ground_probe_max: f32,
    /// 0x14134ACA0
    pub decide_launch_air_dz: f32,
    /// 0x14134D0E0
    pub face_rate: f32,
    /// 0x14134D0E0
    pub face_timer: f32,
    /// 0x141349CC0
    pub traction_speed: f32,
    /// 0x141349CC0
    pub traction_stop_dist: f32,
    /// 0x141349CC0
    pub traction_rate_on: f32,
    /// 0x141349CC0
    pub traction_rate_off_ground: f32,
    /// 0x141349CC0
    pub traction_rate_off_air: f32,
    /// 0x141349CC0
    pub traction_first_air_scale: f32,
    /// 0x141349CC0
    pub traction_dist_clamp: f32,
    /// 0x141349CC0
    pub traction_dz_clamp: f32,
    /// 0x141349CC0
    pub traction_z_gain: f32,
    /// 0x141330D30
    pub traction_z_initial_gain: f32,
    /// 0x141330D30
    pub traction_z_initial_max: f32,
    /// 0x141349CC0
    pub traction_z_decay: f32,
    /// 0x1413378B0
    pub air_gravity_scale: f32,
    /// 0x1413378B0
    pub air_hold_time: f32,
    /// 0x141349CC0
    pub root_scale_rule: &'static str,
    /// 0x141423210
    pub hitstop_small: f32,
    /// 0x141423210
    pub hitstop_big: f32,
    /// 0x141423210
    pub style_max: i32,
    /// 0x141423210
    pub style_add_rate: f32,
    /// 0x141423210
    pub style_add_rate_lv0: f32,
    /// 0x141423210
    pub style_add_rate_lv1: f32,
    /// 0x141423210
    pub style_add_rate_lv2: f32,
    /// 0x141423210
    pub style_dec_start_time: f32,
    /// 0x141423210
    pub style_dec_interval: f32,
    /// 0x141423210
    pub style_dec: i32,
    /// 0x141423210
    pub guard_hold_time: f32,
    /// 0x141423210
    pub reaction_cooldown: f32,
    /// 0x140632240
    pub damage_min_base: f32,
    /// 0x140632240
    pub damage_epsilon: f32,
    /// 0x140632240
    pub damage_resist_max_percent: f32,
    /// 0x140632240
    pub damage_combo_up_per_step: f32,
    /// 0x140632240
    pub damage_counter_up: f32,
    /// 0x140632240
    pub damage_critical_counter: f32,
    /// 0x140632240
    pub damage_attribute_up: f32,
    /// 0x140632240
    pub damage_buster: f32,
    /// 0x140632240
    pub damage_rapid_fire_resist_field: &'static str,
    /// 0x140632240
    pub damage_zero_result: f32,
    /// 0x140632240
    pub damage_random: &'static str,
    /// 0x141404E50
    pub target_near_range: f32,
    /// 0x141404E50
    pub target_min_dist: f32,
    /// 0x141404E50
    pub target_max_range: f32,
    /// 0x141404E50
    pub target_switch_range: f32,
    /// 0x141404E50
    pub target_facing_weight: f32,
    /// 0x141404E50
    pub target_origin_push: f32,
    /// 0x141341340
    pub attack_cancel_delay: f32,
    /// 0x141335120
    pub attack_whiff_air_command_cooldown: f32,
    /// 0x141335120
    pub attack_whiff_air_airslide_cooldown: f32,
    /// 0x141335120
    pub attack_whiff_air_airguard_cooldown: f32,
    /// 0x1413378B0
    pub attack_air_cooldown_arm: f32,
    /// 0x141426330
    pub attack_hit_confirm_required: f32,
    /// 0x1416220D0
    pub attack_request_phase: &'static str,
    /// 0x1416220D0
    pub attack_request_extra_latency_frames: i32,
    /// 0x1416220D0
    pub command_kind_attack: f32,
    /// 0x14141B470
    pub command_attack_reaction_block: f32,
    /// 0x14141B470
    pub command_grand_magic_cooldown: f32,
    /// 0x141415A50
    pub command_reaction_cooldown_time: f32,
    /// 0x14142B3F0
    pub launch_unique_id: f32,
    /// 0x14142B3F0
    pub launch_stock_available: i32,
    /// 0x14134ACA0
    pub launch_groundbreaker_dz_max: f32,
    /// 0x14134ACA0
    pub launch_rising_dirz: f32,
    /// 0x14134ACA0
    pub launch_combo_after: i32,
    /// 0x141331AF0
    pub recoil_travel: f32,
    /// 0x141437330
    pub counter_forced_kind_default: f32,
    /// 0x1413F85D0
    pub just_guard_timer_countdown_fn: &'static str,
    /// 0x141430A60
    pub friend_finish_active_for_solo: f32,
    /// 0x1413378B0
    pub attack_move_skip_calc_velocity: f32,
    /// 0x141333210
    pub attack_move_begin_stop: f32,
    /// 0x141349CC0
    pub traction_xy_speed: f32,
    /// 0x141349CC0
    pub traction_xy_rate_open: f32,
    /// 0x141349CC0
    pub traction_xy_rate_stop: f32,
    /// 0x141349CC0
    pub traction_xy_rate_stop_air: f32,
    /// 0x1413364D0
    pub traction_approach_snap: f32,
    /// 0x141349CC0
    pub traction_dxy_max: f32,
    /// 0x141349CC0
    pub traction_z_decay_rate: f32,
    /// 0x141349CC0
    pub traction_z_rearm_gain: f32,
    /// 0x141349CC0
    pub traction_dive_blend: f32,
    /// 0x141349CC0
    pub traction_scale_inside: f32,
    /// 0x14056B420
    pub root_scale_clamp: &'static str,
    /// 0x1413378B0
    pub air_attack_gravity_scale: f32,
    /// 0x1413378B0
    pub air_attack_mode_start: f32,
    /// 0x141349CC0
    pub air_attack_mode_after_window: f32,
    /// 0x141349CC0
    pub air_attack_fall_accel_window_to_change: f32,
    /// 0x141330D30
    pub target_lead_time: f32,
    /// 0x1413417F0
    pub level_rate: f32,
    /// 0x140637430
    pub body_push_level_sora: f32,
    /// 0x14057AE30
    pub body_push_capsule_blocks_pawns: f32,
    /// 0x14057ABD0
    pub body_push_shape_radius: f32,
    /// 0x14062A920
    pub hit_trace_channel: f32,
    /// 0x14062A920
    pub hit_sweep_ignore_channels: &'static str,
    /// 0x14062A920
    pub hit_sweep_ignore_pawn_roots: &'static str,
    /// 0x140642CA0
    pub hit_sweep_world_static_needs_maphit: f32,
    /// 0x140643F40
    pub hit_degenerate_radius: f32,
    /// 0x140647950
    pub hit_capsule_min_half_length: f32,
    /// 0x140647950
    pub hit_first_tick_zero_sweep: f32,
    /// 0x144526380
    pub hit_max_touches: i32,
    /// 0x140618940
    pub hit_exec_rate_roll_scale: f32,
    /// 0x140618940
    pub hit_exec_rate_fail_records_victim: f32,
    /// 0x140618940
    pub hit_bad_status_roll_scale: f32,
    /// 0x140618940
    pub hit_map_floor_cos: f32,
    /// 0x141342760
    pub hit_state_floor_cos: f32,
    /// 0x141342760
    pub hit_state_ceiling_cos: f32,
    /// 0x141342760
    pub hit_state_low_wall_height: f32,
    /// 0x141777570
    pub hit_weapon_map_height_limit: f32,
    /// 0x140618940
    pub hit_nearest_only_initial: f32,
    /// 0x140625500
    pub hit_counter_byte_max: i32,
    /// 0x140570870
    pub hit_default_row_power: f32,
    /// 0x140645900
    pub hit_rate_clamp_max: f32,
    /// 0x141343800
    pub hit_reflect_clash_only_first_swing: f32,
    /// 0x140618940
    pub hit_clash_stronger_result: f32,
    /// 0x14061FE20
    pub hit_reflect1_level_pivot: f32,
    /// 0x14061EA30
    pub knockback_degenerate_sq: f32,
    /// 0x14061EA30
    pub knockback_initial: &'static str,
    /// 0x14061EA30
    pub knockback_flattened: f32,
    /// 0x141444090
    pub gain_hitstop_applied: f32,
    /// 0x141442DE0
    pub gain_focus_per_hit: f32,
    /// 0x141442DE0
    pub gain_style_needs_hp_loss: f32,
    /// 0x1413E9000
    pub gain_style_form_up_bonus: f32,
    /// 0x1413E9000
    pub gain_style_default_kind: f32,
    /// 0x1413E9000
    pub gain_style_bonus_halving: f32,
    /// 0x1413E9000
    pub gain_style_blocked_state: f32,
    /// 0x140628960
    pub effect_random_offset_radius: f32,
    /// 0x140628960
    pub effect_sound_volume: f32,
    /// 0x140628960
    pub effect_no_sound_result: f32,
    /// 0x140628960
    pub effect_map_hit_uses_window_sound: f32,
    /// 0x14041F340
    pub effect_critical_flag_ever_set: f32,
    /// 0x141404E50
    pub target_lock_off_range: f32,
    /// 0x141404E50
    pub target_switch_max: i32,
    /// 0x141448550
    pub target_origin_height: f32,
    /// 0x141404E50
    pub target_crowd_range: f32,
    /// 0x141404E50
    pub target_non_enemy_range_factor: f32,
    /// 0x141404E50
    pub target_near_remap_factor: f32,
    /// 0x141404E50
    pub target_initial_best_margin: f32,
    /// 0x1413F0700
    pub target_tap_time: f32,
    /// 0x1413F0750
    pub target_los_channel: f32,
    /// 0x141423210
    pub target_dive_range_xy: f32,
    /// 0x141423210
    pub target_dive_limit_z: f32,
    /// 0x14060AB70
    pub lockon_cam_blend_time: f32,
    /// 0x14060AB70
    pub lockon_cam_blend_exponent: f32,
    /// 0x1405FC7E0
    pub lockon_cam_distance_min: f32,
    /// 0x1405FC7E0
    pub lockon_cam_distance_rate: f32,
    /// 0x1405FC450
    pub lockon_cam_base_pitch_offset: f32,
    /// 0x140606190
    pub lockon_cam_base_pitch_floor: f32,
    /// 0x140613300
    pub lockon_cam_look_yaw_bias: f32,
    /// 0x1405FC450
    pub lockon_cam_pitch_min: f32,
    /// 0x1405FC450
    pub lockon_cam_pitch_max: f32,
    /// 0x1405FC450
    pub lockon_cam_yaw_limit: f32,
    /// 0x140607390
    pub lockon_cam_stick_far_dist: f32,
    /// 0x140607390
    pub lockon_cam_stick_near_dist: f32,
    /// 0x140607390
    pub lockon_cam_stick_far_rate: f32,
    /// 0x1405FFA60
    pub lockon_cam_auto_low_dist: f32,
    /// 0x1405FFA60
    pub lockon_cam_auto_high_dist: f32,
    /// 0x1405FFA60
    pub lockon_cam_auto_rate_gain: f32,
    /// 0x140611170
    pub lockon_cam_offscreen_snap: f32,
    /// 0x1406100D0
    pub lockon_cam_lead_factor: f32,
    /// 0x1406100D0
    pub lockon_cam_target_jump_speed: f32,
    /// 0x1405DEFA0
    pub lockon_cam_framing_start: f32,
    /// 0x1413518D0
    pub hit_small_power: f32,
    /// 0x141351860
    pub hit_small_brake: f32,
    /// 0x141351950
    pub hit_small_start_falling_speed: f32,
    /// 0x141351010
    pub hit_small_air_velocity_z: f32,
    /// 0x141351270
    pub hit_blow_power: f32,
    /// 0x141351340
    pub hit_blow_velocity_z_ground: f32,
    /// 0x141351340
    pub hit_blow_velocity_z_air: f32,
    /// 0x1413511D0
    pub hit_blow_brake_ground: f32,
    /// 0x1413511D0
    pub hit_blow_brake_air: f32,
    /// 0x141350F90
    pub hit_blow_air_start_brake_time: f32,
    /// 0x1413512E0
    pub hit_blow_start_falling_speed: f32,
    /// 0x141351A40
    pub hit_toss_power: f32,
    /// 0x1413519E0
    pub hit_toss_flight_duration: f32,
    /// 0x141351980
    pub hit_toss_fall_gravity_scale: f32,
    /// 0x141351590
    pub hit_launch_power: f32,
    /// 0x141351530
    pub hit_launch_flight_duration: f32,
    /// 0x1413514D0
    pub hit_launch_fall_gravity_scale: f32,
    /// 0x1413517E0
    pub hit_parabola_power: f32,
    /// 0x141351690
    pub hit_parabola_angle: f32,
    /// 0x141351710
    pub hit_parabola_brake: f32,
    /// 0x141351780
    pub hit_parabola_limit_brake: f32,
    /// 0x141351090
    pub hit_beat_power: f32,
    /// 0x141351110
    pub hit_beat_rebound: f32,
    /// 0x141351170
    pub hit_beat_rebound_gravity_scale: f32,
    /// 0x14043A5D0
    pub hit_wall_angle: f32,
    /// 0x14043A5E0
    pub hit_wall_min_speed: f32,
    /// 0x141365930
    pub hit_recover_delay: f32,
    /// 0x14135D740
    pub hit_turn_alpha_rate: f32,
    /// 0x140431E10
    pub hit_brake_rate: f32,
    /// 0x14135FD80
    pub hit_move_length_rate: f32,
    /// 0x14135B300
    pub hit_blow_min_flight_time: f32,
    /// 0x14135B300
    pub hit_blow_drop_exit: f32,
    /// 0x14135EEA0
    pub hit_toss_rise_gravity_scale: f32,
    /// 0x14135EEA0
    pub hit_toss_hover_gravity_scale: f32,
    /// 0x14135EEA0
    pub hit_toss_fall_timeout: f32,
    /// 0x14135C410
    pub hit_launch_fall_timeout: f32,
    /// 0x14135CAB0
    pub hit_parabola_loop_time: f32,
    /// 0x14135CAB0
    pub hit_parabola_air_timeout: f32,
    /// 0x14135A870
    pub hit_beat_trace_range: f32,
    /// 0x14135A870
    pub hit_beat_radius_factor: f32,
    /// 0x14135A870
    pub hit_beat_land_blend: f32,
    /// 0x1405A94E0
    pub hit_dir_front_half_angle: f32,
    /// 0x1405A2A00
    pub hp_combo_leave_window: f32,
    /// 0x1405A2A00
    pub hp_last_leave_min_hp: f32,
    /// 0x14141B470
    pub hit_reaction_block_after_command: f32,
    /// 0x14134C5E0
    pub recovery_command_time: f32,
    /// 0x141332930
    pub recovery_blow_counter_command: f32,
    /// 0x141332930
    pub recovery_blow_counter_attack_kind: f32,
    /// 0x1413733D0
    pub die_speed: f32,
    /// 0x1413733D0
    pub die_velocity_z: f32,
    /// 0x1413733D0
    pub die_brake: f32,
    /// 0x1413733D0
    pub die_phase1_time: f32,
    /// 0x1405BE820
    pub revive_no_damage_time: f32,
    /// 0x1405BE820
    pub revive_hp_fraction: f32,
    /// 0x140582270
    pub gravity_base: f32,
    /// 0x141423210
    pub form_gauge_max: f32,
    /// 0x1413E9000
    pub form_gauge_add_rate: f32,
    /// 0x1413E9000
    pub form_gauge_add_rate_lv0: f32,
    /// 0x1413E9000
    pub form_gauge_add_rate_lv1: f32,
    /// 0x1413E9000
    pub form_gauge_add_rate_lv2: f32,
    /// 0x1413E9000
    pub form_gauge_critical_add_rate: f32,
    /// 0x1413E9000
    pub form_gauge_critical_add_rate_lv0: f32,
    /// 0x1413E9000
    pub form_gauge_critical_add_rate_lv1: f32,
    /// 0x1413E9000
    pub form_gauge_critical_add_rate_lv2: f32,
    /// 0x1413E9000
    pub form_gauge_critical_charge_rate: f32,
    /// 0x1413E9000
    pub form_gauge_form_up_bonus: f32,
    /// 0x1413E9000
    pub form_gauge_chain_first: f32,
    /// 0x1413E9000
    pub form_gauge_chain_decay: f32,
    /// 0x1413E9000
    pub form_gauge_default_cmd: f32,
    /// 0x1413E9000
    pub form_gauge_no_gain_state: f32,
    /// 0x1413E9000
    pub form_gauge_dec_start_time: f32,
    /// 0x14141C4F0
    pub form_gauge_dec_interval: f32,
    /// 0x14141C4F0
    pub form_gauge_dec_amount: f32,
    /// 0x1413F0ED0
    pub form_gauge_auto_add_interval: f32,
    /// 0x1413F0ED0
    pub form_gauge_auto_add_points: f32,
    /// 0x1413F0ED0
    pub form_kingdom_key_command: f32,
    /// 0x14140DA40
    pub form_command_life: f32,
    /// 0x14140DA40
    pub form_command_life_per_form_time: f32,
    /// 0x14140DA40
    pub form_command_life_cap: f32,
    /// 0x14141A9D0
    pub form_duration: f32,
    /// 0x14141A9D0
    pub form_duration_default: f32,
    /// 0x14141A9D0
    pub form_level: i32,
    /// 0x14141B470
    pub form_exec_block_time: f32,
    /// 0x141432F00
    pub form_attack_state: f32,
    /// 0x141433570
    pub form_max_combo: i32,
    /// 0x141433A60
    pub form_finishers: i32,
    /// 0x1413CBA70
    pub form_wear_form: f32,
    /// 0x14140E500
    pub form_world_slow_rate: f32,
    /// 0x1413D0C50
    pub form_stylechange_face_time: f32,
    /// 0x1413D3AC0
    pub form_stylechange_turn_gain: f32,
    /// 0x141414C70
    pub form_finish_level_max: f32,
    /// 0x141414C70
    pub form_finish_cmd_lv1: f32,
    /// 0x141414C70
    pub form_finish_cmd_lv2: f32,
    /// 0x141414C70
    pub form_finish_cmd_lv3: f32,
    /// 0x1413EE030
    pub form_finish_offset_time: f32,
    /// 0x1413EE030
    pub form_finish_offset_time_weapon_change: f32,
    /// 0x141447840
    pub form_finish_stylefinish_state: f32,
    /// 0x1413E1FC0
    pub form_finish_sonicrave_dash_root_len: f32,
    /// 0x1413E1FC0
    pub form_finish_sonicrave_end_root_len: f32,
    /// 0x1413E1FC0
    pub form_finish_sonicrave_dashes_before_finish: i32,
    /// 0x1413E1800
    pub form_finish_lastarcanum_windows_before_break: i32,
    /// 0x1413E1800
    pub form_finish_lastarcanum_root_len: &'static str,
    /// 0x1413E1800
    pub form_finish_stop_dist: &'static str,
    /// 0x141401D10
    pub form_end_block_states: &'static str,
    /// 0x1413E9D30
    pub form_end_time_break_status: f32,
    /// 0x1416158F0
    pub queue_capacity: f32,
    /// 0x1416158F0
    pub queue_evict_min_total: f32,
    /// 0x1416158F0
    pub queue_start_delay: f32,
    /// 0x1416158F0
    pub queue_appear_time: f32,
    /// 0x141619E60
    pub queue_class_formchange: f32,
    /// 0x141619E60
    pub queue_class_finish: f32,
    /// 0x141619E60
    pub queue_class_reaction: f32,
    /// 0x141619E60
    pub queue_class_grand_magic: f32,
    /// 0x14140DA40
    pub queue_life_grand_magic: f32,
    /// 0x14140DA40
    pub queue_life_attraction: f32,
    /// 0x14140DA40
    pub queue_life_team_attack: f32,
    /// 0x1413CF7D0
    pub combo_carry_time: f32,
    /// 0x1413E1FC0
    pub form_finish_sonicrave_root_scale_factor: f32,
    /// 0x1413E1800
    pub form_finish_lastarcanum_end_drop_height: f32,
    /// 0x1413E1FC0
    pub form_finish_gravity_scale: f32,
    /// 0x141341B10
    pub chase_command: f32,
    /// 0x14134C660
    pub chase_prompt_time: f32,
    /// 0x14134AA80
    pub chase_warp_delay: f32,
    /// 0x14134AA80
    pub chase_warp_distance: f32,
    /// 0x14134AA80
    pub chase_warp_drop: f32,
}

pub static SORA_COMBAT_PARAMS: std::sync::LazyLock<SoraCombatParams> = std::sync::LazyLock::new(|| {
    let s = crate::sheets::Sheet::named("sora-combat-params");
    SoraCombatParams {
        combo_base_max: s.int("combo.base_max"),
        combo_finishers_base: s.int("combo.finishers_base"),
        combo_second_form_max: s.int("combo.second_form_max"),
        combo_second_form_finishers: s.int("combo.second_form_finishers"),
        swim_exit_delay: s.num("swim.exit_delay"),
        decide_far_dist: s.num("decide.far_dist"),
        decide_crowd_count: s.int("decide.crowd_count"),
        decide_crowd_radius: s.num("decide.crowd_radius"),
        decide_dist_offset: s.num("decide.dist_offset"),
        decide_air_below_dz: s.num("decide.air_below_dz"),
        decide_air_down_dz: s.num("decide.air_down_dz"),
        decide_rise_dz: s.num("decide.rise_dz"),
        decide_rise_speed: s.num("decide.rise_speed"),
        decide_ground_probe_range: s.num("decide.ground_probe_range"),
        decide_ground_probe_max: s.num("decide.ground_probe_max"),
        decide_launch_air_dz: s.num("decide.launch_air_dz"),
        face_rate: s.num("face.rate"),
        face_timer: s.num("face.timer"),
        traction_speed: s.num("traction.speed"),
        traction_stop_dist: s.num("traction.stop_dist"),
        traction_rate_on: s.num("traction.rate_on"),
        traction_rate_off_ground: s.num("traction.rate_off_ground"),
        traction_rate_off_air: s.num("traction.rate_off_air"),
        traction_first_air_scale: s.num("traction.first_air_scale"),
        traction_dist_clamp: s.num("traction.dist_clamp"),
        traction_dz_clamp: s.num("traction.dz_clamp"),
        traction_z_gain: s.num("traction.z_gain"),
        traction_z_initial_gain: s.num("traction.z_initial_gain"),
        traction_z_initial_max: s.num("traction.z_initial_max"),
        traction_z_decay: s.num("traction.z_decay"),
        air_gravity_scale: s.num("air.gravity_scale"),
        air_hold_time: s.num("air.hold_time"),
        root_scale_rule: s.text("root.scale_rule"),
        hitstop_small: s.num("hitstop.small"),
        hitstop_big: s.num("hitstop.big"),
        style_max: s.int("style.max"),
        style_add_rate: s.num("style.add_rate"),
        style_add_rate_lv0: s.num("style.add_rate_lv0"),
        style_add_rate_lv1: s.num("style.add_rate_lv1"),
        style_add_rate_lv2: s.num("style.add_rate_lv2"),
        style_dec_start_time: s.num("style.dec_start_time"),
        style_dec_interval: s.num("style.dec_interval"),
        style_dec: s.int("style.dec"),
        guard_hold_time: s.num("guard.hold_time"),
        reaction_cooldown: s.num("reaction.cooldown"),
        damage_min_base: s.num("damage.min_base"),
        damage_epsilon: s.num("damage.epsilon"),
        damage_resist_max_percent: s.num("damage.resist_max_percent"),
        damage_combo_up_per_step: s.num("damage.combo_up_per_step"),
        damage_counter_up: s.num("damage.counter_up"),
        damage_critical_counter: s.num("damage.critical_counter"),
        damage_attribute_up: s.num("damage.attribute_up"),
        damage_buster: s.num("damage.buster"),
        damage_rapid_fire_resist_field: s.text("damage.rapid_fire_resist_field"),
        damage_zero_result: s.num("damage.zero_result"),
        damage_random: s.text("damage.random"),
        target_near_range: s.num("target.near_range"),
        target_min_dist: s.num("target.min_dist"),
        target_max_range: s.num("target.max_range"),
        target_switch_range: s.num("target.switch_range"),
        target_facing_weight: s.num("target.facing_weight"),
        target_origin_push: s.num("target.origin_push"),
        attack_cancel_delay: s.num("attack.cancel_delay"),
        attack_whiff_air_command_cooldown: s.num("attack.whiff_air.command_cooldown"),
        attack_whiff_air_airslide_cooldown: s.num("attack.whiff_air.airslide_cooldown"),
        attack_whiff_air_airguard_cooldown: s.num("attack.whiff_air.airguard_cooldown"),
        attack_air_cooldown_arm: s.num("attack.air.cooldown_arm"),
        attack_hit_confirm_required: s.num("attack.hit_confirm.required"),
        attack_request_phase: s.text("attack.request.phase"),
        attack_request_extra_latency_frames: s.int("attack.request.extra_latency_frames"),
        command_kind_attack: s.num("command.kind.attack"),
        command_attack_reaction_block: s.num("command.attack.reaction_block"),
        command_grand_magic_cooldown: s.num("command.grand_magic.cooldown"),
        command_reaction_cooldown_time: s.num("command.reaction_cooldown_time"),
        launch_unique_id: s.num("launch.unique_id"),
        launch_stock_available: s.int("launch.stock_available"),
        launch_groundbreaker_dz_max: s.num("launch.groundbreaker_dz_max"),
        launch_rising_dirz: s.num("launch.rising_dirz"),
        launch_combo_after: s.int("launch.combo_after"),
        recoil_travel: s.num("recoil.travel"),
        counter_forced_kind_default: s.num("counter.forced_kind.default"),
        just_guard_timer_countdown_fn: s.text("just_guard.timer_countdown_fn"),
        friend_finish_active_for_solo: s.num("friend_finish.active_for_solo"),
        attack_move_skip_calc_velocity: s.num("attack.move.skip_calc_velocity"),
        attack_move_begin_stop: s.num("attack.move.begin_stop"),
        traction_xy_speed: s.num("traction.xy.speed"),
        traction_xy_rate_open: s.num("traction.xy.rate_open"),
        traction_xy_rate_stop: s.num("traction.xy.rate_stop"),
        traction_xy_rate_stop_air: s.num("traction.xy.rate_stop_air"),
        traction_approach_snap: s.num("traction.approach_snap"),
        traction_dxy_max: s.num("traction.dxy_max"),
        traction_z_decay_rate: s.num("traction.z.decay_rate"),
        traction_z_rearm_gain: s.num("traction.z.rearm_gain"),
        traction_dive_blend: s.num("traction.dive.blend"),
        traction_scale_inside: s.num("traction.scale.inside"),
        root_scale_clamp: s.text("root_scale.clamp"),
        air_attack_gravity_scale: s.num("air_attack.gravity_scale"),
        air_attack_mode_start: s.num("air_attack.mode_start"),
        air_attack_mode_after_window: s.num("air_attack.mode_after_window"),
        air_attack_fall_accel_window_to_change: s.num("air_attack.fall_accel_window_to_change"),
        target_lead_time: s.num("target_lead.time"),
        level_rate: s.num("level.rate"),
        body_push_level_sora: s.num("body_push.level_sora"),
        body_push_capsule_blocks_pawns: s.num("body_push.capsule_blocks_pawns"),
        body_push_shape_radius: s.num("body_push.shape_radius"),
        hit_trace_channel: s.num("hit.trace_channel"),
        hit_sweep_ignore_channels: s.text("hit.sweep_ignore_channels"),
        hit_sweep_ignore_pawn_roots: s.text("hit.sweep_ignore_pawn_roots"),
        hit_sweep_world_static_needs_maphit: s.num("hit.sweep_world_static_needs_maphit"),
        hit_degenerate_radius: s.num("hit.degenerate_radius"),
        hit_capsule_min_half_length: s.num("hit.capsule_min_half_length"),
        hit_first_tick_zero_sweep: s.num("hit.first_tick_zero_sweep"),
        hit_max_touches: s.int("hit.max_touches"),
        hit_exec_rate_roll_scale: s.num("hit.exec_rate_roll_scale"),
        hit_exec_rate_fail_records_victim: s.num("hit.exec_rate_fail_records_victim"),
        hit_bad_status_roll_scale: s.num("hit.bad_status_roll_scale"),
        hit_map_floor_cos: s.num("hit.map_floor_cos"),
        hit_state_floor_cos: s.num("hit.state_floor_cos"),
        hit_state_ceiling_cos: s.num("hit.state_ceiling_cos"),
        hit_state_low_wall_height: s.num("hit.state_low_wall_height"),
        hit_weapon_map_height_limit: s.num("hit.weapon_map_height_limit"),
        hit_nearest_only_initial: s.num("hit.nearest_only_initial"),
        hit_counter_byte_max: s.int("hit.counter_byte_max"),
        hit_default_row_power: s.num("hit.default_row_power"),
        hit_rate_clamp_max: s.num("hit.rate_clamp_max"),
        hit_reflect_clash_only_first_swing: s.num("hit.reflect_clash_only_first_swing"),
        hit_clash_stronger_result: s.num("hit.clash_stronger_result"),
        hit_reflect1_level_pivot: s.num("hit.reflect1_level_pivot"),
        knockback_degenerate_sq: s.num("knockback.degenerate_sq"),
        knockback_initial: s.text("knockback.initial"),
        knockback_flattened: s.num("knockback.flattened"),
        gain_hitstop_applied: s.num("gain.hitstop_applied"),
        gain_focus_per_hit: s.num("gain.focus_per_hit"),
        gain_style_needs_hp_loss: s.num("gain.style_needs_hp_loss"),
        gain_style_form_up_bonus: s.num("gain.style_form_up_bonus"),
        gain_style_default_kind: s.num("gain.style_default_kind"),
        gain_style_bonus_halving: s.num("gain.style_bonus_halving"),
        gain_style_blocked_state: s.num("gain.style_blocked_state"),
        effect_random_offset_radius: s.num("effect.random_offset_radius"),
        effect_sound_volume: s.num("effect.sound_volume"),
        effect_no_sound_result: s.num("effect.no_sound_result"),
        effect_map_hit_uses_window_sound: s.num("effect.map_hit_uses_window_sound"),
        effect_critical_flag_ever_set: s.num("effect.critical_flag_ever_set"),
        target_lock_off_range: s.num("target.lock_off_range"),
        target_switch_max: s.int("target.switch_max"),
        target_origin_height: s.num("target.origin_height"),
        target_crowd_range: s.num("target.crowd_range"),
        target_non_enemy_range_factor: s.num("target.non_enemy_range_factor"),
        target_near_remap_factor: s.num("target.near_remap_factor"),
        target_initial_best_margin: s.num("target.initial_best_margin"),
        target_tap_time: s.num("target.tap_time"),
        target_los_channel: s.num("target.los_channel"),
        target_dive_range_xy: s.num("target.dive_range_xy"),
        target_dive_limit_z: s.num("target.dive_limit_z"),
        lockon_cam_blend_time: s.num("lockon_cam.blend_time"),
        lockon_cam_blend_exponent: s.num("lockon_cam.blend_exponent"),
        lockon_cam_distance_min: s.num("lockon_cam.distance_min"),
        lockon_cam_distance_rate: s.num("lockon_cam.distance_rate"),
        lockon_cam_base_pitch_offset: s.num("lockon_cam.base_pitch_offset"),
        lockon_cam_base_pitch_floor: s.num("lockon_cam.base_pitch_floor"),
        lockon_cam_look_yaw_bias: s.num("lockon_cam.look_yaw_bias"),
        lockon_cam_pitch_min: s.num("lockon_cam.pitch_min"),
        lockon_cam_pitch_max: s.num("lockon_cam.pitch_max"),
        lockon_cam_yaw_limit: s.num("lockon_cam.yaw_limit"),
        lockon_cam_stick_far_dist: s.num("lockon_cam.stick_far_dist"),
        lockon_cam_stick_near_dist: s.num("lockon_cam.stick_near_dist"),
        lockon_cam_stick_far_rate: s.num("lockon_cam.stick_far_rate"),
        lockon_cam_auto_low_dist: s.num("lockon_cam.auto_low_dist"),
        lockon_cam_auto_high_dist: s.num("lockon_cam.auto_high_dist"),
        lockon_cam_auto_rate_gain: s.num("lockon_cam.auto_rate_gain"),
        lockon_cam_offscreen_snap: s.num("lockon_cam.offscreen_snap"),
        lockon_cam_lead_factor: s.num("lockon_cam.lead_factor"),
        lockon_cam_target_jump_speed: s.num("lockon_cam.target_jump_speed"),
        lockon_cam_framing_start: s.num("lockon_cam.framing_start"),
        hit_small_power: s.num("hit.small.power"),
        hit_small_brake: s.num("hit.small.brake"),
        hit_small_start_falling_speed: s.num("hit.small.start_falling_speed"),
        hit_small_air_velocity_z: s.num("hit.small.air_velocity_z"),
        hit_blow_power: s.num("hit.blow.power"),
        hit_blow_velocity_z_ground: s.num("hit.blow.velocity_z_ground"),
        hit_blow_velocity_z_air: s.num("hit.blow.velocity_z_air"),
        hit_blow_brake_ground: s.num("hit.blow.brake_ground"),
        hit_blow_brake_air: s.num("hit.blow.brake_air"),
        hit_blow_air_start_brake_time: s.num("hit.blow.air_start_brake_time"),
        hit_blow_start_falling_speed: s.num("hit.blow.start_falling_speed"),
        hit_toss_power: s.num("hit.toss.power"),
        hit_toss_flight_duration: s.num("hit.toss.flight_duration"),
        hit_toss_fall_gravity_scale: s.num("hit.toss.fall_gravity_scale"),
        hit_launch_power: s.num("hit.launch.power"),
        hit_launch_flight_duration: s.num("hit.launch.flight_duration"),
        hit_launch_fall_gravity_scale: s.num("hit.launch.fall_gravity_scale"),
        hit_parabola_power: s.num("hit.parabola.power"),
        hit_parabola_angle: s.num("hit.parabola.angle"),
        hit_parabola_brake: s.num("hit.parabola.brake"),
        hit_parabola_limit_brake: s.num("hit.parabola.limit_brake"),
        hit_beat_power: s.num("hit.beat.power"),
        hit_beat_rebound: s.num("hit.beat.rebound"),
        hit_beat_rebound_gravity_scale: s.num("hit.beat.rebound_gravity_scale"),
        hit_wall_angle: s.num("hit.wall.angle"),
        hit_wall_min_speed: s.num("hit.wall.min_speed"),
        hit_recover_delay: s.num("hit.recover_delay"),
        hit_turn_alpha_rate: s.num("hit.turn_alpha_rate"),
        hit_brake_rate: s.num("hit.brake_rate"),
        hit_move_length_rate: s.num("hit.move_length_rate"),
        hit_blow_min_flight_time: s.num("hit.blow.min_flight_time"),
        hit_blow_drop_exit: s.num("hit.blow.drop_exit"),
        hit_toss_rise_gravity_scale: s.num("hit.toss.rise_gravity_scale"),
        hit_toss_hover_gravity_scale: s.num("hit.toss.hover_gravity_scale"),
        hit_toss_fall_timeout: s.num("hit.toss.fall_timeout"),
        hit_launch_fall_timeout: s.num("hit.launch.fall_timeout"),
        hit_parabola_loop_time: s.num("hit.parabola.loop_time"),
        hit_parabola_air_timeout: s.num("hit.parabola.air_timeout"),
        hit_beat_trace_range: s.num("hit.beat.trace_range"),
        hit_beat_radius_factor: s.num("hit.beat.radius_factor"),
        hit_beat_land_blend: s.num("hit.beat.land_blend"),
        hit_dir_front_half_angle: s.num("hit.dir.front_half_angle"),
        hp_combo_leave_window: s.num("hp.combo_leave_window"),
        hp_last_leave_min_hp: s.num("hp.last_leave_min_hp"),
        hit_reaction_block_after_command: s.num("hit.reaction_block_after_command"),
        recovery_command_time: s.num("recovery.command_time"),
        recovery_blow_counter_command: s.num("recovery.blow_counter_command"),
        recovery_blow_counter_attack_kind: s.num("recovery.blow_counter_attack_kind"),
        die_speed: s.num("die.speed"),
        die_velocity_z: s.num("die.velocity_z"),
        die_brake: s.num("die.brake"),
        die_phase1_time: s.num("die.phase1_time"),
        revive_no_damage_time: s.num("revive.no_damage_time"),
        revive_hp_fraction: s.num("revive.hp_fraction"),
        gravity_base: s.num("gravity.base"),
        form_gauge_max: s.num("form.gauge.max"),
        form_gauge_add_rate: s.num("form.gauge.add_rate"),
        form_gauge_add_rate_lv0: s.num("form.gauge.add_rate_lv0"),
        form_gauge_add_rate_lv1: s.num("form.gauge.add_rate_lv1"),
        form_gauge_add_rate_lv2: s.num("form.gauge.add_rate_lv2"),
        form_gauge_critical_add_rate: s.num("form.gauge.critical_add_rate"),
        form_gauge_critical_add_rate_lv0: s.num("form.gauge.critical_add_rate_lv0"),
        form_gauge_critical_add_rate_lv1: s.num("form.gauge.critical_add_rate_lv1"),
        form_gauge_critical_add_rate_lv2: s.num("form.gauge.critical_add_rate_lv2"),
        form_gauge_critical_charge_rate: s.num("form.gauge.critical_charge_rate"),
        form_gauge_form_up_bonus: s.num("form.gauge.form_up_bonus"),
        form_gauge_chain_first: s.num("form.gauge.chain_first"),
        form_gauge_chain_decay: s.num("form.gauge.chain_decay"),
        form_gauge_default_cmd: s.num("form.gauge.default_cmd"),
        form_gauge_no_gain_state: s.num("form.gauge.no_gain_state"),
        form_gauge_dec_start_time: s.num("form.gauge.dec_start_time"),
        form_gauge_dec_interval: s.num("form.gauge.dec_interval"),
        form_gauge_dec_amount: s.num("form.gauge.dec_amount"),
        form_gauge_auto_add_interval: s.num("form.gauge.auto_add_interval"),
        form_gauge_auto_add_points: s.num("form.gauge.auto_add_points"),
        form_kingdom_key_command: s.num("form.kingdom_key.command"),
        form_command_life: s.num("form.command.life"),
        form_command_life_per_form_time: s.num("form.command.life_per_form_time"),
        form_command_life_cap: s.num("form.command.life_cap"),
        form_duration: s.num("form.duration"),
        form_duration_default: s.num("form.duration_default"),
        form_level: s.int("form.level"),
        form_exec_block_time: s.num("form.exec_block_time"),
        form_attack_state: s.num("form.attack_state"),
        form_max_combo: s.int("form.max_combo"),
        form_finishers: s.int("form.finishers"),
        form_wear_form: s.num("form.wear_form"),
        form_world_slow_rate: s.num("form.world_slow_rate"),
        form_stylechange_face_time: s.num("form.stylechange.face_time"),
        form_stylechange_turn_gain: s.num("form.stylechange.turn_gain"),
        form_finish_level_max: s.num("form.finish.level_max"),
        form_finish_cmd_lv1: s.num("form.finish.cmd_lv1"),
        form_finish_cmd_lv2: s.num("form.finish.cmd_lv2"),
        form_finish_cmd_lv3: s.num("form.finish.cmd_lv3"),
        form_finish_offset_time: s.num("form.finish.offset_time"),
        form_finish_offset_time_weapon_change: s.num("form.finish.offset_time_weapon_change"),
        form_finish_stylefinish_state: s.num("form.finish.stylefinish_state"),
        form_finish_sonicrave_dash_root_len: s.num("form.finish.sonicrave.dash_root_len"),
        form_finish_sonicrave_end_root_len: s.num("form.finish.sonicrave.end_root_len"),
        form_finish_sonicrave_dashes_before_finish: s.int("form.finish.sonicrave.dashes_before_finish"),
        form_finish_lastarcanum_windows_before_break: s.int("form.finish.lastarcanum.windows_before_break"),
        form_finish_lastarcanum_root_len: s.text("form.finish.lastarcanum.root_len"),
        form_finish_stop_dist: s.text("form.finish.stop_dist"),
        form_end_block_states: s.text("form.end.block_states"),
        form_end_time_break_status: s.num("form.end.time_break_status"),
        queue_capacity: s.num("queue.capacity"),
        queue_evict_min_total: s.num("queue.evict_min_total"),
        queue_start_delay: s.num("queue.start_delay"),
        queue_appear_time: s.num("queue.appear_time"),
        queue_class_formchange: s.num("queue.class.formchange"),
        queue_class_finish: s.num("queue.class.finish"),
        queue_class_reaction: s.num("queue.class.reaction"),
        queue_class_grand_magic: s.num("queue.class.grand_magic"),
        queue_life_grand_magic: s.num("queue.life.grand_magic"),
        queue_life_attraction: s.num("queue.life.attraction"),
        queue_life_team_attack: s.num("queue.life.team_attack"),
        combo_carry_time: s.num("combo.carry_time"),
        form_finish_sonicrave_root_scale_factor: s.num("form.finish.sonicrave.root_scale_factor"),
        form_finish_lastarcanum_end_drop_height: s.num("form.finish.lastarcanum.end_drop_height"),
        form_finish_gravity_scale: s.num("form.finish.gravity_scale"),
        chase_command: s.num("chase.command"),
        chase_prompt_time: s.num("chase.prompt_time"),
        chase_warp_delay: s.num("chase.warp_delay"),
        chase_warp_distance: s.num("chase.warp_distance"),
        chase_warp_drop: s.num("chase.warp_drop"),
    }
});
