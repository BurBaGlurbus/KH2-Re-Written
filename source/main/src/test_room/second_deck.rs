//! The second command deck (menu 6, FUN_1403f3860): Attack, Summon, Party
//! and Limit, shown in place of the top deck with left (FUN_1403f2470), and
//! the lists its rows open (FUN_1403f29e0 fills them every frame): Summon
//! (menu 7, FUN_1403e6d10), Party (menu 8) and Limit (menu 9,
//! FUN_1403e7b30). Row colours follow FUN_140401d60 and FUN_1403b0f20's
//! submenu rule (5 when the list is empty, 1 when no entry is usable).
//!
//! Summons, the party commands and the party limits do not run yet: an
//! entry executed from a list closes the menu (as the original queues the
//! command) and is logged.
use super::*;
use data::{command_table::Command, item_table};
use visual::field_command_menu::{Decision, Item, SECOND_DECK};

pub const SUMMON: u16 = 0x48;
pub const PARTY: u16 = 0x49;
pub const LIMIT: u16 = 0x51;
/// The deck's Limit row (FUN_1403f3860: Attack, Summon, Party, Limit).
pub const LIMIT_ROW: usize = 3;
/// Shown while a summon is out (FUN_1403de250 != 0).
pub const DISMISS: u16 = 0x13c;
/// The empty row (message 0x8503, icon 0; FUN_140401d60 gives it 5).
pub const BLANK: u16 = 0x68;
/// Row 2 while Genie is out (0x25d, submenu 3: his forms).
pub const GENIE: u16 = 0x25d;
/// Menu 3 as FUN_1403f3230 builds it with Genie out.
pub const GENIE_LIST: usize = 3;
/// FUN_1403de370's forms (1..5 without Anti) and FUN_1403de240's command
/// for each (0x1405c83a6).
const GENIE_FORMS: [(u8, u16); 4] = [(1, 0x217), (2, 0x218), (4, 0x219), (5, 0x21a)];
/// Menu kinds of the lists.
pub const SUMMON_LIST: usize = 7;
pub const PARTY_LIST: usize = 8;
pub const LIMIT_LIST: usize = 9;
/// 0x1405c8fa0 (FUN_1403e6080): the Party list's command by character id
/// (2 Donald, 3 Goofy, 5 Auron, 6 Mulan, 7 Aladdin, 8 Jack Sparrow, 9 Beast,
/// 10 Jack Skellington, 11 Simba, 12 Tron, 13 Riku, 15 Ping).
const MEMBER_COMMANDS: [u16; 16] = [
    0, 0, 0x5c, 0x5d, 0, 0x5e, 0x60, 0x72, 0x62, 0x63, 0x64, 0x65, 0x66, 0xb4, 0, 0x5f,
];
/// Item category of an ability (item +2), whose +4 is its bit.
const ABILITY_KIND: u8 = 0x13;

/// 00battle `sumn` (0x40-byte rows after an 8-byte header): the summon's
/// command, the charm item that offers it, its object entry (+4, the one
/// FUN_1403de610 spawns), a second object (+8: Tinker Bell) and its
/// finisher's command (+0xc).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SummonRow {
    pub command: u16,
    pub charm: u16,
    pub object: u16,
    pub extra: u16,
    pub limit: u16,
}

/// 00battle `limt` (0x40-byte rows after an 8-byte header): id, the three
/// participants (character ids; 200-204 are summons), the command (+0x28),
/// the ability the first participant needs (+0x2a) and the world (+0x2c, 0
/// any).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LimitRow {
    pub id: u8,
    pub members: [u8; 3],
    pub command: u16,
    pub ability: u16,
    pub world: u8,
    /// +4: the limit file's name (`limit/<lang>/<file>`).
    pub file: [u8; 32],
}

impl LimitRow {
    pub fn file_name(&self) -> String {
        let end = self.file.iter().position(|&b| b == 0).unwrap_or(32);
        String::from_utf8_lossy(&self.file[..end]).into_owned()
    }
}

fn rows(table: &[u8]) -> impl Iterator<Item = &[u8; 0x40]> {
    let count = table
        .get(4..8)
        .map_or(0, |w| u32::from_le_bytes(w.try_into().unwrap()) as usize);
    table
        .get(8..)
        .unwrap_or_default()
        .as_chunks::<0x40>()
        .0
        .iter()
        .take(count)
}

