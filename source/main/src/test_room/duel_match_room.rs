//! The duel's match in the room. The host starts a match when the lobby's
//! players are both ready, steps it each tick (duel_match.rs), resets and
//! places the Soras for each round, and sends it on every change. Both
//! instances read it on the host's clock (the joiner on the snapshot it
//! shows), hold the Soras still outside the fight, play its sounds as they
//! see it change, and draw it: the countdown, the clock, each round's end
//! and the result: its title over the field and, in the game's pause menu,
//! Rematch, Leave and the match's numbers (duel_camp.rs). START during a
//! match opens Keep fighting / Forfeit there; Forfeit asks once more before
//! it gives the match away.

use super::duel_lobby::Stage;
use super::duel_lobby_room::LobbyRoom;
use super::duel_match::{COUNT_STEP, GO_SHOWN, Match, Phase, View, Why, count_at};
use super::duel_net::{DuelNet, Status, fighter_slots};
use super::duel_wire::{FORFEIT, MATCH, REMATCH};
use super::*;
use net::wire::Writer;
use net::{RELIABLE, Role};

/// The game's own cues, the Olympus Coliseum cups' where it has them
/// (private trace `duel-cues-01`): the timer's beep (dk::TIMER,
/// FUN_14018d830) for each count and the last seconds of a timed match; the
/// cup match's start stinger (se007 #13, a cup mission's start sound) with
/// the intro; a cup round won with rounds left (se007 #16, FUN_1403a35c0
/// state 3); a cup match won (se000 #53, the cups' complete sound); the
/// game over a lost cup match ends in (se000 0x13, FUN_1403fc7a0); the
/// minigames' finish (se000 #44) for a draw. GO keeps the menus' decide
/// [duel choice: the minigames' start #47 carries its go in a layer the
/// room's player would sound together with its beeps].
const SE_BEEP: u32 = 43;
const SE_START: u32 = 7013;
const SE_ROUND: u32 = 7016;
const SE_GO: u32 = 2;
const SE_WON: u32 = 53;
const SE_LOST: u32 = 0x13;
const SE_DRAW: u32 = 44;
/// The menus' cursor and decide on the result screen.
const SE_CURSOR: u32 = 1;
/// The cups' bank, read once when a duel first needs it.
const CUP_BANK: &str = "se/se007.win32.scd";
/// System effect elements: a flash on both Soras at GO (the item-use
/// element at the user), the level-up sparkle on the winner (element 16).
const FX_GO: u16 = 0;
const FX_WON: u16 = 16;
/// The intro shot runs through 3 and 2 of a match's first countdown; the
/// result's shot eases in over two and a half seconds.
const INTRO_TICKS: u64 = 2 * COUNT_STEP;
const RESULT_SHOT: u64 = 150;
/// [Duel choice] The result's title stays over the field, with the
/// winner's shot, this long before the result's menu opens; it fades over
/// the last TITLE_FADE ticks.
const RESULT_HOLD: u64 = 90;

/// How long the title holds: a match that ended without a round's end (a
/// player left or forfeited) starts its banner at the result, so it holds
/// a second longer for the banner's entrance.
fn hold(m: &Match) -> u64 {
    match m.why {
        Some(Why::Left | Why::Forfeit) => RESULT_HOLD + 60,
        _ => RESULT_HOLD,
    }
}
const TITLE_FADE: f32 = 10.;
/// Logical pad bits of mode 7.
const UP: u64 = 0x10;
const DOWN: u64 = 0x20;
const DECIDE: u64 = 0x4;
/// The result's entries in the pause menu.
const ENTRY_REMATCH: u8 = pause_menu::CALLER_ENTRY;
const ENTRY_LEAVE: u8 = pause_menu::CALLER_ENTRY + 1;
/// The Forfeit menu's entries.
const ENTRY_KEEP: u8 = pause_menu::CALLER_ENTRY + 2;
const ENTRY_FORFEIT: u8 = pause_menu::CALLER_ENTRY + 3;
/// The result's Quick rematch: the same builds and bans, straight to the
/// countdown [duel choice; a plain Rematch from either player goes back to
/// the lobby].
const ENTRY_QUICK: u8 = pause_menu::CALLER_ENTRY + 4;

#[derive(Resource, Default)]
pub struct MatchRoom {
    pub m: Option<Match>,
    /// Host: the match the joiner has.
    sent: Option<Match>,
    /// What this instance last played sounds for: (phase, since, count).
    heard: Option<(Phase, u64, Option<u8>)>,
    /// The result screen's cursor: 0 Rematch, 1 Leave.
    cursor: usize,
    /// The seconds left a timed fight last beeped at.
    beeped: Option<u64>,
    /// The cups' bank: read on its own thread once a duel's lobby opens
    /// (it is larger than a frame can wait for), registered when read.
    bank_job: Option<std::thread::JoinHandle<Result<Vec<u8>, String>>>,
    bank_tried: bool,
    /// The Forfeit menu is wanted, and Forfeit was picked once (the next
    /// pick gives the match away).
    forfeit_menu: bool,
    forfeit_armed: bool,
    /// [Test] `--duel-forfeit` has forfeited.
    forfeit_done: bool,
    /// The phase, round and start the last cleanup looked at.
    swept: Option<(Phase, u8, u64)>,
    /// The local tick the result's Results page came up (its numbers count
    /// up from there).
    results_at: Option<u64>,
    /// The Struggle's 2D sheets (`MatchArt`), read on their own thread once
    /// a duel's lobby opens.
    art_job: Option<std::thread::JoinHandle<Result<MatchArt, String>>>,
    pub art: Option<Result<MatchArt, String>>,
}

/// [Duel choice] The match's numbers and titles are the Struggle's, the
/// game's own one-on-one match (msn/us/TT05_MS401.bar, the Struggle
/// tournament): its timer sheet `ti_t` (dk::TIMER's readout and its 3, 2,
/// 1, 0), its start `st_t` ("STRUGGLE!") and its end `ed_t` ("YOU WIN!" /
/// "YOU LOSE" on their bar). Each is drawn as the game authored it.
pub struct MatchArt {
    pub timer: Sheet,
    pub start: Sheet,
    pub end: Sheet,
}

