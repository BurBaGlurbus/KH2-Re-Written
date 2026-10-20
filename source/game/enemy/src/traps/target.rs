//! Traps on the actor an object is aiming at (the record at sysobj+0xbf8), and
//! the small battle-record traps of table 2.
//!
//! The battle target table's selection (FUN_14040c870 over DAT_142a16370, with
//! the engine's generator) and search kinds 0, 1, 3 and 5 are translated;
//! kinds 4 and 6 (the same with the object's own range and angle parameters)
//! are not and answer Sora, and the host's
//! [`Env::pick_target`](crate::env::Env::pick_target) fills in for objects
//! this crate does not hold.

use crate::script::{Fault, Memory, Slot};
use crate::sysobj::{Target, Vec3, class_flag, state_flag};
use crate::world::{
    Ctx, OBJECT_HANDLES, ObjId, PLAYER_HANDLE, TARGET_DUP_HANDLES, TARGET_RECORD_OFFSET, TrapFn,
    object_handle,
};

type Args = [Slot; 16];

/// Target id meaning Sora.
pub const PLAYER_TARGET: u32 = u32::MAX;
/// A target id with this bit names a script-driven object.
pub const OBJECT_TARGET: u32 = 0x4000_0000;

pub fn lookup(table: u16, index: u16) -> Option<TrapFn> {
    Some(match (table, index) {
        (1, 120) => target_pos,
        (1, 121) => target_search,
        (1, 131) => target_dup,
        (1, 162) => target_set_before_player,
        (1, 132) => target_free,
        (2, 5) => enemy_is_no_control,
        (2, 8) => damage_is_reaction,
        (2, 9) => set_sheet,
        (2, 23) => btlobj_target,
        (2, 88) => get_attacker,
        _ => return None,
    })
}

/// Where a record handle points.
enum Slot3 {
    Object(u32),
    Dup(usize),
    /// Sora's lock-on record; the host decides what it names.
    Lockon,
}

fn locate(handle: u32) -> Option<Slot3> {
    if handle == crate::world::LOCKON_RECORD {
        return Some(Slot3::Lockon);
    }
    if let Some(d) = handle.checked_sub(TARGET_DUP_HANDLES)
        && d < 0x0100_0000
    {
        return Some(Slot3::Dup((d / 16) as usize));
    }
    let o = handle.checked_sub(OBJECT_HANDLES)?;
    (o % 0x1000 == TARGET_RECORD_OFFSET && o < 0x0100_0000).then_some(Slot3::Object(o / 0x1000))
}

fn record(ctx: &mut Ctx<'_>, handle: u32) -> Result<Target, Fault> {
    match locate(handle).ok_or(Fault::Memory(handle))? {
        Slot3::Object(id) => Ok(ctx.object(id)?.sys.target),
        Slot3::Dup(i) => ctx
            .world
            .target_dups
            .get(i)
            .copied()
            .flatten()
            .ok_or(Fault::Memory(handle)),
        Slot3::Lockon => Ok(Target {
            id: ctx.env.lockon_target().unwrap_or(0),
            kind: -1,
            group: 0,
        }),
    }
}

/// The record target id naming an object: Sora, or the object.
pub(super) fn target_id_of(id: crate::world::ObjId) -> u32 {
    if id == crate::world::PLAYER_OBJECT {
        PLAYER_TARGET
    } else {
        OBJECT_TARGET | id
    }
}

/// The record at `handle`, for the other trap modules.
pub(super) fn record_of(ctx: &mut Ctx<'_>, handle: u32) -> Result<Target, Fault> {
    record(ctx, handle)
}

pub(super) fn set_record(ctx: &mut Ctx<'_>, handle: u32, t: Target) -> Result<(), Fault> {
    match locate(handle).ok_or(Fault::Memory(handle))? {
        Slot3::Object(id) => ctx.object(id)?.sys.target = t,
        // The player's lock-on belongs to the player.
        Slot3::Lockon => {}
        Slot3::Dup(i) => {
            *ctx.world
                .target_dups
                .get_mut(i)
                .ok_or(Fault::Memory(handle))? = Some(t)
        }
    }
    Ok(())
}