pub fn read_summons(sumn: &[u8]) -> Vec<SummonRow> {
    rows(sumn)
        .map(|r| SummonRow {
            command: u16::from_le_bytes([r[0], r[1]]),
            charm: u16::from_le_bytes([r[2], r[3]]),
            object: u16::from_le_bytes([r[4], r[5]]),
            extra: u16::from_le_bytes([r[8], r[9]]),
            limit: u16::from_le_bytes([r[0xc], r[0xd]]),
        })
        .collect()
}

pub fn read_limits(limt: &[u8]) -> Vec<LimitRow> {
    rows(limt)
        .map(|r| LimitRow {
            id: r[0],
            members: [r[1], r[2], r[3]],
            command: u16::from_le_bytes([r[0x28], r[0x29]]),
            ability: u16::from_le_bytes([r[0x2a], r[0x2b]]),
            world: r[0x2c],
            file: r[4..0x24].try_into().expect("32 bytes"),
        })
        .collect()
}

/// What the deck's rules read from the room this frame.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Room {
    /// The party slots 1 and 2 in the field: character id and whether the
    /// member is up (not KO).
    pub members: Vec<(u8, bool)>,
    pub airborne: bool,
    /// The battle flag (DAT_142a11404).
    pub battle: bool,
    pub mp: i32,
    pub charging: bool,
    /// A limit is running (DAT_142a24cc0).
    pub limit_running: bool,
    /// Full Drive bars (stat +0x1b1) and whether a form is on.
    pub drive_level: u8,
    pub in_form: bool,
    /// The world index, when the room stands in one.
    pub world: Option<u8>,
    /// Owned charm items.
    pub charms: Vec<u16>,
    /// Ability bits Sora holds (of those the limits ask for).
    pub sora_bits: Vec<u16>,
    /// Commands the current form disables (FUN_1403b6cb0).
    pub masked: Vec<u16>,
    /// The summon state (DAT_142a24edc: 0 none, 1 appearing, 2 out, 3
    /// leaving).
    pub summon_state: u8,
    /// The summon out is Genie (DAT_1409acdd5).
    pub genie: bool,
    /// Drive forms owned (FUN_1403c4720 of FUN_1403c3ef0(form)), by form.
    pub forms_owned: [bool; 7],
}

pub struct SecondDeck {
    pub summons: Vec<SummonRow>,
    pub limits: Vec<LimitRow>,
    pub commands: HashMap<u16, Command>,
    /// Command labels by command id.
    pub names: HashMap<u16, Vec<u8>>,
    /// Ability item id to its bit (item +4) for ability items.
    pub ability_bits: HashMap<u16, u16>,
    /// The open list's commands in row order, and its kind.
    pub list: Vec<u16>,
    pub list_kind: usize,
}

pub fn load(battle: &[u8], system: &[u8]) -> Result<SecondDeck, Box<dyn std::error::Error>> {
    let commands = data::command_table::read(bar_entry(system, b"cmd\0")?)?
        .into_iter()
        .map(|c| (c.id, c))
        .collect();
    let ability_bits = item_table::read(bar_entry(system, b"item")?)?
        .into_iter()
        .filter(|i| i.kind == ABILITY_KIND)
        .map(|i| (i.id, i.amount))
        .collect();
    Ok(SecondDeck {
        summons: read_summons(bar_entry(battle, b"sumn")?),
        limits: read_limits(bar_entry(battle, b"limt")?),
        commands,
        names: HashMap::new(),
        ability_bits,
        list: Vec::new(),
        list_kind: 0,
    })
}

impl SecondDeck {
    /// (command, message id) of every label the deck and its lists show.
    pub fn wanted_names(&self) -> Vec<(u16, u32)> {
        let mut ids = vec![SUMMON, PARTY, LIMIT, DISMISS, BLANK, GENIE];
        ids.extend(GENIE_FORMS.iter().map(|f| f.1));
        ids.extend(self.summons.iter().map(|s| s.command));
        ids.extend(MEMBER_COMMANDS.iter().filter(|&&c| c != 0));
        ids.extend(self.limits.iter().map(|l| l.command));
        // The limits' follow-ups (execute 81) show as reaction commands.
        ids.extend(
            self.commands
                .values()
                .filter(|c| c.execute == 81)
                .map(|c| c.id),
        );
        ids.sort_unstable();
        ids.dedup();
        ids.into_iter()
            .filter_map(|id| Some((id, self.commands.get(&id)?.name)))
            .collect()
    }

