//! Two unattended long runs for measuring a session over time.
//!
//! `--soak <empty-dir>` (`--soak-ticks`, `--soak-rooms w:a,...`,
//! `--soak-rooms-every`): the magic tour's first part (spells on the ground
//! and in the air, attacks into casts) repeats for the whole run, a Drive
//! form is taken and reverted in turn, and with a room list Sora walks
//! through those rooms.
//!
//! `--room-survey <empty-dir>` (`--survey-rooms w:a,...`, `--survey-ms`,
//! `--survey-settle`): every installed room in turn (or the rooms named),
//! standing at its first entrance with the camera turning, one `room_survey`
//! line per room with its frame times and what it holds.
use super::*;
use bevy::ecs::entity::Entities;
use player::forms;

/// The forms the soak takes in turn.
const FORMS: [u8; 5] = [
    forms::VALOR,
    forms::WISDOM,
    forms::LIMIT,
    forms::MASTER,
    forms::FINAL,
];
/// Ticks between form changes, and from one to its revert.
const FORM_EVERY: u64 = 1600;
const FORM_HOLD: u64 = 800;
/// The default soak: five minutes of field time.
pub const DEFAULT_TICKS: u64 = 18_000;
/// The default time between room changes (ticks).
pub const DEFAULT_ROOMS_EVERY: u64 = 3600;
/// What each surveyed room gets: ticks to settle, then real time measured.
pub const DEFAULT_SURVEY_SETTLE: u64 = 90;
pub const DEFAULT_SURVEY_MS: u64 = 4000;

/// Parses `w:a,w:a` room lists.
pub fn parse_rooms(text: &str) -> Option<Vec<(u8, u8)>> {
    text.split(',')
        .map(|r| {
            let (w, a) = r.split_once(':')?;
            Some((w.parse().ok()?, a.parse().ok()?))
        })
        .collect()
}

/// The soak's Drive request this tick: a form, or 0 to revert.
pub fn form_request(tick: u64) -> Option<u8> {
    let k = tick / FORM_EVERY;
    match tick % FORM_EVERY {
        0 if k > 0 => Some(FORMS[(k as usize - 1) % FORMS.len()]),
        FORM_HOLD if k > 0 => Some(0),
        _ => None,
    }
}

/// The soak: the next room on the list, every so often.
#[derive(Resource)]
pub struct SoakRooms {
    pub rooms: Vec<(u8, u8)>,
    pub every: u64,
    next: usize,
    last: u64,
}

impl SoakRooms {
    pub fn new(rooms: Vec<(u8, u8)>, every: u64) -> Self {
        SoakRooms {
            rooms,
            every,
            next: 0,
            last: 0,
        }
    }
}

fn start_warp(area: &mut AreaRes, target: (u8, u8)) {
    let target = (target.0, target.1, 0);
    area.transition = Some(Transition {
        stage: Stage::Closing,
        iris: None,
        fade: Fade::default(),
        target,
        job: start_area_job(&area.source, target),
    });
}

pub(crate) fn soak_rooms(
    mut soak: ResMut<SoakRooms>,
    game: Res<Game>,
    area: Option<ResMut<AreaRes>>,
) {
    let Some(mut area) = area else { return };
    if area.transition.is_some() || area.pending.is_some() || soak.rooms.is_empty() {
        return;
    }
    if game.tick < soak.last + soak.every {
        return;
    }
    soak.last = game.tick;
    let room = soak.rooms[soak.next % soak.rooms.len()];
    soak.next += 1;
    start_warp(&mut area, room);
}

/// The survey's place in its list and the room's frame times.
#[derive(Resource)]
pub struct Survey {
    pub rooms: Vec<(u8, u8)>,
    pub settle: u64,
    pub ms: u64,
    next: usize,
    /// The tick the room was ready, and the real time measuring began.
    ready: Option<u64>,
    started: Option<f64>,
    times: Vec<f64>,
    loads: u32,
}

impl Survey {
    pub fn new(rooms: Vec<(u8, u8)>, settle: u64, ms: u64) -> Self {
        Survey {
            rooms,
            settle,
            ms,
            next: 0,
            ready: None,
            started: None,
            times: Vec::with_capacity(4096),
            loads: 0,
        }
    }
}