/// Whether the record at `handle` names something that still exists
/// (FUN_1403bde30): Sora, or a target the host can still place.
pub(super) fn record_exists(ctx: &mut Ctx<'_>, handle: u32) -> bool {
    let Ok(t) = record(ctx, handle) else {
        return false;
    };
    target_point(ctx, t).is_some()
}

/// Where a record's target is, if it still exists.
pub(super) fn target_point(ctx: &mut Ctx<'_>, t: Target) -> Option<Vec3> {
    if t.id == 0 {
        return None;
    }
    if t.id == PLAYER_TARGET {
        return Some(ctx.env.player_position());
    }
    if t.id & OBJECT_TARGET != 0 {
        let o = ctx.object(t.id & !OBJECT_TARGET).ok()?;
        return (!o.sys.gone).then_some(o.sys.position);
    }
    ctx.env.target_position(t.id)
}

/// The record handle's target position (`target_pos`), for other modules.
pub(super) fn record_point(ctx: &mut Ctx<'_>, handle: u32) -> Option<Vec3> {
    let t = record(ctx, handle).ok()?;
    target_point(ctx, t)
}

/// 1:120 FUN_140430350 -> FUN_1403bdde0: the centre of the target's TARGET
/// shape, in the shared result vector.
fn target_pos(ctx: &mut Ctx<'_>, m: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let t = record(ctx, args[0].value)?;
    let p = target_point(ctx, t).unwrap_or([0.; 3]);
    let cell = ctx.world.static_cell(1, 0, 1);
    for (k, lane) in [p[0], -p[1], p[2], 1.0].iter().enumerate() {
        ctx.write_f32(m, cell + 4 * k as u32, *lane);
    }
    args[0] = Slot::address(cell);
    Ok(())
}

/// One entry of the battle target table (DAT_142a16370): who the enemies
/// pick among, and how soon (0 is first, otherwise the lower number wins).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BattleTarget {
    pub priority: i32,
    pub object: Option<ObjId>,
}

/// How many objects the battle target table holds.
pub const BATTLE_TARGET_SLOTS: usize = 8;

/// FUN_1403ba720: the object exists and none of the hidden/removed flags
/// (`flags & 0x10080000`) is set.
fn usable(ctx: &mut Ctx<'_>, id: ObjId) -> bool {
    ctx.object(id)
        .is_ok_and(|o| !o.sys.gone && o.sys.flags & crate::world::SEARCH_SKIP_FLAGS == 0)
}

/// 1:162 FUN_1404304b0 -> FUN_1403bd410 -> FUN_14040c9d0: put the object in the
/// battle target table with a priority. Entries whose object has become
/// unusable are cleared first; the object keeps its slot if it has one, else
/// takes the last free (or unusable) slot.
fn target_set_before_player(
    ctx: &mut Ctx<'_>,
    m: &mut Memory<'_>,
    args: &mut Args,
) -> Result<(), Fault> {
    let id = ctx.sys_of_ai(m, args[0].value)?;
    let priority = args[1].as_int();
    for k in 0..BATTLE_TARGET_SLOTS {
        if let Some(o) = ctx.world.battle_targets[k].object
            && !usable(ctx, o)
        {
            ctx.world.battle_targets[k].object = None;
        }
    }
    if !usable(ctx, id) {
        return Ok(());
    }
    let mut slot = None;
    for k in 0..BATTLE_TARGET_SLOTS {
        let held = ctx.world.battle_targets[k].object;
        match held {
            Some(o) if o == id => {
                slot = Some(k);
                break;
            }
            Some(o) if usable(ctx, o) => {}
            _ => slot = Some(k),
        }
    }
    if let Some(k) = slot {
        ctx.world.battle_targets[k] = BattleTarget {
            priority,
            object: Some(id),
        };
    }
    Ok(())
}

