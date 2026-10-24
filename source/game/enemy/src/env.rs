//! What the host game supplies to the enemy engine: motion clip data, the
//! collision world and the other actors. All of it is read from the installed
//! game by the host; this crate holds no game data.

use crate::sysobj::Vec3;

/// Motion data of one model, in ticks. The host selects an explicit
/// evaluation profile; absence of a range or flag is `None`, never a guess.
pub trait Clips {
    /// Clip length in ticks (0 = the motion slot does not exist).
    fn length(&self, slot: usize) -> u32;
    fn loop_start(&self, slot: usize) -> u32;
    /// Root displacement over the step after `tick`, in model space.
    fn root_delta(&self, slot: usize, tick: u32) -> Vec3;
    /// Motion clock producer: the native 0 -> 2 first step, then +1.
    fn advance(&self, _slot: usize, tick: u32) -> u32 {
        tick.saturating_add(1)
    }
    /// The clip's trigger tables (`None` = the slot has none).
    fn events(&self, slot: usize) -> Option<&crate::motion::MotionEvents>;
    /// The clip finished on the step that produced `tick` (the end pulse).
    fn ended(&self, slot: usize, tick: u32) -> bool {
        tick >= self.length(slot)
    }
    fn in_loop(&self, slot: usize, tick: u32) -> bool {
        tick >= self.loop_start(slot)
    }
}

/// A body's map collision record (`CB`, obj+0x6d0) as the object's model
/// gives it: the first BAR-type-23 entry of kind 0 or 4 (FUN_1403c9bc0(.., 0x11)
/// in FUN_1403b3200): radius (+0x10), height (+0x12), the sphere's centre
/// above the feet (+0x0e), mode 1 for a kind-4 entry, and the class the
/// object's constructor set (CB+0x70 bits 3-6).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MapBody {
    pub radius: f32,
    pub height: f32,
    pub offset: f32,
    pub mode: u32,
    pub class: u32,
}

/// One body-versus-map move (FUN_14016fec0, then FUN_140170c10).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MapMove {
    /// Feet position after the move.
    pub position: Vec3,
    /// CB+0x74 bit 0 (obj+0x744): no floor under the body.
    pub no_floor: bool,
    /// The floor plane's normal (obj+0x758) in the engine's frame, for the
    /// next tick's ground projection; (0, -1, 0) without a floor.
    pub normal: Vec3,
}

