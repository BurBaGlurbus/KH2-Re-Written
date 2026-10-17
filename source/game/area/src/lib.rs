//! Field areas: progress-driven programs, the ARD spawn scripts, spawn
//! points and their zones (area-to-area exits), translated from the PC exe.
//! Positions are engine coordinates (Y down).
pub mod progress;
pub mod script;
pub mod world_state;
pub mod zone;

use data::ard::{Entity, SpawnPoint, SpawnSet};
use script::AreaSetup;
use zone::Zone;

/// π and 2π as the exe stores them (DAT_140623b78, DAT_140623ba4).
pub const PI: f32 = std::f32::consts::PI;
pub const TWO_PI: f32 = std::f32::consts::TAU;

/// FUN_1403b6f20: an fmod wrap of an angle.
pub fn wrap(a: f32) -> f32 {
    if 0.0 < a {
        (a + PI) % TWO_PI - PI
    } else {
        PI - (PI - a) % TWO_PI
    }
}

/// FUN_1403b7470: yaw → forward (sin, 0, cos).
pub fn forward(yaw: f32) -> [f32; 3] {
    let a = wrap(yaw);
    [a.sin(), 0., a.cos()]
}

/// The objentry id that stands for the current party leader.
pub const PLAYER: u32 = 566;
pub const FRIEND_1: u32 = 567;
pub const FRIEND_2: u32 = 568;

/// A spawn object (the 0x58-byte runtime record).
#[derive(Clone, Debug)]
pub struct SpawnObject {
    pub set: u32,
    pub point: SpawnPoint,
    /// bit0 disabled / fired, bit1 activated, bit3 zone hit this frame,
    /// bit4 guide shown this frame.
    pub flags: u32,
    pub deferred: bool,
    pub zones: Vec<Zone>,
    pub timer: f32,
    /// +0x0e "triggered" byte of the record.
    pub triggered: bool,
}

/// An entity the area creates, with the object yaw FUN_1403fe590 passes on.
#[derive(Clone, Debug)]
pub struct Spawn {
    pub set: u32,
    /// Index into `Area::objects` of the spawn point that holds it.
    pub point: usize,
    pub entity: Entity,
    /// `wrap(π − rotY)`.
    pub yaw: f32,
}

/// The player state the zone task reads.
#[derive(Clone, Copy, Debug, Default)]
pub struct PlayerProbe {
    pub position: [f32; 3],
    pub yaw: f32,
    /// Move direction (+0x10), unit or zero.
    pub move_direction: [f32; 3],
    /// +0x1c: 0 still, 2 walk, 8 run.
    pub move_speed: u32,
    /// FUN_1403bfb30: the player may act and move.
    pub can_move: bool,
}

/// What a zone hit asks the game to do.
#[derive(Clone, Debug, PartialEq)]
pub enum Request {
    /// FUN_140152990 with the location `{world, area, entrance}` and
    /// programs −1; `world_changed` selects the fade presentation.
    Jump {
        world: u8,
        area: u8,
        entrance: u8,
        world_changed: bool,
    },
    /// FUN_140167fc0: show the destination's name this frame (opt 2
    /// appends sys 0x8add).
    Guide { world: u8, area: u8, suffix: bool },
    /// FUN_140167ef0 with FUN_1403f0470's message: the destination is
    /// story-blocked and the guide zone is faced (shown for 6 s).
    Blocked { world: u8, area: u8 },
    /// Type 3: raise signal `signal` with the point id.
    Signal { signal: u32, id: u16 },
    /// Type 8: switch background groups (0-based).
    BgGroups { on: Option<u8>, off: Option<u8> },
}

pub struct Area {
    pub world: u8,
    pub area: u8,
    pub entrance: u8,
    pub setup: AreaSetup,
    pub objects: Vec<SpawnObject>,
    /// DAT_142a10500 bit 2: set when a jump was requested; zones stop.
    pub locked: bool,
}