    /// The ability bits the limits ask of Sora (character 1).
    pub fn sora_limit_bits(&self) -> Vec<u16> {
        self.limits
            .iter()
            .filter(|l| l.members[0] == 1)
            .filter_map(|l| self.ability_bits.get(&l.ability).copied())
            .collect()
    }

    fn item(&self, command: u16, colour: usize) -> Item {
        Item {
            label: self.names.get(&command).cloned().unwrap_or_default(),
            icon: self.commands.get(&command).map_or(0, |c| c.icon),
            colour,
        }
    }

    /// FUN_1403b6cf0 and FUN_1403b6cb0, the tail of FUN_140401d60: flag
    /// bit 1 commands are unusable in the air, flag bit 16 ones outside a
    /// battle, and the form's mask.
    fn masked(&self, room: &Room, command: u16) -> bool {
        let flags = self.commands.get(&command).map_or(0, |c| c.flags);
        (flags & 2 != 0 && room.airborne)
            || (flags & 0x1_0000 != 0 && !room.battle)
            || room.masked.contains(&command)
    }

    fn with_mask(&self, room: &Room, command: u16, colour: usize) -> usize {
        if self.masked(room, command) {
            5
        } else {
            colour
        }
    }

    /// Menu 7 (FUN_1403e6d10): each summon whose charm is owned, in table
    /// order. An entry (execute 72, no submenu) takes Drive's test: usable
    /// with its cost (+0x1c) in full bars outside a form [I: the player
    /// state bits and FUN_1404005e0 are not modelled].
    pub fn summon_list(&self, room: &Room) -> Vec<(u16, usize)> {
        self.summons
            .iter()
            .filter(|s| room.charms.contains(&s.charm))
            .map(|s| {
                let cost = self.commands.get(&s.command).map_or(0, |c| c.cost);
                let colour = if !room.in_form && room.drive_level >= cost {
                    0
                } else {
                    5
                };
                (s.command, self.with_mask(room, s.command, colour))
            })
            .collect()
    }

    /// Menu 8: the two party slots' members (FUN_1403e3680) with their
    /// command from 0x1405c8fa0; 5 while a limit runs (FUN_1403e8f30) or
    /// the member is down (FUN_1403c38f0).
    pub fn party_list(&self, room: &Room) -> Vec<(u16, usize)> {
        room.members
            .iter()
            .take(2)
            .filter_map(|&(character, up)| {
                let command = *MEMBER_COMMANDS.get(character as usize)?;
                (command != 0).then(|| {
                    let colour = if room.limit_running || !up { 5 } else { 0 };
                    (command, self.with_mask(room, command, colour))
                })
            })
            .collect()
    }

    /// Menu 9 (FUN_1403e7b30 with FUN_1403e81a0): every limit whose
    /// participants are all in the field, whose world is this one (or
    /// any), whose first participant has its ability, and that is not a
    /// summon's; the first row of each id wins. Colour from FUN_1403d88e0
    /// (all-MP cost: 2 while MP is short), then 5 outside a battle, while
    /// MP charges or a limit runs (FUN_1403e75d0) or with a participant
    /// down (FUN_1403e7e10). The room's members carry no ability record,
    /// so their limit abilities count as held [test-room choice].
    pub fn limit_list(&self, room: &Room) -> Vec<(u16, usize)> {
        let present = |m: u8| m == 0 || m == 1 || room.members.iter().any(|&(c, _)| c == m);
        let up = |m: u8| m <= 1 || room.members.iter().any(|&(c, up)| c == m && up);
        let mut seen = 0u32;
        let mut out = Vec::new();
        for l in &self.limits {
            let summon = l.members[1].wrapping_add(0x38) < 5;
            if summon
                || l.ability == 0
                || !(l.world == 0 || Some(l.world) == room.world)
                || !l.members.iter().all(|&m| present(m))
            {
                continue;
            }
            if l.members[0] == 1
                && let Some(bit) = self.ability_bits.get(&l.ability)
                && !room.sora_bits.contains(bit)
            {
                continue;
            }
            let bit = 1u32 << (l.id & 0x1f);
            if seen & bit != 0 {
                continue;
            }
            seen |= bit;
            let cost = self.commands.get(&l.command).map_or(0, |c| c.cost);
            let mut colour =
                player::limit_form::colour(cost, room.limit_running, room.mp, room.charging);
            colour = self.with_mask(room, l.command, colour);
            let usable = room.battle
                && !room.charging
                && !room.limit_running
                && l.members.iter().all(|&m| up(m));
            if !usable {
                colour = 5;
            }
            out.push((l.command, colour));
        }
        out
    }

