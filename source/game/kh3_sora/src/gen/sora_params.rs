// The types of the reference's sheet sora-movement; values come from the player's files and the executable's
// constants at run time (crate::sheets).

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SoraParams {
    /// 0x140582270
    pub gravity_scale: f32,
    /// 0x140582270
    pub gravity_rate: f32,
    /// 0x140577F30
    pub gravity_jump_up_scale: f32,
    /// 0x140577F30
    pub gravity_jump_duration_time: f32,
    /// 0x140577F30
    pub gravity_jump_duration_scale: f32,
    /// 0x141395450
    pub gravity_small_jump_scale: f32,
    /// 0x14057ECB0
    pub jump_power_normal: f32,
    /// 0x140583A40
    pub jump_max_height: f32,
    /// 0x141423210
    pub speed_run: f32,
    /// 0x1413AED70
    pub speed_sprint: f32,
    /// 0x1413AED70
    pub speed_sprint_delay: f32,
    /// 0x14057D050
    pub speed_walk_rate: f32,
    /// 0x141423210
    pub speed_force_walk: f32,
    /// 0x1413F1AB0
    pub input_stick_active_min: f32,
    /// 0x141440030
    pub input_walk_run_threshold: f32,
    /// 0x1413AED70
    pub input_run_grace_time: f32,
    /// 0x1413AED70
    pub accel_ground_factor: f32,
    /// 0x1413868C0
    pub accel_air_factor: f32,
    /// 0x140512380
    pub accel_default: f32,
    /// 0x140580B30
    pub air_control: f32,
    /// 0x14058D0C0
    pub air_braking_deceleration: f32,
    /// 0x14058D0C0
    pub air_lateral_friction: f32,
    /// 0x1405900B0
    pub ground_friction: f32,
    /// 0x140577890
    pub ground_braking_friction_factor: f32,
    /// 0x1405900B0
    pub ground_braking_deceleration: f32,
    /// 0x1413AED70
    pub ground_stop_accel_sq_max: f32,
    /// 0x1413AED70
    pub turn_run_alpha_per_60hz_frame: f32,
    /// 0x1413AED70
    pub turn_run_stop_rate: f32,
    /// 0x141395450
    pub turn_air_rate: f32,
    /// 0x1413944D0
    pub turn_idle_rate: f32,
    /// 0x1413AED70
    pub turn_turn_around_angle: f32,
    /// 0x140590DA0
    pub turn_rotation_rate_yaw: f32,
    /// 0x140570EB0
    pub floor_walkable_angle: f32,
    /// 0x140589A60
    pub floor_walkable_z: f32,
    /// 0x140570EB0
    pub floor_max_step_height: f32,
    /// 0x141423210
    pub floor_always_check: bool,
    /// 0x140570EB0
    pub floor_can_walk_off_ledges: bool,
    /// 0x140570EB0
    pub floor_maintain_horizontal_ground_velocity: bool,
    /// 0x1405DF310
    pub capsule_radius: f32,
    /// 0x1405DF310
    pub capsule_half_height: f32,
    /// 0x1405DF310
    pub capsule_center_z: f32,
    /// 0x140570EB0
    pub sim_max_time_step: f32,
    /// 0x140570EB0
    pub sim_max_iterations: f32,
    /// 0x1413868C0
    pub fall_step_down_height: f32,
    /// 0x141395450
    pub fall_step_down_time: f32,
    /// 0x141395450
    pub land_light_height_max: f32,
    /// 0x141395450
    pub land_roll_height_min: f32,
    /// 0x141395450
    pub land_length: f32,
    /// 0x1411FBA40
    pub land_roll_control_return: f32,
    /// 0x141395450
    pub land_roll_length: f32,
    /// 0x1413AED70
    pub anim_run_start_length: f32,
    /// 0x1413AED70
    pub anim_run_end_length: f32,
    /// 0x1413AED70
    pub anim_walk_end_length: f32,
    /// 0x1445C4F70
    pub gravity_world_z: f32,
    /// 0x14058BCA0
    pub gravity_terminal_speed: f32,
    /// 0x14139AB60
    pub jump_launch_delay: f32,
    /// 0x141395450
    pub jump_small_jump_min_time: f32,
    /// 0x1405A5D80
    pub jump_max_hold_time: f32,
    /// 0x14142A3D0
    pub jump_coyote_time: f32,
    /// 0x1413F1AB0
    pub jump_buffer_time: f32,
    /// 0x1445AF120
    pub input_axis_dead_zone: f32,
    /// 0x1405B9BD0
    pub input_move_scale: f32,
    /// 0x1405744E0
    pub floor_dist_min: f32,
    /// 0x14057FA80
    pub floor_dist_max: f32,
    /// 0x1405744E0
    pub floor_dist_target: f32,
    /// 0x14057FA80
    pub floor_sweep_distance_walking: f32,
    /// 0x14057FA80
    pub floor_sweep_distance_falling: f32,
    /// 0x140596260
    pub floor_perch_radius: f32,
    /// 0x14057FA80
    pub floor_falling_perch_additional_height: f32,
    /// 0x14057FA80
    pub floor_perch_additional_height: f32,
    /// 0x14057FA80
    pub floor_max_perch_dist_walking: f32,
    /// 0x14057FA80
    pub floor_max_perch_dist_falling: f32,
    /// 0x14057D100
    pub floor_perch_probe_min_half_height: f32,
    /// 0x140589BA0
    pub floor_sweep_edge_reject: f32,
    /// 0x1405887B0
    pub edge_fall_check_enabled: bool,
    /// 0x1405887B0
    pub edge_sphere_radius: f32,
    /// 0x1405887B0
    pub edge_contact_limit_walking: f32,
    /// 0x1405887B0
    pub edge_line_max_traces: f32,
    /// 0x14057BD10
    pub edge_float_enabled: bool,
    /// 0x14057BD10
    pub edge_float_slide: f32,
    /// 0x140597570
    pub step_up_floor_dist_offset: f32,
    /// 0x140597570
    pub step_up_down_extra: f32,
    /// 0x140597570
    pub step_up_side_z_max: f32,
    /// 0x140597570
    pub step_up_capsule_radius_arg: f32,
    /// 0x14057DC90
    pub walk_ramp_flat_limit: f32,
    /// 0x1405900B0
    pub walk_launch_z: f32,
    /// 0x144016CA0
    pub move_penetration_default_depth: f32,
    /// 0x14401AA70
    pub move_over_max_speed_factor: f32,
    /// 0x140577890
    pub move_braking_sub_step: f32,
    /// 0x140596E10
    pub move_spawn_frame_skip: bool,
    /// 0x14057AE30
    pub move_encroach_check: f32,
    /// 0x14058D0C0
    pub fall_two_wall_nudge: f32,
    /// 0x14058D0C0
    pub fall_xy_zero_threshold: f32,
    /// 0x140577F30
    pub gravity_hang_steps_60hz: f32,
    /// 0x14063A650
    pub probe_range: f32,
    /// 0x14063A650
    pub probe_sphere_radius: f32,
    /// 0x14063A650
    pub probe_start_height: f32,
    /// 0x14063A650
    pub probe_max_slope: f32,
    /// 0x1413A3D20
    pub probe_near_ground_max: f32,
    /// 0x141395450
    pub land_snap_height: f32,
    /// 0x1411FBA40
    pub land_control_return: f32,
    /// 0x141423210
    pub collision_profile: &'static str,
    /// 0x1405BC470
    pub frame_order: &'static str,
    /// 0x144590F00
    pub frame_max_delta_time: f32,
    /// 0x140595730
    pub anim_pose_tick_in_movement: bool,
    /// 0x140EDD580
    pub pad_jump_button: &'static str,
    /// 0x1414B6570
    pub key_jump: &'static str,
    /// 0x14442C190
    pub key_move: &'static str,
    /// 0x14442C190
    pub key_walk_modifier: f32,
}

