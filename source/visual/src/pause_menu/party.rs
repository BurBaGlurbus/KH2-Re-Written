//! The camp's Party screens (camp-08 spec): one card per party member with
//! its 3D model above a name box (state 0x14: FUN_140320b30 /
//! FUN_14031fbb0), the second pick and swap (0x16: FUN_14031f9a0). Sora
//! (card 0) is never picked; with fewer than three members the cards
//! cannot move (no hand, LightCur or marker) and the help says so. The
//! event-driven notice (0x15) and world guests are not built.
use super::*;

const PRIO_PARTY_PANEL: u32 = 0x753d;
const PRIO_CARD: u32 = 0x753e;
const PRIO_MARKER: u32 = 0x753f;
/// Tz::MenuObj in Party (FUN_1402ffba0(M, 0x7545)).
const PRIO_PARTY_MODEL: u32 = 0x7545;
const PANEL: [usize; 3] = [312, 313, 314];
/// Cards: slots 0–2, the reserve slot 3.
const CARD: [usize; 2] = [315, 316];
const MARKER: usize = 317;
/// Name boxes: Sora / locked, switchable.
const NAME: [usize; 2] = [318, 319];

/// The screens' sprites and lists.
#[derive(Default)]
pub(super) struct Ui {
    panel: Option<Id>,
    pub(super) cards: MenuList,
    pub(super) swap: MenuList,
    names: Vec<Id>,
    marker: Option<Id>,
    /// Which member's model each card shows (the models stay numbered as the
    /// field's list is).
    pub(super) models: Vec<usize>,
    /// `models` when the screen opened.
    entered: Vec<usize>,
    /// The notice's lines and labels.
    notice: Vec<Id>,
}

impl Ui {
    pub(super) fn with_models(models: Vec<usize>) -> Self {
        Self {
            models,
            ..Self::default()
        }
    }

    pub(super) fn model_of(&self, card: usize) -> usize {
        self.models.get(card).copied().unwrap_or(card)
    }
}

/// State 0x15's texts (FUN_140320fc0).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PartyNotice {
    /// 0x846d, 0x846e, 0x846f, 0x9fcf.
    pub title: Vec<u8>,
    pub in_label: Vec<u8>,
    pub out_label: Vec<u8>,
    pub ok: Vec<u8>,
    /// The names that came and went (FUN_140320680).
    pub came: Vec<u8>,
    pub went: Vec<u8>,
    /// 0xcc69 or a lock message; empty for none.
    pub note: Vec<u8>,
}

/// The notice's box, in, idle and out (0x1405bad90).
const NOTICE: [usize; 3] = [9, 10, 11];
/// The in and out lines and the third, and the labels' group (0x1405bbb30).
const NOTICE_LINES: [usize; 3] = [0, 1, 123];
const NOTICE_LABEL: usize = 123;
const PRIO_NOTICE: u32 = 0x7547;

impl Camp {
    /// FUN_140320fc0 over FUN_140320b30: the Party screen with the notice on it.
    /// After it the second pick's keys stay swapped, as FUN_140320fc0 leaves
    /// them (party-spec §5.1).
    pub(super) fn enter_party_notice(&mut self, seq: &Sequence) {
        let Some(n) = self.data.party_notice.clone() else {
            return;
        };
        self.enter_party(seq);
        self.state = State::PartyNotice;
        self.substate = 0;
        self.party_ui.swap.keys = Some((bits::RIGHT, bits::LEFT));
        let h = self.data.party_help[2].clone();
        self.show_help(seq, h);
        self.open_dialog_in(seq, NOTICE, n.title, vec![n.ok]);
        let Some(boxed) = self.item_ui.dialog_box() else {
            return;
        };
        let mut lines = Vec::new();
        let mut y = seq.groups[NOTICE[0]].ui_padding as f32;
        for (k, text) in [n.came, n.went, n.note].into_iter().enumerate() {
            let group = NOTICE_LINES[k];
            let g = &seq.groups[group];
            let mut t = Text::new(text, TEXT_SIZE);
            t.align = 2;
            let s = self
                .sprites
                .create(seq, PRIO_NOTICE, ART, group, None, Some(t));
            self.child(s, boxed);
            self.sprites.get_mut(s).offset = [0., y];
            self.closers.push((s, group));
            y += g.ui_padding as f32;
            lines.push(s);
        }
        for (k, label) in [n.in_label, n.out_label].into_iter().enumerate() {
            let mut t = Text::new(label, TEXT_SIZE);
            t.align = 2;
            let s = self
                .sprites
                .create(seq, PRIO_NOTICE, ART, NOTICE_LABEL, None, Some(t));
            self.child(s, lines[k]);
            lines.push(s);
        }
        self.party_ui.notice = lines;
    }

