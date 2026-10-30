//! Small state traps: setters and getters on fields of the engine object that
//! scripts use directly, and the services recorded as unsupported.

use super::math::{native, native_own, native_own_yaw, read_native, read_sanitized};
use crate::script::{Fault, Memory, Slot, Thread};
use crate::world::{Ctx, Event, Object, SideThread, TrapFn, object_handle};

type Args = [Slot; 16];

pub fn lookup(table: u16, index: u16) -> Option<TrapFn> {
    Some(match (table, index) {
        (1, 38) => set_team,
        (1, 53) => disable_collision,
        (1, 54) => reset_collision,
        (1, 97) => spec,
        (1, 114) => search_by_entry,
        (1, 222) => get_entry_id,
        (0, 8) => vector_dump,
        (0, 78) => vector_inter,
        (1, 36) => bg_hide,
        (1, 37) => bg_show,
        (1, 42) => control_off,
        (1, 43) => control_on,
        (1, 73) => obj_hover,
        (1, 141) => bg_ground_pos,
        (1, 153) => reset_pos,
        (1, 156) => obj_hop,
        (6, 42) => obj_hop_direct,
        (0, 49) => effect_is_alive,
        (1, 0) => sysobj_appear,
        (1, 24) => area_area,
        (1, 48) => message_is_end,
        (1, 76) => motion_unhook,
        (1, 93) => sysobj_party,
        (1, 129) => obj_blow,
        (1, 155) => add_hp,
        (1, 353) => start_mpdrive,
        (1, 166) => target_attack,
        (1, 167) => act_start_pri,
        (1, 191) => lockon_target,
        (1, 214) => obj_hook_stop,
        (1, 221) => can_decide_command,
        (1, 227) => jump_direct,
        (1, 292) => search_by_unit_arg,
        (2, 1) => exec_damage_blow,
        (2, 2) => exec_damage_small,
        (2, 3) => exec_damage_hitback,
        (2, 6) => is_damage_motion,
        (2, 7) => damage_reaction,
        (2, 85) => damage_element,
        (2, 86) => damage_get_hitback,
        (2, 87) => exec_damage_large,
        (2, 91) => damage_reaction_type,
        (2, 92) => damage_is_finish,
        (6, 2) => player_get_form,
        (6, 53) => bghit_is_hit,
        (6, 55) => bghit_get_kind,
        (1, 1) => set_rot,
        (1, 21) => effect_start,
        (1, 22) => effect_start_pos,
        (1, 52) => enable_collision,
        (1, 49) => enable_reaction_command,
        (1, 50) => disable_reaction_command,
        (1, 51) => reset_reaction_command,
        (1, 88) => obj_target_pos,
        (1, 130) => obj_cmp,
        (1, 149) => effect_start_bind_other,
        (1, 160) => target_set_obj,
        (1, 170) => unit_group,
        (1, 225) => is_star,
        (2, 15) => damage_exec_reaction,
        (2, 64) => damage_orig_reaction,
        (6, 9) => target_dist,
        (6, 51) => bghit_check_line,
        (6, 54) => bghit_cross_pos,
        (1, 59) => is_fly,
        (1, 99) => float_height,
        (1, 100) => jump_height_to_uptime,
        (1, 107) => motion_speed,
        (1, 112) => obj_hook,
        (1, 140) => target_is_exist,
        (1, 179) => set_fall_motion,
        (1, 180) => set_land_motion,
        (1, 209) => obj_dead,
        (2, 45) => damage_attack_param_id,
        (1, 75) => motion_hook,
        (1, 77) => motion_is_hook,
        (1, 101) => motion_is_capture,
        (1, 143) => search_by_serial,
        (1, 210) => search_by_part,
        (1, 145) => touch_zone,
        (1, 148) => set_pos,
        (1, 213) => obj_part,
        (1, 218) => set_unit_arg,
        (1, 250) => set_lockon_ratio,
        (1, 262) => voice,
        (5, 17) => check_rtn_flag,
        (5, 27) => set_rtn_flag,
        (5, 33) => set_rtn_time,
        (2, 20) => magic_start_thread,
        (2, 21) => teamwork_alloc,
        (2, 76) => set_karma_limit,
        _ => return None,
    })
}

/// 1:38 FUN_14042bd30: +0x4dc.
fn set_team(ctx: &mut Ctx<'_>, m: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let id = ctx.sys_of_ai(m, args[0].value)?;
    ctx.object(id)?.sys.team = args[1].as_int();
    Ok(())
}

/// 1:53 FUN_14042ede0 -> FUN_1403f1f80.
fn disable_collision(ctx: &mut Ctx<'_>, m: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let id = ctx.sys_of_ai(m, args[0].value)?;
    ctx.object(id)?.sys.disable_collision(args[1].value);
    Ok(())
}

/// 1:54 FUN_14042ee10 -> FUN_1403f2000.
fn reset_collision(ctx: &mut Ctx<'_>, m: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let id = ctx.sys_of_ai(m, args[0].value)?;
    ctx.object(id)?.sys.reset_collision(args[1].value);
    Ok(())
}

/// 1:97 FUN_14042faa0: the object's spec block (+0x128). Installed scripts
/// only write its word 12 (+0x134, the turn rate).
fn spec(ctx: &mut Ctx<'_>, m: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let id = ctx.sys_of_ai(m, args[0].value)?;
    args[0] = Slot::address(object_handle(id) + 0x128);
    Ok(())
}

/// Note a search in the trace (development aid) and bind its result.
fn finish_search(
    ctx: &mut Ctx<'_>,
    m: &mut Memory<'_>,
    ai: u32,
    kind: &'static str,
    value: i32,
    index: i32,
    found: Option<crate::world::ObjId>,
) -> Result<(), Fault> {
    if ctx.world.trace.is_some() && ctx.world.searches.len() < 256 {
        ctx.world.searches.push(crate::world::SearchRecord {
            kind,
            value,
            index,
            found: found.is_some(),
        });
    }
    ctx.bind_ai(m, ai, found)
}

/// 1:114 FUN_14042ddb0 -> FUN_1403b4110: bind the AI object to the engine
/// object whose entry is the given object id (0x236 is Sora). A miss binds
/// nothing (+4 becomes 0).
fn search_by_entry(ctx: &mut Ctx<'_>, m: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let entry = args[1].value;
    let found = if entry == crate::world::PLAYER_ENTRY {
        Some(crate::world::PLAYER_OBJECT)
    } else {
        ctx.search(|o| o.sys.entry_id == entry)
    };
    finish_search(ctx, m, args[0].value, "entry", entry as i32, 0, found)
}