    /// Menu 3 with Genie out (FUN_1403f3230): Sora's owned forms but Anti
    /// (FUN_1403de370), each as Genie's form command aimed at Genie.
    /// [I] Colour 0 (FUN_1403b0f20 with Genie's record).
    pub fn genie_list(&self, room: &Room) -> Vec<(u16, usize)> {
        GENIE_FORMS
            .iter()
            .filter(|&&(form, _)| room.forms_owned[form as usize])
            .map(|&(_, command)| (command, self.with_mask(room, command, 0)))
            .collect()
    }

    /// FUN_1403b0f20's submenu rule on a row's colour.
    fn list_rule(colour: usize, list: &[(u16, usize)]) -> usize {
        if list.is_empty() {
            5
        } else if list.iter().all(|e| e.1 >= 4) {
            1
        } else {
            colour
        }
    }

    /// Menu 6 (FUN_1403f3860): Attack as on the top deck; Dismiss while a
    /// summon is out, else Summon when the list has an entry (else the
    /// empty row); Genie's forms (0x25d) while Genie is out (state 2), else
    /// Party; Limit.
    pub fn deck(&self, room: &Room, attack: &Item) -> (Vec<Item>, Vec<Option<usize>>) {
        let summons = self.summon_list(room);
        let party = self.party_list(room);
        let limits = self.limit_list(room);
        let mut items = vec![attack.clone()];
        let mut opens = vec![None];
        if room.summon_state != 0 {
            items.push(self.item(DISMISS, self.with_mask(room, DISMISS, 0)));
            opens.push(None);
        } else if summons.is_empty() {
            items.push(self.item(BLANK, 5));
            opens.push(None);
        } else {
            // FUN_1403b6d50: a command with a submenu skips Drive's test.
            let colour = Self::list_rule(self.with_mask(room, SUMMON, 0), &summons);
            items.push(self.item(SUMMON, colour));
            opens.push(Some(SUMMON_LIST));
        }
        if room.summon_state == super::summon::state::OUT && room.genie {
            let forms = self.genie_list(room);
            let colour = Self::list_rule(self.with_mask(room, GENIE, 0), &forms);
            items.push(self.item(GENIE, colour));
            opens.push(Some(GENIE_LIST));
        } else {
            let colour = Self::list_rule(self.with_mask(room, PARTY, 0), &party);
            items.push(self.item(PARTY, colour));
            opens.push(Some(PARTY_LIST));
        }
        // FUN_1403d88e0: no limit row has command 0x51, so it starts at 0.
        let colour = Self::list_rule(self.with_mask(room, LIMIT, 0), &limits);
        items.push(self.item(LIMIT, colour));
        opens.push(Some(LIMIT_LIST));
        (items, opens)
    }

    /// The list a row opens, as menu items.
    pub fn list(&self, room: &Room, kind: usize) -> Vec<(u16, usize)> {
        match kind {
            SUMMON_LIST => self.summon_list(room),
            PARTY_LIST => self.party_list(room),
            LIMIT_LIST => self.limit_list(room),
            GENIE_LIST => self.genie_list(room),
            _ => Vec::new(),
        }
    }

    pub fn items(&self, list: &[(u16, usize)]) -> Vec<Item> {
        list.iter()
            .map(|&(c, colour)| self.item(c, colour))
            .collect()
    }
}