    /// FUN_14031ff80.
    pub(super) fn party_notice(
        &mut self,
        seq: &Sequence,
        pad: &Pad,
        sounds: &mut Vec<u32>,
    ) -> bool {
        let (pick, moved) = self.item_ui.choices.input(pad);
        if self.substate == 2 {
            let lines = self.party_ui.notice.clone();
            if !self.dialog_alive() && lines.iter().all(|&s| !self.sprites.alive(s)) {
                self.party_ui.notice.clear();
                self.state = State::Party;
                self.substate = 0;
            } else if pad.trig != 0 {
                self.kill_dialog();
                for s in lines {
                    self.sprites.kill(s);
                }
            }
            return false;
        }
        if self.substate == 0 && self.group_of(self.item_ui.dialog_box()) == Some(NOTICE[1]) {
            self.substate = 1;
        }
        if self.substate != 1 {
            return false;
        }
        if let Some(s) = pick_sound(pick, moved, true) {
            sounds.push(s);
        }
        if matches!(pick, Pick::Row(0) | Pick::Cancel) {
            self.close_opt(seq, self.item_ui.dialog_box(), NOTICE[2]);
            for &r in &self.item_ui.choices.rows.clone() {
                self.close_opt(seq, Some(r), items::CHOICE_ROWS.close);
            }
            for s in self.party_ui.notice.clone() {
                self.sprites.kill(s);
            }
            self.substate = 2;
        }
        moved
    }

    /// The name cards' model pastes (party-spec §4): member i's render on
    /// card i at the card's context, with the card's alpha.
    pub(super) fn party_models(&self) -> Option<ModelLayer> {
        if !matches!(
            self.state,
            State::Party | State::PartySwap | State::PartyNotice
        ) {
            return None;
        }
        let panel_alpha = self
            .party_ui
            .panel
            .filter(|&p| self.sprites.alive(p))
            .map(|p| self.sprites.get(p).ctx_color[3])?;
        let members: Vec<_> = self
            .party_ui
            .names
            .iter()
            .enumerate()
            .filter(|&(_, &n)| self.sprites.alive(n))
            .map(|(i, &n)| {
                let c = self.sprites.context(n);
                let a = self.sprites.get(n).ctx_color[3] * panel_alpha;
                (self.party_ui.model_of(i), [c[0] - 256., c[1] - 208.], a)
            })
            .collect();
        (!members.is_empty()).then_some((PRIO_PARTY_MODEL, members))
    }