/// 1:143 FUN_14042ddf0 -> FUN_1403b41e0: the object whose spawn record serial
/// (the ushort at +0x1e) equals the argument, 0x70 when the argument is 0.
/// Objects without a spawn record read as serial 0.
fn search_by_serial(ctx: &mut Ctx<'_>, m: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let serial = if args[1].value == 0 {
        0x70
    } else {
        args[1].value
    };
    let found = ctx.search(|o| u32::from(o.sys.spawn_serial) == serial);
    finish_search(ctx, m, args[0].value, "serial", serial as i32, 0, found)
}

/// 1:210 FUN_14042de30 -> FUN_1403b4180: the object whose entry part id
/// (+0x4c) equals the argument.
fn search_by_part(ctx: &mut Ctx<'_>, m: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let part = args[1].value;
    let found = ctx.search(|o| u32::from(o.sys.entry_part) == part);
    finish_search(ctx, m, args[0].value, "part", part as i32, 0, found)
}

/// 1:292 FUN_14042de70: the object whose unit argument `index` (+0x9f4 + 4 *
/// index) equals the value. The executable does not bound `index`; a script
/// index past the stored arguments reads as no match.
fn search_by_unit_arg(ctx: &mut Ctx<'_>, m: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let (index, value) = (args[1].as_int(), args[2].as_int());
    let found = ctx.search(|o| {
        usize::try_from(index)
            .ok()
            .and_then(|i| o.sys.spawn_args.get(i))
            .is_some_and(|a| *a == value)
    });
    finish_search(ctx, m, args[0].value, "unit_arg", value, index, found)
}

/// 1:222 FUN_14042bb10 -> FUN_1403b5770: the first word of the object entry.
fn get_entry_id(ctx: &mut Ctx<'_>, m: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let id = ctx.sys_of_ai(m, args[0].value)?;
    let entry = ctx.object(id)?.sys.entry_id;
    args[0] = Slot::int(entry as i32);
    Ok(())
}

/// 2:76 FUN_140431e30: a raw 32-bit store of the script's float into
/// +0xd4c.
fn set_karma_limit(ctx: &mut Ctx<'_>, m: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let id = ctx.sys_of_ai(m, args[0].value)?;
    ctx.object(id)?.sys.karma_limit = f32::from_bits(args[1].value);
    Ok(())
}

/// 2:20 FUN_140432cd0 -> FUN_1403c5ec0(DAT_142a25048, entry, args + 8, 1):
/// a detached thread of the running program at word `args[0]` with one
/// argument (the second slot), sharing its work area; the magic task
/// steps it every frame until its code returns.
fn magic_start_thread(ctx: &mut Ctx<'_>, _: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let handle = ctx.world.next_magic_handle();
    let me = &mut *ctx.me;
    let thread = Thread::with_sizes(
        me.script.stack_bytes,
        me.script.temp_bytes,
        side_slot(handle),
        args[0].as_int(),
        &[args[1]],
    );
    me.threads.push(SideThread {
        handle,
        thread,
        hooked: None,
    });
    args[0] = Slot::address(handle);
    Ok(())
}

/// The frame slot of a side thread: the twelve the child threads use.
fn side_slot(handle: u32) -> u32 {
    3 + (handle / 16) % 12
}

/// 2:21 FUN_140432d70 -> FUN_1403f6df0: the zeroed block shared under a key.
/// A second request for the key sets the block's first byte.
fn teamwork_alloc(ctx: &mut Ctx<'_>, m: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let (key, size) = (args[0].as_int(), args[1].as_int().max(0) as u32);
    let (handle, new) = ctx.world.teamwork_block(key, size + 1);
    if !new {
        let first = ctx.read_u32(m, handle).unwrap_or(0);
        ctx.write_u32(m, handle, first & !0xff | 1);
    }
    args[0] = Slot::address(handle);
    Ok(())
}

/// 1:59 FUN_14042ef50.
fn is_fly(ctx: &mut Ctx<'_>, m: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let id = ctx.sys_of_ai(m, args[0].value)?;
    let fly = ctx.object(id)?.sys.is_fly();
    args[0] = Slot::int(fly as i32);
    Ok(())
}

/// 1:75 FUN_14042f460 -> FUN_1403c7c10: switch to the hook table `n` of the
/// motion set when the model has one.
fn motion_hook(ctx: &mut Ctx<'_>, m: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let id = ctx.sys_of_ai(m, args[0].value)?;
    let hook = args[1].as_int();
    if ctx.env.motion_hook(id, hook) {
        ctx.object(id)?.sys.set_motion_hook(Some(hook));
    }
    Ok(())
}

/// 1:76 FUN_14042f490 -> FUN_1403c8f10: back to the plain motion set.
fn motion_unhook(ctx: &mut Ctx<'_>, m: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let id = ctx.sys_of_ai(m, args[0].value)?;
    ctx.object(id)?.sys.set_motion_hook(None);
    Ok(())
}

/// 1:77 FUN_14042f4c0: a hook table is in use (+0x160).
fn motion_is_hook(ctx: &mut Ctx<'_>, m: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let id = ctx.sys_of_ai(m, args[0].value)?;
    let hooked = ctx.object(id)?.sys.motion_hook.is_some();
    args[0] = Slot::int(hooked as i32);
    Ok(())
}

/// 1:101 FUN_14042d360: +0x18c bit 14 (a captured motion plays).
fn motion_is_capture(ctx: &mut Ctx<'_>, m: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let id = ctx.sys_of_ai(m, args[0].value)?;
    let bit = ctx.object(id)?.sys.motion.flags.word >> 14 & 1;
    args[0] = Slot::int(bit as i32);
    Ok(())
}

/// 1:145 FUN_14042b6f0: +0xcf4.
fn touch_zone(ctx: &mut Ctx<'_>, m: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let id = ctx.sys_of_ai(m, args[0].value)?;
    let v = ctx.object(id)?.sys.rtn_time[7];
    args[0] = Slot::int(v as i32);
    Ok(())
}

/// 1:148 FUN_14042b800 -> FUN_1403b64d0: place the object (w = 1).
fn set_pos(ctx: &mut Ctx<'_>, m: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let v = read_sanitized(ctx, m, args[1].value)?;
    let id = ctx.sys_of_ai(m, args[0].value)?;
    let sys = &mut ctx.object(id)?.sys;
    let v = native_own(sys.camera_space, v);
    sys.position = [v[0], v[1], v[2]];
    Ok(())
}

/// 1:213 FUN_14042bae0 -> FUN_1403b5b20: the entry's part id (+0x4c).
fn obj_part(ctx: &mut Ctx<'_>, m: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let id = ctx.sys_of_ai(m, args[0].value)?;
    let part = ctx.object(id)?.sys.entry_part;
    args[0] = Slot::int(i32::from(part));
    Ok(())
}

