//! The battle target table, on synthetic objects.
use super::*;
use crate::env::NullEnv;
use crate::sysobj::{class_flag, state_flag};
use crate::traps::test_support::{ai_object, call, frame, world_with};
use crate::world::{Object, World};

fn register(world: &mut World, id: u32, priority: i32) {
    let mut f = frame();
    let mut me = Object::bare(0);
    let ai = ai_object(&mut f, 0, id);
    call(
        world,
        &mut me,
        &mut f,
        target_set_before_player,
        &[Slot::address(ai), Slot::int(priority)],
    );
}

fn pick(world: &mut World) -> Option<ObjId> {
    let mut me = Object::bare(0);
    let mut ctx = Ctx {
        world,
        me: &mut me,
        env: &NullEnv,
    };
    pick_battle_target(&mut ctx)
}

#[test]
fn priority_zero_wins_then_the_lowest_priority() {
    let mut world = world_with(&[0., 10., 20., 30.]);
    register(&mut world, 1, 5);
    register(&mut world, 2, 3);
    assert_eq!(pick(&mut world), Some(2));
    register(&mut world, 3, 0);
    assert_eq!(pick(&mut world), Some(3));
}

#[test]
fn a_registered_object_keeps_its_slot_and_takes_the_new_priority() {
    let mut world = world_with(&[0., 10.]);
    register(&mut world, 1, 5);
    register(&mut world, 1, 2);
    let used = world
        .battle_targets
        .iter()
        .filter(|t| t.object == Some(1))
        .count();
    assert_eq!(used, 1);
    assert!(world.battle_targets.iter().any(|t| t.priority == 2));
}

#[test]
fn gone_objects_leave_the_table_and_ties_draw_from_the_engine_generator() {
    let mut world = world_with(&[0., 10., 20., 30.]);
    register(&mut world, 1, 4);
    register(&mut world, 2, 4);
    // Each object takes the last free slot, so object 2 (slot 6) is met
    // before object 1 (slot 7). The first draw of the engine generator
    // (seed 1) is 0x10dcd, a small fraction of the range: the first met.
    assert_eq!(pick(&mut world), Some(2));
    world.objects[2].as_mut().unwrap().sys.gone = true;
    assert_eq!(pick(&mut world), Some(1));
    register(&mut world, 3, 9);
    assert!(world.battle_targets.iter().all(|t| t.object != Some(2)));
}

#[test]
fn kind_zero_search_uses_the_table_and_falls_back_to_sora() {
    let mut world = world_with(&[0., 10.]);
    let mut me = Object::bare(0);
    let mut ctx = Ctx {
        world: &mut world,
        me: &mut me,
        env: &NullEnv,
    };
    assert_eq!(search(&mut ctx, 0, 0).unwrap().id, PLAYER_TARGET);
    register(ctx.world, 1, 7);
    assert_eq!(search(&mut ctx, 0, 0).unwrap().id, OBJECT_TARGET | 1);
}

#[test]
fn a_record_naming_an_object_reads_back_that_objects_handle() {
    use crate::sysobj::Target;
    use crate::world::{object_handle, target_record_word};
    let t = Target {
        id: target_id_of(3),
        kind: 0,
        group: 0,
    };
    assert_eq!(target_record_word(t, 0), object_handle(3));
    assert_eq!(target_record_word(t, 8), object_handle(3));
    let sora = Target {
        id: target_id_of(crate::world::PLAYER_OBJECT),
        kind: 0,
        group: 0,
    };
    assert_eq!(target_record_word(sora, 0), crate::world::PLAYER_HANDLE);
}

#[test]
fn nearest_target_kinds_pick_the_closest_enemy_and_prefer_flagged_ones() {
    let mut world = world_with(&[0., 100., 400.]);
    for k in 1..=3 {
        world.objects[k].as_mut().unwrap().sys.class_flags = class_flag::ENEMY;
    }
    // Object 1 searches. It finds itself first (distance 0) unless it is not
    // an enemy-class object.
    world.objects[1].as_mut().unwrap().sys.class_flags = 0;
    let mut me = Object::bare(0);
    me.sys.class_flags = 0;
    let mut ctx = Ctx {
        world: &mut world,
        me: &mut me,
        env: &NullEnv,
    };
    let near = search(&mut ctx, 3, 1).unwrap();
    assert_eq!((near.id, near.kind, near.group), (OBJECT_TARGET | 2, 3, 0));
    // A flagged far object wins kind 5 and not kind 3.
    ctx.world.objects[3].as_mut().unwrap().sys.class_flags =
        class_flag::ENEMY | class_flag::PRIORITY_TARGET;
    assert_eq!(search(&mut ctx, 3, 1).unwrap().id, OBJECT_TARGET | 2);
    assert_eq!(search(&mut ctx, 5, 1).unwrap().id, OBJECT_TARGET | 3);
    // Dead-marked and gone objects are not targets.
    ctx.world.objects[3].as_mut().unwrap().sys.gone = true;
    ctx.world.objects[2].as_mut().unwrap().sys.state_flags |= state_flag::DEAD_MARKED;
    assert_eq!(search(&mut ctx, 3, 1).unwrap().id, 0);
}

#[test]
fn an_invalid_target_is_searched_again_only_by_an_enemy() {
    for (flags, expected) in [(class_flag::ENEMY, PLAYER_TARGET), (0, OBJECT_TARGET | 2)] {
        let mut world = world_with(&[0., 10.]);
        world.objects[2].as_mut().unwrap().sys.gone = true;
        let o = world.objects[1].as_mut().unwrap();
        o.sys.class_flags = flags;
        o.sys.target = Target {
            id: OBJECT_TARGET | 2,
            ..Target::default()
        };
        let mut me = Object::bare(0);
        let mut f = frame();
        let ai = ai_object(&mut f, 0, 1);
        call(
            &mut world,
            &mut me,
            &mut f,
            btlobj_target,
            &[Slot::address(ai)],
        );
        assert_eq!(world.objects[1].as_ref().unwrap().sys.target.id, expected);
    }
}