/// FUN_14040c870: the battle target table's choice. A usable entry of
/// priority 0 wins at once; else the lowest priority, and among equals one
/// drawn with the engine's generator.
pub(super) fn pick_battle_target(ctx: &mut Ctx<'_>) -> Option<ObjId> {
    let mut best: Option<i32> = None;
    let mut chosen: Vec<ObjId> = Vec::new();
    for k in 0..BATTLE_TARGET_SLOTS {
        let BattleTarget { priority, object } = ctx.world.battle_targets[k];
        let Some(o) = object else { continue };
        if !usable(ctx, o) {
            continue;
        }
        if priority == 0 {
            return Some(o);
        }
        match best {
            Some(b) if priority == b => chosen.push(o),
            Some(b) if priority > b => {}
            _ => {
                best = Some(priority);
                chosen = vec![o];
            }
        }
    }
    let n = chosen.len() as u64;
    if n == 0 {
        return None;
    }
    let draw = u64::from(ctx.world.engine_rng.step());
    Some(chosen[(draw / (1u64 << 32).div_ceil(n)) as usize])
}

/// FUN_1403be7f0: the nearest TARGET shape of the object list's enemy-class
/// objects that are alive, measured from the searching object (less `bias`
/// for the objects that are preferred targets).
fn nearest_target(
    ctx: &mut Ctx<'_>,
    from: ObjId,
    bias: f32,
) -> Result<Option<(ObjId, u32)>, Fault> {
    let own = ctx.object(from)?.sys.position;
    let mut ids: Vec<ObjId> = ctx.world.objects.iter().flatten().map(|o| o.id).collect();
    ids.push(ctx.me.id);
    ids.sort_unstable();
    ids.dedup();
    let mut best: Option<(ObjId, u32, f32)> = None;
    for id in ids {
        let o = ctx.object(id)?;
        let alive = !o.sys.gone
            && o.sys.hp > 0
            && o.sys.state_flags & state_flag::DEAD_MARKED == 0
            && o.sys.flags & crate::world::SEARCH_SKIP_FLAGS == 0;
        let class = o.sys.class_flags;
        if class & class_flag::ENEMY == 0 || !alive {
            continue;
        }
        for (shape, at) in super::searcher::shapes(ctx, id) {
            let mut distance = len3(sub3(at, own));
            if class & class_flag::PRIORITY_TARGET != 0 {
                distance -= bias;
            }
            if best.is_none_or(|(_, _, d)| distance < d) {
                best = Some((id, shape, distance));
            }
        }
    }
    Ok(best.map(|(id, shape, _)| (id, shape)))
}