/// 1:218 FUN_14042db40 -> FUN_1403b65b0: spawn argument `index` (+0x9f4).
fn set_unit_arg(ctx: &mut Ctx<'_>, m: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let id = ctx.sys_of_ai(m, args[0].value)?;
    let (index, value) = (args[1].as_int(), args[2].as_int());
    if let Some(slot) = usize::try_from(index)
        .ok()
        .and_then(|i| ctx.object(id).ok()?.sys.spawn_args.get_mut(i))
    {
        *slot = value;
    }
    Ok(())
}

/// 1:250 FUN_1404308d0: the lock-on ratio.
fn set_lockon_ratio(ctx: &mut Ctx<'_>, _: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    ctx.world.lockon_ratio = args[0].as_float();
    Ok(())
}

/// 1:262 FUN_14042bde0: the object's voice (virtual call 0xa0 on its class).
fn voice(ctx: &mut Ctx<'_>, m: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let object = ctx.sys_of_ai(m, args[0].value)?;
    ctx.world.events.push(Event::Voice {
        object,
        id: args[1].as_int(),
        arg: args[2].as_int(),
    });
    Ok(())
}

/// 5:17 FUN_140557ed0 -> FUN_140417630.
fn check_rtn_flag(ctx: &mut Ctx<'_>, m: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let id = ctx.sys_of_ai(m, args[0].value)?;
    let set = ctx.object(id)?.sys.rtn_flag(args[1].value);
    args[0] = Slot::int(set as i32);
    Ok(())
}

/// 5:27 FUN_140557f10 -> FUN_140418800.
fn set_rtn_flag(ctx: &mut Ctx<'_>, m: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let id = ctx.sys_of_ai(m, args[0].value)?;
    ctx.object(id)?.sys.set_rtn_flag(args[1].value);
    Ok(())
}

/// 5:33 FUN_140557f40 -> FUN_140418820: copy nine words of time parameters.
fn set_rtn_time(ctx: &mut Ctx<'_>, m: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let id = ctx.sys_of_ai(m, args[0].value)?;
    let mut words = [0u32; 9];
    for (k, w) in words.iter_mut().enumerate() {
        *w = ctx
            .read_u32(m, args[1].value + 4 * k as u32)
            .ok_or(Fault::Memory(args[1].value))?;
    }
    ctx.object(id)?.sys.rtn_time = words;
    Ok(())
}

/// 1:99 FUN_14042fb50 -> FUN_1403b87e0: minus the height of the object above
/// the ground at its position (0 on it).
fn float_height(ctx: &mut Ctx<'_>, m: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let id = ctx.sys_of_ai(m, args[0].value)?;
    let p = ctx.object(id)?.sys.position;
    let floor = ctx.env.floor(p[0], p[2], p[1]).unwrap_or(p[1]);
    let above = (p[1] - floor).max(0.);
    args[0] = Slot::float(-above);
    Ok(())
}

/// 1:100 FUN_14042fbc0 -> FUN_1403b73f0: the time a jump of this apex height
/// takes to reach it, sqrt(|2h / g|).
fn jump_height_to_uptime(
    _: &mut Ctx<'_>,
    _: &mut Memory<'_>,
    args: &mut Args,
) -> Result<(), Fault> {
    let h = args[0].as_float();
    args[0] = Slot::float(((h + h) / player::actor_physics::G).abs().sqrt());
    Ok(())
}

/// 1:107 FUN_14042fce0: +0x2a8.
fn motion_speed(ctx: &mut Ctx<'_>, m: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let id = ctx.sys_of_ai(m, args[0].value)?;
    ctx.object(id)?.sys.motion_speed = args[1].as_float();
    Ok(())
}

/// 1:179 and 1:180 FUN_14042ce00 / FUN_14042ce30: the fall and land motions
/// of the current jump (+0x118, +0x11c).
fn set_fall_motion(ctx: &mut Ctx<'_>, m: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let id = ctx.sys_of_ai(m, args[0].value)?;
    ctx.object(id)?.sys.jump_motions.fall = args[1].as_int();
    Ok(())
}

fn set_land_motion(ctx: &mut Ctx<'_>, m: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let id = ctx.sys_of_ai(m, args[0].value)?;
    ctx.object(id)?.sys.jump_motions.land = args[1].as_int();
    Ok(())
}

/// 1:112 FUN_14042ca20: start magic thread `n` bound to the object (and to
/// a second object when one is named) as its hook; the thread the object
/// held before is stopped. The host runs the thread.
fn obj_hook(ctx: &mut Ctx<'_>, m: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let id = ctx.sys_of_ai(m, args[0].value)?;
    // FUN_14042ca20 passes the second argument only when it names
    // something (a null word adds none).
    let other = if args[2].value == 0 {
        None
    } else {
        super::service::object_of(ctx, m, args[2].value)
    };
    let handle = ctx.world.next_magic_handle();
    let old = ctx.object(id)?.sys.hook.replace(handle);
    if let Some(old) = old {
        ctx.world.events.push(Event::MagicStop { handle: old });
    }
    if ctx.world.limit.is_some() {
        // A limit scene runs its participants' action threads itself.
        ctx.world.events.push(Event::MagicStart {
            object: id,
            id: args[1].as_int(),
            arguments: [0; 4],
            handle,
            hook: true,
            other,
        });
        return Ok(());
    }
    let mut thread_args = vec![Slot::address(object_handle(id))];
    if let Some(o) = other {
        thread_args.push(Slot::address(object_handle(o)));
    }
    let me = &mut *ctx.me;
    let thread = Thread::with_sizes(
        me.script.stack_bytes,
        me.script.temp_bytes,
        side_slot(handle),
        args[1].as_int(),
        &thread_args,
    );
    me.threads.push(SideThread {
        handle,
        thread,
        hooked: Some(id),
    });
    Ok(())
}

/// 1:214 FUN_14042cab0: stop the hook thread.
fn obj_hook_stop(ctx: &mut Ctx<'_>, m: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let id = ctx.sys_of_ai(m, args[0].value)?;
    if let Some(old) = ctx.object(id)?.sys.hook.take() {
        ctx.world.events.push(Event::MagicStop { handle: old });
    }
    Ok(())
}

/// 1:140 FUN_140430440 -> FUN_1403bde30: the target record names something
/// that still exists.
fn target_is_exist(ctx: &mut Ctx<'_>, m: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let alive = super::target::record_exists(ctx, m, args[0].value);
    args[0] = Slot::int(alive as i32);
    Ok(())
}

