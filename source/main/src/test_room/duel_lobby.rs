//! The duel's lobby, before each match: the host's rules, then each player
//! bans up to two abilities (for both), then each builds a loadout (Keyblade,
//! abilities within the AP limit, item slots) and readies up. The host
//! holds the lobby: the joiner sends what it wants (`Intent`), the host
//! checks it against the duel's limits and sends the lobby back to both.

use net::wire::{Reader, Writer};
use std::collections::BTreeMap;

/// Abilities each player may ban per match.
pub(crate) const MAX_BANS: usize = 2;

/// Where the lobby is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Stage {
    /// Each player picks up to two abilities to ban, or skips.
    Bans,
    /// Each player builds a loadout and readies up.
    Loadout,
    /// Both are ready: the match starts.
    Start,
}

/// A player's build for the match.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Loadout {
    pub keyblade: u16,
    /// Ability slots as the camp writes them (item id, 0x8000 = equipped).
    pub abilities: Vec<u16>,
    /// Battle item slots (0 = empty).
    pub items: Vec<u16>,
}

/// Where a player is in the lobby (the other player sees it).
pub(crate) const SECTION_LIST: u8 = 0;
pub(crate) const SECTION_BANS: u8 = 1;
pub(crate) const SECTION_CAMP: u8 = 2;

/// One player's place in the lobby.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Seat {
    pub bans: Vec<u16>,
    pub bans_done: bool,
    pub loadout: Option<Loadout>,
    pub ready: bool,
    /// The lobby's list, the ban picker or the camp.
    pub section: u8,
}

/// What a player asks of the lobby.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Intent {
    /// The player's bans for this match (none = skip); final.
    Ban(Vec<u16>),
    /// The player's build (it un-readies them).
    Loadout(Loadout),
    Ready(bool),
    /// Where the player now is (`SECTION_*`).
    Section(u8),
    /// Whether the player's install can show the Mirage Arena (the host
    /// keeps the duel in the test room's arena when either cannot).
    Arena(bool),
}

/// What the duel allows, from the installed tables.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Limits {
    /// Every ability item Sora has in a duel, with repeats (as slots).
    pub abilities: Vec<u16>,
    /// Each ability's AP cost.
    pub cost: BTreeMap<u16, i32>,
    /// The AP the equipped abilities may use.
    pub ap_max: i32,
    pub keyblades: Vec<u16>,
    /// The items a slot may hold.
    pub items: Vec<u16>,
    pub item_slots: usize,
}

impl Limits {
    /// The abilities a player may ban: each one Sora has, once.
    pub(crate) fn ban_pool(&self) -> Vec<u16> {
        let mut pool: Vec<u16> = self.abilities.iter().map(|a| a & 0x7fff).collect();
        pool.sort_unstable();
        pool.dedup();
        pool
    }

    /// The ability slots a player starts from: every ability not banned,
    /// none equipped.
    pub(crate) fn slots_without(&self, banned: &[u16]) -> Vec<u16> {
        self.abilities
            .iter()
            .map(|a| a & 0x7fff)
            .filter(|a| !banned.contains(a))
            .collect()
    }

    /// AP the build's equipped abilities use.
    pub(crate) fn ap_used(&self, slots: &[u16]) -> i32 {
        slots
            .iter()
            .filter(|s| *s & 0x8000 != 0)
            .map(|s| self.cost.get(&(s & 0x7fff)).copied().unwrap_or(0))
            .sum()
    }

    /// Why a build breaks the duel's limits, if it does.
    pub(crate) fn check(&self, l: &Loadout, banned: &[u16]) -> Result<(), String> {
        if !self.keyblades.contains(&l.keyblade) {
            return Err("that Keyblade is not in the duel".into());
        }
        // The slots must be the duel's abilities (banned ones out), each
        // as many times as Sora has it.
        let mut have: BTreeMap<u16, i32> = BTreeMap::new();
        for a in self.slots_without(banned) {
            *have.entry(a).or_default() += 1;
        }
        for s in &l.abilities {
            let id = s & 0x7fff;
            if id == 0 {
                continue;
            }
            if banned.contains(&id) {
                return Err("a banned ability is in the build".into());
            }
            let n = have.entry(id).or_default();
            *n -= 1;
            if *n < 0 {
                return Err("an ability the duel does not give".into());
            }
        }
        let used = self.ap_used(&l.abilities);
        if used > self.ap_max {
            return Err(format!("{used} AP used, {} allowed", self.ap_max));
        }
        if l.items.len() > self.item_slots {
            return Err("more item slots than Sora has".into());
        }
        if l.items.iter().any(|i| *i != 0 && !self.items.contains(i)) {
            return Err("an item the duel does not stock".into());
        }
        Ok(())
    }
}