const ART_FILE: &str = "msn/us/TT05_MS401.bar";
/// The start's "STRUGGLE!" as it bursts and fades (45 frames, GO's time).
const START_GO: usize = 6;
/// The end's bar, "YOU WIN!" and "YOU LOSE".
const END_BAR: usize = 3;
const END_WIN: usize = 1;
const END_LOSE: usize = 2;

fn load_art(source: &data::probe_source::ProbeSource) -> Result<MatchArt, String> {
    let library = data::path_lookup::Library::open(source).map_err(|e| e.to_string())?;
    let f = library
        .find(ART_FILE)
        .ok_or(format!("{ART_FILE} is not installed"))?;
    let sheet = |name: &[u8; 4]| {
        hud::load_nth(source, f.index, f.ordinal, f.name_hash, name, Some(0))
            .map_err(|e| e.to_string())
    };
    Ok(MatchArt {
        timer: sheet(b"ti_t")?,
        start: sheet(b"st_t")?,
        end: sheet(b"ed_t")?,
    })
}

/// Which of the art's sheets a sprite draws from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArtSheet {
    Timer = 0,
    Start = 1,
    End = 2,
}

/// [Duel choice] The Results page's numbers: each row's value comes in
/// REVEAL_STEP ticks after the one above and counts up over COUNT_TICKS.
const REVEAL_FIRST: f32 = 8.;
const REVEAL_STEP: f32 = 10.;
const COUNT_TICKS: f32 = 30.;

/// How far row `i`'s count has got `elapsed` ticks in (None: all of it).
fn counted(elapsed: Option<f32>, i: usize) -> Option<f32> {
    let Some(e) = elapsed else {
        return Some(1.);
    };
    let k = (e - REVEAL_FIRST - REVEAL_STEP * i as f32) / COUNT_TICKS;
    (k >= 0.).then(|| 1. - (1. - k.min(1.)).powi(3))
}

impl MatchRoom {
    /// The result screen holds the pad.
    pub fn menu_up(&self) -> bool {
        self.m.is_some_and(|m| m.phase == Phase::Over)
    }
}

/// The host's clock as this instance sees it.
fn now(net: &DuelNet, game: &Game) -> u64 {
    match net.role {
        Role::Host => game.tick,
        Role::Join => net.shown_snapshot.unwrap_or(0),
    }
}

/// A Sora back to full for a round: HP, MP, the Drive gauge, out of any
/// form, standing.
fn refill(game: &mut Game) {
    let v = &mut game.sora.vitals;
    v.hp = v.target.max_hp;
    v.ko_ticks = 0;
    v.invulnerable = 0.;
    let p = &mut game.sora.player;
    if p.dead() {
        p.revive();
    }
    p.mp = magic::Mp::new(p.mp.max);
    p.mp_charging = false;
    p.drive.refill();
    drive_room::end_form(game);
}

/// The cleanup a change of the match's phase calls for.
fn round_change(
    before: Option<(Phase, u8, u64)>,
    now: Option<(Phase, u8, u64)>,
) -> Option<cleanup::Scope> {
    use cleanup::Scope;
    if before == now {
        return None;
    }
    match now {
        Some((Phase::RoundOver, ..)) => Some(Scope::RoundEnd),
        Some((Phase::Over, ..)) => Some(Scope::MatchEnd),
        Some((Phase::Countdown, ..)) => Some(Scope::RoundStart),
        Some((Phase::Fight, ..)) => None,
        None => Some(Scope::MatchEnd),
    }
}

/// Host: both Soras refilled and at the arena's starts.
fn reset_round(game: &mut Game) {
    cleanup::each_sora(game, refill);
    duel_arena::place(game);
}