/// 1:209 FUN_14042dcf0 -> FUN_1403d3f10: the object is finished and removed.
fn obj_dead(ctx: &mut Ctx<'_>, m: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let id = ctx.sys_of_ai(m, args[0].value)?;
    ctx.object(id)?.sys.gone = true;
    ctx.world.events.push(Event::ObjectDead { object: id });
    Ok(())
}

/// 2:45 FUN_1404320a0: the attack parameter id of the hit being handled.
fn damage_attack_param_id(
    ctx: &mut Ctx<'_>,
    _: &mut Memory<'_>,
    args: &mut Args,
) -> Result<(), Fault> {
    let handle = args[0].value;
    let id = crate::world::object_field(handle).map_or(0, |(id, _)| id);
    let v = ctx.object(id).map_or(0, |o| o.sys.damage_param_id);
    args[0] = Slot::int(v as i32);
    Ok(())
}

/// 1:0 FUN_14042b130 -> FUN_1403df930: `sysobj_appear`, make an object of the
/// entry appear (at the routine's default position; scripts place it
/// afterwards). The host's [`crate::world::Spawner`] builds it; without one,
/// or when it declines, the script gets a null object (the executable returns
/// 0 for an entry it cannot make).
fn sysobj_appear(ctx: &mut Ctx<'_>, _: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let entry = args[0].value & 0x0fff_ffff;
    let me = ctx.me.id;
    // The maker stays in the engine's list while the new object's OBJ_INIT
    // runs (Chicken Little's fireworks search for him by entry).
    let maker = std::mem::replace(ctx.me, Object::bare(me));
    ctx.world.held.push(Box::new(maker));
    let id = ctx.world.appear(ctx.env, me, entry);
    if let Some(k) = ctx.world.held.iter().rposition(|o| o.id == me) {
        *ctx.me = *ctx.world.held.remove(k);
    }
    if id.is_none() {
        ctx.world.events.push(Event::Unsupported("sysobj_appear"));
    }
    args[0] = Slot::address(id.map_or(0, object_handle));
    Ok(())
}

/// 1:1 FUN_14042bf60 -> FUN_1403b6560: set the heading; the wished heading
/// follows it, so no turn is pending.
fn set_rot(ctx: &mut Ctx<'_>, m: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let id = ctx.sys_of_ai(m, args[0].value)?;
    let sys = &mut ctx.object(id)?.sys;
    let yaw = native_own_yaw(sys.camera_space, args[1].as_float());
    sys.yaw = yaw;
    sys.wish_yaw = None;
    Ok(())
}

/// 1:49 FUN_14042ecf0 -> FUN_14040a590: the script enables command
/// `args[1]` for collision group `args[2]` (-1 any) of the object (+0x540).
fn enable_reaction_command(
    ctx: &mut Ctx<'_>,
    m: &mut Memory<'_>,
    args: &mut Args,
) -> Result<(), Fault> {
    let id = ctx.sys_of_ai(m, args[0].value)?;
    let (command, group) = (args[1].as_int(), args[2].as_int());
    let g = &mut ctx.object(id)?.sys.reaction_groups;
    crate::reaction::set(&mut g.script, command, group, true);
    Ok(())
}

/// 1:50 FUN_14042ed30 -> FUN_14040a500: the script disables it.
fn disable_reaction_command(
    ctx: &mut Ctx<'_>,
    m: &mut Memory<'_>,
    args: &mut Args,
) -> Result<(), Fault> {
    let id = ctx.sys_of_ai(m, args[0].value)?;
    let (command, group) = (args[1].as_int(), args[2].as_int());
    let g = &mut ctx.object(id)?.sys.reaction_groups;
    crate::reaction::set(&mut g.script, command, group, false);
    Ok(())
}

/// 1:51 FUN_14042ed70 -> FUN_14040a6e0: the script's entry is dropped.
fn reset_reaction_command(
    ctx: &mut Ctx<'_>,
    m: &mut Memory<'_>,
    args: &mut Args,
) -> Result<(), Fault> {
    let id = ctx.sys_of_ai(m, args[0].value)?;
    let (command, group) = (args[1].as_int(), args[2].as_int());
    let g = &mut ctx.object(id)?.sys.reaction_groups;
    crate::reaction::reset(&mut g.script, command, group);
    Ok(())
}

/// 1:52 FUN_14042edb0 -> FUN_1403f1fa0.
fn enable_collision(ctx: &mut Ctx<'_>, m: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let id = ctx.sys_of_ai(m, args[0].value)?;
    ctx.object(id)?.sys.enable_collision(args[1].value);
    Ok(())
}

fn start_effect(
    ctx: &mut Ctx<'_>,
    object: u32,
    effect: i32,
    arguments: [i32; 2],
    position: Option<[f32; 3]>,
    bound: bool,
    other: Option<u32>,
) -> u32 {
    let handle = ctx.world.next_effect_handle();
    ctx.world.events.push(Event::EffectStart {
        object,
        effect,
        arguments,
        handle,
        position,
        bound,
        other,
    });
    handle
}

/// 1:21 FUN_14042d440 -> FUN_1403b6630: start an effect at the object. [U]
/// The effect system is the host's; the request is an event.
fn effect_start(ctx: &mut Ctx<'_>, m: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let id = ctx.sys_of_ai(m, args[0].value)?;
    let (e, a, b) = (args[1].as_int(), args[2].as_int(), args[3].as_int());
    let handle = start_effect(ctx, id, e, [a, b], None, false, None);
    args[0] = Slot::address(handle);
    Ok(())
}

/// 1:22 FUN_14042d4b0 -> FUN_1403b66c0: start an effect at a position.
fn effect_start_pos(ctx: &mut Ctx<'_>, m: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let id = ctx.sys_of_ai(m, args[0].value)?;
    let v = read_native(ctx, m, args[2].value)?;
    let (e, a, b) = (args[1].as_int(), args[3].as_int(), args[4].as_int());
    let handle = start_effect(ctx, id, e, [a, b], Some([v[0], v[1], v[2]]), false, None);
    args[0] = Slot::address(handle);
    Ok(())
}

/// 1:149 FUN_14042d5c0 -> FUN_1403b6670: an effect bound to another object.
fn effect_start_bind_other(
    ctx: &mut Ctx<'_>,
    m: &mut Memory<'_>,
    args: &mut Args,
) -> Result<(), Fault> {
    let id = ctx.sys_of_ai(m, args[0].value)?;
    let other = ctx.sys_of_ai(m, args[2].value).ok();
    let (e, a, b) = (args[1].as_int(), args[3].as_int(), args[4].as_int());
    let handle = start_effect(ctx, id, e, [a, b], None, true, other);
    args[0] = Slot::address(handle);
    Ok(())
}