impl Area {
    /// Registers every set the scripts named (FUN_1403a4e80), in order.
    /// `set(name)` returns the type-12 entry named `name`.
    pub fn new(
        world: u8,
        area: u8,
        entrance: u8,
        setup: AreaSetup,
        mut set: impl FnMut(u32) -> Option<SpawnSet>,
    ) -> Self {
        let mut objects = Vec::new();
        for r in &setup.spawns {
            let Some(s) = set(r.name) else { continue };
            for point in s.points {
                let mut flags = 0;
                // FUN_1403fdce0: type 5 or flags bit0 start disabled.
                if point.kind == 5 || point.flags & 1 != 0 {
                    flags |= 1;
                }
                let timer = if point.kind == 2 {
                    u16::from_le_bytes([point.header[0x1c], point.header[0x1d]]) as f32
                } else {
                    0.
                };
                objects.push(SpawnObject {
                    set: r.name,
                    point,
                    flags,
                    deferred: r.deferred,
                    zones: Vec::new(),
                    timer,
                    triggered: false,
                });
            }
        }
        Self {
            world,
            area,
            entrance,
            setup,
            objects,
            locked: false,
        }
    }

    /// FUN_1403a4e10 → FUN_1403ff550 for every non-deferred object: builds
    /// zones and returns the entities to create. `objentry_type(id)` is the
    /// 00objentry type byte, `None` when the id has no record.
    pub fn activate(&mut self, objentry_type: impl Fn(u32) -> Option<u8>) -> Vec<Spawn> {
        let mut out = Vec::new();
        for (object, o) in self
            .objects
            .iter_mut()
            .enumerate()
            .filter(|(_, o)| !o.deferred)
        {
            o.flags |= 2;
            for (i, a) in o.point.activators.iter().enumerate() {
                if (o.point.kind == 6 || a.option.wrapping_sub(3) > 1)
                    && let Some(z) = Zone::new(a, i)
                {
                    o.zones.push(z);
                }
            }
            if o.point.kind == 9 {
                continue;
            }
            for e in &o.point.entities {
                let Some(kind) = objentry_type(e.object) else {
                    continue;
                };
                if e.spawn_type == 1 && e.spawn_arg != self.entrance {
                    continue;
                }
                // Type 16 objects need FUN_140551d20 [not traced]; skipped.
                if kind == 0x10 {
                    continue;
                }
                // Spawn types 0, 1 and 3 create the entity unless the point
                // starts disabled; type 2 only counts (respawn points).
                if matches!(e.spawn_type, 0 | 1 | 3) && o.flags & 1 == 0 {
                    out.push(Spawn {
                        set: o.set,
                        point: object,
                        entity: e.clone(),
                        yaw: wrap(PI - e.rotation[1]),
                    });
                }
            }
        }
        out
    }

    /// Entities of spawn type 2 (zone pop-in, FUN_1403fe320 case 5) of
    /// activated points, in file order. `point` is the object index.
    pub fn zone_spawns(&self, objentry_type: impl Fn(u32) -> Option<u8>) -> Vec<Spawn> {
        let mut out = Vec::new();
        for (object, o) in self.objects.iter().enumerate() {
            if o.deferred || o.flags & 2 == 0 {
                continue;
            }
            for e in &o.point.entities {
                if e.spawn_type == 2 && objentry_type(e.object).is_some() {
                    out.push(Spawn {
                        set: o.set,
                        point: object,
                        entity: e.clone(),
                        yaw: wrap(PI - e.rotation[1]),
                    });
                }
            }
        }
        out
    }

    /// The player entity this entrance uses, if any.
    pub fn player_spawn(spawns: &[Spawn]) -> Option<&Spawn> {
        spawns.iter().find(|s| s.entity.object == PLAYER)
    }

