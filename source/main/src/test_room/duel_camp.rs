//! The duel's screens in the game's own pause menu (the camp): the lobby,
//! a vote and a match's result each ask for it with their entries and
//! pages (pause_menu::DuelPages), and one holds it at a time. A request
//! names its screen and a version of it: a camp open for another screen,
//! or an older version, closes first, and camp_tick opens the new one once
//! it has gone. A vote goes before the others (they give the camp up while
//! one is open).

use super::*;
use visual::pause_menu::{Camp, Entry};

/// The screens that ask for the camp.
pub const LOBBY: u8 = 1;
pub const VOTE: u8 = 2;
pub const RESULT: u8 = 3;
pub const FORFEIT: u8 = 4;

pub use super::extension::{CampItem as Item, CampRequest as Request};

/// Characters on one line of the camp's help box.
const HELP_LINE: usize = 48;

/// A help line in game text, broken between words to fit the help box
/// (0x02, the text's new line).
pub fn help_text(s: &str) -> Vec<u8> {
    let mut lines: Vec<String> = vec![String::new()];
    for word in s.split(' ') {
        let line = lines.last_mut().expect("one line");
        if !line.is_empty() && line.len() + 1 + word.len() > HELP_LINE {
            lines.push(word.to_string());
        } else {
            if !line.is_empty() {
                line.push(' ');
            }
            line.push_str(word);
        }
    }
    let mut out = Vec::new();
    for (i, l) in lines.iter().enumerate() {
        if i > 0 {
            out.push(0x02);
        }
        out.extend(pause_menu::encode_text(l));
    }
    out
}

/// A screen's entry of its own (the camp hands its pick back).
pub fn entry(id: u8, label: &str, help: &str) -> Item {
    Item::New(Entry {
        id,
        label: pause_menu::encode_text(label),
        help: help_text(help),
    })
}

/// Keeps the camp on screen `who`, version `want` (None: this screen wants
/// none). Returns the open camp when it is this screen's.
pub fn hold<'a>(
    camp: &'a mut CampRes,
    game: &mut Game,
    who: u8,
    want: Option<u32>,
    make: impl FnOnce() -> Request,
) -> Option<&'a mut Camp> {
    let mine = want.map(|v| (who, v));
    let open = camp
        .open
        .as_ref()
        .and_then(|m| m.data.duel.as_ref())
        .map(|d| d.owner);
    match open {
        Some(o) if Some(o) == mine => return camp.open.as_mut(),
        // This screen's older version, or another's while this one wants
        // the camp: it closes, and the request waits.
        Some(o) if o.0 == who || mine.is_some() => {
            if let Some(m) = camp.open.as_mut() {
                m.exit = true;
            }
            return None;
        }
        Some(_) => return None,
        None => {}
    }
    let pending = game.camp_request.as_ref().map(|r| r.pages.owner);
    match mine {
        None => {
            if pending.is_some_and(|p| p.0 == who) {
                game.camp_request = None;
                game.camp_open_request = false;
            }
        }
        Some(m) => {
            let free = camp.open.is_none() && camp.curtain.is_none() && camp.journal.is_none();
            if free && pending != Some(m) && pending.is_none_or(|p| p.0 == who) {
                let mut r = make();
                r.pages.owner = m;
                r.pages.keep_open = true;
                game.camp_request = Some(r);
                game.camp_open_request = true;
            }
        }
    }
    None
}

/// [Test] `--duel-shots`: a capture now (the screens ask for theirs at the
/// moments worth a look), once per `key`: a capture holds the clock, and
/// a joiner's view of the host's clock can stand still meanwhile.
pub fn shot(game: &mut Game, key: (u8, u64, u64)) {
    if !game.ext.online_mut().duel_shots
        || game.autoplay.is_none()
        || !game.ext.online_mut().duel_shot_keys.insert(key)
    {
        return;
    }
    let at = game.tick;
    let from = game.captures_taken.min(game.captures.len());
    let i = from + game.captures[from..].partition_point(|&c| c <= at);
    game.captures.insert(i, at);
}

/// The buttons the duel's help lines name, for the device last used (the
/// camp's glyph family: keyboard, Xbox, PlayStation, other) [duel choice;
/// the keyboard's are the room's default bindings, visual::input].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Prompts {
    pub start: &'static str,
    /// Left and right on the D-pad.
    pub left: &'static str,
    pub right: &'static str,
}

impl Default for Prompts {
    fn default() -> Self {
        Prompts {
            start: "M",
            left: "Left arrow",
            right: "Right arrow",
        }
    }
}

pub fn prompts(camp: &CampRes) -> Prompts {
    let pad = |start| Prompts {
        start,
        left: "Left",
        right: "Right",
    };
    match camp.glyph {
        _ if camp.keyboard_last => Prompts::default(),
        1 => pad("Menu"),
        2 => pad("Options"),
        3 => pad("Start"),
        _ => Prompts::default(),
    }
}

/// The caller's pick on the camp's top level, taken.
pub fn picked(menu: &mut Camp) -> Option<u8> {
    menu.data.duel.as_mut().and_then(|d| d.picked.take())
}

/// A shown-only row: a label and its value.
pub fn info_row(label: &str, value: &str, help: &str) -> pause_menu::DuelRow {
    pause_menu::DuelRow {
        label: pause_menu::encode_text(label),
        choices: vec![pause_menu::encode_text(value)],
        selected: 0,
        help: help_text(help),
        locked: true,
    }
}

/// Sets entry `id`'s help line (the camp shows a change at once).
pub fn set_help(menu: &mut Camp, id: u8, help: &str) {
    let bytes = help_text(help);
    if let Some(e) = menu.data.entries.iter_mut().find(|e| e.id == id)
        && e.help != bytes
    {
        e.help = bytes;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn help_lines_break_between_words() {
        let t = help_text("An ability neither player can equip this match. Left and Right choose.");
        let lines: Vec<&[u8]> = t.split(|&b| b == 0x02).collect();
        assert_eq!(lines.len(), 2);
        assert!(lines.iter().all(|l| l.len() <= HELP_LINE));
        assert_eq!(
            lines[1],
            pause_menu::encode_text("Left and Right choose.").as_slice()
        );
        assert_eq!(help_text("Short."), pause_menu::encode_text("Short."));
    }
}