/// The lobby (the host's; the joiner shows the copy it receives).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Lobby {
    pub stage: Stage,
    /// Seat 0 the host, 1 the joiner.
    pub seats: [Seat; 2],
    /// Matches started in this session (each has its own bans).
    pub match_no: u32,
}

impl Default for Lobby {
    fn default() -> Self {
        Lobby {
            stage: Stage::Bans,
            seats: Default::default(),
            match_no: 0,
        }
    }
}

impl Lobby {
    /// Every ability banned this match (both players' bans).
    pub(crate) fn banned(&self) -> Vec<u16> {
        let mut out: Vec<u16> = self.seats.iter().flat_map(|s| s.bans.clone()).collect();
        out.sort_unstable();
        out.dedup();
        out
    }

    /// Host: a player's intent, checked; the lobby moves on when both are
    /// through a stage.
    pub(crate) fn apply(
        &mut self,
        seat: usize,
        intent: Intent,
        limits: &Limits,
    ) -> Result<(), String> {
        let banned = self.banned();
        let s = self.seats.get_mut(seat).ok_or("no such player")?;
        match intent {
            Intent::Ban(mut bans) => {
                if self.stage != Stage::Bans || s.bans_done {
                    return Err("the bans are closed".into());
                }
                bans.sort_unstable();
                bans.dedup();
                if bans.len() > MAX_BANS {
                    return Err(format!("at most {MAX_BANS} bans"));
                }
                let pool = limits.ban_pool();
                if bans.iter().any(|b| !pool.contains(b)) {
                    return Err("that ability cannot be banned".into());
                }
                s.bans = bans;
                s.bans_done = true;
            }
            Intent::Loadout(l) => {
                if self.stage != Stage::Loadout {
                    return Err("builds open after the bans".into());
                }
                limits.check(&l, &banned)?;
                s.loadout = Some(l);
                s.ready = false;
            }
            Intent::Ready(ready) => {
                if self.stage != Stage::Loadout {
                    return Err("not choosing builds".into());
                }
                if ready && s.loadout.is_none() {
                    return Err("no build yet".into());
                }
                s.ready = ready;
            }
            Intent::Section(section) => {
                if section > SECTION_CAMP {
                    return Err("no such section".into());
                }
                s.section = section;
            }
            // The lobby room keeps it (it outlives a match's lobby).
            Intent::Arena(_) => {}
        }
        self.advance();
        Ok(())
    }

    fn advance(&mut self) {
        if self.stage == Stage::Bans && self.seats.iter().all(|s| s.bans_done) {
            self.stage = Stage::Loadout;
        }
        if self.stage == Stage::Loadout && self.seats.iter().all(|s| s.ready) {
            self.stage = Stage::Start;
        }
    }

    /// Host: the next match's lobby: new bans, the builds chosen again
    /// (each player's last build is where the camp starts).
    pub(crate) fn next_match(&mut self) {
        let n = self.match_no + 1;
        *self = Lobby {
            match_no: n,
            ..Lobby::default()
        };
    }

    /// Host: the rules changed; nobody stays ready on the old ones.
    pub(crate) fn rules_changed(&mut self) {
        if self.stage == Stage::Loadout {
            for s in &mut self.seats {
                s.ready = false;
            }
        }
    }
}

fn put_list(w: &mut Writer, list: &[u16]) {
    w.u8(list.len().min(255) as u8);
    for &v in list.iter().take(255) {
        w.u16(v);
    }
}

fn get_list(r: &mut Reader, max: usize) -> Option<Vec<u16>> {
    let n = r.u8()? as usize;
    if n > max {
        return None;
    }
    (0..n).map(|_| r.u16()).collect()
}

fn put_loadout(w: &mut Writer, l: &Loadout) {
    w.u16(l.keyblade);
    put_list(w, &l.abilities);
    put_list(w, &l.items);
}

fn get_loadout(r: &mut Reader) -> Option<Loadout> {
    Some(Loadout {
        keyblade: r.u16()?,
        abilities: get_list(r, 255)?,
        items: get_list(r, 16)?,
    })
}

