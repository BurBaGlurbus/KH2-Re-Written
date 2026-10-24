//! Synthetic objects and programs only.
use super::*;
use crate::env::NullEnv;
use crate::script::Memory;
use crate::traps::test_support::{Frame, ai_object, call, frame, vector, world_with};
use crate::world::{AppearRequest, Appeared, Object, Spawner, World, object_handle};

fn setup() -> (World, Object, Frame) {
    (World::default(), Object::bare(0), frame())
}

#[test]
fn a_light_fades_in_and_out_and_keeps_its_fog_and_flags() {
    let (mut world, mut me, mut f) = setup();
    let create = lookup(1, 181).unwrap();
    let light = call(&mut world, &mut me, &mut f, create, &[Slot::int(1)]).value;
    let slot = world.light_slot(light).expect("a light");
    assert_eq!(world.lights[slot].as_ref().unwrap().fade.value, 1.0);
    assert_eq!(world.lights[slot].as_ref().unwrap().color, [0x80; 4]);
    // Colour: r, g, b and a fade in over 4 ticks.
    let color = lookup(1, 183).unwrap();
    call(
        &mut world,
        &mut me,
        &mut f,
        color,
        &[
            Slot::address(light),
            Slot::int(10),
            Slot::int(20),
            Slot::int(30),
            Slot::float(4.),
        ],
    );
    assert_eq!(
        world.lights[slot].as_ref().unwrap().color,
        [10, 20, 30, 0x80]
    );
    assert_eq!(world.lights[slot].as_ref().unwrap().fade.value, 0.0);
    for _ in 0..4 {
        world.tick_attacks(&NullEnv);
    }
    assert_eq!(world.lights[slot].as_ref().unwrap().fade.value, 1.0);
    let fadeout = lookup(1, 184).unwrap();
    call(
        &mut world,
        &mut me,
        &mut f,
        fadeout,
        &[Slot::address(light), Slot::float(4.)],
    );
    world.tick_attacks(&NullEnv);
    assert_eq!(world.lights[slot].as_ref().unwrap().fade.value, 0.75);
    let fog = lookup(1, 258).unwrap();
    call(
        &mut world,
        &mut me,
        &mut f,
        fog,
        &[Slot::address(light), Slot::float(900.)],
    );
    assert_eq!(world.lights[slot].as_ref().unwrap().fog[1], 900.);
    let flag = lookup(1, 182).unwrap();
    call(
        &mut world,
        &mut me,
        &mut f,
        flag,
        &[Slot::address(light), Slot::int(3)],
    );
    assert_eq!(world.lights[slot].as_ref().unwrap().flags[0], 8);
}

#[test]
fn a_vacuum_is_made_shaped_and_freed() {
    let (mut world, mut me, mut f) = setup();
    let at = vector(&mut f, 0, [1., 2., 3., 1.]);
    let create = lookup(2, 77).unwrap();
    let vac = call(
        &mut world,
        &mut me,
        &mut f,
        create,
        &[Slot::address(at), Slot::float(7.)],
    )
    .value;
    let slot = world.vacuum_slot(vac).expect("a vacuum");
    let v = world.vacuums[slot].clone().unwrap();
    assert_eq!(v.at_18, [7.; 3]);
    assert_eq!((v.at_28, v.at_2c), (100., 0.01));
    let range = lookup(7, 32).unwrap();
    call(
        &mut world,
        &mut me,
        &mut f,
        range,
        &[Slot::address(vac), Slot::float(250.)],
    );
    let flag = lookup(2, 79).unwrap();
    call(
        &mut world,
        &mut me,
        &mut f,
        flag,
        &[Slot::address(vac), Slot::int(2)],
    );
    let v = world.vacuums[slot].clone().unwrap();
    assert_eq!((v.effective_range, v.flags), (250., 4));
    let destroy = lookup(2, 78).unwrap();
    call(&mut world, &mut me, &mut f, destroy, &[Slot::address(vac)]);
    assert!(world.vacuum_slot(vac).is_none());
}

#[test]
fn the_status_bits_scripts_set_show_in_the_leave_queries() {
    let (mut world, mut me, mut f) = setup();
    let query = crate::traps::lookup(1, 206).unwrap();
    assert_eq!(call(&mut world, &mut me, &mut f, query, &[]).as_int(), 0);
    let start = lookup(1, 204).unwrap();
    call(&mut world, &mut me, &mut f, start, &[]);
    assert_eq!(call(&mut world, &mut me, &mut f, query, &[]).as_int(), 1);
    let end = lookup(1, 205).unwrap();
    call(&mut world, &mut me, &mut f, end, &[]);
    assert_eq!(call(&mut world, &mut me, &mut f, query, &[]).as_int(), 0);
}

struct BareSpawner;

impl Spawner for BareSpawner {
    fn appear(&mut self, request: &AppearRequest) -> Option<Appeared> {
        (request.entry != 999).then(|| Appeared {
            payload: None,
            sys: crate::sysobj::Sysobj::bare(),
        })
    }
}