/// 1:88 FUN_14042f880 -> FUN_1403b5cb0: the position of the object's own
/// target.
fn obj_target_pos(ctx: &mut Ctx<'_>, m: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let id = ctx.sys_of_ai(m, args[0].value)?;
    let t = ctx.object(id)?.sys.target;
    let p = super::target::target_point(ctx, t).unwrap_or([0.; 3]);
    let cell = ctx.world.static_cell(1, 88, 0);
    for (k, lane) in native([p[0], p[1], p[2], 1.0]).iter().enumerate() {
        ctx.write_f32(m, cell + 4 * k as u32, *lane);
    }
    args[0] = Slot::address(cell);
    Ok(())
}

/// 1:130 FUN_14042b5d0: the difference of two AI objects' engine objects
/// (0 when they are the same, or either is missing).
fn obj_cmp(ctx: &mut Ctx<'_>, m: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let (a, b) = (args[0].value, args[1].value);
    let diff = if a != 0 && b != 0 {
        let x = ctx.read_u32(m, a + 4).ok_or(Fault::Memory(a))?;
        let y = ctx.read_u32(m, b + 4).ok_or(Fault::Memory(b))?;
        x.wrapping_sub(y) as i32
    } else {
        0
    };
    args[0] = Slot::int(diff);
    Ok(())
}

/// 1:160 FUN_140430470 -> FUN_1403bee60: point a target record at an object.
fn target_set_obj(ctx: &mut Ctx<'_>, m: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let handle = args[0].value;
    // A null object empties the record (FUN_1403bee60 with nothing).
    if ctx.read_u32(m, args[1].value + 4) == Some(0) {
        return super::target::set_record(ctx, m, handle, crate::sysobj::Target::default());
    }
    let t = match ctx.sys_ref_ai(m, args[1].value)? {
        crate::world::SysRef::Player => crate::sysobj::Target {
            id: super::target::PLAYER_TARGET,
            kind: -1,
            group: 0,
        },
        crate::world::SysRef::Object(id) => crate::sysobj::Target {
            id: super::target::OBJECT_TARGET | id,
            kind: -1,
            group: 0,
        },
    };
    super::target::set_record(ctx, m, handle, t)
}

/// 1:170 FUN_14042dac0: the entry's unit group.
fn unit_group(ctx: &mut Ctx<'_>, m: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let id = ctx.sys_of_ai(m, args[0].value)?;
    let g = ctx.object(id)?.sys.entry_group;
    args[0] = Slot::int(i32::from(g));
    Ok(())
}

/// 1:225 FUN_14042bcb0: vt+0xb8 of a player-type object, "cannot be
/// damaged" (FUN_1403a8ec0 -> FUN_1403d65a0 for Sora: his invulnerable
/// ranges and timers, which the host knows); 0 for any other object.
fn is_star(ctx: &mut Ctx<'_>, m: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let star = match ctx.sys_ref_ai(m, args[0].value) {
        Ok(crate::world::SysRef::Player) => ctx.env.player_invulnerable(),
        _ => false,
    };
    args[0] = Slot::int(star as i32);
    Ok(())
}

/// 2:15 FUN_140432050: the damage record's "reaction executed" bit.
fn damage_exec_reaction(
    ctx: &mut Ctx<'_>,
    _: &mut Memory<'_>,
    args: &mut Args,
) -> Result<(), Fault> {
    let id = crate::world::object_field(args[0].value).map_or(0, |(id, _)| id);
    ctx.object(id)?.sys.damage_exec = true;
    Ok(())
}

/// 2:64 FUN_140431f40: the damage record's original reaction (+0x27).
fn damage_orig_reaction(
    ctx: &mut Ctx<'_>,
    _: &mut Memory<'_>,
    args: &mut Args,
) -> Result<(), Fault> {
    let id = crate::world::object_field(args[0].value).map_or(0, |(id, _)| id);
    let r = ctx.object(id)?.sys.damage_orig_reaction;
    args[0] = Slot::int(r as i32);
    Ok(())
}