/// One fixed step of the match.
#[allow(clippy::too_many_arguments)]
pub fn tick(
    net: Option<ResMut<DuelNet>>,
    lobby: Option<ResMut<LobbyRoom>>,
    room: Option<ResMut<MatchRoom>>,
    mut rig: ResMut<CameraRig>,
    mut exits: MessageWriter<AppExit>,
    mut audio: ResMut<Audio>,
    gallery: Res<enemy_gallery::EnemyGallery>,
    mut camp: Option<ResMut<CampRes>>,
    mut game: ResMut<Game>,
) {
    let (Some(mut net), Some(mut lobby), Some(mut room)) = (net, lobby, room) else {
        return;
    };
    let (net, lobby, room, game) = (&mut *net, &mut *lobby, &mut *room, &mut *game);
    // The cups' sounds: their bank, read from the install once a duel runs.
    if !room.bank_tried && lobby.up {
        room.bank_tried = true;
        let source = Arc::clone(&gallery.source);
        room.bank_job = Some(std::thread::spawn(move || {
            data::path_lookup::Library::open(&source)
                .and_then(|mut l| l.read_limited(CUP_BANK, SOUND_BANK_LIMIT))
                .map_err(|e| e.to_string())
        }));
    }
    if room.bank_job.as_ref().is_some_and(|j| j.is_finished())
        && let Some(job) = room.bank_job.take()
    {
        let bank = match job.join() {
            Ok(Ok(bytes)) => audio.banks.insert(bytes).map_err(|e| e.to_string()),
            Ok(Err(e)) => Err(e),
            Err(_) => Err("the reading thread stopped".into()),
        };
        game.event(serde_json::json!({"event":"duel_bank","file":CUP_BANK,
            "bank":bank.as_ref().ok(),"error":bank.as_ref().err()}));
    }
    if room.art_job.is_none() && room.art.is_none() && lobby.up {
        let source = Arc::clone(&gallery.source);
        room.art_job = Some(std::thread::spawn(move || load_art(&source)));
    }
    if room.art_job.as_ref().is_some_and(|j| j.is_finished())
        && let Some(job) = room.art_job.take()
    {
        let art = job
            .join()
            .unwrap_or_else(|_| Err("the reading thread stopped".into()));
        game.event(serde_json::json!({"event":"duel_art","file":ART_FILE,
            "error":art.as_ref().err()}));
        room.art = Some(art);
    }
    let playing = matches!(net.status, Status::Playing);
    // A session that ended keeps its result on screen; a new one starts
    // with no match.
    if !playing && !matches!(net.status, Status::Ended(..)) {
        if room.swept.is_some() {
            game.cleanups.push(cleanup::Scope::MatchEnd);
        }
        *room = MatchRoom {
            art: room.art.take(),
            art_job: room.art_job.take(),
            bank_tried: room.bank_tried,
            ..MatchRoom::default()
        };
        game.ext.online_mut().duel_match = false;
        return;
    }
    let host = net.role == Role::Host;
    let seat = usize::from(!host);
    for m in std::mem::take(&mut net.inbox) {
        match (net.role, m.first().copied()) {
            (Role::Host, Some(FORFEIT)) => {
                let tick = game.tick;
                if let Some(mm) = room.m.as_mut()
                    && mm.forfeit(1, tick)
                {
                    game.event(serde_json::json!({"event":"duel_forfeit","tick":tick,"seat":1}));
                }
            }
            (Role::Host, Some(REMATCH)) => {
                if let Some(mm) = room.m.as_mut()
                    && mm.phase == Phase::Over
                {
                    mm.rematch[1] = m.get(1).is_some_and(|&b| b != 0);
                    mm.quick[1] = m.get(1) == Some(&2);
                }
            }
            (Role::Join, Some(MATCH)) => {
                if let Some(mm) = duel_match::decode_match(MATCH, &m) {
                    room.m = Some(mm);
                }
            }
            _ => net.inbox.push(m),
        }
    }
    let now = now(net, game);
    if host {
        // The match starts when the lobby's players are both ready.
        if playing && lobby.lobby.stage == Stage::Start && room.m.is_none() {
            room.m = Some(Match::new(now));
            game.ext.online_mut().duel_dealt = [0; 2];
            game.ext.online_mut().duel_landed = [0; 2];
            reset_round(game);
            game.event(serde_json::json!({"event":"duel_match","tick":now,
                "mode":game.ext.online().duel_rules.info().name,"match":lobby.lobby.match_no}));
        }
        if let Some(m) = room.m.as_mut() {
            let rival = game.rivals.iter().find(|r| r.slot == fighter_slots::JOIN);
            let view = View {
                hp: [
                    game.sora.vitals.hp,
                    rival.map_or(0, |r| r.fighter.vitals.hp),
                ],
                max_hp: [
                    game.sora.vitals.target.max_hp,
                    rival.map_or(1, |r| r.fighter.vitals.target.max_hp),
                ],
                here: [true, playing && rival.is_some()],
            };
            m.dealt = game.ext.online().duel_dealt;
            m.hits = game.ext.online().duel_landed;
            let rules = game.ext.online().duel_rules;
            for cue in m.step(now, &rules, &view) {
                match cue {
                    duel_match::Cue::NextRound => {
                        reset_round(game);
                        rig.recenter_queued = true;
                    }
                    duel_match::Cue::RoundOver | duel_match::Cue::Over => {
                        game.event(serde_json::json!({"event":"duel_round","tick":now,
                            "phase":format!("{:?}", m.phase),"round":m.round,"wins":m.wins,
                            "winner":m.winner,"why":m.why.map(|w| format!("{w:?}")),
                            "dealt":m.dealt,"hits":m.hits}));
                    }
                    _ => {}
                }
            }
            // Both asked for a rematch: back to the lobby (new bans), or,
            // both quick, the next match at once with the same builds.
            if m.phase == Phase::Over && m.rematch == [true, true] && playing {
                let quick = m.quick == [true, true];
                if quick {
                    lobby.lobby.match_no += 1;
                } else {
                    lobby.lobby.next_match();
                }
                room.m = None;
                reset_round(game);
                game.event(serde_json::json!({"event":"duel_rematch","tick":now,"quick":quick}));
            }
        }
        if playing && room.sent != room.m {
            room.sent = room.m;
            if let Some(m) = &room.m {
                net.session
                    .send(RELIABLE, duel_match::encode_match(MATCH, m));
            }
        }
    }
    // The other player gone mid-match: on the joiner the host's leaving is
    // its win (the host's own step sees the joiner gone).
    if !host
        && !playing
        && let Some(m) = room.m.as_mut()
        && m.phase != Phase::Over
    {
        m.winner = Some(1);
        m.why = Some(Why::Left);
        m.phase = Phase::Over;
        m.since = now;
    }
    // What a round leaves ends with it, on both instances.
    let seen = room.m.map(|m| (m.phase, m.round, m.began));
    if let Some(scope) = round_change(room.swept, seen) {
        game.cleanups.push(scope);
    }
    room.swept = seen;
    game.ext.online_mut().duel_match = room.m.is_some();
    game.ext.online_mut().duel_frozen = lobby.up || room.m.is_some_and(|m| !m.fighting());
    sounds(room, seat, now, game);
    // [Test] `--duel-shots`: the countdown's names and numeral, and GO.
    if let Some(m) = room.m
        && matches!(
            (m.phase, now.saturating_sub(m.since)),
            (Phase::Countdown, 100) | (Phase::Fight, 3 | 20) | (Phase::RoundOver, 4)
        )
    {
        duel_camp::shot(game, (2, m.since, now.saturating_sub(m.since)));
    }
    let fov = rig.camera.vertical_fov();
    game.ext.online_mut().duel_camera = shot(room.m.as_ref(), seat, now, game, fov);
    forfeit_menu(room, net, camp.as_deref_mut(), lobby.up, now, game);
    // The result screen: Rematch or Leave, in the game's pause menu when
    // its art is loaded (else on the labels, with the pad).
    let over = room.m.is_some_and(|m| m.phase == Phase::Over);
    let mut pick = None;
    if let Some(camp) = camp.as_deref_mut() {
        let want = room
            .m
            .filter(|m| {
                over && !game.ext.online().duel_vote_up && now.saturating_sub(m.since) >= hold(m)
            })
            .map(|m| m.began as u32);
        // The Results page opens with its numbers still to count.
        let text = room.m.map(|m| result_text(&m, seat, net, game, Some(0.)));
        let them = net.peer.clone().unwrap_or_else(|| "Opponent".into());
        let menu = duel_camp::hold(camp, game, duel_camp::RESULT, want, || {
            let (pages, help) = text.clone().unwrap_or_default();
            let mut entries = Vec::new();
            if playing {
                entries.push(duel_camp::entry(ENTRY_REMATCH, "Rematch", &help));
                entries.push(duel_camp::entry(
                    ENTRY_QUICK,
                    "Quick rematch",
                    "The same builds and bans, straight to the countdown.",
                ));
            }
            entries.push(duel_camp::entry(
                ENTRY_LEAVE,
                "Leave",
                "Back to the Multiplayer screen.",
            ));
            entries.push(duel_camp::entry(7, "Results", "The match's numbers."));
            duel_camp::Request {
                entries,
                on_config: false,
                pages,
            }
        });
        if let (Some(menu), Some(m)) = (menu, room.m) {
            // The Results page: once it has opened, its numbers count up,
            // one row after another.
            if menu.state == pause_menu::State::Config && menu.substate == 1 {
                let at = *room.results_at.get_or_insert(game.tick);
                let e = game.tick.saturating_sub(at) as f32;
                // [Test] `--duel-shots`: the numbers mid-count.
                if e == 30. {
                    duel_camp::shot(game, (5, m.since, 30));
                }
                let (pages, _) = result_text(&m, seat, net, game, Some(e));
                if let Some(d) = menu.data.duel.as_mut()
                    && d.rows != pages.rows
                {
                    d.rows = pages.rows;
                }
            } else if menu.state != pause_menu::State::Config {
                room.results_at = None;
            }
            let (_, help) = text.unwrap_or_default();
            let wants = |s: usize| {
                if m.quick[s] {
                    "a quick rematch"
                } else {
                    "a rematch"
                }
            };
            let status = |base: &str| {
                if m.rematch[seat] {
                    format!("{base} You asked for {}; waiting for {them}.", wants(seat))
                } else if m.rematch[1 - seat] {
                    format!("{base} {them} wants {}.", wants(1 - seat))
                } else {
                    base.to_string()
                }
            };
            duel_camp::set_help(menu, ENTRY_REMATCH, &status(&help));
            duel_camp::set_help(
                menu,
                ENTRY_QUICK,
                &status("The same builds and bans, straight to the countdown."),
            );
            pick = duel_camp::picked(menu);
        }
    }
    let Some(m) = room.m.as_mut() else { return };
    if m.phase != Phase::Over {
        return;
    }
    if game.autoplay.is_some() {
        // [Test] Unattended: both ask for a rematch two seconds in. On the
        // way the title and the result's menu are captured (`--duel-shots`),
        // then the cursor goes down to Results and opens it, and that is
        // captured too.
        let t = now.saturating_sub(m.since);
        let rematch_at = if game.ext.online().duel_shots {
            240 + RESULT_HOLD
        } else {
            120
        };
        if playing && t >= rematch_at && !m.rematch[seat] {
            pick = Some(if game.ext.online().duel_quick {
                ENTRY_QUICK
            } else {
                ENTRY_REMATCH
            });
        }
        if game.ext.online().duel_shots {
            if t == 40 || t == 150 + RESULT_HOLD {
                duel_camp::shot(game, (3, m.since, t));
            }
            let on = camp.as_deref().and_then(|c| c.open.as_ref()).filter(|o| {
                o.data
                    .duel
                    .as_ref()
                    .is_some_and(|d| d.owner.0 == duel_camp::RESULT)
            });
            if let Some(o) = on
                && (70 + RESULT_HOLD..104 + RESULT_HOLD).contains(&t)
                && t.is_multiple_of(6)
                && o.state == pause_menu::State::Top
            {
                game.camp_press = if o.top_entry() == Some(7) { 0x4 } else { 0x20 };
            }
        }
    } else if camp.is_none() {
        let pressed = game.pad.pressed | game.pad.repeat & (UP | DOWN);
        if pressed & (UP | DOWN) != 0 {
            room.cursor = 1 - room.cursor;
            game.sound_queue.push((SE_CURSOR, "result"));
        }
        if pressed & DECIDE != 0 {
            game.sound_queue.push((SE_GO, "result"));
            pick = Some(if room.cursor == 1 || !playing {
                ENTRY_LEAVE
            } else {
                ENTRY_REMATCH
            });
        }
    }
    match pick {
        Some(ENTRY_LEAVE) => {
            // Leave: the duel closes and the launcher comes back on its
            // Multiplayer screen; the other player sees "left".
            exits.write(AppExit::Success);
        }
        Some(e @ (ENTRY_REMATCH | ENTRY_QUICK)) if playing => {
            // The same choice again takes it back; the other one changes it.
            let quick = e == ENTRY_QUICK;
            let want = !(m.rematch[seat] && m.quick[seat] == quick);
            m.rematch[seat] = want;
            m.quick[seat] = want && quick;
            if !host {
                let mut w = Writer::default();
                w.u8(REMATCH);
                w.u8(if !want {
                    0
                } else if quick {
                    2
                } else {
                    1
                });
                net.session.send(RELIABLE, w.finish());
            }
        }
        _ => {}
    }
}

