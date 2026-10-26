//! The duel's music: while the players are in the duel's arena (the lobby
//! over it, the countdown and the fight) a battle theme of the game plays,
//! a new one each match, the host's pick sent to the joiner so both play the
//! same; it fades out at the result and when the duel ends, and the room's
//! own music comes back after the session. The match's cues play over it.
//!
//! The pool is the game's own battle themes: every room's battle track in
//! 03system `arif` (each world's rows of field and battle pairs, bgm-input
//! A4) that differs from the row's field track [duel choice: which of them
//! plays is random].

use super::duel_match::Phase;
use super::duel_net::{DuelNet, Status};
use super::duel_wire::MUSIC;
use super::*;
use net::wire::{Reader, Writer};
use net::{RELIABLE, Role};
use std::thread::JoinHandle;

/// The duel's music voice.
const KEY: u32 = 9_000_002;
/// Fades in and out, in seconds.
const FADE_IN: f32 = 1.;
const FADE_OUT: f32 = 2.;

type Track = (u32, u16);

#[derive(Resource, Default)]
pub(crate) struct DuelMusic {
    pool_job: Option<JoinHandle<Result<Vec<u16>, String>>>,
    pool: Option<Vec<u16>>,
    /// This match's track: (match, track number); the host picks it, the
    /// joiner is told.
    chosen: Option<Track>,
    /// Host: what the joiner was told.
    sent: Option<Track>,
    loading: Option<(Track, JoinHandle<Result<sound::Music, String>>)>,
    loaded: Option<(Track, sound::Music)>,
    playing: Option<Track>,
    /// The room's own music was paused for the duel's.
    room_paused: bool,
}

/// The battle themes the install's `arif` table names.
fn battle_pool(source: &ProbeSource) -> Result<Vec<u16>, String> {
    let mut library = data::path_lookup::Library::open(source).map_err(|e| e.to_string())?;
    let sys = library.read("03system.bin").map_err(|e| e.to_string())?;
    let table = bar::read_bar(&sys).map_err(|e| e.to_string())?;
    let e = table
        .entries
        .iter()
        .find(|e| e.name_bytes == *b"arif")
        .ok_or("no arif table")?;
    let arif = sys
        .get(e.offset as usize..e.offset as usize + e.size as usize)
        .ok_or("arif past the file")?;
    let mut pool = battle_tracks(arif);
    pool.retain(|&t| library.find(&track_name(t)).is_some());
    Ok(pool)
}