pub static SORA_PARAMS: std::sync::LazyLock<SoraParams> = std::sync::LazyLock::new(|| {
    let s = crate::sheets::Sheet::named("sora-movement");
    SoraParams {
        gravity_scale: s.num("gravity.scale"),
        gravity_rate: s.num("gravity.rate"),
        gravity_jump_up_scale: s.num("gravity.jump_up_scale"),
        gravity_jump_duration_time: s.num("gravity.jump_duration_time"),
        gravity_jump_duration_scale: s.num("gravity.jump_duration_scale"),
        gravity_small_jump_scale: s.num("gravity.small_jump_scale"),
        jump_power_normal: s.num("jump.power_normal"),
        jump_max_height: s.num("jump.max_height"),
        speed_run: s.num("speed.run"),
        speed_sprint: s.num("speed.sprint"),
        speed_sprint_delay: s.num("speed.sprint_delay"),
        speed_walk_rate: s.num("speed.walk_rate"),
        speed_force_walk: s.num("speed.force_walk"),
        input_stick_active_min: s.num("input.stick_active_min"),
        input_walk_run_threshold: s.num("input.walk_run_threshold"),
        input_run_grace_time: s.num("input.run_grace_time"),
        accel_ground_factor: s.num("accel.ground_factor"),
        accel_air_factor: s.num("accel.air_factor"),
        accel_default: s.num("accel.default"),
        air_control: s.num("air.control"),
        air_braking_deceleration: s.num("air.braking_deceleration"),
        air_lateral_friction: s.num("air.lateral_friction"),
        ground_friction: s.num("ground.friction"),
        ground_braking_friction_factor: s.num("ground.braking_friction_factor"),
        ground_braking_deceleration: s.num("ground.braking_deceleration"),
        ground_stop_accel_sq_max: s.num("ground.stop_accel_sq_max"),
        turn_run_alpha_per_60hz_frame: s.num("turn.run_alpha_per_60hz_frame"),
        turn_run_stop_rate: s.num("turn.run_stop_rate"),
        turn_air_rate: s.num("turn.air_rate"),
        turn_idle_rate: s.num("turn.idle_rate"),
        turn_turn_around_angle: s.num("turn.turn_around_angle"),
        turn_rotation_rate_yaw: s.num("turn.rotation_rate_yaw"),
        floor_walkable_angle: s.num("floor.walkable_angle"),
        floor_walkable_z: s.num("floor.walkable_z"),
        floor_max_step_height: s.num("floor.max_step_height"),
        floor_always_check: s.flag("floor.always_check"),
        floor_can_walk_off_ledges: s.flag("floor.can_walk_off_ledges"),
        floor_maintain_horizontal_ground_velocity: s.flag("floor.maintain_horizontal_ground_velocity"),
        capsule_radius: s.num("capsule.radius"),
        capsule_half_height: s.num("capsule.half_height"),
        capsule_center_z: s.num("capsule.center_z"),
        sim_max_time_step: s.num("sim.max_time_step"),
        sim_max_iterations: s.num("sim.max_iterations"),
        fall_step_down_height: s.num("fall.step_down_height"),
        fall_step_down_time: s.num("fall.step_down_time"),
        land_light_height_max: s.num("land.light_height_max"),
        land_roll_height_min: s.num("land.roll_height_min"),
        land_length: s.num("land.length"),
        land_roll_control_return: s.num("land.roll_control_return"),
        land_roll_length: s.num("land.roll_length"),
        anim_run_start_length: s.num("anim.run_start_length"),
        anim_run_end_length: s.num("anim.run_end_length"),
        anim_walk_end_length: s.num("anim.walk_end_length"),
        gravity_world_z: s.num("gravity.world_z"),
        gravity_terminal_speed: s.num("gravity.terminal_speed"),
        jump_launch_delay: s.num("jump.launch_delay"),
        jump_small_jump_min_time: s.num("jump.small_jump_min_time"),
        jump_max_hold_time: s.num("jump.max_hold_time"),
        jump_coyote_time: s.num("jump.coyote_time"),
        jump_buffer_time: s.num("jump.buffer_time"),
        input_axis_dead_zone: s.num("input.axis_dead_zone"),
        input_move_scale: s.num("input.move_scale"),
        floor_dist_min: s.num("floor.dist_min"),
        floor_dist_max: s.num("floor.dist_max"),
        floor_dist_target: s.num("floor.dist_target"),
        floor_sweep_distance_walking: s.num("floor.sweep_distance_walking"),
        floor_sweep_distance_falling: s.num("floor.sweep_distance_falling"),
        floor_perch_radius: s.num("floor.perch_radius"),
        floor_falling_perch_additional_height: s.num("floor.falling_perch_additional_height"),
        floor_perch_additional_height: s.num("floor.perch_additional_height"),
        floor_max_perch_dist_walking: s.num("floor.max_perch_dist_walking"),
        floor_max_perch_dist_falling: s.num("floor.max_perch_dist_falling"),
        floor_perch_probe_min_half_height: s.num("floor.perch_probe_min_half_height"),
        floor_sweep_edge_reject: s.num("floor.sweep_edge_reject"),
        edge_fall_check_enabled: s.flag("edge.fall_check_enabled"),
        edge_sphere_radius: s.num("edge.sphere_radius"),
        edge_contact_limit_walking: s.num("edge.contact_limit_walking"),
        edge_line_max_traces: s.num("edge.line_max_traces"),
        edge_float_enabled: s.flag("edge.float_enabled"),
        edge_float_slide: s.num("edge.float_slide"),
        step_up_floor_dist_offset: s.num("step_up.floor_dist_offset"),
        step_up_down_extra: s.num("step_up.down_extra"),
        step_up_side_z_max: s.num("step_up.side_z_max"),
        step_up_capsule_radius_arg: s.num("step_up.capsule_radius_arg"),
        walk_ramp_flat_limit: s.num("walk.ramp_flat_limit"),
        walk_launch_z: s.num("walk.launch_z"),
        move_penetration_default_depth: s.num("move.penetration_default_depth"),
        move_over_max_speed_factor: s.num("move.over_max_speed_factor"),
        move_braking_sub_step: s.num("move.braking_sub_step"),
        move_spawn_frame_skip: s.flag("move.spawn_frame_skip"),
        move_encroach_check: s.num("move.encroach_check"),
        fall_two_wall_nudge: s.num("fall.two_wall_nudge"),
        fall_xy_zero_threshold: s.num("fall.xy_zero_threshold"),
        gravity_hang_steps_60hz: s.num("gravity.hang_steps_60hz"),
        probe_range: s.num("probe.range"),
        probe_sphere_radius: s.num("probe.sphere_radius"),
        probe_start_height: s.num("probe.start_height"),
        probe_max_slope: s.num("probe.max_slope"),
        probe_near_ground_max: s.num("probe.near_ground_max"),
        land_snap_height: s.num("land.snap_height"),
        land_control_return: s.num("land.control_return"),
        collision_profile: s.text("collision.profile"),
        frame_order: s.text("frame.order"),
        frame_max_delta_time: s.num("frame.max_delta_time"),
        anim_pose_tick_in_movement: s.flag("anim.pose_tick_in_movement"),
        pad_jump_button: s.text("pad.jump_button"),
        key_jump: s.text("key.jump"),
        key_move: s.text("key.move"),
        key_walk_modifier: s.num("key.walk_modifier"),
    }
});
