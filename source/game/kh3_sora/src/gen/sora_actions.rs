// The types of the reference's sheet sora-actions; values come from the player's files and the executable's
// constants at run time (crate::sheets).

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ActionParams {
    /// 0x141434010
    pub jump_type_high_ability: f32,
    /// 0x141434010
    pub jump_power_high: f32,
    /// 0x141434010
    pub jump_power_super: f32,
    /// 0x141434010
    pub jump_power_double: f32,
    /// 0x141423210
    pub jump_second_min_velocity_z: f32,
    /// 0x14139AB60
    pub jump_high_start_notify: f32,
    /// 0x14139AB60
    pub jump_double_start_notify: f32,
    /// 0x14139AB60
    pub jump_super_start_notify: f32,
    /// 0x14142A3D0
    pub jump_air_jump_ability: f32,
    /// 0x141395450
    pub jump_exit_check_delay: f32,
    /// 0x1413832C0
    pub jump_momentum_keep: f32,
    /// 0x141395450
    pub divefall_enabled: bool,
    /// 0x141395450
    pub divefall_start_height: f32,
    /// 0x141395450
    pub divefall_min_fall_time: f32,
    /// 0x141395450
    pub divefall_move_speed: f32,
    /// 0x141395450
    pub divefall_gravity_scale: f32,
    pub land_roll_root_distance: f32,
    /// 0x14055A970
    pub land_pose_notify_default: f32,
    /// 0x141386050
    pub land_pose_light_r_clips: &'static str,
    /// 0x141386050
    pub land_pose_light_l_clips: &'static str,
    pub land_light_rl_control_return: f32,
    /// 0x141427CC0
    pub dodge_ability: f32,
    /// 0x1413F1AB0
    pub dodge_request_stick_min: f32,
    /// 0x141444A50
    pub dodge_direction_stick_min: f32,
    /// 0x141373EE0
    pub dodge_length: f32,
    pub dodge_root_distance: f32,
    /// 0x141368880
    pub dodge_reaction_type: f32,
    /// 0x141373EE0
    pub dodge_invincible_until: f32,
    /// 0x141373EE0
    pub dodge_cancel_time: f32,
    /// 0x141373EE0
    pub dodge_restore_rate: f32,
    /// 0x1414294C0
    pub guard_ability: f32,
    /// 0x14139C310
    pub guard_length: f32,
    /// 0x14139AD70
    pub guard_window_end: f32,
    /// 0x141427CC0
    pub guard_hit_enable_input: f32,
    /// 0x141392080
    pub guard_turn_time: f32,
    /// 0x14138E820
    pub guard_pushback_speed: &'static str,
    /// 0x141392080
    pub guard_pushback_decay: f32,
    /// 0x14139A4E0
    pub guard_air_cooldown: f32,
    /// 0x14143CAF0
    pub guard_regen_ability: f32,
    /// 0x141425780
    pub airslide_ability: f32,
    /// 0x141436D20
    pub airdodge_ability: f32,
    /// 0x141425780
    pub airslide_stick_min: f32,
    /// 0x141340A10
    pub airslide_length: f32,
    /// 0x141340A10
    pub airslide_root_distance: f32,
    /// 0x141432EB0
    pub airslide_root_scale_lv1: f32,
    /// 0x141432EB0
    pub airslide_root_scale_lv2: f32,
    /// 0x141432EB0
    pub airslide_root_scale_lv3: f32,
    /// 0x141340A10
    pub airdodge_length: f32,
    /// 0x141340A10
    pub airdodge_root_distance: f32,
    /// 0x141332930
    pub airslide_reaction_type: f32,
    /// 0x1413347F0
    pub airslide_cooldown: f32,
    /// 0x141340A10
    pub airslide_fall_exit_stick_min: f32,
    /// 0x14143D8F0
    pub airrecovery_ability: f32,
    /// 0x141340A10
    pub airrecovery_length: f32,
    /// 0x141429390
    pub glide_ability: f32,
    /// 0x141384D20
    pub glide_gravity_scale: f32,
    /// 0x141391D00
    pub glide_speed: f32,
    /// 0x141391D00
    pub glide_turn_rate: f32,
    /// 0x14143C5B0
    pub glide_yaw_stick_min: f32,
    /// 0x14142E320
    pub superslide_ability: f32,
    /// 0x1414354B0
    pub superslide_time: f32,
    /// 0x1413D7C40
    pub superslide_speed: f32,
    /// 0x1413D7C40
    pub superslide_min_speed: f32,
    /// 0x1413D7C40
    pub superslide_brake: f32,
    /// 0x1413D7C40
    pub superslide_turn_rate: f32,
    /// 0x1413CD270
    pub superslide_aim_cone_cos: f32,
    /// 0x1413D7C40
    pub superslide_end_length: f32,
    /// 0x1413AED70
    pub stance_weapon_keep_time: f32,
    /// 0x1413944D0
    pub stance_idle_sheathe_delay: f32,
    /// 0x1413944D0
    pub stance_idle_break_count: f32,
    /// 0x1413944D0
    pub stance_battle_start_length: f32,
    /// 0x141435CE0
    pub wallrun_enabled: bool,
    /// 0x1413D87E0
    pub wallrun_root_scale_up: f32,
    /// 0x1413D87E0
    pub wallrun_root_scale_side: f32,
    /// 0x1413D87E0
    pub wallrun_side_switch_angle: f32,
    /// 0x1413D87E0
    pub wallrun_down_switch_angle: f32,
    /// 0x1413D87E0
    pub wallrun_side_move_speed: f32,
    /// 0x1413D87E0
    pub wallrun_switch_hold_time: f32,
    /// 0x1415102E0
    pub pole_rot_max: f32,
    /// 0x14143F0C0
    pub pole_spin_ability: f32,
    /// 0x1405A7740
    pub rail_check_range: f32,
    /// 0x1413D1EA0
    pub slope_speed: f32,
    /// 0x140634E60
    pub climb_enabled: bool,
    /// 0x140638900
    pub dangle_offset: &'static str,
    /// 0x1413E3E40
    pub swim_surface_speed: f32,
    /// 0x14136ABE0
    pub floating_gravity_scale: f32,
    /// 0x14143AAA0
    pub dodge_from_jump: bool,
    /// 0x141427CC0
    pub dodge_from_attack_bit: &'static str,
    /// 0x141373EE0
    pub dodge_iframes_60hz: i32,
    /// 0x141373EE0
    pub dodge_mark_times: &'static str,
    /// 0x14139A4E0
    pub guard_enable_change_time: f32,
    /// 0x14139C310
    pub guard_hit_length: f32,
    /// 0x1413851D0
    pub guard_hit_guard_until: f32,
    /// 0x1413851D0
    pub guard_lookat_distance: f32,
    /// 0x1413851D0
    pub guard_pushback_strength2_kinds: &'static str,
    /// 0x14138E820
    pub guard_pushback_repeat_scale: f32,
    pub guard_arc_half_angle: f32,
    /// 0x141414B70
    pub guard_just_guard_time: f32,
    /// 0x1414145C0
    pub guard_counter_window: f32,
    /// 0x14143CAF0
    pub guard_reflect_level_min: f32,
    /// 0x1413AE9A0
    pub reflect_pushback_speed: f32,
    /// 0x1413AE9A0
    pub reflect_pushback_decay: f32,
    /// 0x1413AE9A0
    pub reflect_turn_rate: f32,
    /// 0x1413A4FF0
    pub reflect_length_ground: f32,
    /// 0x1413A4FF0
    pub reflect_length_air: f32,
    /// 0x1413AE9A0
    pub reflect_riskdodge_window: f32,
    /// 0x1413A4FF0
    pub reflect_reaction_type: f32,
    /// 0x14142CD20
    pub riskdodge_ability: f32,
    /// 0x1413A5730
    pub riskdodge_max_walk_speed: f32,
    /// 0x1413A5730
    pub riskdodge_max_acceleration: f32,
    /// 0x1413A5730
    pub riskdodge_reaction_type: f32,
    /// 0x1413BD170
    pub riskdodge_counter_command_time: f32,
    /// 0x1413C31E0
    pub riskdodge_slow_motion: &'static str,
    /// 0x1413BB660
    pub riskdodge_underslide_distance: &'static str,
    /// 0x1413BB660
    pub riskdodge_underslide_root_scale: f32,
    /// 0x1413BB660
    pub riskdodge_underslide_length: f32,
    /// 0x1413BA340
    pub riskdodge_slideturn_radius_min: f32,
    /// 0x1413BA340
    pub riskdodge_slideturn_angles: &'static str,
    /// 0x1413BA340
    pub riskdodge_slideturn_input: f32,
    /// 0x1413BA340
    pub riskdodge_slideturn_brake: f32,
    /// 0x1413BA340
    pub riskdodge_slideturn_length: f32,
    /// 0x1413B8280
    pub riskdodge_overslide_length: f32,
    /// 0x1413BC250
    pub riskdodge_warp_delay: f32,
    /// 0x14061FE20
    pub reaction_ghost_invincible: f32,
    /// 0x14061FE20
    pub reaction_guard: f32,
    /// 0x14061FE20
    pub reaction_weak_guard: f32,
    /// 0x141425780
    pub airslide_next_queue_allowed: &'static str,
    /// 0x143F4EA20
    pub airslide_root_scale_axes: &'static str,
    /// 0x141340A10
    pub airslide_pole_probe_offset: &'static str,
    /// 0x1413347F0
    pub airslide_end_velocity: f32,
    /// 0x141340A10
    pub airslide_head_rot_restore: f32,
    /// 0x141365930
    pub airrecovery_recoverable_after: f32,
    /// 0x14134C5E0
    pub airrecovery_counter_window: f32,
    /// 0x141429390
    pub glide_entry_substeps: &'static str,
    /// 0x141384D20
    pub glide_begin_velocity: f32,
    /// 0x14138C840
    pub glide_end_velocity: f32,
    /// 0x141384D20
    pub glide_sets_airslide_cooldown: f32,
    /// 0x141432780
    pub glide_lock_time: f32,
    /// 0x1413D7C40
    pub superslide_probe_delay: f32,
    /// 0x14058E570
    pub superslide_end_root: &'static str,
    /// 0x1413D7C40
    pub superslide_end_cooldown: f32,
    /// 0x1413CC460
    pub superslide_movement_mode: f32,
    /// 0x14058E570
    pub flying_fluid_friction_factor: f32,
    /// 0x1405795F0
    pub cmc_direct_velocity: &'static str,
    /// 0x141109390
    pub floating_gravity_scale_special: f32,
    /// 0x14137A680
    pub floating_speed: f32,
    /// 0x141440600
    pub floating_stick_min: f32,
    /// 0x14137A680
    pub floating_turn_rate: f32,
    /// 0x14137A680
    pub floating_heading_rate: f32,
    /// 0x14137A680
    pub floating_coast_decay: f32,
    /// 0x14137A680
    pub floating_ground_refresh_dist: f32,
    /// 0x1413EA6C0
    pub floating_switch: &'static str,
    /// 0x1413837F0
    pub jump_second_enable_max_velocity_z: f32,
    /// 0x1413868C0
    pub jump_second_consumed_at: &'static str,
    /// 0x141395450
    pub jump_small_jump_clip_time: f32,
    /// 0x140577F30
    pub jump_high_height_held: f32,
    /// 0x141395450
    pub jump_ledge_probe_up: f32,
    /// 0x141395450
    pub jump_rail_probe_air_range: f32,
    /// 0x1413832C0
    pub jump_flow_inertial_velocity: f32,
    /// 0x141395450
    pub jump_flow_end_height: &'static str,
    /// 0x141395450
    pub jump_air_turn_rate: f32,
    /// 0x141395450
    pub land_light_max_height: f32,
    /// 0x141395450
    pub land_roll_min_height: f32,
    /// 0x141434330
    pub land_b_suffix_rule: &'static str,
    /// 0x1413F7340
    pub stance_battle_flag_source: &'static str,
    /// 0x140EA0B20
    pub stance_battle_linger: f32,
    /// 0x141433F80
    pub stance_idle_name_rule: &'static str,
    /// 0x1413AED70
    pub stance_run_battle_start: &'static str,
    /// 0x141780380
    pub stance_weapon_kind_plain: f32,
    /// 0x14138F6F0
    pub idle_field_9c: f32,
    /// 0x141435CE0
    pub wallrun_ability_flag_bit: f32,
    /// 0x14142E320
    pub flow_ability_flag_bit: f32,
    /// 0x140E84470
    pub flow_ability_flags_default: f32,
    /// 0x1406402E0
    pub wallrun_surface_pitch_max: f32,
    /// 0x1406402E0
    pub wallrun_entry_stick_min: f32,
    /// 0x1406402E0
    pub wallrun_entry_up_dot: f32,
    /// 0x1406402E0
    pub wallrun_entry_side_dot: f32,
    /// 0x14063EF00
    pub wallrun_probe_radius: f32,
    /// 0x1406402E0
    pub wallrun_entry_clear_up: f32,
    /// 0x1406402E0
    pub wallrun_entry_clear_side: f32,
    /// 0x1406402E0
    pub wallrun_entry_wall_up: f32,
    /// 0x1406402E0
    pub wallrun_entry_wall_side: f32,
    /// 0x14063EF00
    pub wallrun_track_back: f32,
    /// 0x14063EF00
    pub wallrun_track_len: f32,
    /// 0x1413D87E0
    pub wallrun_track_angle_max: f32,
    /// 0x14063EF00
    pub wallrun_contact_radius: f32,
    /// 0x14063EF00
    pub wallrun_contact_len: f32,
    /// 0x14063EF00
    pub wallrun_up_block_line: f32,
    /// 0x14063EF00
    pub wallrun_up_look: f32,
    /// 0x14063EF00
    pub wallrun_side_look: f32,
    /// 0x1413D87E0
    pub wallrun_ceiling_height: f32,
    /// 0x1413D87E0
    pub wallrun_input_grace: f32,
    /// 0x1413D87E0
    pub wallrun_blocked_ratio: f32,
    /// 0x1413D87E0
    pub wallrun_blocked_time: f32,
    /// 0x1413D87E0
    pub wallrun_side_move_blocked_scale: f32,
    /// 0x1413D87E0
    pub wallrun_loop_speed: f32,
    /// 0x1413D87E0
    pub wallrun_change_reverse_scale: f32,
    /// 0x1413D87E0
    pub wallrun_sub4_early_time: f32,
    /// 0x1413CF500
    pub wallrun_end_velocity_scale: f32,
    /// 0x1413CCD10
    pub wallrun_camera_blend: f32,
    /// 0x14143D8F0
    pub wallrun_jump_ability: f32,
    /// 0x14142E320
    pub wallrun_kick_ability: f32,
    /// 0x14063E620
    pub wallrun_ledge_radius: f32,
    /// 0x14142F990
    pub wallkick_ability: f32,
    /// 0x14142F990
    pub wallkick_probe_radius: f32,
    /// 0x14142F990
    pub wallkick_probe_height: f32,
    /// 0x14142F990
    pub wallkick_probe_len: f32,
    /// 0x14142F990
    pub wallkick_surface_pitch_max: f32,
    /// 0x141511E90
    pub wallkick_front_dot: f32,
    /// 0x141511E90
    pub wallkick_jump_min_time: f32,
    /// 0x141445D80
    pub wallkick_stick_min: f32,
    /// 0x141511E90
    pub wallkick_effect_group: f32,
    /// 0x1405A7500
    pub pole_probe_range: f32,
    /// 0x1413432C0
    pub pole_probe_offset_fwd: f32,
    /// 0x1413432C0
    pub pole_probe_offset_up: f32,
    /// 0x140F8C870
    pub pole_check_radius_default: f32,
    /// 0x14143F0C0
    pub pole_swing_ability: f32,
    /// 0x1415102E0
    pub pole_approach_time: f32,
    /// 0x1415102E0
    pub pole_spin_hang_offset: f32,
    /// 0x140F965E0
    pub pole_horizontal_axis_z_max: f32,
    /// 0x1414417B0
    pub pole_flow_stick_min: f32,
    /// 0x141509AA0
    pub pole_camera_blend: f32,
    /// 0x1405A7740
    pub rail_check_air_range: f32,
    /// 0x141395450
    pub rail_probe_height: f32,
    /// 0x1405A7740
    pub rail_probe_box_pad_min: f32,
    /// 0x1405A7740
    pub rail_probe_near_limit_sq: f32,
    /// 0x1405A7740
    pub rail_end_margin: f32,
    /// 0x1405A7740
    pub rail_blizzard_min_life: f32,
    /// 0x141510EE0
    pub rail_speed: f32,
    /// 0x140F8B5B0
    pub rail_node_speed_default: f32,
    /// 0x140F8B5B0
    pub rail_slope_rate_defaults: &'static str,
    /// 0x141513460
    pub rail_jump_inertial_velocity: f32,
    /// 0x141513460
    pub rail_jump_inertial_brake: f32,
    /// 0x141514E90
    pub rail_start_pause: f32,
    /// 0x141510EE0
    pub rail_turn_dot: f32,
    /// 0x141514AD0
    pub rail_turn_lockout: f32,
    /// 0x141514D70
    pub rail_turn_clip_time: f32,
    /// 0x141510EE0
    pub rail_narrow_blend: f32,
    /// 0x141516600
    pub rail_lean_lookahead: f32,
    /// 0x14058ECF0
    pub rail_rotation_slerp: f32,
    /// 0x141510EE0
    pub rail_end_launch_max_down: f32,
    /// 0x141513460
    pub rail_end_launch_jump_type: f32,
    /// 0x1413AD0B0
    pub rail_effect_group: f32,
    /// 0x1413A8E00
    pub rail_obstacle_sweep: &'static str,
    /// 0x140F930A0
    pub hopping_check_radius_default: f32,
    /// 0x14142A140
    pub hopping_probe_radius_idle: f32,
    /// 0x14142A140
    pub hopping_probe_radius_run: f32,
    /// 0x14142A140
    pub hopping_probe_radius_jump: f32,
    /// 0x1405A6F40
    pub hopping_probe_limit_factor: f32,
    /// 0x1405A6F40
    pub hopping_probe_dir_free_sq: f32,
    /// 0x1413AED70
    pub hopping_run_stick_min: f32,
    /// 0x141393270
    pub hopping_turn_stick_min: f32,
    /// 0x141393270
    pub hopping_next_stick_min: f32,
    /// 0x141393270
    pub hopping_next_search: &'static str,
    /// 0x141393270
    pub hopping_turn_rate: f32,
    /// 0x141385D80
    pub hopping_accel_factor: f32,
    /// 0x14138F650
    pub hopping_reset_speed: f32,
    /// 0x141393270
    pub hopping_clip_time: f32,
    /// 0x141393270
    pub hopping_chain_blend: f32,
    /// 0x141348BA0
    pub athletic_slide_time: f32,
    /// 0x141348BA0
    pub athletic_slide_speed: f32,
    /// 0x141348BA0
    pub athletic_v0_time_factor: f32,
    /// 0x141348BA0
    pub athletic_v0_near_factor: f32,
    /// 0x141348BA0
    pub athletic_max_accel_factor: f32,
    /// 0x141348BA0
    pub athletic_request_speed_factor: f32,
    /// 0x141348BA0
    pub athletic_near_distance: f32,
    /// 0x141348BA0
    pub athletic_near_speed: f32,
    /// 0x141348BA0
    pub athletic_arrive_margin: f32,
    /// 0x140F8C720
    pub athletic_close_range_default: f32,
    /// 0x141448550
    pub athletic_ref_point_z: f32,
    /// 0x141348BA0
    pub athletic_pitch_up: f32,
    /// 0x141348BA0
    pub athletic_pitch_down: f32,
    /// 0x141348BA0
    pub athletic_turn_rate_launch: f32,
    /// 0x141348BA0
    pub athletic_turn_rate_travel: f32,
    /// 0x141336DB0
    pub athletic_stuck_ratio: f32,
    /// 0x141336DB0
    pub athletic_stuck_time: f32,
    /// 0x141346600
    pub athletic_attack_slow_rate: f32,
    /// 0x141346600
    pub athletic_attack_slow_time: f32,
    /// 0x14142E990
    pub athletic_kick_anim: f32,
    /// 0x14141FDC0
    pub athletic_lockon_limit: f32,
    /// 0x14141FDC0
    pub athletic_lockon_range: f32,
    /// 0x14141FDC0
    pub athletic_screen_radius_candidate: f32,
    /// 0x14141FDC0
    pub athletic_screen_radius_aim: f32,
    /// 0x1413EE400
    pub athletic_los_sweep_param: f32,
    /// 0x141331780
    pub athletic_landing_probe_param: f32,
    /// 0x1413321B0
    pub athletic_pole_range_min: f32,
    /// 0x141331C40
    pub athletic_ledge_probe_up: f32,
    /// 0x141348BA0
    pub athletic_effect_group: f32,
    /// 0x141332F90
    pub athletic_bodycoll_type: f32,
    /// 0x141332F90
    pub athletic_invincible: f32,
    /// 0x141332F90
    pub athletic_movement_mode: f32,
    /// 0x140F477F0
    pub athletic_shootflow_point_type: f32,
    /// 0x14141FDC0
    pub athletic_unlock_bit: f32,
    /// 0x141348BA0
    pub athletic_clip_slide_end_len: f32,
    /// 0x141346600
    pub athletic_clip_enemy_end_len: f32,
    /// 0x141395450
    pub dangle_probe_up: f32,
    /// 0x141395450
    pub dangle_probe_down: f32,
    /// 0x140638900
    pub dangle_probe_radius_near: f32,
    /// 0x141395450
    pub dangle_probe_radius_far: f32,
    /// 0x140638900
    pub dangle_probe_side_yaw: f32,
    /// 0x140638900
    pub dangle_space_radius: f32,
    /// 0x140638900
    pub dangle_grip_radius: f32,
    /// 0x140638900
    pub dangle_geometry_flag: &'static str,
    /// 0x141427830
    pub dangle_gate_velz_rising: f32,
    /// 0x14150F220
    pub dangle_cooldown: f32,
    /// 0x141440600
    pub dangle_input_min_stick: f32,
    /// 0x14150F220
    pub dangle_climb_angle: f32,
    /// 0x14150F220
    pub dangle_climb_angle_pad: f32,
    /// 0x14150F220
    pub dangle_release_angle_pad: f32,
    /// 0x141508DE0
    pub dangle_begin_backoff: f32,
    /// 0x141508DE0
    pub dangle_movement_mode: f32,
    /// 0x14150F220
    pub dangle_enable_change: f32,
    /// 0x14150F220
    pub dangle_rise_root: &'static str,
    /// 0x140634E60
    pub climb_version: f32,
    /// 0x140634E60
    pub climb_radius: f32,
    /// 0x140634E60
    pub climb_bottom: f32,
    /// 0x140634E60
    pub climb_front: f32,
    /// 0x140634E60
    pub climb_front_far: f32,
    /// 0x140634E60
    pub climb_front_near: f32,
    /// 0x140634E60
    pub climb_final_space: f32,
    /// 0x140634E60
    pub climb_height_max: f32,
    /// 0x140634E60
    pub climb_height_middle: f32,
    /// 0x140634E60
    pub climb_height_high: f32,
    /// 0x140634E60
    pub climb_facing_dot: f32,
    /// 0x140634E60
    pub climb_headroom: f32,
    /// 0x140634E60
    pub climb_geometry_flag: &'static str,
    /// 0x1413533B0
    pub climb_lift_factor: f32,
    /// 0x1413533B0
    pub climb_traction_end: f32,
    /// 0x141440600
    pub climb_stick_cone: f32,
    /// 0x14134F120
    pub climb_blend_in: f32,
    /// 0x141350570
    pub climb_end_velocity_scale: f32,
    /// 0x1413CB400
    pub slope_accel_time: f32,
    /// 0x1413D1EA0
    pub slope_min_speed: f32,
    /// 0x1413D1EA0
    pub slope_run_exit_speed: f32,
    /// 0x14063D9C0
    pub slope_start_angle: f32,
    /// 0x14063D9C0
    pub slope_start_angle_forced: f32,
    /// 0x14063D9C0
    pub slope_facing_dot: f32,
    /// 0x14063D9C0
    pub slope_clearance: f32,
    /// 0x14063D1F0
    pub slope_continue_angle: f32,
    /// 0x14063D9C0
    pub slope_geometry_flag: &'static str,
    /// 0x1413D1EA0
    pub slope_yaw_rate: f32,
    /// 0x1413D1EA0
    pub slope_pitch_offset: f32,
    /// 0x1413D1EA0
    pub slope_decel_factor: f32,
    /// 0x1413D1EA0
    pub slope_blocked_ratio: f32,
    /// 0x1413D1EA0
    pub slope_uphill_end_angle: f32,
    /// 0x1413CB400
    pub slope_blend_in: f32,
    /// 0x14142E570
    pub sway_cone_dot: f32,
    /// 0x1413B2960
    pub sway_min_speed_ratio: f32,
    /// 0x1413B2960
    pub sway_min_stick: f32,
    /// 0x1413D8110
    pub sway_turn_rate: f32,
    /// 0x1413CC7B0
    pub sway_blend_in: f32,
    /// 0x1413D8110
    pub sway_enable_change: f32,
    /// 0x1413D8110
    pub sway_root_peak: f32,
    /// 0x140589410
    pub swim_surface_offset: f32,
    /// 0x1413D8630
    pub swim_exit_margin: f32,
    /// 0x140582300
    pub swim_depth_probe_extra: f32,
    /// 0x1405BB810
    pub swim_surface_to_dive_velz: f32,
    /// 0x1413CC890
    pub swim_entry_velz_clamp: f32,
    /// 0x14058FC40
    pub swim_surface_buoyancy: f32,
    /// 0x14058FC40
    pub swim_braking: f32,
    /// 0x1405749F0
    pub swim_surface_rise_speed: f32,
    /// 0x1405749F0
    pub swim_surface_ease: f32,
    /// 0x1413E3E40
    pub swim_dive_low_speed: f32,
    /// 0x1413E3E40
    pub swim_dive_high_speed: f32,
    /// 0x1413E3E40
    pub swim_dive_speed: f32,
    /// 0x1413E3E40
    pub swim_rise_speed: f32,
    /// 0x1413DAB60
    pub swim_dash_speed: f32,
    /// 0x1413DAB60
    pub swim_dash_time: f32,
    /// 0x1413DA070
    pub swim_dash_disable_input: f32,
    /// 0x1413DA070
    pub swim_dive_rot_yaw: f32,
    /// 0x1413DA070
    pub swim_dive_rot_pitch: f32,
    /// 0x1413DB020
    pub swim_turn_rate: f32,
    /// 0x1413DB020
    pub swim_turn_dot: f32,
    /// 0x1405892F0
    pub swim_water_volume: &'static str,
    /// 0x14142E850
    pub swim_dive_allowed: &'static str,
    /// 0x1413BB660
    pub riskdodge_underslide_distance_min: f32,
    /// 0x1413BB660
    pub riskdodge_underslide_distance_max: f32,
    /// 0x1413BA340
    pub riskdodge_slideturn_angle_rate: f32,
    /// 0x140638900
    pub dangle_offset_back: f32,
    /// 0x140638900
    pub dangle_offset_down: f32,
    /// 0x140638900
    pub dangle_probe_forward: f32,
    /// 0x141393270
    pub hopping_next_search_start: f32,
    /// 0x141393270
    pub hopping_next_search_limit: f32,
    /// 0x141393270
    pub hopping_next_search_step: f32,
    /// 0x141393270
    pub hopping_next_search_radius: f32,
    /// 0x1406402E0
    pub wallrun_entry_clear_off: f32,
    /// 0x1406402E0
    pub wallrun_entry_clear_height: f32,
    /// 0x14137A680
    pub floating_ground_probe_range: f32,
}