/// 6:9 FUN_14053e3e0 -> FUN_1403bdd90: the distance from a target record's
/// position to a point.
fn target_dist(ctx: &mut Ctx<'_>, m: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let v = read_native(ctx, m, args[1].value)?;
    let p = super::target::record_point(ctx, m, args[0].value).unwrap_or([0.; 3]);
    let d = [p[0] - v[0], p[1] - v[1], p[2] - v[2]];
    args[0] = Slot::float((d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt());
    Ok(())
}

/// FUN_14040c2b0's hit record (the script's buffer): the crossing point
/// at +0 (vec4), the polygon at +0x14 (-1 for none), the dynamic collider
/// at +0x18 (0 for the map).
const RECORD_POLYGON: u32 = 0x14;
const RECORD_COLLIDER: u32 = 0x18;

/// 6:51 FUN_14053eb70 -> FUN_14040c2b0: `(record, a, b, mask)`. Whether the
/// map of collision class `mask` crosses the segment from `a` to `b`; the
/// record takes the crossing and its polygon.
fn bghit_check_line(ctx: &mut Ctx<'_>, m: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let record = args[0].value;
    let a = read_native(ctx, m, args[1].value)?;
    let b = read_native(ctx, m, args[2].value)?;
    let mask = args[3].value;
    ctx.write_u32(m, record + RECORD_POLYGON, u32::MAX);
    ctx.write_u32(m, record + RECORD_COLLIDER, 0);
    ctx.write_u32(m, record + RECORD_COLLIDER + 4, 0);
    let hit = ctx
        .env
        .line_hit_polygon([a[0], a[1], a[2]], [b[0], b[1], b[2]], mask);
    if let Some((at, polygon)) = hit {
        for (k, lane) in native([at[0], at[1], at[2], 1.0]).iter().enumerate() {
            ctx.write_f32(m, record + 4 * k as u32, *lane);
        }
        // [I] A host that cannot name the polygon (the generated arena, the
        // attack ray) records polygon 0, so the record still reads as a hit.
        ctx.write_u32(m, record + RECORD_POLYGON, polygon.max(0) as u32);
    }
    args[0] = Slot::int(hit.is_some() as i32);
    Ok(())
}

/// The record's polygon (-1 for none).
pub(super) fn record_polygon(ctx: &Ctx<'_>, m: &Memory<'_>, record: u32) -> i32 {
    ctx.read_u32(m, record + RECORD_POLYGON)
        .map_or(-1, |p| p as i32)
}

/// 6:53 FUN_14053ecd0: the record names a polygon.
fn bghit_is_hit(ctx: &mut Ctx<'_>, m: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let hit = record_polygon(ctx, m, args[0].value) != -1;
    args[0] = Slot::int(hit as i32);
    Ok(())
}

/// 6:55 FUN_14053ed40 -> FUN_14040c380: the surface nibble of the
/// record's polygon (FUN_140170910).
fn bghit_get_kind(ctx: &mut Ctx<'_>, m: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let polygon = record_polygon(ctx, m, args[0].value);
    let kind = ctx.env.map_surface(polygon).unwrap_or(0);
    args[0] = Slot::int(kind as i32);
    Ok(())
}

/// 6:54 FUN_14053ed00: the record's crossing point, copied to the static
/// vector 6:52 also uses (DAT_1407883d8).
fn bghit_cross_pos(ctx: &mut Ctx<'_>, m: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let record = args[0].value;
    let cell = ctx.world.static_cell(6, 54, 0);
    for k in 0..4 {
        let lane = ctx.read_f32(m, record + 4 * k).unwrap_or(0.);
        ctx.write_f32(m, cell + 4 * k, lane);
    }
    args[0] = Slot::address(cell);
    Ok(())
}

macro_rules! recorded_traps {
    ($($name:ident = $label:literal;)*) => {$(
        /// A service this build does not provide. [U] The call is recorded and
        /// does nothing else.
        fn $name(ctx: &mut Ctx<'_>, _: &mut Memory<'_>, _: &mut Args) -> Result<(), Fault> {
            ctx.world.events.push(Event::Unsupported($label));
            Ok(())
        }
    )*};
}

recorded_traps! {}

macro_rules! constant_int_traps {
    ($($name:ident = $value:literal, $label:literal;)*) => {$(
        /// A query whose source this build does not have. [U] Returns the
        /// stated constant.
        fn $name(_: &mut Ctx<'_>, _: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
            let _ = $label;
            args[0] = Slot::int($value);
            Ok(())
        }
    )*};
}

constant_int_traps! {
    message_is_end = 1, "method_message_is_end";
}

/// 1:155 FUN_14042dc60: the object's HP change (vt+0xe8: FUN_1403d2eb0, or
/// Sora's FUN_1403a85f0) by `args[1]` on pool `args[2]` (`args[3]` the heal
/// voice flag); the new HP comes back. Sora's change is the host's: it is
/// handed on and the clamped value returned.
fn add_hp(ctx: &mut Ctx<'_>, m: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let (delta, pool) = (args[1].as_int(), args[2].as_int());
    let index = usize::try_from(pool).unwrap_or(usize::MAX);
    let hp = match ctx.sys_ref_ai(m, args[0].value)? {
        crate::world::SysRef::Player => {
            let (hp, max) = ctx.env.player_hp().unwrap_or((0, 0));
            let mut words = [0u32; 8];
            words[..3].copy_from_slice(&[args[1].value, args[2].value, args[3].value]);
            ctx.world.events.push(Event::Service {
                name: "obj_add_hp",
                object: Some(crate::world::PLAYER_OBJECT),
                other: None,
                words,
                vector: None,
            });
            (hp + delta).clamp(0, max)
        }
        crate::world::SysRef::Object(id) if id == ctx.me.id => {
            let Ctx { world, me, env } = ctx;
            me.change_hp_queued(world, *env, delta, index);
            me.sys.pool(index).map_or(0, |p| p.hp)
        }
        crate::world::SysRef::Object(id) => {
            // [U] Another object's death hook is not run from here.
            let sys = &mut ctx.object(id)?.sys;
            let p = sys.pool(index).unwrap_or_default();
            sys.set_pool_hp(index, p.hp.saturating_add(delta));
            sys.pool(index).map_or(0, |p| p.hp)
        }
    };
    args[0] = Slot::int(hp);
    Ok(())
}

/// 1:353 FUN_14042dd70 -> FUN_1403d4a20 -> FUN_1403c20a0: the object's MP
/// drops to 0 and an MP charge of `args[1]` times its PRTY +0x1c starts
/// (sheet +0x180 = 0, +0x1bc = +0x1c0 = the length). Only Sora has MP;
/// the host carries it out.
fn start_mpdrive(ctx: &mut Ctx<'_>, m: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let object = super::service::object_of(ctx, m, args[0].value);
    let mut words = [0u32; 8];
    words[0] = args[1].value;
    ctx.world.events.push(Event::Service {
        name: "obj_start_mpdrive",
        object,
        other: None,
        words,
        vector: None,
    });
    Ok(())
}

/// 1:166 FUN_140430510: the word at +8 of a target record, the attack of an
/// attack-borne reaction (FUN_140407b40 offers it, FUN_1403a8b10 hands the
/// record to the script's trigger 11).
fn target_attack(ctx: &mut Ctx<'_>, m: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let record = args[0].value;
    let word = ctx.read_u32(m, record + 8).unwrap_or(0);
    args[0] = Slot::address(word);
    Ok(())
}

/// 1:191 FUN_14042b9c0: the record at +4 of the object's lock-on block
/// (+0xdc8). Only Sora has one; the host says what it names.
fn lockon_target(_: &mut Ctx<'_>, _: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    args[0] = Slot::address(crate::world::LOCKON_RECORD);
    Ok(())
}

/// 0:49 FUN_14041e1f0 -> FUN_1402be790: the effect still plays.
fn effect_is_alive(ctx: &mut Ctx<'_>, _: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let alive = ctx.env.effect_active(args[0].value);
    args[0] = Slot::int(alive as i32);
    Ok(())
}

/// 1:24 FUN_14042e350: the current area number.
fn area_area(ctx: &mut Ctx<'_>, _: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    args[0] = Slot::int(ctx.env.area_number());
    Ok(())
}

/// 6:2 FUN_14053dc10: Sora's current drive form.
fn player_get_form(ctx: &mut Ctx<'_>, _: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    args[0] = Slot::int(ctx.env.player_form());
    Ok(())
}

/// 1:93 FUN_14042fa10: a party member's object. Only Sora (member 0) exists
/// here.
fn sysobj_party(_: &mut Ctx<'_>, _: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let h = if args[0].as_int() == 0 {
        crate::world::PLAYER_HANDLE
    } else {
        0
    };
    args[0] = Slot::address(h);
    Ok(())
}

/// 1:167 FUN_14042c7d0 -> FUN_1403caa90: start an act unless the running one
/// outranks it.
fn act_start_pri(ctx: &mut Ctx<'_>, m: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let handle = args[1].value;
    let name = m.string(handle, 128).ok_or(Fault::Memory(handle))?;
    let id = ctx.sys_of_ai(m, args[0].value)?;
    let obj = ctx.object(id)?;
    let actions = obj.acts.start_if_not_outranked(&name);
    obj.apply_actions(None, actions);
    Ok(())
}