/// The lobby as the host sends it.
pub(crate) fn encode_lobby(kind: u8, lobby: &Lobby) -> Vec<u8> {
    let mut w = Writer::default();
    w.u8(kind);
    w.u8(lobby.stage as u8);
    w.u32(lobby.match_no);
    for s in &lobby.seats {
        put_list(&mut w, &s.bans);
        w.u8(u8::from(s.bans_done) | u8::from(s.ready) << 1);
        w.u8(s.section);
        match &s.loadout {
            Some(l) => {
                w.u8(1);
                put_loadout(&mut w, l);
            }
            None => w.u8(0),
        }
    }
    w.finish()
}

pub(crate) fn decode_lobby(kind: u8, bytes: &[u8]) -> Option<Lobby> {
    let mut r = Reader::new(bytes);
    if r.u8()? != kind {
        return None;
    }
    let stage = match r.u8()? {
        0 => Stage::Bans,
        1 => Stage::Loadout,
        2 => Stage::Start,
        _ => return None,
    };
    let match_no = r.u32()?;
    let mut seats: [Seat; 2] = Default::default();
    for s in &mut seats {
        s.bans = get_list(&mut r, MAX_BANS)?;
        let flags = r.u8()?;
        s.bans_done = flags & 1 != 0;
        s.ready = flags & 2 != 0;
        s.section = r.u8()?;
        s.loadout = match r.u8()? {
            0 => None,
            _ => Some(get_loadout(&mut r)?),
        };
    }
    r.is_empty().then_some(Lobby {
        stage,
        seats,
        match_no,
    })
}

/// A joiner's intent as it travels.
pub(crate) fn encode_intent(kind: u8, intent: &Intent) -> Vec<u8> {
    let mut w = Writer::default();
    w.u8(kind);
    match intent {
        Intent::Ban(bans) => {
            w.u8(0);
            put_list(&mut w, bans);
        }
        Intent::Loadout(l) => {
            w.u8(1);
            put_loadout(&mut w, l);
        }
        Intent::Ready(ready) => {
            w.u8(2);
            w.u8(u8::from(*ready));
        }
        Intent::Section(section) => {
            w.u8(3);
            w.u8(*section);
        }
        Intent::Arena(ok) => {
            w.u8(4);
            w.u8(u8::from(*ok));
        }
    }
    w.finish()
}

pub(crate) fn decode_intent(kind: u8, bytes: &[u8]) -> Option<Intent> {
    let mut r = Reader::new(bytes);
    if r.u8()? != kind {
        return None;
    }
    let intent = match r.u8()? {
        0 => Intent::Ban(get_list(&mut r, MAX_BANS)?),
        1 => Intent::Loadout(get_loadout(&mut r)?),
        2 => Intent::Ready(r.u8()? != 0),
        3 => Intent::Section(r.u8()?),
        4 => Intent::Arena(r.u8()? != 0),
        _ => return None,
    };
    r.is_empty().then_some(intent)
}

#[cfg(test)]
mod tests {
    use super::*;

    const KK: u16 = 41;
    const OATH: u16 = 42;

    fn limits() -> Limits {
        Limits {
            // Two Combo Plus, one Guard, one Second Chance.
            abilities: vec![0x1a2, 0x1a2, 0x52, 0x19f],
            cost: [(0x1a2, 1), (0x52, 2), (0x19f, 4)].into(),
            ap_max: 6,
            keyblades: vec![KK, OATH],
            items: vec![1, 2, 3],
            item_slots: 3,
        }
    }

    fn build(abilities: Vec<u16>) -> Loadout {
        Loadout {
            keyblade: OATH,
            abilities,
            items: vec![1, 2, 0],
        }
    }

    #[test]
    fn bans_close_when_both_players_are_done_and_apply_to_both() {
        let l = limits();
        let mut lobby = Lobby::default();
        assert!(lobby.apply(0, Intent::Ban(vec![0x52, 0x52]), &l).is_ok());
        assert_eq!(lobby.seats[0].bans, vec![0x52], "repeats count once");
        assert!(
            lobby.apply(0, Intent::Ban(vec![]), &l).is_err(),
            "bans are final"
        );
        assert_eq!(lobby.stage, Stage::Bans);
        assert!(lobby.apply(1, Intent::Ban(vec![1, 2, 3]), &l).is_err());
        assert!(lobby.apply(1, Intent::Ban(vec![0x9999]), &l).is_err());
        assert!(
            lobby.apply(1, Intent::Ban(vec![]), &l).is_ok(),
            "skipping is a choice"
        );
        assert_eq!(lobby.stage, Stage::Loadout);
        assert_eq!(lobby.banned(), vec![0x52]);
    }

