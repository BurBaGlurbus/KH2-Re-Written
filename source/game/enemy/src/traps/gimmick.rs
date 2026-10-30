//! The natives only boss-class, field gimmick and a few party scripts call:
//! numbers, flags, status bits and small object questions. What needs a host
//! that is not here (missions, belt conveyors, the Pete curtain's drawing) is
//! in [`super::service`]; each answer here that stands in for one says so.

use crate::script::{Fault, Memory, Slot};
use crate::world::{Ctx, TrapFn, status};

type Args = [Slot; 16];

pub fn lookup(table: u16, index: u16) -> Option<TrapFn> {
    Some(match (table, index) {
        (0, 22) => abs,
        (0, 57) => saveflag_set,
        (0, 59) => saveflag_check,
        (0, 69) => menuflag_set,
        (0, 70) => progress_is_second,
        (0, 73) => menuflag_reset,
        (0, 84) => vector_angle,
        (1, 144) => obj_serial,
        (1, 154) => secure_mode_start,
        (1, 178) => path_movement,
        (1, 189) => history_mark,
        (1, 190) => is_history_mark,
        (1, 196) => answer_zero,
        (1, 208) => is_hover,
        (1, 239) => is_unit_active,
        (1, 245) => target_clear_before_player,
        (1, 264) => secure_mode_end,
        (1, 338) => answer_zero,
        (1, 350) => close_pete_curtain,
        (1, 351) => open_pete_curtain,
        (1, 361) => answer_zero,
        (2, 26) => attack_exec_reflect,
        (2, 31) => attack_dup,
        (2, 32) | (2, 33) => answer_zero,
        (2, 93) => damage_is_normal,
        (4, 30) | (4, 55) => answer_zero,
        (6, 61) => obj_is_hook,
        (10, 24) | (10, 38) | (10, 50) => answer_zero,
        (10, 55) => searcher_check_hide_from_friend,
        _ => return None,
    })
}

fn int(args: &mut Args, v: i32) {
    args[0] = Slot::int(v);
}

/// [U] The value comes from something the room does not keep, and nothing
/// the enemies' scripts do hangs on it being more than 0: 1:196 FUN_14042e3d0
/// the entrance (DAT_14071700a), 1:338 FUN_140431450 FUN_14040bb00's prize
/// return, 1:361 FUN_1404312e0 FUN_1403c1be0's sheet level, 2:32 and 2:33 the
/// damage record's attack row (+0xc, +0xe), 4:30 FUN_1403fb750's combo, 4:55
/// FUN_140433570 the mission lock (DAT_142a10500), 10:24 and 10:38 and 10:50
/// a friend's ability, its attack's finish, its sheet's MP (+0x180).
fn answer_zero(_: &mut Ctx<'_>, _: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    int(args, 0);
    Ok(())
}

/// 0:22 FUN_14041ca70.
fn abs(_: &mut Ctx<'_>, _: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let v = args[0].as_int();
    int(args, v.wrapping_abs());
    Ok(())
}

fn set_bit(words: &mut Vec<u32>, bit: u32, on: bool) {
    let w = (bit >> 5) as usize;
    if words.len() <= w {
        words.resize(w + 1, 0);
    }
    if on {
        words[w] |= 1 << (bit & 31);
    } else {
        words[w] &= !(1 << (bit & 31));
    }
}

/// 0:57 FUN_14041e6c0: a bit of DAT_1409abcb0.
fn saveflag_set(ctx: &mut Ctx<'_>, _: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    set_bit(&mut ctx.world.save_flags, args[0].value, true);
    Ok(())
}

/// 0:59 FUN_14041e6e0 -> FUN_140433a00.
fn saveflag_check(ctx: &mut Ctx<'_>, _: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let bit = args[0].value;
    let word = ctx.world.save_flags.get((bit >> 5) as usize).copied();
    int(args, (word.unwrap_or(0) >> (bit & 31) & 1) as i32);
    Ok(())
}

/// 0:69 FUN_14041e850: a bit of DAT_1409abc40. [I] FUN_1403f0ea0, the menu's
/// refresh, has nothing to do here.
fn menuflag_set(ctx: &mut Ctx<'_>, _: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    set_bit(&mut ctx.world.menu_flags, args[0].value, true);
    Ok(())
}

/// 0:73 FUN_14041e860.
fn menuflag_reset(ctx: &mut Ctx<'_>, _: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    set_bit(&mut ctx.world.menu_flags, args[0].value, false);
    Ok(())
}