    /// FUN_140320b30: tab 0, the panel, a card and a name box per member,
    /// the models, list 1 (LEFT / RIGHT) and the help.
    pub(super) fn enter_party(&mut self, seq: &Sequence) {
        self.state = State::Party;
        self.substate = 0;
        self.party_ui.entered = self.party_ui.models.clone();
        self.create_tab(seq, 0);
        let panel = self
            .sprites
            .create(seq, PRIO_PARTY_PANEL, ART, PANEL[0], Some(PANEL[1]), None);
        self.closers.push((panel, PANEL[2]));
        self.party_ui.panel = Some(panel);
        let names: Vec<Vec<u8>> = self
            .data
            .panels
            .members
            .iter()
            .map(|m| m.name.clone())
            .collect();
        let n = names.len().min(4);
        let card_pitch = seq.groups[CARD[0]].text_position_x as f32;
        let name_pitch = seq.groups[NAME[0]].text_position_x as f32;
        let mut cards = Vec::new();
        self.party_ui.names.clear();
        for (i, name) in names.into_iter().take(n).enumerate() {
            let g = CARD[usize::from(i == 3)];
            let r = self.sprites.create(seq, PRIO_CARD, ART, g, None, None);
            self.child(r, panel);
            self.sprites.get_mut(r).offset = [card_pitch * i as f32, 0.];
            cards.push(r);
            let mut t = Text::new(name, TEXT_SIZE);
            t.align = 2;
            let locked = self.data.party_locked.get(i).copied().unwrap_or(false);
            let nb = self.sprites.create(
                seq,
                PRIO_CARD,
                ART,
                NAME[usize::from(i != 0 && !locked)],
                None,
                Some(t),
            );
            self.child(nb, panel);
            self.sprites.get_mut(nb).offset = [name_pitch * i as f32, 0.];
            self.party_ui.names.push(nb);
        }
        // Sora (card 0) and locked members are skipped; the cursor starts on
        // the first other.
        let locked: Vec<usize> = (1..n)
            .filter(|&i| self.data.party_locked.get(i).copied().unwrap_or(false))
            .collect();
        let cursor = (1..n)
            .find(|i| !locked.contains(i))
            .unwrap_or(usize::from(n > 1));
        let mut list = MenuList::new(cards.clone(), cursor);
        list.keys = Some((bits::LEFT, bits::RIGHT));
        list.skip = locked.iter().fold(1, |m, &i| m | 1 << i);
        let movable = n >= 3 && n - locked.len() >= 3;
        if !movable {
            list.mask &= !0x13;
        }
        self.party_ui.cards = list;
        let mut swap = MenuList::new(cards, 0);
        swap.keys = Some((bits::LEFT, bits::RIGHT));
        self.party_ui.swap = swap;
        self.item_ui.no_pointer = !movable;
        self.model_time = Some(0.);
        // FUN_140320b30 by count alone, so three with one locked still ask to
        // select a member.
        let h = self.data.party_help[usize::from(self.party_ui.cards.total >= 3)].clone();
        self.show_help(seq, h);
    }

    /// FUN_140321420: the pulsing marker on the active hand's card.
    fn party_marker(&mut self, seq: &Sequence) {
        let card = self
            .glove
            .filter(|&g| self.sprites.alive(g))
            .and_then(|g| self.sprites.get(g).parent);
        let Some(card) = card else {
            if let Some(m) = self.party_ui.marker.take() {
                self.sprites.kill(m);
            }
            return;
        };
        let offset = self.sprites.get(card).offset;
        let m = match self.party_ui.marker {
            Some(m) if self.sprites.alive(m) => m,
            _ => {
                let m = self
                    .sprites
                    .create(seq, PRIO_MARKER, ART, MARKER, None, None);
                self.child(m, self.party_ui.panel.unwrap());
                self.party_ui.marker = Some(m);
                m
            }
        };
        self.sprites.get_mut(m).offset = offset;
    }

    /// FUN_14031fbb0.
    pub(super) fn party(&mut self, seq: &Sequence, pad: &Pad, sounds: &mut Vec<u32>) -> bool {
        let (pick, moved) = self.party_ui.cards.input(pad);
        if self.substate == 2 {
            let dead = !self.alive_opt(self.party_ui.panel) && !self.tab_alive(0);
            if dead {
                self.item_ui.no_pointer = false;
                self.pending = None;
                self.return_top(seq);
            } else if pad.trig != 0 {
                self.kill_opt(self.party_ui.panel);
                self.kill_tab(0);
            }
            return false;
        }
        if (pick != Pick::None || moved) && self.group_of(self.party_ui.panel) != Some(PANEL[1]) {
            if let Some(p) = self.party_ui.panel {
                self.sprites.set(seq, p, PANEL[1], None);
            }
            if let Some(tab) = &self.tabs[0]
                && self.sprites.alive(tab.id)
            {
                let id = tab.id;
                self.sprites.set(seq, id, TAB[1], None);
            }
        }
        if self.substate == 0 && self.group_of(self.party_ui.panel) == Some(PANEL[1]) {
            self.substate = 1;
        }
        if let Some(s) = pick_sound(pick, moved, true) {
            sounds.push(s);
        }
        match pick {
            Pick::Start => {
                self.party_left();
                self.exit = true;
            }
            Pick::Cancel => {
                self.party_left();
                self.hide_help(seq);
                self.close_tab(seq, 0);
                self.close_opt(seq, self.party_ui.panel, PANEL[2]);
                self.pending = Some(State::Top);
                self.substate = 2;
            }
            Pick::Row(a) => {
                self.state = State::PartySwap;
                self.substate = 0;
                let n = self.party_ui.swap.total;
                self.party_ui.swap.skip = self.party_ui.cards.skip | 1 << a;
                let b = if a + 1 >= n || self.party_ui.swap.skip >> (a + 1) & 1 != 0 {
                    a.saturating_sub(1)
                } else {
                    a + 1
                };
                self.party_ui.swap.cursor = b;
                let h = self.data.party_help[2].clone();
                self.show_help(seq, h);
            }
            _ => {}
        }
        self.party_marker(seq);
        moved
    }