/// The Forfeit menu: START during a match opens it (the fight goes on
/// meanwhile [duel rule: nothing pauses a duel]); Cancel, START or Keep
/// fighting close it; Forfeit asks once more, then gives the match to the
/// other player.
fn forfeit_menu(
    room: &mut MatchRoom,
    net: &mut DuelNet,
    camp: Option<&mut CampRes>,
    lobby_up: bool,
    now: u64,
    game: &mut Game,
) {
    let playing = matches!(net.status, Status::Playing);
    let running = playing && !lobby_up && room.m.is_some_and(|m| m.phase != Phase::Over);
    if std::mem::take(&mut game.start_pressed) && running && !room.forfeit_menu {
        room.forfeit_menu = true;
        room.forfeit_armed = false;
    }
    if !running {
        room.forfeit_menu = false;
    }
    // [Test] `--duel-forfeit`: START two seconds into the first fight, down
    // to Forfeit, and Forfeit twice.
    if game.ext.online().duel_forfeit
        && game.autoplay.is_some()
        && !room.forfeit_done
        && let Some(m) = room.m.filter(|m| m.phase == Phase::Fight)
    {
        let t = now.saturating_sub(m.since);
        game.camp_press = match t {
            120 => 0x2,
            150 => 0x20,
            160 | 190 => 0x4,
            _ => 0,
        };
        if t == 175 {
            duel_camp::shot(game, (4, m.since, t));
        }
    }
    let Some(camp) = camp else {
        return;
    };
    let them = net
        .peer
        .clone()
        .unwrap_or_else(|| "the other player".into());
    let want = room
        .m
        .filter(|_| room.forfeit_menu && !game.ext.online().duel_vote_up)
        .map(|m| m.began as u32);
    let menu = duel_camp::hold(camp, game, duel_camp::FORFEIT, want, || {
        duel_camp::Request {
            entries: vec![
                duel_camp::entry(ENTRY_KEEP, "Keep fighting", "Back to the fight."),
                duel_camp::entry(
                    ENTRY_FORFEIT,
                    "Forfeit",
                    &format!("Give the match to {them}."),
                ),
            ],
            on_config: false,
            pages: pause_menu::DuelPages::default(),
        }
    });
    let Some(menu) = menu else {
        return;
    };
    let close = menu.data.duel.as_mut().is_some_and(|d| {
        let c = std::mem::take(&mut d.cancel);
        std::mem::take(&mut d.start) || c
    });
    match duel_camp::picked(menu) {
        Some(ENTRY_FORFEIT) if room.forfeit_armed => {
            room.forfeit_menu = false;
            room.forfeit_done = true;
            if net.role == Role::Host {
                if let Some(m) = room.m.as_mut()
                    && m.forfeit(0, now)
                {
                    game.event(serde_json::json!({"event":"duel_forfeit","tick":now,"seat":0}));
                }
            } else {
                net.session.send(RELIABLE, vec![FORFEIT]);
                game.event(serde_json::json!({"event":"duel_forfeit_sent","tick":now}));
            }
        }
        Some(ENTRY_FORFEIT) => {
            room.forfeit_armed = true;
            duel_camp::set_help(
                menu,
                ENTRY_FORFEIT,
                &format!("Sure? Pick Forfeit again to give {them} the match."),
            );
        }
        Some(ENTRY_KEEP) => room.forfeit_menu = false,
        _ if close => room.forfeit_menu = false,
        _ => {}
    }
}