/// 1:227 FUN_14042cd40: a jump through the mode's own entry, leaving at once
/// (no take-off event).
fn jump_direct(ctx: &mut Ctx<'_>, m: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let id = ctx.sys_of_ai(m, args[0].value)?;
    let v = read_sanitized(ctx, m, args[1].value)?;
    ctx.object(id)?.sys.jump_now([v[0], 0., -v[2]], v[1], v[3]);
    Ok(())
}

/// 1:129 FUN_14042b550 -> FUN_1403d3e50: knock the object away with this
/// velocity.
fn obj_blow(ctx: &mut Ctx<'_>, m: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let id = ctx.sys_of_ai(m, args[0].value)?;
    let v = read_native(ctx, m, args[1].value)?;
    ctx.object(id)?.sys.blow_now([v[0], v[1], v[2]]);
    Ok(())
}

/// 2:7 FUN_140431f10: the damage record's reaction byte (+0x26).
fn damage_reaction(ctx: &mut Ctx<'_>, _: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let id = crate::world::object_field(args[0].value).map_or(0, |(id, _)| id);
    let r = ctx.object(id)?.sys.damage_reaction;
    args[0] = Slot::int((r & 0xff) as i32);
    Ok(())
}

/// 2:85 FUN_140432170: the element of the attack that hit.
fn damage_element(ctx: &mut Ctx<'_>, _: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let id = crate::world::object_field(args[0].value).map_or(0, |(id, _)| id);
    let e = ctx.object(id)?.sys.damage_element;
    args[0] = Slot::int(i32::from(e));
    Ok(())
}

/// 2:86 FUN_1404321a0: the hit-back amount of the attack. [U] Reads 0.
fn damage_get_hitback(_: &mut Ctx<'_>, _: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    args[0] = Slot::float(0.);
    Ok(())
}

/// 2:91 FUN_1404321d0: the class of a reaction number (FUN_1403ce980 on
/// the argument itself).
fn damage_reaction_type(_: &mut Ctx<'_>, _: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let r = args[0].as_int();
    args[0] = Slot::int(player::combat::reaction_class(r as u8) as i32);
    Ok(())
}

/// 2:92 FUN_1404321f0 -> FUN_1403cedc0: the record's original reaction is
/// one of the finishing ones (6, 7, 10, 11).
fn damage_is_finish(ctx: &mut Ctx<'_>, _: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let id = crate::world::object_field(args[0].value).map_or(0, |(id, _)| id);
    let r = ctx.object(id)?.sys.damage_orig_reaction;
    args[0] = Slot::int(matches!(r, 6 | 7 | 10 | 11) as i32);
    Ok(())
}

/// 2:6 FUN_140431d50 -> FUN_1403d4880: a damage motion (slots 10..39)
/// plays.
fn is_damage_motion(ctx: &mut Ctx<'_>, m: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let id = ctx.sys_of_ai(m, args[0].value)?;
    let playing = (10..=39).contains(&ctx.object(id)?.sys.motion.slot);
    args[0] = Slot::int(playing as i32);
    Ok(())
}

/// 2:1 FUN_140431b60 -> FUN_1403d3f50: the script's own blow of the object it
/// holds the damage record of: the motions (attack before, attack behind), up
/// and speed.
fn exec_damage_blow(ctx: &mut Ctx<'_>, m: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let id = ctx.sys_of_ai(m, args[0].value)?;
    let env = ctx.env;
    let sys = &mut ctx.object(id)?.sys;
    let clips = env.clips(sys.model);
    sys.exec_damage_blow(
        args[2].as_int().max(0) as usize,
        args[3].as_int().max(0) as usize,
        args[4].as_float(),
        args[5].as_float(),
        clips,
    );
    Ok(())
}

/// 2:2 FUN_140431c40 -> FUN_1403d4260.
fn exec_damage_small(ctx: &mut Ctx<'_>, m: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let id = ctx.sys_of_ai(m, args[0].value)?;
    let env = ctx.env;
    let sys = &mut ctx.object(id)?.sys;
    let clips = env.clips(sys.model);
    sys.exec_damage_small(args[2].as_float(), clips);
    Ok(())
}

/// 2:3 FUN_140431c90 -> FUN_1403d4120.
fn exec_damage_hitback(
    ctx: &mut Ctx<'_>,
    m: &mut Memory<'_>,
    args: &mut Args,
) -> Result<(), Fault> {
    let id = ctx.sys_of_ai(m, args[0].value)?;
    ctx.object(id)?.sys.exec_damage_hitback(args[2].as_float());
    Ok(())
}

/// 2:87 FUN_140431bf0 -> FUN_1403d4140.
fn exec_damage_large(ctx: &mut Ctx<'_>, m: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let id = ctx.sys_of_ai(m, args[0].value)?;
    let env = ctx.env;
    let sys = &mut ctx.object(id)?.sys;
    let clips = env.clips(sys.model);
    sys.exec_damage_large(args[2].as_float(), clips);
    Ok(())
}
/// 0:8 FUN_14041d1a0: a debug print that does nothing in the shipped build.
fn vector_dump(_: &mut Ctx<'_>, _: &mut Memory<'_>, _: &mut Args) -> Result<(), Fault> {
    Ok(())
}

/// 1:42 and 1:43 FUN_14042c530 / FUN_14042c540: the global control-off bits.
fn control_off(ctx: &mut Ctx<'_>, _: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    ctx.world.set_control_bit(args[0].value, true);
    Ok(())
}

fn control_on(ctx: &mut Ctx<'_>, _: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    ctx.world.set_control_bit(args[0].value, false);
    Ok(())
}

/// 1:36 and 1:37 FUN_14042e690 / FUN_14042e6a0: hide or show a map part.
fn bg_hide(ctx: &mut Ctx<'_>, _: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    ctx.world.set_bg_part(args[0].value, false);
    Ok(())
}

fn bg_show(ctx: &mut Ctx<'_>, _: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    ctx.world.set_bg_part(args[0].value, true);
    Ok(())
}

/// 1:153 FUN_14042b880 -> FUN_1403b6100: place the object where it will not
/// be interpolated from.
fn reset_pos(ctx: &mut Ctx<'_>, m: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let v = read_sanitized(ctx, m, args[1].value)?;
    let id = ctx.sys_of_ai(m, args[0].value)?;
    let sys = &mut ctx.object(id)?.sys;
    let v = native_own(sys.camera_space, v);
    sys.position = [v[0], v[1], v[2]];
    sys.knock = None;
    Ok(())
}

