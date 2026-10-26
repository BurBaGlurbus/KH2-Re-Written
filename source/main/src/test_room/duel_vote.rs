//! A vote between the two players, for any choice a duel puts to both
//! (the party, the arena, the rules, later). Each player casts a ballot;
//! the same choice wins, different choices go to the host's coin flip,
//! sent over the reliable channel with its seed so both screens play the
//! same flip. Only the F3 test vote uses it for now.

use super::duel_net::{DuelNet, Status};
use super::duel_wire::{BALLOT, VOTE};
use super::*;
use net::wire::{Reader, Writer};
use net::{RELIABLE, Role};

/// se000: cursor, decide.
const SE_MOVE: u32 = 1;
const SE_DECIDE: u32 = 2;
/// Logical pad bits of mode 7.
const UP: u64 = 0x10;
const DOWN: u64 = 0x20;
const DECIDE: u64 = 0x4;

/// Ticks the coin spins before it lands, and how often it turns over.
pub(crate) const FLIP_TICKS: u64 = 90;
const TURN_TICKS: u64 = 6;
/// Ticks the result stays on screen.
const RESULT_TICKS: u64 = 150;

/// A vote's question and choices.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Vote {
    /// What the vote is for (a caller's own number).
    pub topic: u8,
    pub question: String,
    pub options: Vec<String>,
    /// Each seat's ballot (0 the host, 1 the joiner).
    pub ballots: [Option<u8>; 2],
    pub outcome: Option<Outcome>,
}

/// How a vote ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Outcome {
    pub choice: u8,
    /// The coin flip's seed when the ballots differed.
    pub flip: Option<u32>,
}

impl Vote {
    pub(crate) fn new(topic: u8, question: &str, options: &[&str]) -> Self {
        Vote {
            topic,
            question: question.to_string(),
            options: options.iter().map(|o| o.to_string()).collect(),
            ballots: [None; 2],
            outcome: None,
        }
    }

    /// A seat's ballot; once cast it stands.
    pub(crate) fn cast(&mut self, seat: usize, choice: u8) -> Result<(), String> {
        if self.outcome.is_some() {
            return Err("the vote is over".into());
        }
        if usize::from(choice) >= self.options.len() {
            return Err("no such choice".into());
        }
        let ballot = self.ballots.get_mut(seat).ok_or("no such player")?;
        if ballot.is_some() {
            return Err("already voted".into());
        }
        *ballot = Some(choice);
        Ok(())
    }

    /// Host: once both have voted, the outcome; a tie flips the coin with
    /// `seed`.
    pub(crate) fn decide(&mut self, seed: u32) -> Option<Outcome> {
        if self.outcome.is_none()
            && let [Some(a), Some(b)] = self.ballots
        {
            self.outcome = Some(if a == b {
                Outcome {
                    choice: a,
                    flip: None,
                }
            } else {
                Outcome {
                    choice: flip_result(seed, a, b),
                    flip: Some(seed),
                }
            });
        }
        self.outcome
    }
}

/// The coin: the seed's low bit picks one of the two ballots.
pub(crate) fn flip_result(seed: u32, a: u8, b: u8) -> u8 {
    if seed & 1 == 0 { a } else { b }
}

/// Which of the two tied ballots the spinning coin shows at a tick of its
/// flip (it turns every few ticks and lands on the result).
pub(crate) fn flip_face(seed: u32, a: u8, b: u8, ticks: u64) -> u8 {
    if ticks >= FLIP_TICKS {
        return flip_result(seed, a, b);
    }
    // Turns still to come; the last one shows the result.
    let turns = (FLIP_TICKS - ticks - 1) / TURN_TICKS;
    let land = flip_result(seed, a, b);
    if turns.is_multiple_of(2) {
        land
    } else if land == a {
        b
    } else {
        a
    }
}