/// 0:70 FUN_14041db90 -> FUN_14039ac80: DAT_1409ab5c4 bit 11. [I] A first
/// visit: the room has no story state.
fn progress_is_second(_: &mut Ctx<'_>, _: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    int(args, 0);
    Ok(())
}

/// 0:84 FUN_14041d740 -> FUN_1403b7510: the angle between two unit vectors,
/// their NaN lanes zeroed in place.
fn vector_angle(ctx: &mut Ctx<'_>, m: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let read = |at: u32| -> Result<[f32; 3], Fault> {
        let mut v = [0f32; 3];
        for (k, c) in v.iter_mut().enumerate() {
            let a = at + k as u32 * 4;
            let lane = ctx.read_f32(m, a).ok_or(Fault::Memory(a))?;
            *c = if lane.is_nan() { 0. } else { lane };
        }
        Ok(v)
    };
    let (a, b) = (read(args[0].value)?, read(args[1].value)?);
    let dot = a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    args[0] = Slot::float(if dot >= 1. {
        0.
    } else if dot <= -1. {
        std::f32::consts::PI
    } else {
        dot.acos()
    });
    Ok(())
}

/// 1:144 FUN_14042b6c0 -> FUN_1403b5bf0: the spawn record's serial (+0x9f0 ->
/// +0x1e), 0 without one.
fn obj_serial(ctx: &mut Ctx<'_>, m: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let id = ctx.sys_of_ai(m, args[0].value)?;
    let serial = ctx.object(id)?.sys.spawn_serial;
    int(args, i32::from(serial));
    Ok(())
}

/// 1:154 FUN_140430800.
fn secure_mode_start(ctx: &mut Ctx<'_>, _: &mut Memory<'_>, _: &mut Args) -> Result<(), Fault> {
    ctx.world.status |= status::SECURE_MODE;
    Ok(())
}

/// 1:264 FUN_140430810.
fn secure_mode_end(ctx: &mut Ctx<'_>, _: &mut Memory<'_>, _: &mut Args) -> Result<(), Fault> {
    ctx.world.status &= !status::SECURE_MODE;
    Ok(())
}

/// 1:350 FUN_140430930.
fn close_pete_curtain(ctx: &mut Ctx<'_>, _: &mut Memory<'_>, _: &mut Args) -> Result<(), Fault> {
    ctx.world.status |= status::PETE_CURTAIN;
    Ok(())
}

/// 1:351 FUN_140430940.
fn open_pete_curtain(ctx: &mut Ctx<'_>, _: &mut Memory<'_>, _: &mut Args) -> Result<(), Fault> {
    ctx.world.status &= !status::PETE_CURTAIN;
    Ok(())
}

/// 1:178 FUN_14042d930 -> FUN_1403b86a0: with no route (+0x630 null) a zero
/// vector of w 0, in the trap's own cell. [I] A route's movement is not
/// answered.
fn path_movement(ctx: &mut Ctx<'_>, m: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let cell = ctx.world.static_cell(1, 178, 0);
    for k in 0..4 {
        ctx.write_f32(m, cell + k * 4, 0.);
    }
    args[0] = Slot::address(cell);
    Ok(())
}

/// 1:189 FUN_14042cae0 -> FUN_1404196d0: the serial goes on the list at
/// DAT_142ae6690.
fn history_mark(ctx: &mut Ctx<'_>, m: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let id = ctx.sys_of_ai(m, args[0].value)?;
    let serial = ctx.object(id)?.sys.spawn_serial;
    ctx.world.history_marks.insert(serial);
    Ok(())
}

/// 1:190 FUN_14042cb00 -> FUN_1404196b0.
fn is_history_mark(ctx: &mut Ctx<'_>, m: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let id = ctx.sys_of_ai(m, args[0].value)?;
    let serial = ctx.object(id)?.sys.spawn_serial;
    let marked = ctx.world.history_marks.contains(&serial);
    int(args, marked as i32);
    Ok(())
}

/// 1:208 FUN_14042d0a0 -> FUN_1404229d0: physics mode 6.
fn is_hover(ctx: &mut Ctx<'_>, m: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let id = ctx.sys_of_ai(m, args[0].value)?;
    let hover = ctx.object(id)?.sys.steer.is_some_and(|s| s.hover);
    int(args, hover as i32);
    Ok(())
}

