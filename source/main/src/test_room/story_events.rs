//! `KH2RW_STORY_EVENTS=1`: an area's story events start from the field. On
//! arrival the area raises signal 0 (FUN_140152cd0 → FUN_1401561b0): the entry
//! it picks sets its flag at once (FUN_1403f61a0), and when it names an event
//! that event plays and the entry's jump follows its end (FUN_1403ac350,
//! FUN_1403a46e0). An event writes no story state, so the field does all the
//! writes, hands over one image and the event program starts the next field.
//!
//! Not here yet: other signals, a player's skip (FUN_1403a55b0), events with no
//! jump (signal 0x4c) and those over a running field (`evtp` flags bit 0 clear).
use super::*;
use crate::area::{Fired, SignalJump};
use save::slots::field;

/// `evtp` rows whose flags bit 0 is clear play over the running field.
const OVER_THE_FIELD: [i16; 6] = [11, 12, 13, 14, 15, 17];

pub fn enabled() -> bool {
    std::env::var_os("KH2RW_STORY_EVENTS").is_some()
}

/// The event name's bytes (a type-22 entry's name).
fn event_name(name: u32) -> String {
    let bytes = name.to_le_bytes();
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(4);
    String::from_utf8_lossy(&bytes[..end]).into_owned()
}

/// An entry this step can carry out.
fn playable(mode: i16, jump: &Option<SignalJump>) -> bool {
    matches!(jump, Some(SignalJump::Field { .. })) && !OVER_THE_FIELD.contains(&(mode & 0x3f))
}

/// Once per arrival, when the area is up.
pub fn tick(
    mut game: ResMut<Game>,
    area: Option<ResMut<AreaRes>>,
    camp: Option<Res<CampRes>>,
    mut arrived: Local<Option<(u8, u8)>>,
) {
    if !enabled() {
        return;
    }
    let (Some(mut area), Some(camp)) = (area, camp) else {
        return;
    };
    let a = &mut *area;
    if a.transition.is_some() || a.pending.is_some() {
        return;
    }
    let here = (a.current.world, a.current.area);
    if *arrived == Some(here) {
        return;
    }
    *arrived = Some(here);
    let tick = game.tick;
    // Load-time writes: op 0x19 (FUN_1403a24c0).
    for flag in a.current.state.setup.set_flags.clone() {
        if !a.source.progress().test(flag) {
            a.source.progress_mut().set(flag);
        }
    }
    let Some(index) = ::area::script::find_entry(&a.current.state.setup.events, 0, 0) else {
        return;
    };
    let entry = a.current.state.setup.events[index].clone();
    let jump = a.source.entry_jump(&entry);
    if let Some((_, mode)) = entry.event
        && !playable(mode, &jump)
    {
        game.event(serde_json::json!({"event":"story_event_left","tick":tick,
            "world":here.0,"area":here.1,"mode":mode,"jump":format!("{jump:?}")}));
        return;
    }
    let Fired { event, flag } = a
        .source
        .fire_entry(&mut a.current.state.setup.events, index);
    let name = event.map(|(name, _)| event_name(name));
    // FUN_1403a46e0: sub-op 4 works on the row of the jump's world; sub-op 7's
    // mode shows the notice, 2 also stripping whoever left.
    let notice = entry.party.filter(|&p| p >= 0).and_then(|party| {
        let world = match jump {
            Some(SignalJump::Field { world, .. }) => world,
            _ => here.0,
        };
        let preset = a.source.member_preset(party as usize)?;
        let old = game_save::world_party(&game.save.party_table, world);
        let new = game_save::row_with_presets(old, &[preset]);
        if let Some(to) = game.save.party_table.get_mut(world as usize) {
            *to = new;
        }
        let mode = entry.mode.unwrap_or(0);
        if mode == 2 {
            game_save::strip_departing(&mut game, &old, &new);
        }
        (mode != 0).then_some((old, mode))
    });
    game.event(
        serde_json::json!({"event":"story_signal","tick":tick,"world":here.0,
        "area":here.1,"programs":a.current.programs,"flag":flag,"story_event":name,
        "jump":format!("{jump:?}")}),
    );
    let Some(SignalJump::Field {
        world,
        area: place,
        entrance,
        programs,
    }) = jump
    else {
        return;
    };
    match name {
        Some(name) => {
            let code = ::area::progress::WORLDS
                .get(here.0 as usize)
                .copied()
                .unwrap_or("zz");
            start_event(
                &mut game,
                &camp,
                a,
                &format!("{code}:{}:{name}", here.1),
                (world, place, entrance),
                programs,
                notice,
            );
        }
        // No event: the jump as a gate's (FUN_1403a46e0 → FUN_140152990).
        None => {
            // [I] FUN_140152a90 waits for the camp: the notice opens as the
            // next room starts here, where no camp waits for the jump.
            game.party_notice = notice;
            let mut fade = Fade::default();
            fade.fade_out(15, false);
            a.transition = Some(Transition {
                stage: Stage::Black,
                iris: None,
                fade,
                target: (world, place, entrance),
                job: start_area_job_with(&a.source, (world, place, entrance), programs),
            });
        }
    }
}

/// The image at the jump's place, and the event program, which starts the next
/// field when it ends (`--then-room`).
fn start_event(
    game: &mut Game,
    c: &CampRes,
    a: &AreaRes,
    event: &str,
    (world, number, entrance): (u8, u8, u8),
    programs: [i16; 3],
    notice: Option<([u8; 4], i16)>,
) {
    let mut image = game_save::capture(game, c, Some(a));
    image.set_u8(field::WORLD, world);
    image.set_u8(field::AREA, number);
    image.set_u8(field::ENTRANCE, entrance);
    image.seal();
    let tick = game.tick;
    if let Some(path) = &game.save.dump
        && let Err(e) = std::fs::write(path, image.bytes())
    {
        eprintln!("Save dump rejected: {e}");
    }
    let args = game.save.args.clone();
    let config = ::area::progress::WORLDS
        .get(world as usize)
        .zip(args.get(2))
        .and_then(|(code, arena)| {
            let programs = (programs != [-1; 3]).then_some(programs);
            crate::launcher::landing_room_config(arena, code, number, entrance, programs, &[])
                .map_err(|e| eprintln!("The next field could not start: {e}"))
                .ok()
        });
    // An unattended run stops at the hand-over: its captures are the room's.
    let started = (game.autoplay.is_none() || crate::save_files::chain_test())
        && crate::save_files::write_handoff(&image).is_ok()
        && match (args.first(), args.get(1), &config) {
            (Some(root), Some(manifest), Some(config)) => crate::save_files::start_next(
                vec![
                    "event".into(),
                    root.to_string_lossy().into_owned(),
                    manifest.to_string_lossy().into_owned(),
                    "--event".into(),
                    event.to_owned(),
                    "--then-room".into(),
                    config.to_string_lossy().into_owned(),
                ]
                .into_iter()
                .chain(notice.into_iter().flat_map(|(old, mode)| {
                    [
                        "--party-notice".to_owned(),
                        party_room::notice_arg(old, mode),
                    ]
                }))
                .collect::<Vec<_>>(),
            ),
            _ => false,
        };
    if started {
        game.limit = Some(tick);
    }
    game.event(
        serde_json::json!({"event":"story_event","tick":tick,"name":event,
        "world":world,"area":number,"entrance":entrance,"programs":programs,"started":started}),
    );
}