pub(crate) fn encode_vote(vote: &Vote) -> Vec<u8> {
    let mut w = Writer::default();
    w.u8(VOTE);
    w.u8(vote.topic);
    w.text(&vote.question, 80);
    w.u8(vote.options.len().min(8) as u8);
    for o in vote.options.iter().take(8) {
        w.text(o, 40);
    }
    for b in vote.ballots {
        w.u8(b.unwrap_or(u8::MAX));
    }
    match vote.outcome {
        Some(o) => {
            w.u8(1);
            w.u8(o.choice);
            match o.flip {
                Some(seed) => {
                    w.u8(1);
                    w.u32(seed);
                }
                None => w.u8(0),
            }
        }
        None => w.u8(0),
    }
    w.finish()
}

pub(crate) fn decode_vote(bytes: &[u8]) -> Option<Vote> {
    let mut r = Reader::new(bytes);
    if r.u8()? != VOTE {
        return None;
    }
    let topic = r.u8()?;
    let question = r.text(80)?;
    let n = r.u8()? as usize;
    if !(2..=8).contains(&n) {
        return None;
    }
    let options = (0..n).map(|_| r.text(40)).collect::<Option<Vec<_>>>()?;
    let mut ballots = [None; 2];
    for b in &mut ballots {
        let v = r.u8()?;
        *b = (v != u8::MAX).then_some(v);
    }
    let outcome = match r.u8()? {
        0 => None,
        _ => {
            let choice = r.u8()?;
            let flip = match r.u8()? {
                0 => None,
                _ => Some(r.u32()?),
            };
            Some(Outcome { choice, flip })
        }
    };
    let valid = |c: &u8| usize::from(*c) < n;
    (r.is_empty()
        && ballots.iter().flatten().all(valid)
        && outcome.is_none_or(|o| valid(&o.choice)))
    .then_some(Vote {
        topic,
        question,
        options,
        ballots,
        outcome,
    })
}

/// The duel's vote in the room: the one running (if any), this player's
/// cursor, and when its outcome arrived.
#[derive(Resource, Default)]
pub(crate) struct VoteRoom {
    pub vote: Option<Vote>,
    cursor: usize,
    /// The tick the outcome reached this instance (the flip's clock).
    decided_at: Option<u64>,
    /// Host: the vote the joiner has.
    sent: Option<Vote>,
}

/// The F3 test vote's topic.
pub(crate) const TOPIC_TEST: u8 = 0;

impl VoteRoom {
    /// The vote holds the pad: one is open and this player has not voted.
    pub(crate) fn menu_up(&self, seat: usize) -> bool {
        self.vote
            .as_ref()
            .is_some_and(|v| v.ballots[seat].is_none())
    }

    /// Host: puts a question to both players.
    pub(crate) fn start(&mut self, vote: Vote) {
        *self = VoteRoom {
            vote: Some(vote),
            ..VoteRoom::default()
        };
    }
}

fn seat_of(net: &DuelNet) -> usize {
    usize::from(net.role == Role::Join)
}