/// [Duel choice] A flash over the field at GO (faint gold) and at a
/// knock-out (white), fading over a few frames by the host's clock; `frac`
/// is the part of a tick since it, so it fades as smoothly at any frame
/// rate.
pub fn flash(room: &MatchRoom, net: &DuelNet, game: &Game, frac: f32) -> Option<[f32; 4]> {
    let m = room.m?;
    let t = now(net, game).saturating_sub(m.since) as f32 + frac;
    let (colour, peak, ticks) = match m.phase {
        Phase::Fight => ([2., 1.75, 0.9], 0.35, 12.),
        Phase::RoundOver if m.why == Some(Why::KnockOut) => ([2., 2., 2.], 0.6, 20.),
        _ => return None,
    };
    (t < ticks).then(|| {
        let k = 1. - t / ticks;
        [colour[0], colour[1], colour[2], peak * k * k]
    })
}

/// The result's title, its colour and why the match ended.
fn result_head(m: &Match, seat: usize, net: &DuelNet) -> (&'static str, [f32; 4], String) {
    let me = net.name.as_str();
    let them = net.peer.as_deref().unwrap_or("Opponent");
    let name = |s: usize| if s == seat { me } else { them };
    let (title, colour) = match m.winner {
        Some(w) if usize::from(w) == seat => ("VICTORY", GOLD),
        Some(_) => ("DEFEAT", RED),
        None => ("DRAW", WHITE),
    };
    let why = match (m.why, m.winner) {
        (Some(Why::Left), _) => format!("{them} left the duel"),
        (Some(Why::Forfeit), Some(w)) => format!("{} forfeited", name(1 - usize::from(w))),
        (Some(Why::Time), _) => "Time up: the larger share of HP left".to_string(),
        (_, Some(w)) => format!("{} wins by knock-out", name(usize::from(w))),
        _ => "Both knocked out".to_string(),
    };
    (title, colour, why)
}

/// The result's numbers: (label, this player's - the other's).
/// `elapsed`: ticks since the numbers came up, for their count (None: as
/// they are).
fn result_rows(
    m: &Match,
    seat: usize,
    game: &Game,
    elapsed: Option<f32>,
) -> Vec<(&'static str, String)> {
    let mut rows = Vec::new();
    let pair = |i: usize, [a, b]: [u32; 2]| {
        counted(elapsed, i).map_or(String::new(), |k| {
            let n = |v: u32| (v as f32 * k).round() as u32;
            format!("{} - {}", n(a), n(b))
        })
    };
    if duel_match::wins_needed(&game.ext.online().duel_rules) > 1 {
        let wins = [m.wins[seat], m.wins[1 - seat]].map(u32::from);
        rows.push(("Rounds", pair(rows.len(), wins)));
    }
    let time = m.since.saturating_sub(m.began) as u32;
    rows.push((
        "Match time",
        counted(elapsed, rows.len()).map_or(String::new(), |k| {
            duel_rules::clock((time as f32 * k).round() as u32)
        }),
    ));
    rows.push((
        "Damage dealt",
        pair(rows.len(), [m.dealt[seat], m.dealt[1 - seat]]),
    ));
    let hits = [m.hits[seat], m.hits[1 - seat]].map(u32::from);
    rows.push(("Hits landed", pair(rows.len(), hits)));
    rows
}