/// Frame time statistics in ms: count, median, p95, p99, the 1% low as a
/// frame time (mean of the slowest 1%) and the worst.
pub(crate) fn frame_summary(times: &mut [f64]) -> serde_json::Value {
    if times.is_empty() {
        return serde_json::json!({"frames":0});
    }
    times.sort_by(f64::total_cmp);
    let at = |p: f64| times[((times.len() - 1) as f64 * p) as usize];
    let worst = times.len().div_ceil(100);
    let low = times[times.len() - worst..].iter().sum::<f64>() / worst as f64;
    serde_json::json!({"frames":times.len(),"median_ms":at(0.5),"p95_ms":at(0.95),
        "p99_ms":at(0.99),"low_1pct_ms":low,"worst_ms":times[times.len() - 1]})
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn survey(
    mut survey: ResMut<Survey>,
    mut game: ResMut<Game>,
    area: Option<ResMut<AreaRes>>,
    real: Res<Time<Real>>,
    entities: &Entities,
    mesh_entities: Query<(&Mesh3d, &InheritedVisibility)>,
    meshes: Res<Assets<Mesh>>,
    images: Res<Assets<Image>>,
    gallery: Res<enemy_gallery::EnemyGallery>,
    bobs: Query<(), With<BobRoot>>,
) {
    let Some(mut area) = area else { return };
    let survey = &mut *survey;
    // The list: every installed room, once the source can say.
    if survey.rooms.is_empty() && survey.next == 0 {
        survey.rooms = (0..area::WORLD_COUNT as u8)
            .flat_map(|w| {
                area.source
                    .rooms(w)
                    .iter()
                    .map(|&r| (w, r))
                    .collect::<Vec<_>>()
            })
            .collect();
        let count = survey.rooms.len();
        game.event(serde_json::json!({"event":"room_survey_list","rooms":count}));
    }
    if area.transition.is_some() || area.pending.is_some() {
        return;
    }
    let here = (area.current.world, area.current.area);
    let tick = game.tick;
    let ready = *survey.ready.get_or_insert(tick);
    if tick < ready + survey.settle {
        return;
    }
    let now = real.elapsed_secs_f64();
    let started = *survey.started.get_or_insert(now);
    if (now - started) * 1e3 < survey.ms as f64 {
        survey.times.push(real.delta_secs_f64() * 1e3);
        return;
    }
    // This room is done (the first room of a list is the one the run starts
    // in, measured only if it is on the list).
    let listed = survey.rooms.contains(&here);
    if listed {
        let times = survey.times.len();
        let summary = frame_summary(&mut survey.times);
        let (mut drawn, mut hidden, mut vertices) = (0usize, 0usize, 0usize);
        for (m, v) in &mesh_entities {
            if v.get() {
                drawn += 1;
                vertices += meshes.get(&m.0).map_or(0, Mesh::count_vertices);
            } else {
                hidden += 1;
            }
        }
        let code = area::world_code(here.0);
        game.event(
            serde_json::json!({"event":"room_survey","tick":tick,"world":here.0,
            "code":code,"area":here.1,"frames":times,"times":summary,
            "entities":entities.count_spawned(),"drawn_meshes":drawn,"hidden_meshes":hidden,
            "vertices":vertices,"meshes":meshes.len(),"images":images.len(),
            "npcs":area.current.npcs.len(),"bobs":bobs.iter().count(),
            "actors":gallery.actors.len(),"loads":survey.loads}),
        );
    }
    survey.times.clear();
    survey.ready = None;
    survey.started = None;
    // The next room, skipping the one Sora stands in.
    while survey.next < survey.rooms.len() && survey.rooms[survey.next] == here {
        survey.next += 1;
    }
    match survey.rooms.get(survey.next).copied() {
        Some(room) => {
            survey.next += 1;
            survey.loads += 1;
            start_warp(&mut area, room);
        }
        None => game.autoplay_end = tick,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rooms_parse() {
        assert_eq!(parse_rooms("2:5,4:12"), Some(vec![(2, 5), (4, 12)]));
        assert_eq!(parse_rooms("2"), None);
    }

    #[test]
    fn forms_come_in_turn_and_revert() {
        assert_eq!(form_request(0), None);
        assert_eq!(form_request(FORM_EVERY), Some(FORMS[0]));
        assert_eq!(form_request(FORM_EVERY + FORM_HOLD), Some(0));
        assert_eq!(form_request(FORM_EVERY * 2), Some(FORMS[1]));
    }

    #[test]
    fn the_summary_of_no_frames_says_so() {
        assert_eq!(frame_summary(&mut [])["frames"], 0);
        let mut times: Vec<f64> = (1..=100).map(f64::from).collect();
        let s = frame_summary(&mut times);
        assert_eq!(s["median_ms"], 50.);
        assert_eq!(s["worst_ms"], 100.);
        assert_eq!(s["low_1pct_ms"], 100.);
    }
}