fn sub3(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn len3(v: Vec3) -> f32 {
    (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
}

/// The record `kind` search writes (FUN_1403be2b0): Sora, the battle
/// target table's pick, or (kinds 3 and 5) the nearest enemy-class target.
/// [U] Kinds 4 and 6 (the same with ranges and angles of the object's own
/// parameters) are not translated and answer Sora.
fn search(ctx: &mut Ctx<'_>, kind: i32, from: u32) -> Result<Target, Fault> {
    let obj = ctx.object(from)?;
    let (flags, position) = (obj.sys.flags, obj.sys.position);
    let id = match kind {
        0 => {
            let picked = if flags >> 11 & 1 != 0 {
                None
            } else {
                pick_battle_target(ctx)
                    .map(target_id_of)
                    .or_else(|| ctx.env.pick_target(0, position))
            };
            picked.unwrap_or(PLAYER_TARGET)
        }
        1 => {
            let picked = if flags & 0x1800 != 0 {
                None
            } else {
                pick_battle_target(ctx).map(target_id_of)
            };
            picked.unwrap_or_else(|| {
                let last = ctx.object(from).ok().and_then(|o| o.sys.last_attacker);
                match last {
                    Some(t) if t & OBJECT_TARGET != 0 && !usable(ctx, t & !OBJECT_TARGET) => {
                        PLAYER_TARGET
                    }
                    Some(t) => t,
                    None => PLAYER_TARGET,
                }
            })
        }
        3 | 5 => {
            let bias = if kind == 5 { 10_000.0 } else { 0.0 };
            if let Some((found, shape)) = nearest_target(ctx, from, bias)? {
                return Ok(Target {
                    id: target_id_of(found),
                    kind,
                    group: shape as i32,
                });
            }
            0
        }
        _ => PLAYER_TARGET,
    };
    Ok(Target { id, kind, group: 0 })
}

/// 1:121 FUN_1404303a0 -> FUN_1403be2b0: search for a target of `kind`.
fn target_search(ctx: &mut Ctx<'_>, m: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let from = ctx.sys_of_ai(m, args[1].value)?;
    let t = search(ctx, args[2].as_int(), from)?;
    set_record(ctx, args[0].value, t)
}

/// 1:131 FUN_1404303f0 -> FUN_1403bda80: a copy of a record.
fn target_dup(ctx: &mut Ctx<'_>, _: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let t = record(ctx, args[0].value)?;
    let slot = ctx.world.target_dups.iter().position(Option::is_none);
    let i = match slot {
        Some(i) => i,
        None => {
            ctx.world.target_dups.push(None);
            ctx.world.target_dups.len() - 1
        }
    };
    ctx.world.target_dups[i] = Some(t);
    args[0] = Slot::address(TARGET_DUP_HANDLES + i as u32 * 16);
    Ok(())
}

/// 1:132 FUN_140430420 -> FUN_1403bdba0.
fn target_free(ctx: &mut Ctx<'_>, _: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    if let Some(Slot3::Dup(i)) = locate(args[0].value)
        && let Some(slot) = ctx.world.target_dups.get_mut(i)
    {
        *slot = None;
    }
    Ok(())
}

/// 2:5 FUN_140431d10: +0x18c bit 9 of the AI object's engine object.
fn enemy_is_no_control(
    ctx: &mut Ctx<'_>,
    m: &mut Memory<'_>,
    args: &mut Args,
) -> Result<(), Fault> {
    let id = ctx.sys_of_ai(m, args[0].value)?;
    let blocked = ctx.object(id)?.sys.no_control();
    args[0] = Slot::int(blocked as i32);
    Ok(())
}

/// 2:8 FUN_140431f70: the damage record's reaction byte (+0x26) is above class 1.
fn damage_is_reaction(ctx: &mut Ctx<'_>, _: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let handle = args[0].value;
    let o = handle
        .checked_sub(OBJECT_HANDLES)
        .ok_or(Fault::Memory(handle))?;
    let reaction = ctx.object(o / 0x1000)?.sys.damage_reaction;
    args[0] = Slot::int((player::combat::reaction_class(reaction as u8) > 1) as i32);
    Ok(())
}

/// 2:9 FUN_140431a70 -> FUN_140411350: bind the battle parameter sheet.
/// [U] The host builds the stats from ENMP/LVPM when it creates the object;
/// the sheet number is kept.
fn set_sheet(ctx: &mut Ctx<'_>, m: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let id = ctx.sys_of_ai(m, args[0].value)?;
    ctx.object(id)?.sys.sheet = args[1].as_int();
    Ok(())
}

/// 2:23 FUN_1404319d0: the object's own target record, searched again for
/// Sora when it has become invalid and the object is of the enemy class
/// (+0x6c8 bit 4; a party member keeps its invalid record).
fn btlobj_target(ctx: &mut Ctx<'_>, m: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let id = ctx.sys_of_ai(m, args[0].value)?;
    let sys = &ctx.object(id)?.sys;
    let (t, bit4) = (
        sys.target,
        sys.class_flags & crate::sysobj::class_flag::ENEMY != 0,
    );
    let valid = target_point(ctx, t).is_some();
    if !valid && bit4 && t.kind != 3 {
        let found = search(ctx, 2, id)?;
        ctx.object(id)?.sys.target = found;
    }
    args[0] = Slot::address(object_handle(id) + TARGET_RECORD_OFFSET);
    Ok(())
}

/// 2:88 FUN_140431e60: the last attacker, else Sora.
fn get_attacker(ctx: &mut Ctx<'_>, m: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let id = ctx.sys_of_ai(m, args[0].value)?;
    let attacker = ctx.object(id)?.sys.last_attacker;
    let handle = match attacker {
        Some(a) if a != PLAYER_TARGET && ctx.object(a).is_ok() => object_handle(a),
        _ => PLAYER_HANDLE,
    };
    args[0] = Slot::address(handle);
    Ok(())
}

#[cfg(test)]
mod tests;