pub static ACTION_PARAMS: std::sync::LazyLock<ActionParams> = std::sync::LazyLock::new(|| {
    let s = crate::sheets::Sheet::named("sora-actions");
    ActionParams {
        jump_type_high_ability: s.num("jump.type_high_ability"),
        jump_power_high: s.num("jump.power_high"),
        jump_power_super: s.num("jump.power_super"),
        jump_power_double: s.num("jump.power_double"),
        jump_second_min_velocity_z: s.num("jump.second_min_velocity_z"),
        jump_high_start_notify: s.num("jump.high_start_notify"),
        jump_double_start_notify: s.num("jump.double_start_notify"),
        jump_super_start_notify: s.num("jump.super_start_notify"),
        jump_air_jump_ability: s.num("jump.air_jump_ability"),
        jump_exit_check_delay: s.num("jump.exit_check_delay"),
        jump_momentum_keep: s.num("jump.momentum_keep"),
        divefall_enabled: s.flag("divefall.enabled"),
        divefall_start_height: s.num("divefall.start_height"),
        divefall_min_fall_time: s.num("divefall.min_fall_time"),
        divefall_move_speed: s.num("divefall.move_speed"),
        divefall_gravity_scale: s.num("divefall.gravity_scale"),
        land_roll_root_distance: s.num("land.roll_root_distance"),
        land_pose_notify_default: s.num("land.pose_notify_default"),
        land_pose_light_r_clips: s.text("land.pose_light_r_clips"),
        land_pose_light_l_clips: s.text("land.pose_light_l_clips"),
        land_light_rl_control_return: s.num("land.light_rl_control_return"),
        dodge_ability: s.num("dodge.ability"),
        dodge_request_stick_min: s.num("dodge.request_stick_min"),
        dodge_direction_stick_min: s.num("dodge.direction_stick_min"),
        dodge_length: s.num("dodge.length"),
        dodge_root_distance: s.num("dodge.root_distance"),
        dodge_reaction_type: s.num("dodge.reaction_type"),
        dodge_invincible_until: s.num("dodge.invincible_until"),
        dodge_cancel_time: s.num("dodge.cancel_time"),
        dodge_restore_rate: s.num("dodge.restore_rate"),
        guard_ability: s.num("guard.ability"),
        guard_length: s.num("guard.length"),
        guard_window_end: s.num("guard.window_end"),
        guard_hit_enable_input: s.num("guard.hit_enable_input"),
        guard_turn_time: s.num("guard.turn_time"),
        guard_pushback_speed: s.text("guard.pushback_speed"),
        guard_pushback_decay: s.num("guard.pushback_decay"),
        guard_air_cooldown: s.num("guard.air_cooldown"),
        guard_regen_ability: s.num("guard.regen_ability"),
        airslide_ability: s.num("airslide.ability"),
        airdodge_ability: s.num("airdodge.ability"),
        airslide_stick_min: s.num("airslide.stick_min"),
        airslide_length: s.num("airslide.length"),
        airslide_root_distance: s.num("airslide.root_distance"),
        airslide_root_scale_lv1: s.num("airslide.root_scale_lv1"),
        airslide_root_scale_lv2: s.num("airslide.root_scale_lv2"),
        airslide_root_scale_lv3: s.num("airslide.root_scale_lv3"),
        airdodge_length: s.num("airdodge.length"),
        airdodge_root_distance: s.num("airdodge.root_distance"),
        airslide_reaction_type: s.num("airslide.reaction_type"),
        airslide_cooldown: s.num("airslide.cooldown"),
        airslide_fall_exit_stick_min: s.num("airslide.fall_exit_stick_min"),
        airrecovery_ability: s.num("airrecovery.ability"),
        airrecovery_length: s.num("airrecovery.length"),
        glide_ability: s.num("glide.ability"),
        glide_gravity_scale: s.num("glide.gravity_scale"),
        glide_speed: s.num("glide.speed"),
        glide_turn_rate: s.num("glide.turn_rate"),
        glide_yaw_stick_min: s.num("glide.yaw_stick_min"),
        superslide_ability: s.num("superslide.ability"),
        superslide_time: s.num("superslide.time"),
        superslide_speed: s.num("superslide.speed"),
        superslide_min_speed: s.num("superslide.min_speed"),
        superslide_brake: s.num("superslide.brake"),
        superslide_turn_rate: s.num("superslide.turn_rate"),
        superslide_aim_cone_cos: s.num("superslide.aim_cone_cos"),
        superslide_end_length: s.num("superslide.end_length"),
        stance_weapon_keep_time: s.num("stance.weapon_keep_time"),
        stance_idle_sheathe_delay: s.num("stance.idle_sheathe_delay"),
        stance_idle_break_count: s.num("stance.idle_break_count"),
        stance_battle_start_length: s.num("stance.battle_start_length"),
        wallrun_enabled: s.flag("wallrun.enabled"),
        wallrun_root_scale_up: s.num("wallrun.root_scale_up"),
        wallrun_root_scale_side: s.num("wallrun.root_scale_side"),
        wallrun_side_switch_angle: s.num("wallrun.side_switch_angle"),
        wallrun_down_switch_angle: s.num("wallrun.down_switch_angle"),
        wallrun_side_move_speed: s.num("wallrun.side_move_speed"),
        wallrun_switch_hold_time: s.num("wallrun.switch_hold_time"),
        pole_rot_max: s.num("pole.rot_max"),
        pole_spin_ability: s.num("pole.spin_ability"),
        rail_check_range: s.num("rail.check_range"),
        slope_speed: s.num("slope.speed"),
        climb_enabled: s.flag("climb.enabled"),
        dangle_offset: s.text("dangle.offset"),
        swim_surface_speed: s.num("swim.surface_speed"),
        floating_gravity_scale: s.num("floating.gravity_scale"),
        dodge_from_jump: s.flag("dodge.from_jump"),
        dodge_from_attack_bit: s.text("dodge.from_attack_bit"),
        dodge_iframes_60hz: s.int("dodge.iframes_60hz"),
        dodge_mark_times: s.text("dodge.mark_times"),
        guard_enable_change_time: s.num("guard.enable_change_time"),
        guard_hit_length: s.num("guard.hit_length"),
        guard_hit_guard_until: s.num("guard.hit_guard_until"),
        guard_lookat_distance: s.num("guard.lookat_distance"),
        guard_pushback_strength2_kinds: s.text("guard.pushback_strength2_kinds"),
        guard_pushback_repeat_scale: s.num("guard.pushback_repeat_scale"),
        guard_arc_half_angle: s.num("guard.arc_half_angle"),
        guard_just_guard_time: s.num("guard.just_guard_time"),
        guard_counter_window: s.num("guard.counter_window"),
        guard_reflect_level_min: s.num("guard.reflect_level_min"),
        reflect_pushback_speed: s.num("reflect.pushback_speed"),
        reflect_pushback_decay: s.num("reflect.pushback_decay"),
        reflect_turn_rate: s.num("reflect.turn_rate"),
        reflect_length_ground: s.num("reflect.length_ground"),
        reflect_length_air: s.num("reflect.length_air"),
        reflect_riskdodge_window: s.num("reflect.riskdodge_window"),
        reflect_reaction_type: s.num("reflect.reaction_type"),
        riskdodge_ability: s.num("riskdodge.ability"),
        riskdodge_max_walk_speed: s.num("riskdodge.max_walk_speed"),
        riskdodge_max_acceleration: s.num("riskdodge.max_acceleration"),
        riskdodge_reaction_type: s.num("riskdodge.reaction_type"),
        riskdodge_counter_command_time: s.num("riskdodge.counter_command_time"),
        riskdodge_slow_motion: s.text("riskdodge.slow_motion"),
        riskdodge_underslide_distance: s.text("riskdodge.underslide_distance"),
        riskdodge_underslide_root_scale: s.num("riskdodge.underslide_root_scale"),
        riskdodge_underslide_length: s.num("riskdodge.underslide_length"),
        riskdodge_slideturn_radius_min: s.num("riskdodge.slideturn_radius_min"),
        riskdodge_slideturn_angles: s.text("riskdodge.slideturn_angles"),
        riskdodge_slideturn_input: s.num("riskdodge.slideturn_input"),
        riskdodge_slideturn_brake: s.num("riskdodge.slideturn_brake"),
        riskdodge_slideturn_length: s.num("riskdodge.slideturn_length"),
        riskdodge_overslide_length: s.num("riskdodge.overslide_length"),
        riskdodge_warp_delay: s.num("riskdodge.warp_delay"),
        reaction_ghost_invincible: s.num("reaction.ghost_invincible"),
        reaction_guard: s.num("reaction.guard"),
        reaction_weak_guard: s.num("reaction.weak_guard"),
        airslide_next_queue_allowed: s.text("airslide.next_queue_allowed"),
        airslide_root_scale_axes: s.text("airslide.root_scale_axes"),
        airslide_pole_probe_offset: s.text("airslide.pole_probe_offset"),
        airslide_end_velocity: s.num("airslide.end_velocity"),
        airslide_head_rot_restore: s.num("airslide.head_rot_restore"),
        airrecovery_recoverable_after: s.num("airrecovery.recoverable_after"),
        airrecovery_counter_window: s.num("airrecovery.counter_window"),
        glide_entry_substeps: s.text("glide.entry_substeps"),
        glide_begin_velocity: s.num("glide.begin_velocity"),
        glide_end_velocity: s.num("glide.end_velocity"),
        glide_sets_airslide_cooldown: s.num("glide.sets_airslide_cooldown"),
        glide_lock_time: s.num("glide.lock_time"),
        superslide_probe_delay: s.num("superslide.probe_delay"),
        superslide_end_root: s.text("superslide.end_root"),
        superslide_end_cooldown: s.num("superslide.end_cooldown"),
        superslide_movement_mode: s.num("superslide.movement_mode"),
        flying_fluid_friction_factor: s.num("flying.fluid_friction_factor"),
        cmc_direct_velocity: s.text("cmc.direct_velocity"),
        floating_gravity_scale_special: s.num("floating.gravity_scale_special"),
        floating_speed: s.num("floating.speed"),
        floating_stick_min: s.num("floating.stick_min"),
        floating_turn_rate: s.num("floating.turn_rate"),
        floating_heading_rate: s.num("floating.heading_rate"),
        floating_coast_decay: s.num("floating.coast_decay"),
        floating_ground_refresh_dist: s.num("floating.ground_refresh_dist"),
        floating_switch: s.text("floating.switch"),
        jump_second_enable_max_velocity_z: s.num("jump.second_enable_max_velocity_z"),
        jump_second_consumed_at: s.text("jump.second_consumed_at"),
        jump_small_jump_clip_time: s.num("jump.small_jump_clip_time"),
        jump_high_height_held: s.num("jump.high_height_held"),
        jump_ledge_probe_up: s.num("jump.ledge_probe_up"),
        jump_rail_probe_air_range: s.num("jump.rail_probe_air_range"),
        jump_flow_inertial_velocity: s.num("jump.flow_inertial_velocity"),
        jump_flow_end_height: s.text("jump.flow_end_height"),
        jump_air_turn_rate: s.num("jump.air_turn_rate"),
        land_light_max_height: s.num("land.light_max_height"),
        land_roll_min_height: s.num("land.roll_min_height"),
        land_b_suffix_rule: s.text("land.b_suffix_rule"),
        stance_battle_flag_source: s.text("stance.battle_flag_source"),
        stance_battle_linger: s.num("stance.battle_linger"),
        stance_idle_name_rule: s.text("stance.idle_name_rule"),
        stance_run_battle_start: s.text("stance.run_battle_start"),
        stance_weapon_kind_plain: s.num("stance.weapon_kind_plain"),
        idle_field_9c: s.num("idle.field_9c"),
        wallrun_ability_flag_bit: s.num("wallrun.ability_flag_bit"),
        flow_ability_flag_bit: s.num("flow.ability_flag_bit"),
        flow_ability_flags_default: s.num("flow.ability_flags_default"),
        wallrun_surface_pitch_max: s.num("wallrun.surface_pitch_max"),
        wallrun_entry_stick_min: s.num("wallrun.entry_stick_min"),
        wallrun_entry_up_dot: s.num("wallrun.entry_up_dot"),
        wallrun_entry_side_dot: s.num("wallrun.entry_side_dot"),
        wallrun_probe_radius: s.num("wallrun.probe_radius"),
        wallrun_entry_clear_up: s.num("wallrun.entry_clear_up"),
        wallrun_entry_clear_side: s.num("wallrun.entry_clear_side"),
        wallrun_entry_wall_up: s.num("wallrun.entry_wall_up"),
        wallrun_entry_wall_side: s.num("wallrun.entry_wall_side"),
        wallrun_track_back: s.num("wallrun.track_back"),
        wallrun_track_len: s.num("wallrun.track_len"),
        wallrun_track_angle_max: s.num("wallrun.track_angle_max"),
        wallrun_contact_radius: s.num("wallrun.contact_radius"),
        wallrun_contact_len: s.num("wallrun.contact_len"),
        wallrun_up_block_line: s.num("wallrun.up_block_line"),
        wallrun_up_look: s.num("wallrun.up_look"),
        wallrun_side_look: s.num("wallrun.side_look"),
        wallrun_ceiling_height: s.num("wallrun.ceiling_height"),
        wallrun_input_grace: s.num("wallrun.input_grace"),
        wallrun_blocked_ratio: s.num("wallrun.blocked_ratio"),
        wallrun_blocked_time: s.num("wallrun.blocked_time"),
        wallrun_side_move_blocked_scale: s.num("wallrun.side_move_blocked_scale"),
        wallrun_loop_speed: s.num("wallrun.loop_speed"),
        wallrun_change_reverse_scale: s.num("wallrun.change_reverse_scale"),
        wallrun_sub4_early_time: s.num("wallrun.sub4_early_time"),
        wallrun_end_velocity_scale: s.num("wallrun.end_velocity_scale"),
        wallrun_camera_blend: s.num("wallrun.camera_blend"),
        wallrun_jump_ability: s.num("wallrun.jump_ability"),
        wallrun_kick_ability: s.num("wallrun.kick_ability"),
        wallrun_ledge_radius: s.num("wallrun.ledge_radius"),
        wallkick_ability: s.num("wallkick.ability"),
        wallkick_probe_radius: s.num("wallkick.probe_radius"),
        wallkick_probe_height: s.num("wallkick.probe_height"),
        wallkick_probe_len: s.num("wallkick.probe_len"),
        wallkick_surface_pitch_max: s.num("wallkick.surface_pitch_max"),
        wallkick_front_dot: s.num("wallkick.front_dot"),
        wallkick_jump_min_time: s.num("wallkick.jump_min_time"),
        wallkick_stick_min: s.num("wallkick.stick_min"),
        wallkick_effect_group: s.num("wallkick.effect_group"),
        pole_probe_range: s.num("pole.probe_range"),
        pole_probe_offset_fwd: s.num("pole.probe_offset_fwd"),
        pole_probe_offset_up: s.num("pole.probe_offset_up"),
        pole_check_radius_default: s.num("pole.check_radius_default"),
        pole_swing_ability: s.num("pole.swing_ability"),
        pole_approach_time: s.num("pole.approach_time"),
        pole_spin_hang_offset: s.num("pole.spin_hang_offset"),
        pole_horizontal_axis_z_max: s.num("pole.horizontal_axis_z_max"),
        pole_flow_stick_min: s.num("pole.flow_stick_min"),
        pole_camera_blend: s.num("pole.camera_blend"),
        rail_check_air_range: s.num("rail.check_air_range"),
        rail_probe_height: s.num("rail.probe_height"),
        rail_probe_box_pad_min: s.num("rail.probe_box_pad_min"),
        rail_probe_near_limit_sq: s.num("rail.probe_near_limit_sq"),
        rail_end_margin: s.num("rail.end_margin"),
        rail_blizzard_min_life: s.num("rail.blizzard_min_life"),
        rail_speed: s.num("rail.speed"),
        rail_node_speed_default: s.num("rail.node_speed_default"),
        rail_slope_rate_defaults: s.text("rail.slope_rate_defaults"),
        rail_jump_inertial_velocity: s.num("rail.jump_inertial_velocity"),
        rail_jump_inertial_brake: s.num("rail.jump_inertial_brake"),
        rail_start_pause: s.num("rail.start_pause"),
        rail_turn_dot: s.num("rail.turn_dot"),
        rail_turn_lockout: s.num("rail.turn_lockout"),
        rail_turn_clip_time: s.num("rail.turn_clip_time"),
        rail_narrow_blend: s.num("rail.narrow_blend"),
        rail_lean_lookahead: s.num("rail.lean_lookahead"),
        rail_rotation_slerp: s.num("rail.rotation_slerp"),
        rail_end_launch_max_down: s.num("rail.end_launch_max_down"),
        rail_end_launch_jump_type: s.num("rail.end_launch_jump_type"),
        rail_effect_group: s.num("rail.effect_group"),
        rail_obstacle_sweep: s.text("rail.obstacle_sweep"),
        hopping_check_radius_default: s.num("hopping.check_radius_default"),
        hopping_probe_radius_idle: s.num("hopping.probe_radius_idle"),
        hopping_probe_radius_run: s.num("hopping.probe_radius_run"),
        hopping_probe_radius_jump: s.num("hopping.probe_radius_jump"),
        hopping_probe_limit_factor: s.num("hopping.probe_limit_factor"),
        hopping_probe_dir_free_sq: s.num("hopping.probe_dir_free_sq"),
        hopping_run_stick_min: s.num("hopping.run_stick_min"),
        hopping_turn_stick_min: s.num("hopping.turn_stick_min"),
        hopping_next_stick_min: s.num("hopping.next_stick_min"),
        hopping_next_search: s.text("hopping.next_search"),
        hopping_turn_rate: s.num("hopping.turn_rate"),
        hopping_accel_factor: s.num("hopping.accel_factor"),
        hopping_reset_speed: s.num("hopping.reset_speed"),
        hopping_clip_time: s.num("hopping.clip_time"),
        hopping_chain_blend: s.num("hopping.chain_blend"),
        athletic_slide_time: s.num("athletic.slide_time"),
        athletic_slide_speed: s.num("athletic.slide_speed"),
        athletic_v0_time_factor: s.num("athletic.v0_time_factor"),
        athletic_v0_near_factor: s.num("athletic.v0_near_factor"),
        athletic_max_accel_factor: s.num("athletic.max_accel_factor"),
        athletic_request_speed_factor: s.num("athletic.request_speed_factor"),
        athletic_near_distance: s.num("athletic.near_distance"),
        athletic_near_speed: s.num("athletic.near_speed"),
        athletic_arrive_margin: s.num("athletic.arrive_margin"),
        athletic_close_range_default: s.num("athletic.close_range_default"),
        athletic_ref_point_z: s.num("athletic.ref_point_z"),
        athletic_pitch_up: s.num("athletic.pitch_up"),
        athletic_pitch_down: s.num("athletic.pitch_down"),
        athletic_turn_rate_launch: s.num("athletic.turn_rate_launch"),
        athletic_turn_rate_travel: s.num("athletic.turn_rate_travel"),
        athletic_stuck_ratio: s.num("athletic.stuck_ratio"),
        athletic_stuck_time: s.num("athletic.stuck_time"),
        athletic_attack_slow_rate: s.num("athletic.attack_slow_rate"),
        athletic_attack_slow_time: s.num("athletic.attack_slow_time"),
        athletic_kick_anim: s.num("athletic.kick_anim"),
        athletic_lockon_limit: s.num("athletic.lockon_limit"),
        athletic_lockon_range: s.num("athletic.lockon_range"),
        athletic_screen_radius_candidate: s.num("athletic.screen_radius_candidate"),
        athletic_screen_radius_aim: s.num("athletic.screen_radius_aim"),
        athletic_los_sweep_param: s.num("athletic.los_sweep_param"),
        athletic_landing_probe_param: s.num("athletic.landing_probe_param"),
        athletic_pole_range_min: s.num("athletic.pole_range_min"),
        athletic_ledge_probe_up: s.num("athletic.ledge_probe_up"),
        athletic_effect_group: s.num("athletic.effect_group"),
        athletic_bodycoll_type: s.num("athletic.bodycoll_type"),
        athletic_invincible: s.num("athletic.invincible"),
        athletic_movement_mode: s.num("athletic.movement_mode"),
        athletic_shootflow_point_type: s.num("athletic.shootflow_point_type"),
        athletic_unlock_bit: s.num("athletic.unlock_bit"),
        athletic_clip_slide_end_len: s.num("athletic.clip_slide_end_len"),
        athletic_clip_enemy_end_len: s.num("athletic.clip_enemy_end_len"),
        dangle_probe_up: s.num("dangle.probe_up"),
        dangle_probe_down: s.num("dangle.probe_down"),
        dangle_probe_radius_near: s.num("dangle.probe_radius_near"),
        dangle_probe_radius_far: s.num("dangle.probe_radius_far"),
        dangle_probe_side_yaw: s.num("dangle.probe_side_yaw"),
        dangle_space_radius: s.num("dangle.space_radius"),
        dangle_grip_radius: s.num("dangle.grip_radius"),
        dangle_geometry_flag: s.text("dangle.geometry_flag"),
        dangle_gate_velz_rising: s.num("dangle.gate_velz_rising"),
        dangle_cooldown: s.num("dangle.cooldown"),
        dangle_input_min_stick: s.num("dangle.input_min_stick"),
        dangle_climb_angle: s.num("dangle.climb_angle"),
        dangle_climb_angle_pad: s.num("dangle.climb_angle_pad"),
        dangle_release_angle_pad: s.num("dangle.release_angle_pad"),
        dangle_begin_backoff: s.num("dangle.begin_backoff"),
        dangle_movement_mode: s.num("dangle.movement_mode"),
        dangle_enable_change: s.num("dangle.enable_change"),
        dangle_rise_root: s.text("dangle.rise_root"),
        climb_version: s.num("climb.version"),
        climb_radius: s.num("climb.radius"),
        climb_bottom: s.num("climb.bottom"),
        climb_front: s.num("climb.front"),
        climb_front_far: s.num("climb.front_far"),
        climb_front_near: s.num("climb.front_near"),
        climb_final_space: s.num("climb.final_space"),
        climb_height_max: s.num("climb.height_max"),
        climb_height_middle: s.num("climb.height_middle"),
        climb_height_high: s.num("climb.height_high"),
        climb_facing_dot: s.num("climb.facing_dot"),
        climb_headroom: s.num("climb.headroom"),
        climb_geometry_flag: s.text("climb.geometry_flag"),
        climb_lift_factor: s.num("climb.lift_factor"),
        climb_traction_end: s.num("climb.traction_end"),
        climb_stick_cone: s.num("climb.stick_cone"),
        climb_blend_in: s.num("climb.blend_in"),
        climb_end_velocity_scale: s.num("climb.end_velocity_scale"),
        slope_accel_time: s.num("slope.accel_time"),
        slope_min_speed: s.num("slope.min_speed"),
        slope_run_exit_speed: s.num("slope.run_exit_speed"),
        slope_start_angle: s.num("slope.start_angle"),
        slope_start_angle_forced: s.num("slope.start_angle_forced"),
        slope_facing_dot: s.num("slope.facing_dot"),
        slope_clearance: s.num("slope.clearance"),
        slope_continue_angle: s.num("slope.continue_angle"),
        slope_geometry_flag: s.text("slope.geometry_flag"),
        slope_yaw_rate: s.num("slope.yaw_rate"),
        slope_pitch_offset: s.num("slope.pitch_offset"),
        slope_decel_factor: s.num("slope.decel_factor"),
        slope_blocked_ratio: s.num("slope.blocked_ratio"),
        slope_uphill_end_angle: s.num("slope.uphill_end_angle"),
        slope_blend_in: s.num("slope.blend_in"),
        sway_cone_dot: s.num("sway.cone_dot"),
        sway_min_speed_ratio: s.num("sway.min_speed_ratio"),
        sway_min_stick: s.num("sway.min_stick"),
        sway_turn_rate: s.num("sway.turn_rate"),
        sway_blend_in: s.num("sway.blend_in"),
        sway_enable_change: s.num("sway.enable_change"),
        sway_root_peak: s.num("sway.root_peak"),
        swim_surface_offset: s.num("swim.surface_offset"),
        swim_exit_margin: s.num("swim.exit_margin"),
        swim_depth_probe_extra: s.num("swim.depth_probe_extra"),
        swim_surface_to_dive_velz: s.num("swim.surface_to_dive_velz"),
        swim_entry_velz_clamp: s.num("swim.entry_velz_clamp"),
        swim_surface_buoyancy: s.num("swim.surface_buoyancy"),
        swim_braking: s.num("swim.braking"),
        swim_surface_rise_speed: s.num("swim.surface_rise_speed"),
        swim_surface_ease: s.num("swim.surface_ease"),
        swim_dive_low_speed: s.num("swim.dive_low_speed"),
        swim_dive_high_speed: s.num("swim.dive_high_speed"),
        swim_dive_speed: s.num("swim.dive_speed"),
        swim_rise_speed: s.num("swim.rise_speed"),
        swim_dash_speed: s.num("swim.dash_speed"),
        swim_dash_time: s.num("swim.dash_time"),
        swim_dash_disable_input: s.num("swim.dash_disable_input"),
        swim_dive_rot_yaw: s.num("swim.dive_rot_yaw"),
        swim_dive_rot_pitch: s.num("swim.dive_rot_pitch"),
        swim_turn_rate: s.num("swim.turn_rate"),
        swim_turn_dot: s.num("swim.turn_dot"),
        swim_water_volume: s.text("swim.water_volume"),
        swim_dive_allowed: s.text("swim.dive_allowed"),
        riskdodge_underslide_distance_min: s.num("riskdodge.underslide_distance_min"),
        riskdodge_underslide_distance_max: s.num("riskdodge.underslide_distance_max"),
        riskdodge_slideturn_angle_rate: s.num("riskdodge.slideturn_angle_rate"),
        dangle_offset_back: s.num("dangle.offset_back"),
        dangle_offset_down: s.num("dangle.offset_down"),
        dangle_probe_forward: s.num("dangle.probe_forward"),
        hopping_next_search_start: s.num("hopping.next_search_start"),
        hopping_next_search_limit: s.num("hopping.next_search_limit"),
        hopping_next_search_step: s.num("hopping.next_search_step"),
        hopping_next_search_radius: s.num("hopping.next_search_radius"),
        wallrun_entry_clear_off: s.num("wallrun.entry_clear_off"),
        wallrun_entry_clear_height: s.num("wallrun.entry_clear_height"),
        floating_ground_probe_range: s.num("floating.ground_probe_range"),
    }
});