/// One fixed step: ballots and outcomes over the wire, this player's
/// choice, the result's time on screen.
pub(crate) fn tick(
    net: Option<ResMut<DuelNet>>,
    room: Option<ResMut<VoteRoom>>,
    mut game: ResMut<Game>,
) {
    let (Some(mut net), Some(mut room)) = (net, room) else {
        return;
    };
    let (net, room, game) = (&mut *net, &mut *room, &mut *game);
    if !matches!(net.status, Status::Playing) {
        *room = VoteRoom::default();
        return;
    }
    let seat = seat_of(net);
    // Host: F3's test question.
    if std::mem::take(&mut game.duel_test_vote) && net.role == Role::Host {
        room.start(Vote::new(TOPIC_TEST, "Test vote", &["Left", "Right"]));
        game.sound_queue.push((SE_DECIDE, "vote"));
    }
    for m in std::mem::take(&mut net.inbox) {
        match (net.role, m.first().copied()) {
            (Role::Host, Some(BALLOT)) => {
                let mut r = Reader::new(&m);
                let _ = r.u8();
                if let (Some(topic), Some(choice), Some(v)) = (r.u8(), r.u8(), room.vote.as_mut())
                    && v.topic == topic
                {
                    let _ = v.cast(1, choice);
                }
            }
            (Role::Join, Some(VOTE)) => {
                if let Some(v) = decode_vote(&m) {
                    if room
                        .vote
                        .as_ref()
                        .is_none_or(|old| old.topic != v.topic || old.question != v.question)
                    {
                        room.cursor = 0;
                        room.decided_at = None;
                    }
                    room.vote = Some(v);
                }
            }
            _ => net.inbox.push(m),
        }
    }
    let Some(vote) = room.vote.as_mut() else {
        return;
    };
    // This player's ballot.
    if vote.ballots[seat].is_none() {
        let pressed = if game.autoplay.is_some() {
            // [Test] Unattended: the host takes the first choice, the
            // joiner the second (a tie, so the coin flips).
            0
        } else {
            game.pad.pressed | game.pad.repeat & (UP | DOWN)
        };
        let n = vote.options.len();
        let choice = if game.autoplay.is_some() {
            Some(seat.min(n - 1))
        } else if pressed & UP != 0 {
            room.cursor = (room.cursor + n - 1) % n;
            game.sound_queue.push((SE_MOVE, "vote"));
            None
        } else if pressed & DOWN != 0 {
            room.cursor = (room.cursor + 1) % n;
            game.sound_queue.push((SE_MOVE, "vote"));
            None
        } else {
            (pressed & DECIDE != 0).then_some(room.cursor)
        };
        if let Some(c) = choice {
            game.sound_queue.push((SE_DECIDE, "vote"));
            if net.role == Role::Host {
                let _ = vote.cast(0, c as u8);
            } else {
                vote.ballots[1] = Some(c as u8);
                let mut w = Writer::default();
                w.u8(BALLOT);
                w.u8(vote.topic);
                w.u8(c as u8);
                net.session.send(RELIABLE, w.finish());
            }
        }
    }
    // Host: the outcome, the coin from the clock.
    if net.role == Role::Host && vote.outcome.is_none() {
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.subsec_nanos());
        if let Some(o) = vote.decide(seed) {
            let tick = game.tick;
            game.event(serde_json::json!({"event":"vote_decided","tick":tick,
                "topic":vote.topic,"ballots":vote.ballots,"choice":o.choice,"flip":o.flip}));
        }
    }
    if vote.outcome.is_some() && room.decided_at.is_none() {
        room.decided_at = Some(game.tick);
    }
    // The coin turns over with a cursor sound and lands with a decide.
    if let (Some(at), Some(o)) = (room.decided_at, vote.outcome) {
        let t = game.tick - at;
        if o.flip.is_some() && t < FLIP_TICKS && t % TURN_TICKS == 0 {
            game.sound_queue.push((SE_MOVE, "coin"));
        }
        if o.flip.is_some() && t == FLIP_TICKS {
            game.sound_queue.push((SE_DECIDE, "coin"));
            let tick = game.tick;
            game.event(serde_json::json!({"event":"coin_landed","tick":tick,
                "topic":vote.topic,"choice":o.choice}));
        }
        let shown = if o.flip.is_some() { FLIP_TICKS } else { 0 } + RESULT_TICKS;
        if t >= shown && net.role == Role::Host {
            room.vote = None;
        }
    }
    // Host: the vote to the joiner on every change.
    if net.role == Role::Host && room.sent != room.vote {
        room.sent = room.vote.clone();
        if let Some(v) = &room.vote {
            net.session.send(RELIABLE, encode_vote(v));
        }
    }
    // The joiner lets a finished vote go once its result has been shown.
    if net.role == Role::Join
        && let (Some(at), Some(v)) = (room.decided_at, &room.vote)
        && let Some(o) = v.outcome
    {
        let shown = if o.flip.is_some() { FLIP_TICKS } else { 0 } + RESULT_TICKS;
        if game.tick - at >= shown {
            room.vote = None;
            room.decided_at = None;
        }
    }
}