/// The room's state for the rules.
pub fn room(game: &Game, world: Option<u8>) -> Room {
    let Some(sd) = game.second_deck.as_ref() else {
        return Room::default();
    };
    let members = game
        .party
        .iter()
        .filter(|m| !m.stored)
        .map(|m| (m.kind.character, m.hp > 0))
        .collect();
    let charms = sd
        .summons
        .iter()
        .map(|s| s.charm)
        .filter(|c| {
            game.items
                .as_ref()
                .is_some_and(|it| it.stock.get(c).is_some_and(|&n| n > 0))
        })
        .collect();
    let sora_bits = sd
        .sora_limit_bits()
        .into_iter()
        .filter(|&b| game.sora.player.abilities.has(b))
        .collect();
    let mut commands = vec![SUMMON, PARTY, LIMIT, DISMISS];
    commands.extend(sd.summons.iter().map(|s| s.command));
    commands.extend(sd.limits.iter().map(|l| l.command));
    commands.extend(MEMBER_COMMANDS.iter().filter(|&&c| c != 0));
    let masked = commands
        .into_iter()
        .filter(|&c| game.drive.as_ref().is_some_and(|d| d.masked(c)))
        .collect();
    Room {
        members,
        airborne: game.sora.player.in_air(),
        battle: battle(game),
        mp: game.sora.player.mp.cur,
        charging: game.sora.player.mp_charging,
        limit_running: game.drive.as_ref().is_some_and(|d| d.limit_running),
        drive_level: game.sora.player.drive.level,
        in_form: game.drive.as_ref().is_some_and(|d| d.active),
        world,
        charms,
        sora_bits,
        masked,
        summon_state: super::summon::state_of(game),
        genie: game
            .summon
            .as_ref()
            .is_some_and(|s| s.command == super::summon::GENIE),
        forms_owned: game.drive.as_ref().map_or([false; 7], |d| d.forms.owned),
    }
}

/// The battle flag as the room keeps it (party room or Drive state).
pub fn battle(game: &Game) -> bool {
    game.party_battle.on || game.drive.as_ref().is_some_and(|d| d.battle.on)
}

/// This frame's second-deck rows, and the open list's colours.
pub fn refresh(game: &Game, menu: &mut visual::field_command_menu::CommandMenu, room: &Room) {
    let Some(sd) = game.second_deck.as_ref() else {
        return;
    };
    let attack = menu.items.first().cloned().unwrap_or(Item {
        label: Vec::new(),
        icon: 1,
        colour: 0,
    });
    let (items, opens) = sd.deck(room, &attack);
    menu.second = items;
    menu.second_opens = opens;
    if menu.kind == SECOND_DECK
        && let Some(sub) = menu.sub.as_mut()
        && matches!(sub.kind, SUMMON_LIST | PARTY_LIST | LIMIT_LIST | GENIE_LIST)
    {
        let list = sd.list(room, sub.kind);
        if list.len() == sub.items.len() {
            for (item, (_, c)) in sub.items.iter_mut().zip(&list) {
                item.colour = *c;
            }
        }
    }
}