    #[test]
    fn builds_are_checked_against_bans_ap_keyblades_and_items() {
        let l = limits();
        let mut lobby = Lobby {
            stage: Stage::Loadout,
            seats: [
                Seat {
                    bans: vec![0x52],
                    bans_done: true,
                    ..Seat::default()
                },
                Seat {
                    bans_done: true,
                    ..Seat::default()
                },
            ],
            match_no: 0,
        };
        let ok = build(vec![0x81a2, 0x81a2, 0x819f]);
        assert_eq!(l.ap_used(&ok.abilities), 6);
        assert!(lobby.apply(1, Intent::Loadout(ok.clone()), &l).is_ok());
        let banned = build(vec![0x8052]);
        assert!(lobby.apply(1, Intent::Loadout(banned), &l).is_err());
        let three = build(vec![0x81a2, 0x81a2, 0x81a2]);
        assert!(
            lobby.apply(1, Intent::Loadout(three), &l).is_err(),
            "only two given"
        );
        let mut costly = limits();
        costly.ap_max = 5;
        assert!(costly.check(&ok, &[]).is_err(), "over the AP limit");
        let unequipped = build(vec![0x1a2, 0x19f]);
        assert!(
            costly.check(&unequipped, &[]).is_ok(),
            "unequipped costs nothing"
        );
        let blade = Loadout {
            keyblade: 7,
            ..ok.clone()
        };
        assert!(l.check(&blade, &[]).is_err());
        let item = Loadout {
            items: vec![99],
            ..ok.clone()
        };
        assert!(l.check(&item, &[]).is_err());
        let slots = Loadout {
            items: vec![1, 1, 1, 1],
            ..ok
        };
        assert!(l.check(&slots, &[]).is_err());
    }

    #[test]
    fn the_match_starts_when_both_are_ready_and_a_rule_change_unreadies() {
        let l = limits();
        let mut lobby = Lobby::default();
        lobby.apply(0, Intent::Ban(vec![]), &l).unwrap();
        lobby.apply(1, Intent::Ban(vec![]), &l).unwrap();
        assert!(
            lobby.apply(0, Intent::Ready(true), &l).is_err(),
            "no build yet"
        );
        let b = build(vec![0x81a2]);
        lobby.apply(0, Intent::Loadout(b.clone()), &l).unwrap();
        lobby.apply(1, Intent::Loadout(b.clone()), &l).unwrap();
        lobby.apply(0, Intent::Ready(true), &l).unwrap();
        lobby.rules_changed();
        assert!(!lobby.seats[0].ready);
        lobby.apply(0, Intent::Ready(true), &l).unwrap();
        lobby.apply(1, Intent::Loadout(b), &l).unwrap();
        assert!(!lobby.seats[1].ready, "a new build un-readies");
        lobby.apply(1, Intent::Ready(true), &l).unwrap();
        assert_eq!(lobby.stage, Stage::Start);
        lobby.next_match();
        assert_eq!(lobby.stage, Stage::Bans);
        assert_eq!(lobby.match_no, 1);
        assert!(lobby.banned().is_empty(), "each match has its own bans");
    }

    #[test]
    fn the_lobby_and_intents_round_trip() {
        let lobby = Lobby {
            stage: Stage::Loadout,
            seats: [
                Seat {
                    bans: vec![0x52, 0x19f],
                    bans_done: true,
                    loadout: Some(build(vec![0x81a2, 0x1a2])),
                    ready: true,
                    section: SECTION_CAMP,
                },
                Seat::default(),
            ],
            match_no: 3,
        };
        let bytes = encode_lobby(5, &lobby);
        assert_eq!(decode_lobby(5, &bytes), Some(lobby));
        assert_eq!(decode_lobby(6, &bytes), None);
        assert_eq!(decode_lobby(5, &bytes[..bytes.len() - 1]), None);
        for intent in [
            Intent::Ban(vec![0x52]),
            Intent::Loadout(build(vec![0x81a2])),
            Intent::Ready(true),
            Intent::Section(SECTION_BANS),
        ] {
            assert_eq!(decode_intent(6, &encode_intent(6, &intent)), Some(intent));
        }
        assert_eq!(
            decode_intent(6, &[6, 0, 3, 0, 0, 0, 0, 0, 0]),
            None,
            "three bans"
        );
    }
}