/// Every row's battle track that differs from its field track, sorted and
/// distinct. `arif`: u32 world count at +4; per world a u16 area count and
/// u16 offset at +8 + 4·world; per area 0x40 bytes with 8 (field, battle)
/// u16 pairs at +0x10.
fn battle_tracks(arif: &[u8]) -> Vec<u16> {
    let word = |o: usize| arif.get(o..o + 2).map(|b| u16::from_le_bytes([b[0], b[1]]));
    let worlds = arif
        .get(4..8)
        .map_or(0, |b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .min(64) as usize;
    let mut out = Vec::new();
    for w in 0..worlds {
        let (Some(count), Some(off)) = (word(8 + w * 4), word(10 + w * 4)) else {
            continue;
        };
        for a in 0..usize::from(count) {
            for set in 0..8 {
                let at = usize::from(off) + a * 0x40 + 0x10 + set * 4;
                if let (Some(field), Some(battle)) = (word(at), word(at + 2))
                    && battle != 0
                    && battle != field
                {
                    out.push(battle);
                }
            }
        }
    }
    out.sort_unstable();
    out.dedup();
    out
}

fn track_name(track: u16) -> String {
    format!("bgm/music{track:03}.win32.scd")
}

fn load_track(source: Arc<ProbeSource>, track: u16) -> Result<sound::Music, String> {
    let mut library = data::path_lookup::Library::open(&source).map_err(|e| e.to_string())?;
    let bytes = library
        .read(&track_name(track))
        .map_err(|e| e.to_string())?;
    sound::decode_music(bytes)
}

/// One fixed step: the host's pick, the joiner's word of it, loading, and
/// the music on or off with the duel's arena.
#[allow(clippy::too_many_arguments)]
pub(crate) fn tick(
    net: Option<ResMut<DuelNet>>,
    lobby: Option<Res<duel_lobby_room::LobbyRoom>>,
    duel_match: Option<Res<duel_match_room::MatchRoom>>,
    room_music: Option<Res<MusicRes>>,
    mut music: ResMut<DuelMusic>,
    audio: Res<Audio>,
    gallery: Res<enemy_gallery::EnemyGallery>,
    mut game: ResMut<Game>,
) {
    let (Some(mut net), Some(lobby)) = (net, lobby) else {
        return;
    };
    let (net, m) = (&mut *net, &mut *music);
    let tick = game.tick;
    // The joiner hears the host's pick.
    for msg in std::mem::take(&mut net.inbox) {
        match (net.role, msg.first().copied()) {
            (Role::Join, Some(MUSIC)) => {
                let mut r = Reader::new(&msg);
                let _ = r.u8();
                if let (Some(n), Some(t)) = (r.u32(), r.u16()) {
                    m.chosen = Some((n, t));
                }
            }
            _ => net.inbox.push(msg),
        }
    }
    let playing_session = matches!(net.status, Status::Playing);
    // The pool, once (the host's).
    if net.role == Role::Host && m.pool.is_none() && m.pool_job.is_none() {
        let source = Arc::clone(&gallery.source);
        m.pool_job = Some(std::thread::spawn(move || battle_pool(&source)));
    }
    if m.pool_job.as_ref().is_some_and(|j| j.is_finished())
        && let Some(job) = m.pool_job.take()
    {
        let pool = job
            .join()
            .map_err(|_| "the music pool's thread stopped".to_string())
            .and_then(|r| r);
        game.event(serde_json::json!({"event":"duel_music_pool","tick":tick,
            "pool":pool.as_ref().ok(),"error":pool.as_ref().err()}));
        m.pool = Some(pool.unwrap_or_default());
    }
    // Host: a new track each match, told to the joiner.
    let match_no = lobby.lobby.match_no;
    if net.role == Role::Host
        && playing_session
        && m.chosen.map(|c| c.0) != Some(match_no)
        && let Some(pool) = m.pool.as_ref().filter(|p| !p.is_empty())
    {
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.subsec_nanos() as usize);
        m.chosen = Some((match_no, pool[seed % pool.len()]));
    }
    if net.role == Role::Host && playing_session && m.sent != m.chosen {
        m.sent = m.chosen;
        if let Some((n, t)) = m.chosen {
            let mut w = Writer::default();
            w.u8(MUSIC);
            w.u32(n);
            w.u16(t);
            net.session.send(RELIABLE, w.finish());
        }
    }
    // Loading the chosen track.
    if let Some(c) = m.chosen
        && m.loaded.as_ref().map(|l| l.0) != Some(c)
        && m.loading.as_ref().map(|l| l.0) != Some(c)
    {
        let source = Arc::clone(&gallery.source);
        m.loading = Some((c, std::thread::spawn(move || load_track(source, c.1))));
    }
    if m.loading.as_ref().is_some_and(|l| l.1.is_finished())
        && let Some((c, job)) = m.loading.take()
    {
        match job
            .join()
            .map_err(|_| "the music thread stopped".to_string())
            .and_then(|r| r)
        {
            Ok(track) => {
                game.event(serde_json::json!({"event":"duel_music_loaded","tick":tick,
                    "track":c.1,"match":c.0}));
                m.loaded = Some((c, track));
            }
            Err(e) => game.event(serde_json::json!({"event":"duel_music_failed","tick":tick,
                "track":c.1,"error":e})),
        }
    }
    // On while in the arena (lobby, countdown, fight), off at the result
    // (the joiner holds the last match's result until the next starts; a
    // lobby that is up is the next match's).
    let over = !lobby.up
        && duel_match
            .as_ref()
            .and_then(|d| d.m)
            .is_some_and(|x| x.phase == Phase::Over);
    let want = (playing_session && game.duel && !over)
        .then(|| m.loaded.as_ref().map(|l| l.0))
        .flatten()
        .filter(|c| Some(*c) == m.chosen);
    let Some(output) = &audio.output else { return };
    let Ok(mut mixer) = output.mixer.lock() else {
        return;
    };
    if want != m.playing {
        if m.playing.is_some() {
            mixer.fade(KEY, 0., FADE_OUT, audio::FadeEnd::Stop);
        }
        if let (Some(c), Some((_, track))) = (want, m.loaded.as_ref()) {
            mixer.stop(KEY);
            mixer.play(track.clip.clone(), KEY, 0., 1.);
            mixer.fade(KEY, BGM_LEVEL * track.volume, FADE_IN, audio::FadeEnd::Hold);
            // The room's own music steps aside for the duel's.
            if !m.room_paused {
                for k in MUSIC_KEYS {
                    mixer.fade(k, 0., FADE_IN, audio::FadeEnd::Pause);
                }
                m.room_paused = true;
            }
            game.event(
                serde_json::json!({"event":"duel_music","tick":tick,"track":c.1,
                "match":c.0,"playing":true}),
            );
        } else if m.playing.is_some() {
            game.event(serde_json::json!({"event":"duel_music","tick":tick,"playing":false}));
        }
        m.playing = want;
    }
    game.duel_music = m.room_paused;
    // The session over: the room's music comes back.
    if !playing_session && m.room_paused {
        if let Some(field) = room_music.as_ref().and_then(|r| r.clips[0].as_ref()) {
            mixer.resume(MUSIC_KEYS[0], BGM_LEVEL * field.volume, FADE_OUT);
        }
        m.room_paused = false;
        m.chosen = None;
        m.sent = None;
        game.duel_music = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn battle_tracks_are_the_rows_second_track_when_it_differs() {
        // One world, two areas: (10, 20) and (30, 30) in set 0, (10, 40)
        // in set 1 of area 1.
        let mut arif = vec![0u8; 8];
        arif[4] = 1;
        arif.extend([2, 0, 16, 0]);
        arif.resize(16 + 2 * 0x40, 0);
        let put = |a: &mut Vec<u8>, o: usize, v: u16| a[o..o + 2].copy_from_slice(&v.to_le_bytes());
        put(&mut arif, 16 + 0x10, 10);
        put(&mut arif, 16 + 0x12, 20);
        put(&mut arif, 16 + 0x40 + 0x10, 30);
        put(&mut arif, 16 + 0x40 + 0x12, 30);
        put(&mut arif, 16 + 0x40 + 0x14, 10);
        put(&mut arif, 16 + 0x40 + 0x16, 40);
        assert_eq!(battle_tracks(&arif), vec![20, 40]);
        assert_eq!(track_name(7), "bgm/music007.win32.scd");
    }
}