/// 1:141 FUN_14042e8b0 -> FUN_1403f3dd0: the first map surface on a vertical
/// segment from `pos` up by `reach`. The hit point replaces `out`, otherwise
/// `out` becomes `pos`.
fn bg_ground_pos(ctx: &mut Ctx<'_>, m: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let p = read_native(ctx, m, args[0].value)?;
    let out = args[1].value;
    let reach = args[2].as_float();
    let from = [p[0], p[1], p[2]];
    // The probe runs along the script's +Y, which is down for the engine.
    let to = [p[0], p[1] - reach, p[2]];
    // FUN_1403f3dd0(.., 4): collision class 4.
    let hit = ctx.env.line_hit_class(from, to, 4);
    let at = hit.unwrap_or(from);
    for (k, lane) in native([at[0], at[1], at[2], 1.0]).iter().enumerate() {
        ctx.write_f32(m, out + 4 * k as u32, *lane);
    }
    args[0] = Slot::int(hit.is_some() as i32);
    Ok(())
}

/// 1:156 FUN_14042ce60 -> FUN_1403cbec0: a hop (a short ballistic move in
/// physics mode 9) with its motion. Arguments: the vector (heading, apex
/// height, speed), the rise time, the steering inertia, the motion, its blend
/// and whether the next two motions are the fall and land motions. The hop
/// starts when the motion's take-off event fires.
fn obj_hop(ctx: &mut Ctx<'_>, m: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let id = ctx.sys_of_ai(m, args[0].value)?;
    let v = read_sanitized(ctx, m, args[1].value)?;
    let (time, inertia) = (args[2].as_float(), args[3].as_float());
    let (slot, blend, auto) = (args[4].as_int(), args[5].as_float(), args[6].as_int());
    let env = ctx.env;
    let obj = ctx.object(id)?;
    // The heading turns into the scene; y is the apex height, w the speed.
    obj.sys.arm_hop([v[0], v[1], -v[2], v[3]], time, inertia);
    obj.sys
        .motion_start(slot.max(0) as usize, blend, env.clips(obj.sys.model));
    if auto != 0 {
        obj.sys.jump_motions = crate::sysobj::JumpMotions {
            fall: slot + 1,
            land: slot + 2,
        };
    }
    Ok(())
}

/// 6:42 FUN_14053e990: arm a hop like `obj_hop` (without a motion) and enter
/// the hop mode at once (FUN_1403b4da0 mode 9).
fn obj_hop_direct(ctx: &mut Ctx<'_>, m: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let id = ctx.sys_of_ai(m, args[0].value)?;
    let v = read_sanitized(ctx, m, args[1].value)?;
    let (time, inertia) = (args[2].as_float(), args[3].as_float());
    let sys = &mut ctx.object(id)?.sys;
    sys.arm_hop([v[0], v[1], -v[2], v[3]], time, inertia);
    if let Some(crate::sysobj::PendingHop { hop }) = sys.pending_hop.take() {
        sys.pending_mode = 0;
        sys.enter_hop(hop);
    }
    Ok(())
}

/// 1:73 FUN_14042d010 -> FUN_140422950: hover in physics mode 6, the
/// steering mode with the velocity `normalize(v.xyz) * v.w` and a steering
/// inertia. The vector is the script's (+Y down), as the executable's mode
/// keeps it.
fn obj_hover(ctx: &mut Ctx<'_>, m: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let id = ctx.sys_of_ai(m, args[0].value)?;
    let v = read_native(ctx, m, args[1].value)?;
    let inertia = args[2].as_float();
    ctx.object(id)?.sys.start_hover(v, inertia);
    Ok(())
}

/// 0:78 FUN_14041d490 -> FUN_140140230: `vector_inter(a, b, t)` =
/// `b * (1 - t) + a * t`, in a static result vector.
fn vector_inter(ctx: &mut Ctx<'_>, m: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let a = read_sanitized(ctx, m, args[0].value)?;
    let b = read_sanitized(ctx, m, args[1].value)?;
    let t = args[2].as_float();
    let u = 1.0 - t;
    let cell = ctx.world.static_cell(0, 78, 0);
    for k in 0..4 {
        ctx.write_f32(m, cell + 4 * k as u32, b[k] * u + a[k] * t);
    }
    args[0] = Slot::address(cell);
    Ok(())
}

recorded_traps! {}

#[cfg(test)]
mod tests;

/// 1:221 FUN_14042bbf0 -> FUN_1403b6950: whether the object may take the
/// command now (its command lockout and motion windows, FUN_1403ba070),
/// with a target (an AI object, here the friend the command is for).
fn can_decide_command(ctx: &mut Ctx<'_>, m: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let id = ctx.sys_of_ai(m, args[0].value)?;
    let command = args[1].as_int() as u16;
    let other = ctx.sys_of_ai(m, args[2].value).ok();
    let me = ctx.object(id)?.sys.clone();
    let them = match other {
        Some(o) => Some((o, ctx.object(o)?.sys.clone())),
        None => None,
    };
    let ok = decides(
        ctx.env,
        command,
        (id, &me),
        them.as_ref().map(|(o, s)| (*o, s)),
    );
    args[0] = Slot::int(ok as i32);
    Ok(())
}

/// FUN_1403b6950: the command's +0x1f names whose windows decide it: 0 the
/// user's, 1 the target's, 2 both; anything else refuses it.
pub fn decides(
    env: &dyn crate::env::Env,
    command: u16,
    user: (crate::world::ObjId, &crate::sysobj::Sysobj),
    target: Option<(crate::world::ObjId, &crate::sysobj::Sysobj)>,
) -> bool {
    let takes =
        |(id, s): (crate::world::ObjId, &crate::sysobj::Sysobj)| object_takes(env, id, s, command);
    match env.command_decider(command) {
        0 => takes(user),
        1 => target.is_some_and(takes),
        2 => takes(user) && target.is_some_and(takes),
        _ => false,
    }
}

/// FUN_1403ba070 for one object: its motion's own windows first
/// ([`Sysobj::command_window`]); then for Sora the host's answer (his
/// kit's command lockout and windows), for another object its free windows
/// ([`Sysobj::free_window`]). [I] Sora's flag-only terms are left to the
/// host, which keeps his lockout.
pub fn object_takes(
    env: &dyn crate::env::Env,
    id: crate::world::ObjId,
    sys: &crate::sysobj::Sysobj,
    command: u16,
) -> bool {
    sys.command_window(command, env.command_chain(command))
        .unwrap_or_else(|| {
            let flags = env.command_flags(command);
            if id == crate::world::PLAYER_OBJECT {
                env.can_decide_command(id, command)
            } else {
                sys.free_window(flags)
            }
        })
}