    /// FUN_1403a4f90 / FUN_1403ff000 for one frame. `blocked(world, area)`
    /// is FUN_1403f0560 ('dsa'); `event_running` covers FUN_1403abc80's
    /// event and mission gates.
    pub fn tick(
        &mut self,
        player: &PlayerProbe,
        event_running: bool,
        blocked: impl Fn(u8, u8) -> bool,
    ) -> Vec<Request> {
        let mut out = Vec::new();
        if self.locked || event_running {
            for o in &mut self.objects {
                o.flags &= !0x18;
            }
            return out;
        }
        let current_world = self.world;
        let mut jumped = false;
        for o in self.objects.iter_mut().filter(|o| o.flags & 2 != 0) {
            o.flags &= !0x18;
            if o.flags & 1 != 0 || jumped {
                continue;
            }
            if o.point.kind == 4 {
                // Zone-tag points act on every object, not the player
                // (FUN_1403ff000 type 4); objects other than the player are
                // not modelled here.
                continue;
            }
            if o.timer > 0. {
                o.timer -= 1.0;
            }
            for zi in 0..o.zones.len() {
                let z = &o.zones[zi];
                if o.point.kind == 6 && z_option(&o.point, z) != 1 {
                    continue;
                }
                if !z.contains(player.position) {
                    continue;
                }
                o.flags |= 8;
                let (stop, req) = hit(o, zi, player, current_world, &blocked);
                if let Some(r) = req {
                    if matches!(r, Request::Jump { .. }) {
                        jumped = true;
                    }
                    out.push(r);
                }
                if stop {
                    break;
                }
            }
        }
        if jumped {
            self.locked = true;
        }
        out
    }
}

/// Type 6 tests only zones whose u16 +0x2c is 1.
fn z_option(p: &SpawnPoint, z: &Zone) -> u16 {
    let raw = &p.activators[z.activator].raw;
    u16::from_le_bytes([raw[0x2c], raw[0x2d]])
}

/// FUN_1403fe320 for the spawn types that act on the player. Returns
/// (stop testing further zones of this object, request).
fn hit(
    o: &mut SpawnObject,
    zi: usize,
    player: &PlayerProbe,
    current_world: u8,
    blocked: &impl Fn(u8, u8) -> bool,
) -> (bool, Option<Request>) {
    match o.point.kind {
        1 => {
            o.triggered = true;
            (false, exit(o, zi, player, current_world, blocked))
        }
        3 => {
            o.triggered = true;
            o.flags |= 1;
            (
                true,
                Some(Request::Signal {
                    signal: o.point.header_word(),
                    id: o.point.id,
                }),
            )
        }
        8 => {
            let a = &o.point.activators[o.zones[zi].activator];
            let on = (a.group_on != 0).then(|| a.group_on - 1);
            let off = (a.group_off != 0).then(|| a.group_off - 1);
            (false, Some(Request::BgGroups { on, off }))
        }
        // Types 2, 6, 7, 9 and 10 (respawns, battle zones, region ids)
        // are not modelled yet.
        _ => (true, None),
    }
}

/// FUN_1403ffb40.
fn exit(
    o: &mut SpawnObject,
    zi: usize,
    player: &PlayerProbe,
    current_world: u8,
    blocked: &impl Fn(u8, u8) -> bool,
) -> Option<Request> {
    let a = o.point.exit_area();
    let (world, area) = match a {
        100 => (0x12, 1),
        101 => (2, 40),
        _ => (current_world, a),
    };
    let entrance = o.point.exit_entrance();
    let is_blocked = blocked(world, area);
    let z = &o.zones[zi];
    match z.option {
        0 => {
            if o.flags & 1 == 0 && !is_blocked && direction_ok(z, o.point.exit_zone_arg(), player) {
                o.flags |= 1;
                return Some(Request::Jump {
                    world,
                    area,
                    entrance,
                    world_changed: world != current_world,
                });
            }
            None
        }
        1 | 2 if o.flags & 0x10 == 0 => {
            let v = [
                z.center[0] - player.position[0],
                0.,
                z.center[2] - player.position[2],
            ];
            let len = (v[0] * v[0] + v[2] * v[2]).sqrt();
            let v = if len > 0. {
                [v[0] / len, 0., v[2] / len]
            } else {
                v
            };
            let f = forward(player.yaw);
            if v[0] * f[0] + v[1] * f[1] + v[2] * f[2] >= 0.0 {
                o.flags |= 0x10;
                return Some(if is_blocked {
                    Request::Blocked { world, area }
                } else {
                    Request::Guide {
                        world,
                        area,
                        suffix: z.option == 2,
                    }
                });
            }
            None
        }
        _ => None,
    }
}