/// The second deck's part of a decision: its rows open their lists, and
/// an executed entry is logged. Returns what is left for the top deck's
/// handling (Attack, an unusable row).
pub fn decide(
    game: &mut Game,
    menu: &mut visual::field_command_menu::CommandMenu,
    decision: Decision,
    room: &Room,
) -> Decision {
    let tick = game.tick;
    let Some(sd) = game.second_deck.as_mut() else {
        return decision;
    };
    match decision {
        Decision::Open(row) => {
            if let Some(kind) = menu.opens_of(row) {
                let list = sd.list(room, kind);
                sd.list = list.iter().map(|e| e.0).collect();
                sd.list_kind = kind;
                let items = sd.items(&list);
                menu.open_sub(kind, items);
                game.event(serde_json::json!({"event":"second_deck_list","tick":tick,
                    "kind":kind,"commands":list.iter().map(|e| e.0).collect::<Vec<_>>(),
                    "colours":list.iter().map(|e| e.1).collect::<Vec<_>>()}));
            }
            Decision::None
        }
        Decision::ExecuteSub { row, index } => {
            let command = sd.list.get(index).copied();
            let limit = sd.list_kind == LIMIT_LIST;
            let summon = sd.list_kind == SUMMON_LIST;
            let genie = sd.list_kind == GENIE_LIST;
            game.event(
                serde_json::json!({"event":"second_deck_command","tick":tick,
                "row":row,"command":command,"built":limit || summon || genie}),
            );
            // A party limit starts with the tick's Sora update (party_limit);
            // a summon with the summon tick (summon.rs).
            if limit {
                game.limit_request = command;
            } else if summon {
                game.summon_request = command;
            } else if genie {
                // FUN_1403a8b10: the command goes to its target, Genie.
                game.genie_command = command;
            }
            Decision::None
        }
        // Row 1 is Dismiss while a summon is out (execute 316).
        Decision::Execute(1) if super::summon::state_of(game) != 0 => {
            game.event(
                serde_json::json!({"event":"second_deck_command","tick":tick,
                "row":1,"command":DISMISS,"built":true}),
            );
            game.summon_dismiss = true;
            Decision::None
        }
        Decision::Execute(row) if row > 0 => {
            game.event(
                serde_json::json!({"event":"second_deck_command","tick":tick,
                "row":row,"built":false}),
            );
            Decision::None
        }
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn deck() -> SecondDeck {
        let command = |id: u16, icon: u8, cost: u8, flags: u32| {
            (
                id,
                Command {
                    id,
                    name: 0,
                    execute: 0,
                    argument: 0,
                    submenu: -1,
                    icon,
                    flags,
                    cost,
                    time: 0,
                    decider: 0,
                    chain: 0,
                    form_mask: 0,
                    group: 0,
                    reaction: Default::default(),
                },
            )
        };
        SecondDeck {
            summons: vec![
                SummonRow {
                    command: 0x1bf,
                    charm: 0x17f,
                    ..SummonRow::default()
                },
                SummonRow {
                    command: 0x59,
                    charm: 0x9f,
                    ..SummonRow::default()
                },
            ],
            limits: vec![
                // Timeless River's Goofy first, then the plain one.
                LimitRow {
                    id: 3,
                    members: [3, 1, 0],
                    command: 0xff,
                    ability: 0xc9,
                    world: 13,
                    file: [0; 32],
                },
                LimitRow {
                    id: 3,
                    members: [3, 1, 0],
                    command: 0xfe,
                    ability: 0xc9,
                    world: 0,
                    file: [0; 32],
                },
                LimitRow {
                    id: 13,
                    members: [1, 2, 3],
                    command: 0x1e4,
                    ability: 0xc6,
                    world: 0,
                    file: [0; 32],
                },
                LimitRow {
                    id: 13,
                    members: [1, 0, 0],
                    command: 0x26c,
                    ability: 0xc6,
                    world: 0,
                    file: [0; 32],
                },
                // A summon's limit and a Limit Form limit (no ability).
                LimitRow {
                    id: 14,
                    members: [1, 202, 0],
                    command: 0x1d4,
                    ability: 0xa0,
                    world: 0,
                    file: [0; 32],
                },
                LimitRow {
                    id: 21,
                    members: [1, 0, 0],
                    command: 0x2ba,
                    ability: 0,
                    world: 0,
                    file: [0; 32],
                },
            ],
            commands: HashMap::from([
                command(0x1bf, 5, 3, 0x112),
                command(0x59, 5, 3, 0x112),
                command(0x5c, 6, 0, 0x13),
                command(0x5d, 6, 0, 0x13),
                command(0xff, 0, 0xff, 0x110),
                command(0xfe, 0, 0xff, 0x112),
                command(0x1e4, 0, 0xff, 0x111),
                command(0x26c, 0, 0xff, 0x111),
                command(SUMMON, 5, 3, 0x113),
                command(PARTY, 6, 0, 0x13),
                command(LIMIT, 7, 0, 0x111),
                command(BLANK, 0, 0, 0x11),
            ]),
            names: HashMap::new(),
            ability_bits: HashMap::from([(0xc6, 40), (0xc9, 41)]),
            list: Vec::new(),
            list_kind: 0,
        }
    }

    fn field() -> Room {
        Room {
            members: vec![(2, true), (3, true)],
            battle: true,
            mp: 300,
            drive_level: 3,
            sora_bits: vec![40],
            ..Room::default()
        }
    }

    #[test]
    fn reads_summon_and_limit_rows() {
        let mut sumn = vec![2, 0, 0, 0, 1, 0, 0, 0];
        let mut r = [0u8; 0x40];
        r[..0xe].copy_from_slice(&[0xbf, 1, 0x7f, 1, 0xc8, 6, 0, 0, 0xf4, 4, 0, 0, 0x11, 2]);
        sumn.extend(r);
        assert_eq!(
            read_summons(&sumn),
            vec![SummonRow {
                command: 0x1bf,
                charm: 0x17f,
                object: 0x6c8,
                extra: 0x4f4,
                limit: 0x211,
            }]
        );
        let mut limt = vec![2, 0, 0, 0, 1, 0, 0, 0];
        let mut r = [0u8; 0x40];
        r[..4].copy_from_slice(&[13, 1, 2, 3]);
        r[0x28..0x2d].copy_from_slice(&[0xe4, 1, 0xc6, 0, 0]);
        limt.extend(r);
        assert_eq!(
            read_limits(&limt),
            vec![LimitRow {
                id: 13,
                members: [1, 2, 3],
                command: 0x1e4,
                ability: 0xc6,
                world: 0,
                file: [0; 32],
            }]
        );
    }

    #[test]
    fn the_deck_is_attack_summon_party_limit() {
        let d = deck();
        let attack = Item {
            label: vec![1],
            icon: 1,
            colour: 0,
        };
        // No charms: the Summon slot is the empty row.
        let (items, opens) = d.deck(&field(), &attack);
        assert_eq!(
            items.iter().map(|i| (i.icon, i.colour)).collect::<Vec<_>>(),
            [(1, 0), (0, 5), (6, 0), (7, 0)]
        );
        assert_eq!(opens, [None, None, Some(8), Some(9)]);
        // A charm brings Summon. In the air every summon and member entry
        // is unusable (flag bit 1), so their rows read 1 (FUN_1403b0f20
        // overrides the row's own 5); a Limit entry without the flag keeps
        // the Limit row at 0.
        let mut room = field();
        room.charms = vec![0x9f];
        room.airborne = true;
        let (items, opens) = d.deck(&room, &attack);
        assert_eq!(opens[1], Some(7));
        assert_eq!(
            items.iter().map(|i| i.colour).collect::<Vec<_>>(),
            [0, 1, 1, 0]
        );
    }

    #[test]
    fn the_party_list_follows_the_slots() {
        let d = deck();
        let mut room = field();
        assert_eq!(d.party_list(&room), [(0x5c, 0), (0x5d, 0)]);
        room.members[1].1 = false;
        room.limit_running = false;
        assert_eq!(d.party_list(&room), [(0x5c, 0), (0x5d, 5)]);
        room.limit_running = true;
        assert_eq!(d.party_list(&room), [(0x5c, 5), (0x5d, 5)]);
    }

    #[test]
    fn limits_need_their_members_world_and_ability() {
        let d = deck();
        let mut room = field();
        // Goofy's plain row (world 0) and the full Trinity; no summon or
        // Limit Form row.
        assert_eq!(d.limit_list(&room), [(0xfe, 0), (0x1e4, 0)]);
        // Timeless River takes Goofy's own row.
        room.world = Some(13);
        assert_eq!(d.limit_list(&room)[0].0, 0xff);
        // Alone, Sora does the solo Trinity; without the ability, nothing.
        room.members.clear();
        room.world = None;
        assert_eq!(d.limit_list(&room), [(0x26c, 0)]);
        room.sora_bits.clear();
        assert!(d.limit_list(&room).is_empty());
        // Outside a battle the entries are greyed and the row reads 1;
        // short MP is orange (2).
        let mut room = field();
        room.battle = false;
        assert!(d.limit_list(&room).iter().all(|e| e.1 == 5));
        room.battle = true;
        room.mp = 50;
        assert!(d.limit_list(&room).iter().all(|e| e.1 == 2));
    }

    #[test]
    fn with_genie_out_row_two_lists_his_forms() {
        let d = deck();
        let mut room = field();
        room.summon_state = crate::test_room::summon::state::OUT;
        room.genie = true;
        room.forms_owned[1] = true;
        room.forms_owned[3] = true;
        room.forms_owned[5] = true;
        let attack = d.item(BLANK, 0);
        let (items, opens) = d.deck(&room, &attack);
        assert_eq!(items.len(), 4);
        assert_eq!(opens[2], Some(GENIE_LIST));
        // Anti Form (3) is never offered.
        assert_eq!(d.genie_list(&room), [(0x217, 0), (0x21a, 0)]);
        // Another summon keeps the Party row.
        room.genie = false;
        assert_eq!(d.deck(&room, &attack).1[2], Some(PARTY_LIST));
    }
}