/// The result in the pause menu: its Results page and Rematch's help.
fn result_text(
    m: &Match,
    seat: usize,
    net: &DuelNet,
    game: &Game,
    elapsed: Option<f32>,
) -> (pause_menu::DuelPages, String) {
    let (title, _, why) = result_head(m, seat, net);
    let mut rows = vec![duel_camp::info_row(
        "Result",
        &format!("{}{}", &title[..1], title[1..].to_lowercase()),
        &format!("{why}."),
    )];
    let note = "Yours first, then the other player's.";
    rows.extend(
        result_rows(m, seat, game, elapsed)
            .into_iter()
            .map(|(label, value)| duel_camp::info_row(label, &value, note)),
    );
    let pages = pause_menu::DuelPages {
        names: vec![pause_menu::encode_text("Results")],
        rows: vec![rows],
        ..pause_menu::DuelPages::default()
    };
    (
        pages,
        format!("{why}. Play again: new bans, the same rules."),
    )
}

/// The match's sounds, played as this instance sees the match change.
fn sounds(room: &mut MatchRoom, seat: usize, now: u64, game: &mut Game) {
    let Some(m) = room.m else {
        room.heard = None;
        return;
    };
    let count = (m.phase == Phase::Countdown)
        .then(|| count_at(now.saturating_sub(m.since)))
        .flatten();
    // The timer's beep with its 3, 2 and 1 (FUN_14018d830: 2, 1 and 0
    // whole seconds left).
    if m.phase == Phase::Fight
        && let Some(end) = m.clock_end
    {
        let left = end.saturating_sub(now);
        let whole = left / 60;
        if left > 0 && whole <= 2 && room.beeped != Some(whole) {
            room.beeped = Some(whole);
            game.sound_queue.push((SE_BEEP, "match"));
        }
    } else {
        room.beeped = None;
    }
    let key = (m.phase, m.since, count);
    if room.heard == Some(key) {
        return;
    }
    let fresh = room
        .heard
        .is_none_or(|(p, s, _)| (p, s) != (m.phase, m.since));
    room.heard = Some(key);
    // The match's start stinger with the first countdown's intro.
    if fresh && m.phase == Phase::Countdown && m.round == 1 {
        game.sound_queue.push((SE_START, "match"));
    }
    let need = duel_match::wins_needed(&game.ext.online().duel_rules);
    let sound = match m.phase {
        Phase::Countdown => count.map(|_| SE_BEEP),
        Phase::Fight => fresh.then_some(SE_GO),
        // A round's end while the match goes on (the last round's end is
        // the result's).
        Phase::RoundOver => (fresh && m.wins.iter().all(|&w| w < need)).then_some(SE_ROUND),
        Phase::Over => fresh.then(|| match m.winner {
            Some(w) if usize::from(w) == seat => SE_WON,
            Some(_) => SE_LOST,
            None => SE_DRAW,
        }),
    };
    if let Some(id) = sound {
        game.sound_queue.push((id, "match"));
    }
    // The game's own system effects: a flash on both at GO, the sparkle on
    // the winner.
    if fresh {
        let mine = game.sora.player.position;
        let theirs = game.rivals.first().map(|r| r.fighter.player.position);
        let at = |p: [f32; 3]| Vec3::new(p[0], p[1] + 90., p[2]);
        match (m.phase, m.winner) {
            (Phase::Fight, _) => {
                for p in std::iter::once(mine).chain(theirs) {
                    game.effect_queue.push(EffectRequest::System {
                        id: FX_GO,
                        at: at(p),
                    });
                }
            }
            (Phase::Over, Some(w)) => {
                let p = if usize::from(w) == seat {
                    Some(mine)
                } else {
                    theirs
                };
                if let Some(p) = p {
                    game.effect_queue.push(EffectRequest::System {
                        id: FX_WON,
                        at: at(p),
                    });
                }
            }
            _ => {}
        }
    }
}

/// The match's camera shots, worked out from the host's clock: both Soras
/// from the side, orbiting slowly, through 3 and 2 of round 1; at the
/// result the winner's camera pushes in on him from the front, the loser's
/// rises slowly behind his own Sora.
fn shot(
    m: Option<&Match>,
    seat: usize,
    now: u64,
    game: &Game,
    fov: f32,
) -> Option<boss::OverlayView> {
    let m = m?;
    let t = now.saturating_sub(m.since);
    let mine = Vec3::from_array(game.sora.player.position);
    let theirs = game
        .rivals
        .first()
        .map(|r| Vec3::from_array(r.fighter.player.position))?;
    let ease = |k: f32| 1. - (1. - k.clamp(0., 1.)).powi(3);
    let view = |eye: Vec3, target: Vec3| boss::OverlayView {
        eye,
        target,
        roll: 0.,
        fov,
        collision: true,
    };
    match m.phase {
        Phase::Countdown if m.round == 1 && t < INTRO_TICKS => {
            let k = ease(t as f32 / INTRO_TICKS as f32);
            let mid = (mine + theirs) / 2. + Vec3::Y * 90.;
            let span = (theirs - mine) * Vec3::new(1., 0., 1.);
            let side = Vec3::new(-span.z, 0., span.x).normalize_or(Vec3::X);
            let turn = Quat::from_rotation_y(0.35 * k);
            let eye = mid + turn * side * (span.length() * 0.9 + 250. - 120. * k) + Vec3::Y * 80.;
            Some(view(eye, mid))
        }
        Phase::Over => {
            let k = ease(t as f32 / RESULT_SHOT as f32);
            let won = m.winner.is_some_and(|w| usize::from(w) == seat);
            let me = &game.sora.player;
            let forward = Vec3::new(me.yaw.sin(), 0., me.yaw.cos());
            let chest = mine + Vec3::Y * 100.;
            if won {
                let eye = chest + forward * (520. - 300. * k) + Vec3::Y * (60. - 30. * k);
                Some(view(eye, chest))
            } else {
                let eye = chest - forward * 380. + Vec3::Y * (80. + 120. * k);
                Some(view(eye, mine + Vec3::Y * 60.))
            }
        }
        _ => None,
    }
}