/// FUN_140421330 / FUN_1403ba9e0.
fn direction_ok(z: &Zone, mode: u8, p: &PlayerProbe) -> bool {
    match mode {
        0 => true,
        1 | 2 => {
            if !p.can_move || p.move_speed == 0 {
                return false;
            }
            let dir = forward(-z.rot_y - 1.570_796_4);
            let f = forward(p.yaw);
            let dot = |a: [f32; 3], b: [f32; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
            dot(dir, f) >= 0.7 && dot(dir, p.move_direction) >= 0.7
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use data::ard::Activator;

    fn entity(object: u32, spawn_type: u8, spawn_arg: u8, rot_y: f32) -> Entity {
        Entity {
            object,
            position: [1., 2., 3.],
            rotation: [0., rot_y, 0.],
            spawn_type,
            spawn_arg,
            serial: 0,
            arg1: 0,
            arg2: 0,
            talk: 0,
            respawn_delay: 0,
            reaction: 0,
            radius: 0,
            required_count: 0,
            raw: [0; 0x40],
        }
    }

    fn point(kind: u8, entities: Vec<Entity>, activators: Vec<Activator>) -> SpawnPoint {
        let mut header = [0; 0x2c];
        header[0] = kind;
        header[0x1c] = 6;
        header[0x1d] = 3;
        SpawnPoint {
            kind,
            flags: 0,
            id: 40,
            header,
            entities,
            activators,
            walk_paths: vec![],
            return_params: vec![],
            signals: vec![],
        }
    }

    fn box_zone(option: u16) -> Activator {
        Activator {
            shape: 0,
            option,
            center: [0., 0., 100.],
            extent: [50., 50., 50.],
            rotation: [0.; 3],
            flags: 0,
            group_on: 0,
            group_off: 0,
            raw: [0; 0x40],
        }
    }

    fn area(points: Vec<SpawnPoint>, entrance: u8) -> Area {
        let mut setup = AreaSetup::default();
        setup.spawns.push(script::Registered {
            name: 1,
            deferred: false,
        });
        let mut points = Some(points);
        Area::new(2, 8, entrance, setup, |_| {
            Some(SpawnSet {
                version: 2,
                points: points.take()?,
            })
        })
    }

    #[test]
    fn wrap_matches_the_fmod_form() {
        assert!((wrap(PI - 1.7802) - (PI - 1.7802)).abs() < 1e-6);
        assert!((wrap(4.0) - (4.0 - TWO_PI)).abs() < 1e-5);
        assert!((wrap(-4.0) - (TWO_PI - 4.0)).abs() < 1e-5);
    }

    #[test]
    fn entrance_selects_the_player() {
        let mut a = area(
            vec![point(
                1,
                vec![
                    entity(PLAYER, 1, 0, 0.),
                    entity(PLAYER, 1, 1, 1.),
                    entity(900, 0, 0, 0.),
                ],
                vec![],
            )],
            1,
        );
        let spawns = a.activate(|_| Some(1));
        assert_eq!(spawns.len(), 2);
        let p = Area::player_spawn(&spawns).unwrap();
        assert_eq!(p.entity.spawn_arg, 1);
        assert!((p.yaw - (PI - 1.)).abs() < 1e-6);
    }

    #[test]
    fn exit_jumps_once_and_guide_needs_facing() {
        let mut a = area(vec![point(1, vec![], vec![box_zone(0), box_zone(1)])], 0);
        a.activate(|_| Some(1));
        let mut p = PlayerProbe {
            position: [0., 0., 100.],
            // Facing −Z, away from nothing: the guide centre is the player's
            // own position here, so the dot is 0 and the guide shows.
            yaw: PI,
            ..Default::default()
        };
        let r = a.tick(&p, false, |_, _| false);
        assert_eq!(
            r[0],
            Request::Jump {
                world: 2,
                area: 6,
                entrance: 3,
                world_changed: false
            }
        );
        assert!(a.locked);
        assert!(a.tick(&p, false, |_, _| false).is_empty());
        // Outside: nothing.
        let mut b = area(vec![point(1, vec![], vec![box_zone(0)])], 0);
        b.activate(|_| Some(1));
        p.position = [0., 0., 200.];
        assert!(b.tick(&p, false, |_, _| false).is_empty());
        // Blocked destination: no jump.
        p.position = [0., 0., 100.];
        assert!(b.tick(&p, false, |_, _| true).is_empty());
    }
}