/// 1:239 FUN_14042dbb0: 0 only for an object of a unit (+0x9e8) without bit 3
/// of its word. [I] The room keeps no unit records: always 1.
fn is_unit_active(_: &mut Ctx<'_>, _: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    int(args, 1);
    Ok(())
}

/// 1:245 FUN_1403bd290: the battle target table's objects go.
fn target_clear_before_player(
    ctx: &mut Ctx<'_>,
    _: &mut Memory<'_>,
    _: &mut Args,
) -> Result<(), Fault> {
    for t in &mut ctx.world.battle_targets {
        t.object = None;
    }
    Ok(())
}

/// The attack a handle names (`ATTACK_HANDLES + 0x100 * index`).
fn attack_slot(handle: u32) -> Option<usize> {
    let o = handle.checked_sub(crate::world::ATTACK_HANDLES)?;
    (o % 0x100 == 0 && o < 0x0100_0000).then_some((o / 0x100) as usize)
}

/// 2:26 FUN_140432b30: flag bit 6.
fn attack_exec_reflect(
    ctx: &mut Ctx<'_>,
    _: &mut Memory<'_>,
    args: &mut Args,
) -> Result<(), Fault> {
    if let Some(a) = attack_slot(args[0].value)
        .and_then(|k| ctx.world.attacks.get_mut(k))
        .and_then(Option::as_mut)
    {
        a.flags |= 0x40;
    }
    Ok(())
}

/// 2:31 FUN_1404324d0 -> FUN_1404372e0: a new attack with the old one's
/// parameters and another ATKP id. [I] The whole object is copied, where
/// FUN_1404372e0 constructs a fresh one from four of its fields.
fn attack_dup(ctx: &mut Ctx<'_>, _: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let old = attack_slot(args[0].value)
        .and_then(|k| ctx.world.attacks.get(k))
        .and_then(Option::as_ref)
        .cloned();
    let Some(mut new) = old else {
        args[0] = Slot::address(0);
        return Ok(());
    };
    new.param_id = args[1].as_int();
    let k = ctx.world.add_attack(new);
    args[0] = Slot::address(crate::world::ATTACK_HANDLES + 0x100 * k as u32);
    Ok(())
}

/// 2:93 FUN_140432220 -> FUN_1403cee00: the damage record's +0x27 is 0, 1, 3,
/// 4, 6, 7, 15 or 16. [I] Taken to be the attack type byte.
fn damage_is_normal(ctx: &mut Ctx<'_>, _: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let id = crate::world::object_field(args[0].value).map_or(0, |(id, _)| id);
    let kind = ctx.object(id)?.sys.damage_attack_type;
    int(args, matches!(kind, 0 | 1 | 3 | 4 | 6 | 7 | 15 | 16) as i32);
    Ok(())
}

/// 6:61 FUN_14053ede0: +0xd48, the magic thread `obj_hook` started.
fn obj_is_hook(ctx: &mut Ctx<'_>, m: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let id = ctx.sys_of_ai(m, args[0].value)?;
    let hooked = ctx.object(id)?.sys.hook.is_some();
    int(args, hooked as i32);
    Ok(())
}

/// 10:55 FUN_1405371c0 -> FUN_140540310: bit 0 of the searcher's flag halfword
/// (+6). [I] Taken, not yet read by the search.
fn searcher_check_hide_from_friend(
    ctx: &mut Ctx<'_>,
    m: &mut Memory<'_>,
    args: &mut Args,
) -> Result<(), Fault> {
    let at = args[0].value + 4;
    let word = ctx.read_u32(m, at).ok_or(Fault::Memory(at))?;
    let on = u32::from(args[1].value != 0);
    let word = word & !(1 << 16) | on << 16;
    ctx.write_u32(m, at, word)
        .then_some(())
        .ok_or(Fault::Memory(at))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_and_bits_keep_their_words() {
        let mut v = Vec::new();
        set_bit(&mut v, 37, true);
        assert_eq!(v, [0, 1 << 5]);
        set_bit(&mut v, 37, false);
        assert_eq!(v, [0, 0]);
    }

    #[test]
    fn attack_handles_are_a_slot_apart() {
        let base = crate::world::ATTACK_HANDLES;
        assert_eq!(attack_slot(base + 0x300), Some(3));
        assert_eq!(attack_slot(base + 0x304), None);
        assert_eq!(attack_slot(0), None);
    }
}
