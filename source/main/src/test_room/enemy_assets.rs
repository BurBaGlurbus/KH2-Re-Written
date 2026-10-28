//! The object sound bank, voice bank and effect container of every battle
//! enemy the gallery runs, read from its object archive (`obj/<model>.a.us`)
//! on a worker thread the first time it appears. The room's start reads them
//! only for `enemy_battle::SUPPORTED`; the motion sounds (frame event 8), the
//! victim sounds, the voices and the frame-1 effects of every other enemy
//! resolve through the same maps once its archive is in.
use super::*;

/// Its sound bank, its voice bank and its effect container.
type Read = (Option<Vec<u8>>, Option<Vec<u8>>, Option<object_effect::Pax>);

/// Archives asked for, by objentry id, and the reads still running.
#[derive(Default)]
pub struct Pending {
    asked: std::collections::HashSet<u32>,
    jobs: Vec<(u32, String, std::thread::JoinHandle<Read>)>,
}

pub fn service(
    mut pending: Local<Pending>,
    gallery: Option<Res<enemy_gallery::EnemyGallery>>,
    fx: Option<ResMut<Effects>>,
    audio: Option<ResMut<Audio>>,
    game: Option<ResMut<Game>>,
) {
    let (Some(gallery), Some(mut fx), Some(mut audio), Some(mut game)) = (gallery, fx, audio, game)
    else {
        return;
    };
    for actor in &gallery.actors {
        let Some(battle) = actor.battle.as_ref() else {
            continue;
        };
        let object = battle.assets.object_id;
        if fx.enemy_banks.contains_key(&object)
            || fx.enemy_paxes.contains_key(&object)
            || !pending.asked.insert(object)
        {
            continue;
        }
        let Some(entry) = gallery.catalog.entries.get(actor.entry) else {
            continue;
        };
        let name = entry.model_name.clone();
        let source = Arc::clone(&gallery.source);
        let model = name.clone();
        pending.jobs.push((
            object,
            name,
            std::thread::spawn(move || read(&source, &model)),
        ));
    }
    let mut k = 0;
    while k < pending.jobs.len() {
        if !pending.jobs[k].2.is_finished() {
            k += 1;
            continue;
        }
        let (object, name, job) = pending.jobs.swap_remove(k);
        let (bank, voice, pax) = job.join().unwrap_or((None, None, None));
        let mut insert = |bytes: Option<Vec<u8>>| bytes.and_then(|b| audio.banks.insert(b).ok());
        let bank = insert(bank);
        let voice = insert(voice);
        if let Some(n) = bank {
            fx.enemy_banks.insert(object, n);
        }
        if let Some(n) = voice {
            game.enemy_voice_banks.insert(object, n);
        }
        let effects = pax.is_some();
        if let Some(p) = pax {
            fx.enemy_paxes.insert(object, Arc::new(p));
        }
        let tick = game.tick;
        game.event(
            serde_json::json!({"event":"enemy_assets","tick":tick,"object":object,
            "model":name,"bank":bank,"voice":voice,"effects":effects}),
        );
    }
}

fn read(source: &ProbeSource, model: &str) -> Read {
    let Ok(mut library) = data::path_lookup::Library::open(source) else {
        return (None, None, None);
    };
    let Some(f) = library.find(&format!("obj/{model}.a.us")) else {
        return (None, None, None);
    };
    (
        sound::object_bank(source, f.index, f.ordinal, f.name_hash),
        sound::object_voice_bank(source, f.index, f.ordinal, f.name_hash),
        drive_room::object_pax(&mut library, model),
    )
}