/// The vote's lines on screen: (text, highlighted).
pub(crate) fn lines(room: &VoteRoom, net: &DuelNet, game: &Game) -> Vec<(String, bool)> {
    let Some(v) = &room.vote else {
        return Vec::new();
    };
    let seat = seat_of(net);
    let them = net.peer.as_deref().unwrap_or("Opponent");
    let mut out = vec![(format!("Vote: {}", v.question), false)];
    match (v.outcome, room.decided_at) {
        (Some(o), Some(at)) => {
            let t = game.tick - at;
            match o.flip {
                Some(seed) if t < FLIP_TICKS => {
                    let (a, b) = (v.ballots[0].unwrap_or(0), v.ballots[1].unwrap_or(0));
                    let face = flip_face(seed, a, b, t);
                    out.push(("Tie: the coin decides".to_string(), false));
                    out.push((format!("   * {} *", v.options[usize::from(face)]), true));
                }
                _ => {
                    let how = if o.flip.is_some() {
                        ", by coin flip"
                    } else {
                        ""
                    };
                    out.push((
                        format!("Result: {}{how}", v.options[usize::from(o.choice)]),
                        true,
                    ));
                }
            }
        }
        _ => {
            if v.ballots[seat].is_none() {
                for (i, o) in v.options.iter().enumerate() {
                    let on = i == room.cursor;
                    out.push((format!("   {o}"), on));
                }
            } else {
                let mine = v.ballots[seat].map_or("", |c| v.options[usize::from(c)].as_str());
                out.push((format!("   You voted {mine}"), false));
            }
            let theirs = if v.ballots[1 - seat].is_some() {
                "voted"
            } else {
                "voting..."
            };
            out.push((format!("   {them}: {theirs}"), false));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_same_choice_wins_and_a_tie_flips_the_coin() {
        let mut v = Vote::new(TOPIC_TEST, "Arena?", &["Room", "Map"]);
        assert!(v.decide(7).is_none(), "nobody has voted");
        v.cast(0, 1).unwrap();
        assert!(v.cast(0, 0).is_err(), "a ballot stands");
        assert!(v.cast(1, 5).is_err());
        v.cast(1, 1).unwrap();
        assert_eq!(
            v.decide(7),
            Some(Outcome {
                choice: 1,
                flip: None
            })
        );
        let mut tie = Vote::new(TOPIC_TEST, "Arena?", &["Room", "Map"]);
        tie.cast(0, 0).unwrap();
        tie.cast(1, 1).unwrap();
        assert_eq!(
            tie.decide(2),
            Some(Outcome {
                choice: 0,
                flip: Some(2)
            })
        );
        assert_eq!(
            tie.decide(3),
            Some(Outcome {
                choice: 0,
                flip: Some(2)
            }),
            "decided once"
        );
        assert!(tie.cast(1, 0).is_err());
        assert_eq!(flip_result(3, 0, 1), 1);
    }

    #[test]
    fn the_coin_turns_over_and_lands_on_the_result() {
        let (seed, a, b) = (5, 0, 1);
        let land = flip_result(seed, a, b);
        assert_eq!(flip_face(seed, a, b, FLIP_TICKS), land);
        assert_eq!(
            flip_face(seed, a, b, FLIP_TICKS - 1),
            land,
            "the last turn shows it"
        );
        let faces: Vec<u8> = (0..FLIP_TICKS)
            .step_by(TURN_TICKS as usize)
            .map(|t| flip_face(seed, a, b, t))
            .collect();
        assert!(
            faces.windows(2).all(|w| w[0] != w[1]),
            "each turn shows the other side"
        );
    }

    #[test]
    fn votes_round_trip() {
        let mut v = Vote::new(3, "Party?", &["On", "Off", "Host decides"]);
        v.cast(0, 2).unwrap();
        assert_eq!(decode_vote(&encode_vote(&v)), Some(v.clone()));
        v.cast(1, 0).unwrap();
        v.decide(9);
        let bytes = encode_vote(&v);
        assert_eq!(decode_vote(&bytes), Some(v));
        assert_eq!(decode_vote(&bytes[..bytes.len() - 1]), None);
    }
}
