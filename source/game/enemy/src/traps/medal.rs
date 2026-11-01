//! The Port Royal medals (the Grim Reaper's fight): prizes of their own kept
//! in the store the fighters share, and the medal counts of objects (+0xd1c)
//! and Sora. The host moves them ([`crate::world::Medals`]).

use crate::script::{Fault, Memory, Slot};
use crate::world::{Ctx, Medal, PLAYER_OBJECT, TrapFn};
use player::enemy_prizes::Orb;

type Args = [Slot; 16];

/// DAT_1405ce188's PRIZE group. [I] From the descriptor's third word, as the
/// ordinary prizes' table at DAT_1405c7390.
pub const MEDAL_KIND: u8 = 11;

pub fn lookup(table: u16, index: u16) -> Option<TrapFn> {
    Some(match (table, index) {
        (1, 265) => set_medal,
        (1, 266) => get_medal,
        (1, 267) => scatter,
        (1, 339) => vacuum,
        (1, 340) => vacuum_range,
        (1, 341) => count,
        _ => return None,
    })
}

fn draw(ctx: &mut Ctx<'_>) -> f32 {
    ctx.world.rng.step() as f32 / 4_294_967_296.
}

fn medal_count(ctx: &mut Ctx<'_>, id: u32) -> Result<i32, Fault> {
    if id == PLAYER_OBJECT {
        return Ok(ctx.world.teamwork.lock().map_or(0, |t| t.medals.player));
    }
    Ok(ctx.object(id)?.sys.medal)
}

fn set_count(ctx: &mut Ctx<'_>, id: u32, n: i32) -> Result<(), Fault> {
    if id == PLAYER_OBJECT {
        if let Ok(mut t) = ctx.world.teamwork.lock() {
            t.medals.player = n;
        }
        return Ok(());
    }
    ctx.object(id)?.sys.medal = n;
    Ok(())
}

fn add_medal(ctx: &mut Ctx<'_>, medal: Medal) {
    if let Ok(mut t) = ctx.world.teamwork.lock() {
        t.medals.prizes.push(medal);
    }
}

/// 1:265 FUN_14042e110: +0xd1c.
fn set_medal(ctx: &mut Ctx<'_>, m: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let id = ctx.sys_of_ai(m, args[0].value)?;
    set_count(ctx, id, args[1].as_int())
}

/// 1:266 FUN_14042e140: +0xd1c.
fn get_medal(ctx: &mut Ctx<'_>, m: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let id = ctx.sys_of_ai(m, args[0].value)?;
    let v = medal_count(ctx, id)?;
    args[0] = Slot::int(v);
    Ok(())
}

/// 1:267 FUN_14042e190 -> FUN_14040b5a0: up to `n` of the object's medals fall
/// from it.
fn scatter(ctx: &mut Ctx<'_>, m: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let id = ctx.sys_of_ai(m, args[0].value)?;
    let have = medal_count(ctx, id)?;
    let n = args[1].as_int().min(have).max(0);
    let at = ctx.object(id)?.sys.position;
    for _ in 0..n {
        let r = [draw(ctx), draw(ctx), draw(ctx)];
        add_medal(
            ctx,
            Medal {
                orb: Orb::spawn(MEDAL_KIND, at, r),
                target: None,
            },
        );
    }
    set_count(ctx, id, have - n)
}

/// 1:339 FUN_1404314c0 -> FUN_14040bd40: up to `n` of `from`'s medals fly to
/// `to`; true when `from` ran out.
fn vacuum(ctx: &mut Ctx<'_>, m: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let to = ctx.sys_of_ai(m, args[0].value)?;
    let from = ctx.sys_of_ai(m, args[1].value)?;
    let n = args[2].as_int();
    let at = ctx.object(from)?.sys.position;
    let mut have = medal_count(ctx, from)?;
    let mut out = false;
    for _ in 0..n.max(0) {
        if have == 0 {
            out = true;
            break;
        }
        let r = [draw(ctx), draw(ctx), draw(ctx)];
        add_medal(
            ctx,
            Medal {
                orb: Orb::spawn(MEDAL_KIND, at, r),
                target: Some(to),
            },
        );
        have -= 1;
    }
    set_count(ctx, from, have)?;
    args[0] = Slot::int(i32::from(out));
    Ok(())
}