/// A line of the match's display: text, centre, size, colour.
pub struct Label {
    pub text: Vec<u8>,
    pub centre: [f32; 2],
    pub size: f32,
    pub colour: [f32; 4],
    /// Drawn on a dark backing.
    pub backed: bool,
}

const WHITE: [f32; 4] = [1.6, 1.6, 1.6, 1.];
const GOLD: [f32; 4] = [1.9, 1.6, 0.7, 1.];
const RED: [f32; 4] = [1.8, 0.6, 0.5, 1.];

/// [Duel choice] A title's entrance: it settles to its size over
/// TITLE_IN ticks (200 ms), fast first (a strong ease-out).
const TITLE_IN: f32 = 12.;

fn settle(t: u64) -> f32 {
    1. - (1. - (t as f32 / TITLE_IN).min(1.)).powi(3)
}

fn label(text: &str, centre: [f32; 2], size: f32, colour: [f32; 4]) -> Label {
    Label {
        text: pause_menu::encode_text(text),
        centre,
        size,
        colour,
        backed: false,
    }
}

/// The game's sprites of the match this frame (worked out from the host's
/// clock): the countdown's numerals, "STRUGGLE!" at the start, the clock of
/// a timed fight, "YOU WIN!" / "YOU LOSE" at a round's end and the result.
pub fn sprites(
    room: &MatchRoom,
    net: &DuelNet,
    game: &Game,
    in_camp: bool,
) -> Vec<(ArtSheet, Vec<data::sprite_sequence::Quad>)> {
    use visual::mission_timer as timer;
    let (Some(m), Some(Ok(art))) = (room.m, room.art.as_ref()) else {
        return Vec::new();
    };
    let now = now(net, game);
    let t = now.saturating_sub(m.since) as u32;
    let seat = usize::from(net.role == Role::Join);
    let mut out = Vec::new();
    let mut put = |sheet: ArtSheet, quads: Vec<data::sprite_sequence::Quad>| {
        if !quads.is_empty() {
            out.push((sheet, quads));
        }
    };
    let ts = &art.timer.sequence;
    let numeral = |n: usize, age: u32| timer::group_quads(ts, timer::NUMERAL[n], age, [0., 0.]);
    // The clock of a timed fight, from its start to the round's end.
    let clock = m.clock_end.map(|end| end.saturating_sub(now) as u32);
    match m.phase {
        Phase::Countdown => {
            if let Some(n) = count_at(u64::from(t)) {
                put(
                    ArtSheet::Timer,
                    numeral(usize::from(n), t % COUNT_STEP as u32),
                );
            }
        }
        Phase::Fight => {
            if u64::from(t) < GO_SHOWN {
                put(
                    ArtSheet::Start,
                    timer::group_quads(&art.start.sequence, START_GO, t, [0., 0.]),
                );
            }
            if let Some(left) = clock {
                put(
                    ArtSheet::Timer,
                    timer::quads(ts, timer::Side::Centre, left, t),
                );
                if let Some((n, from)) = timer::numeral(left) {
                    put(ArtSheet::Timer, numeral(n, from - left));
                }
            }
        }
        Phase::RoundOver | Phase::Over => {
            if m.why == Some(Why::Time) && m.phase == Phase::RoundOver {
                // The readout stays as it stood (its entrance long done).
                put(
                    ArtSheet::Timer,
                    timer::quads(ts, timer::Side::Centre, 0, u32::MAX),
                );
                put(ArtSheet::Timer, numeral(0, t));
            }
            // The title: a round's, kept through the result (its age runs
            // on from the round's end when the match ended with one).
            let age = match (m.phase, m.why) {
                (Phase::Over, Some(Why::KnockOut | Why::Time)) => t + duel_match::ROUND_OVER as u32,
                _ => t,
            };
            let over_shown = m.phase == Phase::Over && (!in_camp || u64::from(t) >= hold(&m));
            if let Some(w) = m.winner.filter(|_| !over_shown) {
                let text = if usize::from(w) == seat {
                    END_WIN
                } else {
                    END_LOSE
                };
                let es = &art.end.sequence;
                put(
                    ArtSheet::End,
                    timer::group_quads(es, END_BAR, age, [0., 0.]),
                );
                put(ArtSheet::End, timer::group_quads(es, text, age, [0., 0.]));
            }
        }
    }
    out
}