/// The world around the actor.
pub trait Env {
    /// Highest walkable floor at (x, z) below `top`.
    fn floor(&self, x: f32, z: f32, top: f32) -> Option<f32>;
    /// The final position after a requested displacement (walls and floor).
    fn movement(&self, from: Vec3, wished: Vec3) -> Vec3 {
        let mut to = [0, 1, 2].map(|k| from[k] + wished[k]);
        if let Some(y) = self.floor(to[0], to[2], from[1].max(to[1]) + 200.) {
            to[1] = to[1].max(y);
        }
        to
    }
    /// FUN_1403b9090's solve for a body (FUN_14016fec0): a grounded move is
    /// first turned into the floor plane whose normal (engine frame) the
    /// last move left. `None` when the host has no map solver; the object
    /// then moves by [`Env::movement`].
    fn collide(
        &self,
        _body: &MapBody,
        _from: Vec3,
        _displacement: Vec3,
        _airborne: bool,
        _normal: Vec3,
    ) -> Option<MapMove> {
        None
    }
    /// The engine's ground probe (0 level, 1 higher, 2 lower, 3 no ground).
    fn check_step(
        &self,
        _position: Vec3,
        _direction: Vec3,
        _probe: f32,
        _up: f32,
        _down: f32,
    ) -> Option<u8> {
        None
    }
    /// The motion clips of an object's model.
    fn clips(&self, model: u32) -> &dyn Clips;
    /// Sora.
    fn player_position(&self) -> Vec3;
    /// Sora's drive form (`player_get_form`), 0 for none.
    fn player_form(&self) -> i32 {
        0
    }
    /// The current area number (`area_area`).
    fn area_number(&self) -> i32 {
        0
    }
    /// The first point where the map crosses the segment (`bghit_check_line`).
    fn line_hit(&self, _from: Vec3, _to: Vec3) -> Option<Vec3> {
        None
    }
    /// FUN_14040c2b0 -> FUN_140171350: the map's segment ray with the
    /// polygon filter cleared, against the polygons of collision `class`
    /// (the caller's mask: 1 for the ground search, 2 for the air search).
    fn line_hit_class(&self, from: Vec3, to: Vec3, class: u32) -> Option<Vec3> {
        self.line_hit_polygon(from, to, class).map(|(p, _)| p)
    }
    /// [`Env::line_hit_class`] with the polygon hit (FUN_14040c2b0's record
    /// +0x14); -1 when the host cannot name it.
    fn line_hit_polygon(&self, from: Vec3, to: Vec3, _class: u32) -> Option<(Vec3, i32)> {
        self.line_hit(from, to).map(|p| (p, -1))
    }
    /// Whether the map blocks the line between two points.
    fn line_blocked(&self, from: Vec3, to: Vec3) -> bool {
        self.line_hit(from, to).is_some()
    }
    /// Sora's heading about Y (the direction scripts read from the player object).
    fn player_yaw(&self) -> f32 {
        0.
    }
    /// Position of a target actor (centre of its TARGET shape).
    fn target_position(&self, id: u32) -> Option<Vec3>;
    /// Record `id` of the PTYA set of a player-class object (FUN_1403d74e0).
    fn party_attack(&self, _object: u32, _id: u8) -> Option<player::ptya::Record> {
        None
    }
    /// FUN_1403d7520's return: Sora's TARGET radius plus the target's.
    fn target_reach(&self, id: u32) -> Option<f32> {
        self.target_radius(id)
    }
    /// Sora's target record (+0xdc8 + 4) as FUN_1403d78d0 takes it: +0xdc8
    /// is the target holder DAT_142a11208 (built by FUN_1403dcb30, set by
    /// FUN_1403a7a40), whose automatic and manual states FUN_1403dcd70 /
    /// FUN_1403dd650 keep the lock-on's actor or the one targeted without
    /// one.
    fn attack_target(&self) -> Option<u32> {
        self.lockon_target()
    }
    /// The actor Sora has locked on to (`trap_obj_lockon_target`'s record).
    fn lockon_target(&self) -> Option<u32> {
        None
    }
    /// Where bone `bone` of an object is (`None`: the object's own origin),
    /// for the objects carried by it (`obj_attach`).
    fn attach_point(&self, _holder: u32, _bone: i32) -> Option<Vec3> {
        None
    }
    /// The normal of a map polygon (`bg_get_normal`; -1: the polygon of the
    /// last hit). `None`: straight up.
    fn map_normal(&self, _polygon: i32) -> Option<Vec3> {
        None
    }
    /// A map polygon's surface nibble (`attr & 0xf`, FUN_140170910); `None`
    /// when the host has no such polygon.
    fn map_surface(&self, _polygon: i32) -> Option<u32> {
        None
    }
    /// Whether an object may take a command now (FUN_1403ba070: its command
    /// lockout and motion windows).
    fn can_decide_command(&self, _object: u32, _command: u16) -> bool {
        false
    }
    /// The command's flags (+0xc), as FUN_1403ba070 tests them.
    fn command_flags(&self, _command: u16) -> u32 {
        0
    }
    /// The command's +0x1f: whose windows decide it (FUN_1403b6950).
    fn command_decider(&self, _command: u16) -> u8 {
        0
    }
    /// The command's +0x22: the motion window value that also takes it.
    fn command_chain(&self, _command: u16) -> u16 {
        0
    }
    /// Sora's push shapes (his model's object-collision kind 4, FUN_1403b4f10
    /// with FUN_1403c9c00), each centre relative to his origin in the
    /// script's +Y-down frame.
    fn player_push_shapes(&self) -> Vec<crate::traps::flare::Shape> {
        Vec::new()
    }
    /// The movement Sora's controller asks for this frame (`player_get_movement`).
    fn player_move_input(&self) -> Vec3 {
        [0.; 3]
    }
    /// A PRTY movement record (FUN_1403e9590 index), as FUN_14040a480
    /// would copy it to +0x128.
    fn movement_spec(&self, _index: i32) -> Option<player::actor_physics::ObjParams> {
        None
    }
    /// The pad buttons held and pressed this frame, one bit per button
    /// (`pad_is_button`, `pad_is_trigger`).
    fn pad_held(&self) -> u64 {
        0
    }
    fn pad_pressed(&self) -> u64 {
        0
    }
    /// The map's fog (near, far, min, max) a kind 1 light starts with.
    fn default_fog(&self) -> [f32; 4] {
        [0.; 4]
    }
    /// A number the host owns that scripts read: mission timer and score,
    /// enemies left in the area's units, the area cost, prizes. 0 when the
    /// host has none.
    fn service_value(&self, _name: &'static str, _argument: i32) -> i32 {
        0
    }
    /// The camera's answer to a point (its projection or inverse transform).
    fn camera_vector(&self, _v: [f32; 4]) -> Option<[f32; 4]> {
        None
    }
    /// A point in the view (FUN_14019ce40's matrix: x right, y down, z
    /// forward) and the projection's scale (FUN_14019d010, +0x40) in w.
    fn camera_view(&self, _p: Vec3) -> Option<[f32; 4]> {
        None
    }
    /// The direction an effect handle faces.
    fn effect_direction(&self, _handle: u32) -> Option<Vec3> {
        None
    }
    /// The radius of the object's TARGET shape (`obj_target_radius`).
    fn target_radius(&self, _object: u32) -> Option<f32> {
        None
    }
    /// The object's HIT shapes (object collision kind 2): part id and
    /// centre, for `obj_near_parts`.
    fn hit_parts(&self, _object: u32) -> Option<Vec<(u8, Vec3)>> {
        None
    }
    /// The TARGET shapes of an object for the target searcher: (shape id,
    /// centre). `None`: one shape, id 0, at the object's own position.
    fn target_shapes(&self, _object: u32) -> Option<Vec<(u32, Vec3)>> {
        None
    }
    /// The walk path of a spawn point (`unit`), or of any unit of the area
    /// for `None`, by id or group (FUN_1403ff460 / FUN_1403ff4a0 and the
    /// area-wide FUN_1403a5160 / FUN_1403a51c0). An id below zero is the
    /// first path of the unit.
    fn walk_path(
        &self,
        _unit: Option<u32>,
        _key: crate::route::PathKey,
    ) -> Option<crate::route::WalkPath> {
        None
    }
    /// The spawn point with this id (FUN_1403fe2d0).
    fn walk_unit(&self, _id: u32) -> Option<u32> {
        None
    }
    /// How far across a zone (along its first axis, 0 to 1) a position is
    /// (FUN_140421100).
    fn zone_progress(&self, _zone: u32, _position: Vec3) -> f32 {
        0.0
    }
    /// Sora is inside the object's reaction sphere (FUN_140417910).
    fn player_in_reaction(&self, _object: u32) -> bool {
        false
    }
    /// The object's head (its HEAD shape's place, else its target place;
    /// FUN_1403b58c0). `None`: its own position.
    fn head_position(&self, _object: u32) -> Option<Vec3> {
        None
    }
    /// The object's LOOK shape: where it looks from and how far round it may
    /// look, in radians (FUN_1403e4bc0). `None`: no shape.
    fn look_cone(&self, _object: u32) -> Option<(Vec3, f32)> {
        None
    }
    /// The event path points (DAT_142b59840's list): the two ints each point
    /// holds.
    fn event_path(&self) -> Vec<(i32, i32)> {
        Vec::new()
    }
    /// The model has hook motions `hook` (FUN_1403c7c10 finds the table).
    fn motion_hook(&self, _object: u32, _hook: i32) -> bool {
        false
    }
    /// The camera's eye and what it looks at (`camera_eye`, `camera_at`).
    fn camera_eye(&self) -> Option<Vec3> {
        None
    }
    fn camera_at(&self) -> Option<Vec3> {
        None
    }
    /// A point through the inverse of the camera's matrix
    /// (`camera_apply_pos`).
    fn camera_apply_pos(&self, _v: [f32; 4]) -> Option<[f32; 4]> {
        None
    }
    /// The matrix of an object's bone, sixteen floats in the order
    /// `out[k] = m[k]*x + m[4+k]*y + m[8+k]*z + m[12+k]*w`
    /// (FUN_1403b50f0). `None`: no skeleton, the identity.
    fn bone_matrix(&self, _object: u32, _bone: u32) -> Option<[f32; 16]> {
        None
    }
    /// The friend's action record whose id is `id` (FUN_1401cc770: the
    /// personality's 0x80-byte records at header +0xa4, matched on their
    /// first word), as 32 words; `None` when it has none.
    fn action_param(&self, _object: u32, _id: i32) -> Option<[u32; 32]> {
        None
    }
    /// How full mission gauge `index` is, 0 to 1 (FUN_1403fb690).
    fn mission_gauge_ratio(&self, _index: i32) -> f32 {
        0.0
    }
    /// The light cycles' speed parameter (DAT_142af7bc8 + 4).
    fn lightcycle_speed(&self) -> f32 {
        0.0
    }
    /// The battle target table's choice for an actor (`None`: no entry).
    fn pick_target(&self, _kind: i32, _from: Vec3) -> Option<u32> {
        None
    }
    fn is_culled(&self, _position: Vec3) -> bool {
        false
    }
    /// `DAT_142a11400` bit 2: enemies may not leave.
    fn no_leave(&self) -> bool {
        false
    }
    /// Sora cannot be damaged now (his vt+0xb8, FUN_1403d65a0): an
    /// invulnerable motion range or timer.
    fn player_invulnerable(&self) -> bool {
        false
    }
    /// `DAT_142a11400` bit 7: every enemy leaves.
    fn force_leave(&self) -> bool {
        false
    }
    fn event_running(&self) -> bool {
        false
    }
    fn tutorial_open(&self) -> bool {
        false
    }
    fn area_world(&self) -> i32 {
        0
    }
    /// The battle flag (`DAT_142a11404` != 0, FUN_1403abb20).
    fn battle(&self) -> bool {
        false
    }
    /// Peter Pan is out with Tinker Bell (FUN_1403de320: summon state set,
    /// summon 0x132, `DAT_1409acdd7`).
    fn summon_with_tink(&self) -> bool {
        false
    }
    /// The running event's time left (FUN_1403ac190, an event in mode 8).
    fn event_rest_time(&self) -> f32 {
        0.
    }
    /// The command of the running limit (FUN_1403d89f0), 0 for none.
    fn limit_command(&self) -> u16 {
        0
    }
    /// Sora's velocity (+0xa38).
    fn player_velocity(&self) -> Vec3 {
        [0.; 3]
    }
    /// Sora's sheet HP and maximum HP.
    fn player_hp(&self) -> Option<(i32, i32)> {
        None
    }
    /// Sora's sheet MP and maximum MP (+0x180 / +0x184).
    fn player_mp(&self) -> Option<(i32, i32)> {
        None
    }
    /// Sora's sheet +0x1b4: the form or summon time left.
    fn drive_time(&self) -> f32 {
        0.
    }
    /// A game progress flag (FUN_14039a980).
    fn progress_flag(&self, _flag: u32) -> bool {
        false
    }
    /// Where `local` (a point in bone `bone` of the model of `object`, which
    /// stands at `position` facing `yaw`) is in the world
    /// (`trap_obj_apply_bone_matrix`); `None` when the host has no bone data
    /// for the object.
    fn bone_point(
        &self,
        _object: u32,
        _bone: i32,
        _local: Vec3,
        _position: Vec3,
        _yaw: f32,
    ) -> Option<Vec3> {
        None
    }
    /// An effect handle `trap_obj_effect_start_bind` returned is still alive.
    fn effect_active(&self, _handle: u32) -> bool {
        false
    }
    fn effect_position(&self, _handle: u32) -> Option<Vec3> {
        None
    }
}

/// A host with nothing in it: a flat floor at zero, no motion data, Sora at
/// the origin. For tools and tests.
pub struct NullEnv;

struct NoClips;
impl Clips for NoClips {
    fn length(&self, _: usize) -> u32 {
        0
    }
    fn loop_start(&self, _: usize) -> u32 {
        0
    }
    fn root_delta(&self, _: usize, _: u32) -> Vec3 {
        [0.; 3]
    }
    fn events(&self, _: usize) -> Option<&crate::motion::MotionEvents> {
        None
    }
}

impl Env for NullEnv {
    fn floor(&self, _: f32, _: f32, _: f32) -> Option<f32> {
        Some(0.)
    }
    fn clips(&self, _: u32) -> &dyn Clips {
        &NoClips
    }
    fn player_position(&self) -> Vec3 {
        [0.; 3]
    }
    fn target_position(&self, _: u32) -> Option<Vec3> {
        None
    }
}