    /// FUN_14031f650 / FUN_140320480: a changed order zeroes the cursor memory
    /// (FUN_140305ea0).
    fn party_left(&mut self) {
        if self.party_ui.models != self.party_ui.entered {
            self.memory.clear();
        }
    }

    /// The party list rebuilt in the new order (FUN_1402fc800): each
    /// member's pages move with its name; the Status rows keep their bits
    /// (party slots).
    fn swap_members(&mut self, a: usize, b: usize) {
        fn swap<T>(v: &mut [T], a: usize, b: usize) {
            if a < v.len() && b < v.len() {
                v.swap(a, b);
            }
        }
        if self.party_ui.models.is_empty() {
            self.party_ui.models = (0..self.party_ui.names.len()).collect();
        }
        swap(&mut self.party_ui.models, a, b);
        let d = &mut self.data;
        swap(&mut d.panels.members, a, b);
        swap(&mut d.items.equipment, a, b);
        swap(&mut d.abilities.members, a, b);
        swap(&mut d.equip.totals, a, b);
        swap(&mut d.equip.bases, a, b);
        swap(&mut d.equip.form_weapons, a, b);
        swap(&mut d.customize.styles, a, b);
        let bit = |m: usize| if m == 0 { 0 } else { 6 + m as u8 };
        let at = |m: usize| d.status.iter().position(|e| e.bit == bit(m));
        if let (Some(pa), Some(pb)) = (at(a), at(b)) {
            swap(&mut d.status, pa, pb);
            (d.status[pa].bit, d.status[pb].bit) = (bit(a), bit(b));
        }
    }

    /// FUN_14031f9a0: the second pick; deciding swaps the two members.
    pub(super) fn party_swap(&mut self, seq: &Sequence, pad: &Pad, sounds: &mut Vec<u32>) -> bool {
        let (pick, moved) = self.party_ui.swap.input(pad);
        if self.substate == 2 {
            self.state = State::Party;
            self.substate = 1;
            let h = self.data.party_help[usize::from(self.party_ui.cards.total >= 3)].clone();
            self.show_help(seq, h);
            return false;
        }
        self.substate = 1;
        if let Some(s) = pick_sound(pick, moved, true) {
            sounds.push(s);
        }
        match pick {
            Pick::Start => self.exit = true,
            Pick::Cancel => self.substate = 2,
            Pick::Row(b) => {
                let a = self.party_ui.cards.cursor;
                self.swap_members(a, b);
                let (na, nb) = (self.party_ui.names[a], self.party_ui.names[b]);
                let ta = self.sprites.get(na).text.as_ref().map(|t| t.bytes.clone());
                let tb = self.sprites.get(nb).text.as_ref().map(|t| t.bytes.clone());
                if let (Some(ta), Some(tb)) = (ta, tb) {
                    self.set_text(na, tb);
                    self.set_text(nb, ta);
                }
                self.substate = 2;
            }
            _ => {}
        }
        self.party_marker(seq);
        moved
    }
}