/// The match's display this frame (worked out from the host's clock).
/// `in_camp`: the pause menu shows the result's choices and numbers.
/// `art`: the game's sprites stand for the numerals, GO, the clock and the
/// titles (`sprites`); the names, the rounds and why stay text.
pub fn labels(
    room: &MatchRoom,
    net: &DuelNet,
    game: &Game,
    in_camp: bool,
    art: bool,
) -> Vec<Label> {
    let Some(m) = room.m else {
        return Vec::new();
    };
    let now = now(net, game);
    let t = now.saturating_sub(m.since);
    let seat = usize::from(net.role == Role::Join);
    let me = net.name.as_str();
    let them = net.peer.as_deref().unwrap_or("Opponent");
    let name = |s: usize| if s == seat { me } else { them };
    let rules = game.ext.online().duel_rules;
    let best_of = duel_match::wins_needed(&rules) > 1;
    let mut out = Vec::new();
    // The round and the score under the HP gauge (Best of 3).
    if best_of && m.phase != Phase::Over {
        out.push(label(
            &format!(
                "Round {}   {} - {}",
                m.round,
                m.wins[seat],
                m.wins[1 - seat]
            ),
            [256., 48.],
            13.,
            WHITE,
        ));
    }
    match m.phase {
        Phase::Countdown => {
            // The names, then each numeral grows in and fades.
            out.push(label(
                &format!("{me}   vs   {them}"),
                [256., if art { 64. } else { 110. }],
                18.,
                WHITE,
            ));
            if let Some(n) = count_at(t).filter(|_| !art) {
                let k = (t % COUNT_STEP) as f32 / COUNT_STEP as f32;
                let size = 72. * (1.5 - 0.5 * (1. - (1. - k).powi(3)));
                let alpha = if k > 0.8 { (1. - k) / 0.2 } else { 1. };
                out.push(label(
                    &n.to_string(),
                    [256., 180.],
                    size,
                    [GOLD[0], GOLD[1], GOLD[2], alpha],
                ));
            }
        }
        Phase::Fight if art => {}
        Phase::Fight => {
            if t < GO_SHOWN {
                let k = t as f32 / GO_SHOWN as f32;
                out.push(label(
                    "GO!",
                    [256., 180.],
                    80. + 40. * k,
                    [GOLD[0], GOLD[1], GOLD[2], 1. - k],
                ));
            }
            if let Some(end) = m.clock_end {
                let left = end.saturating_sub(now) as u32;
                let colour = if left < 10 * 60 { RED } else { WHITE };
                out.push(label(&duel_rules::clock(left), [256., 24.], 22., colour));
            }
        }
        // The banner says who won the round; a drawn one says so in text.
        Phase::RoundOver if art && m.winner.is_some() => {}
        Phase::RoundOver => {
            let head = match m.why {
                Some(Why::Time) => "TIME UP",
                _ => "K.O.",
            };
            let k = settle(t);
            out.push(label(head, [256., 160.], 40. + 20. * (1. - k), GOLD));
            let line = match m.winner {
                Some(w) => format!("{} wins the round", name(usize::from(w))),
                None => "A draw".to_string(),
            };
            out.push(label(&line, [256., 205.], 16., WHITE));
        }
        Phase::Over if in_camp => {
            // The title over the field until the result's menu opens; the
            // menu holds the rest (its help line says why).
            if t < hold(&m) {
                let (title, colour, why) = result_head(&m, seat, net);
                // With the art the banner is the title; the reason goes
                // under it.
                let fade = ((hold(&m) - t) as f32 / TITLE_FADE).min(1.);
                if art && m.winner.is_some() {
                    out.push(label(
                        &why,
                        [256., 198.],
                        15.,
                        [WHITE[0], WHITE[1], WHITE[2], fade],
                    ));
                    return out;
                }
                let k = settle(t);
                let fade = ((hold(&m) - t) as f32 / TITLE_FADE).min(1.);
                let [r, g, b, _] = colour;
                out.push(label(
                    title,
                    [256., 84.],
                    44. + 16. * (1. - k),
                    [r, g, b, fade],
                ));
                out.push(label(
                    &why,
                    [256., 122.],
                    15.,
                    [WHITE[0], WHITE[1], WHITE[2], fade],
                ));
            }
        }
        Phase::Over => {
            let (title, colour, why) = result_head(&m, seat, net);
            let k = settle(t);
            // The title above, the numbers and choices below: the shot of
            // the Sora shows between them.
            out.push(label(title, [256., 64.], 44. + 16. * (1. - k), colour));
            let mut rows = vec![why];
            rows.extend(
                result_rows(&m, seat, game, None)
                    .into_iter()
                    .map(|(l, v)| format!("{l}   {v}")),
            );
            for (i, r) in rows.iter().enumerate() {
                let mut l = label(r, [256., 292. + 16. * i as f32], 13., WHITE);
                l.backed = true;
                out.push(l);
            }
            let y = 300. + 16. * rows.len() as f32;
            if matches!(net.status, Status::Playing) {
                let want = if m.rematch[seat] {
                    "Rematch: waiting"
                } else {
                    "Rematch"
                };
                for (i, text) in [want, "Leave"].iter().enumerate() {
                    let on = room.cursor == i;
                    let mut l = label(
                        text,
                        [256., y + 18. * i as f32],
                        15.,
                        if on { GOLD } else { WHITE },
                    );
                    l.backed = true;
                    out.push(l);
                }
                if m.rematch[1 - seat] {
                    let mut l = label(
                        &format!("{them} wants a rematch"),
                        [256., y + 40.],
                        13.,
                        WHITE,
                    );
                    l.backed = true;
                    out.push(l);
                }
            } else {
                let mut l = label("Leave", [256., y], 15., GOLD);
                l.backed = true;
                out.push(l);
            }
        }
    }
    out
}

#[cfg(test)]
mod cleanup_tests {
    use super::*;
    use cleanup::Scope;

    #[test]
    fn a_round_s_every_end_and_start_asks_for_a_cleanup() {
        let at = |p| Some((p, 1, 10));
        assert_eq!(round_change(None, None), None);
        assert_eq!(
            round_change(None, at(Phase::Countdown)),
            Some(Scope::RoundStart)
        );
        assert_eq!(round_change(at(Phase::Countdown), at(Phase::Fight)), None);
        assert_eq!(
            round_change(at(Phase::Fight), at(Phase::RoundOver)),
            Some(Scope::RoundEnd)
        );
        assert_eq!(
            round_change(at(Phase::RoundOver), Some((Phase::Countdown, 2, 20))),
            Some(Scope::RoundStart)
        );
        assert_eq!(
            round_change(at(Phase::RoundOver), at(Phase::Over)),
            Some(Scope::MatchEnd)
        );
        assert_eq!(round_change(at(Phase::Over), None), Some(Scope::MatchEnd));
        assert_eq!(round_change(at(Phase::Fight), at(Phase::Fight)), None);
    }
}
