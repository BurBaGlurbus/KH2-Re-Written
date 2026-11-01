//! The natives a mission's own script runs on (table 4 of `msn/<lang>/*.bar`
//! type 3), and the signal any script can post. What each mission call does
//! is the host's: it reads [`crate::world::MissionState::calls`].

use crate::script::{Fault, Memory, Slot};
use crate::world::{Ctx, Event, MissionCall, TrapFn};

type Args = [Slot; 16];

pub fn lookup(table: u16, index: u16) -> Option<TrapFn> {
    Some(match (table, index) {
        (1, 41) => signal_call,
        (4, 3) => complete,
        (4, 11) => failed,
        (4, 16) => dead_boss,
        (4, 22) => exit,
        (4, 54) => lock,
        (4, 55) => is_lock,
        _ => return None,
    })
}

/// 1:41 FUN_14042ec90: FUN_1403aabe0 with the code and its argument.
fn signal_call(ctx: &mut Ctx<'_>, _: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    ctx.world.events.push(Event::Signal {
        code: args[0].as_int(),
        argument: args[1].as_int(),
    });
    Ok(())
}

/// 4:3 FUN_1404334c0.
fn complete(ctx: &mut Ctx<'_>, _: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let call = MissionCall::Complete(args[0].as_int());
    ctx.world.mission.calls.push(call);
    Ok(())
}

/// 4:11 thunk_FUN_1403a3800.
fn failed(ctx: &mut Ctx<'_>, _: &mut Memory<'_>, _: &mut Args) -> Result<(), Fault> {
    ctx.world.mission.calls.push(MissionCall::Failed);
    Ok(())
}

/// 4:16 FUN_1404334d0.
fn dead_boss(ctx: &mut Ctx<'_>, _: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let call = MissionCall::DeadBoss(args[0].as_int());
    ctx.world.mission.calls.push(call);
    Ok(())
}

/// 4:22 FUN_1404334f0.
fn exit(ctx: &mut Ctx<'_>, _: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let call = MissionCall::Exit(args[0].as_int());
    ctx.world.mission.calls.push(call);
    Ok(())
}

/// 4:54 FUN_140433560: DAT_142a10500 bit 0.
fn lock(ctx: &mut Ctx<'_>, _: &mut Memory<'_>, _: &mut Args) -> Result<(), Fault> {
    ctx.world.mission.locked = true;
    Ok(())
}

/// 4:55 FUN_140433570.
fn is_lock(ctx: &mut Ctx<'_>, _: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    args[0] = Slot::int(i32::from(ctx.world.mission.locked));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::env::NullEnv;
    use crate::script::{Instance, Thread};
    use crate::world::{Object, World};

    fn run(world: &mut World, table: u16, index: u16, a: [i32; 2]) -> i32 {
        let mut header = vec![0u8; 44];
        header[..4].copy_from_slice(b"test");
        for at in [16, 20, 24] {
            header[at..at + 4].copy_from_slice(&64u32.to_le_bytes());
        }
        let mut inst = Instance::new(0, &header).unwrap();
        let mut th = Thread::new(&inst, 0, 0, &[]);
        let mut memory = Memory::new(&mut inst, &mut th);
        let mut args = [Slot::default(); 16];
        args[0] = Slot::int(a[0]);
        args[1] = Slot::int(a[1]);
        let mut me = Object::bare(0);
        let mut ctx = Ctx {
            world,
            me: &mut me,
            env: &NullEnv,
        };
        lookup(table, index).unwrap()(&mut ctx, &mut memory, &mut args).unwrap();
        args[0].as_int()
    }

    #[test]
    fn the_lock_answers_and_the_calls_wait_for_the_host() {
        let mut world = World::default();
        assert_eq!(run(&mut world, 4, 55, [0, 0]), 0);
        run(&mut world, 4, 54, [0, 0]);
        assert_eq!(run(&mut world, 4, 55, [0, 0]), 1);
        run(&mut world, 4, 16, [-1, 0]);
        run(&mut world, 4, 11, [0, 0]);
        assert_eq!(
            world.mission.calls,
            [MissionCall::DeadBoss(-1), MissionCall::Failed]
        );
    }

    #[test]
    fn a_script_posts_a_signal_with_its_argument() {
        let mut world = World::default();
        run(&mut world, 1, 41, [4, 1036]);
        assert!(matches!(
            world.events.as_slice(),
            [Event::Signal {
                code: 4,
                argument: 1036
            }]
        ));
    }
}