#[test]
fn sysobj_appear_makes_an_object_the_searches_then_find() {
    let (mut world, mut me, mut f) = setup();
    world.spawner = Some(Box::new(BareSpawner));
    let appear = crate::traps::lookup(1, 0).unwrap();
    let handle = call(&mut world, &mut me, &mut f, appear, &[Slot::int(1895)]).value;
    assert_eq!(handle, object_handle(1));
    assert_eq!(world.objects[1].as_ref().unwrap().sys.entry_id, 1895);
    // A search by entry id reaches it.
    let ai = ai_object(&mut f, 0, 0);
    let search = crate::traps::lookup(1, 114).unwrap();
    call(
        &mut world,
        &mut me,
        &mut f,
        search,
        &[Slot::address(ai), Slot::int(1895)],
    );
    let memory = Memory::new(&mut f.inst, &mut f.th);
    assert_eq!(memory.read_u32(ai + 4), Some(object_handle(1)));
    // An entry the host cannot make gives the script a null object.
    let none = call(&mut world, &mut me, &mut f, appear, &[Slot::int(999)]).value;
    assert_eq!(none, 0);
}

#[test]
fn each_all_walks_the_object_list_in_order() {
    let (_, mut me, mut f) = setup();
    let mut world = world_with(&[0., 0., 0.]);
    world.objects[2].as_mut().unwrap().sys.flags = 0x0008_0000; // hidden from the walk
    let each = lookup(1, 331).unwrap();
    let mut seen = Vec::new();
    let mut at = Slot::address(0);
    for _ in 0..8 {
        let next = call(&mut world, &mut me, &mut f, each, &[at]);
        if next.value == 0 {
            break;
        }
        seen.push(crate::world::object_from_handle(next.value).unwrap());
        at = next;
    }
    assert_eq!(
        seen,
        [0, 1, 3],
        "the caller first, the hidden object skipped"
    );
}

#[test]
fn enemy_each_walks_only_the_enemy_class() {
    let (_, mut me, mut f) = setup();
    me.sys.class_flags = 0;
    let mut world = world_with(&[0., 0.]);
    world.objects[2].as_mut().unwrap().sys.class_flags = 0;
    let each = lookup(2, 4).unwrap();
    let first = call(&mut world, &mut me, &mut f, each, &[Slot::address(0)]);
    assert_eq!(crate::world::object_from_handle(first.value), Some(1));
    assert_eq!(call(&mut world, &mut me, &mut f, each, &[first]).value, 0);
}

#[test]
fn the_shadow_and_peter_pan_drain_bits_and_the_battle_and_tink_queries() {
    let (mut world, mut me, mut f) = setup();
    for (index, bit) in [(286u16, status::HIDE_SHADOW), (287, 0)] {
        call(&mut world, &mut me, &mut f, lookup(1, index).unwrap(), &[]);
        assert_eq!(world.status & status::HIDE_SHADOW, bit);
    }
    call(&mut world, &mut me, &mut f, lookup(6, 19).unwrap(), &[]);
    assert_ne!(world.status & status::PETERPAN_PRIZE_DRAIN, 0);
    call(&mut world, &mut me, &mut f, lookup(6, 20).unwrap(), &[]);
    assert_eq!(world.status & status::PETERPAN_PRIZE_DRAIN, 0);
    // NullEnv: no battle, no Tinker Bell, no event.
    for (table, index) in [(1u16, 244u16), (2, 75)] {
        let r = call(
            &mut world,
            &mut me,
            &mut f,
            lookup(table, index).unwrap(),
            &[],
        );
        assert_eq!(r.as_int(), 0);
    }
    let r = call(&mut world, &mut me, &mut f, lookup(1, 274).unwrap(), &[]);
    assert_eq!(r.as_float(), 0.);
}

/// A host whose friend has one action record (id 0x28, motion 151).
struct Records;

impl crate::env::Env for Records {
    fn floor(&self, _: f32, _: f32, _: f32) -> Option<f32> {
        Some(0.)
    }
    fn clips(&self, _: u32) -> &dyn crate::env::Clips {
        crate::env::Env::clips(&NullEnv, 0)
    }
    fn player_position(&self) -> crate::sysobj::Vec3 {
        [0.; 3]
    }
    fn target_position(&self, _: u32) -> Option<crate::sysobj::Vec3> {
        None
    }
    fn action_param(&self, object: u32, id: i32) -> Option<[u32; 32]> {
        (object == 1 && id == 0x28).then(|| {
            let mut w = [0; 32];
            w[0] = 0x28;
            w[1] = 151;
            w[8] = 1.5f32.to_bits();
            w
        })
    }
}

#[test]
fn an_action_param_is_the_hosts_record_in_engine_memory() {
    let mut world = world_with(&[0.]);
    let mut me = Object::bare(0);
    let mut f = frame();
    let ai = ai_object(&mut f, 0, 1);
    let trap = lookup(10, 35).unwrap();
    let mut run = |id: i32| {
        let mut slots = [Slot::default(); 16];
        slots[0] = Slot::address(ai);
        slots[1] = Slot::int(id);
        let mut memory = Memory::new(&mut f.inst, &mut f.th);
        let mut ctx = Ctx {
            world: &mut world,
            me: &mut me,
            env: &Records,
        };
        trap(&mut ctx, &mut memory, &mut slots).unwrap();
        slots[0].value
    };
    let record = run(0x28);
    assert_ne!(record, 0);
    // The same record comes back at the same place.
    assert_eq!(run(0x28), record);
    assert_eq!(run(0x29), 0);
    assert_eq!(world.static_read(record), Some(0x28));
    assert_eq!(world.static_read(record + 4), Some(151));
    assert_eq!(world.static_read(record + 0x20), Some(1.5f32.to_bits()));
    assert_eq!(world.static_read(record + 0x7c), Some(0));
}