/// 1:340 FUN_140431530 -> FUN_14040bde0: the free medals within `range` of `to`
/// horizontally fly to it.
fn vacuum_range(ctx: &mut Ctx<'_>, m: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let to = ctx.sys_of_ai(m, args[0].value)?;
    let range = args[1].as_float();
    let at = ctx.object(to)?.sys.position;
    if let Ok(mut t) = ctx.world.teamwork.lock() {
        for p in t.medals.prizes.iter_mut().filter(|p| p.target.is_none()) {
            let d = [p.orb.position[0] - at[0], p.orb.position[2] - at[2]];
            if d[0].hypot(d[1]) < range {
                p.target = Some(to);
            }
        }
    }
    Ok(())
}

/// 1:341 FUN_140431570 -> FUN_14040b6a0: the medals out in the room.
fn count(ctx: &mut Ctx<'_>, _: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let n = ctx
        .world
        .teamwork
        .lock()
        .map_or(0, |t| t.medals.prizes.len());
    args[0] = Slot::int(n as i32);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::env::NullEnv;
    use crate::script::{Instance, Thread};
    use crate::world::{Object, World, object_handle};

    /// Runs a native with two objects (1, 2) in the list, `a` its arguments.
    fn run(world: &mut World, index: u16, a: [Slot; 3]) -> Slot {
        let mut header = vec![0u8; 44];
        header[..4].copy_from_slice(b"test");
        for at in [16, 20, 24] {
            header[at..at + 4].copy_from_slice(&64u32.to_le_bytes());
        }
        let mut inst = Instance::new(0, &header).unwrap();
        let mut th = Thread::new(&inst, 0, 0, &[]);
        let base = inst.work_base();
        let mut memory = Memory::new(&mut inst, &mut th);
        let mut args = [Slot::default(); 16];
        for (k, slot) in a.into_iter().enumerate() {
            // An object argument is an AI block whose word 1 holds its handle.
            args[k] = if slot.tag == Slot::address(0).tag {
                let at = base + 8 * k as u32;
                memory.write_u32(at + 4, slot.value);
                Slot::address(at)
            } else {
                slot
            };
        }
        let mut me = Object::bare(0);
        let mut ctx = Ctx {
            world,
            me: &mut me,
            env: &NullEnv,
        };
        lookup(1, index).unwrap()(&mut ctx, &mut memory, &mut args).unwrap();
        args[0]
    }

    fn room() -> World {
        let mut world = World::default();
        world.objects = vec![None, None, None];
        for id in [1, 2] {
            let mut o = Object::bare(id);
            o.sys.position = [100. * id as f32, 0., 0.];
            world.objects[id as usize] = Some(Box::new(o));
        }
        world.objects[1].as_mut().unwrap().sys.medal = 5;
        world
    }

    fn object(id: u32) -> Slot {
        Slot::address(object_handle(id))
    }

    #[test]
    fn scatter_gives_no_more_than_the_object_has() {
        let mut world = room();
        run(&mut world, 267, [object(1), Slot::int(3), Slot::int(0)]);
        assert_eq!(world.objects[1].as_ref().unwrap().sys.medal, 2);
        assert_eq!(run(&mut world, 341, [Slot::int(0); 3]).as_int(), 3);
        run(&mut world, 267, [object(1), Slot::int(9), Slot::int(0)]);
        assert_eq!(world.objects[1].as_ref().unwrap().sys.medal, 0);
        assert_eq!(run(&mut world, 341, [Slot::int(0); 3]).as_int(), 5);
    }

    #[test]
    fn vacuum_flies_medals_and_says_when_the_source_ran_out() {
        let mut world = room();
        let out = run(&mut world, 339, [object(2), object(1), Slot::int(2)]);
        assert_eq!(out.as_int(), 0);
        assert_eq!(world.objects[1].as_ref().unwrap().sys.medal, 3);
        let out = run(&mut world, 339, [object(2), object(1), Slot::int(9)]);
        assert_eq!(out.as_int(), 1);
        let team = world.teamwork.lock().unwrap();
        assert_eq!(team.medals.prizes.len(), 5);
        assert!(team.medals.prizes.iter().all(|m| m.target == Some(2)));
    }

    #[test]
    fn vacuum_range_takes_only_free_medals_in_reach() {
        let mut world = room();
        run(&mut world, 267, [object(1), Slot::int(2), Slot::int(0)]);
        {
            let mut team = world.teamwork.lock().unwrap();
            team.medals.prizes[0].orb.position = [1000., 0., 0.];
            team.medals.prizes[1].orb.position = [210., 0., 0.];
        }
        run(&mut world, 340, [object(2), Slot::float(50.), Slot::int(0)]);
        let team = world.teamwork.lock().unwrap();
        assert_eq!(team.medals.prizes[0].target, None);
        assert_eq!(team.medals.prizes[1].target, Some(2));
    }
}
