//! Sora's level-up panels and bonus levels. `FUN_1403ecf30` raises each
//! level the EXP reaches and `FUN_1403ed7d0` builds the panel's lines (the
//! stats the new row raised, then the growth ability it teaches); a bonus
//! level (`FUN_1403edca0` → `FUN_1403edf90`, rows of 00battle `bons`) adds
//! Maximum HP, MP, Drive bars, slots and abilities and shows its own panel.
//! The panels are `dk::LEVELUP_INFO` queue A (`visual::level_up`).
use super::*;
use data::battle_tables::{self, PlayerLevel};
use visual::level_up::{Entry, Kind};

/// sys.bar messages: "Strength Increased!", "Magic Increased!", "Defense
/// Increased!", "Maximum AP Increased!".
pub const STAT_MESSAGES: [u32; 4] = [0x8ade, 0x8adf, 0x8ae0, 0x8ae1];
/// "Maximum HP Increased!", "Maximum MP Increased!", "Drive Gauge Powered
/// Up!", "Gained Item Slot!", "Gained Accessory Slot!", "Gained Armor
/// Slot!".
pub const BONUS_MESSAGES: [u32; 6] = [0x8ae2, 0x8ae3, 0x8ae4, 0x8ae5, 0x8ae6, 0x8ae7];
/// The panel titles: Donald and Goofy (`FUN_1403e6090`: message 0x851e + the
/// character id; Sora's is the form panels' `sora_title`).
pub const NAME_MESSAGES: [u32; 2] = [0x8520, 0x8521];
/// The unattended `--growth-tour`: EXP to level 7 at tick 100 (six panels,
/// four in the first batch), the bonus levels 3 and 51 at tick 420.
pub const CAPTURES: [u64; 9] = [90, 130, 200, 300, 380, 440, 480, 560, 640];
pub const END: u64 = 700;

/// The Drive gauge stops at nine bars (`FUN_1403edf90`).
const MAX_DRIVE: u8 = 9;

/// A line of a panel before its text is looked up.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Line {
    Stat(usize),
    Bonus(usize),
    Ability(u16),
}

/// One panel waiting for the HUD.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Panel {
    pub kind: Kind,
    /// 0 Sora, 1 Donald, 2 Goofy, 3 others (the group table's index).
    pub character: u8,
    /// The level, or the number of bonus levels taken for a bonus.
    pub level: u8,
    pub lines: Vec<Line>,
}

/// What the panels print: the stat and bonus messages and the `bons` table.
#[derive(Default)]
pub struct Texts {
    pub stat: [Vec<u8>; 4],
    pub bonus: [Vec<u8>; 6],
    /// Donald's and Goofy's names.
    pub names: [Vec<u8>; 2],
    pub bons: Vec<u8>,
}

/// `FUN_1403ed7d0`'s lines for the step from row `old` to `new`: the raised
/// stats in order, then the growth ability of the chosen path. On Critical
/// Sora's AP counts one and a half times (`FUN_1403eda00`).
pub fn level_lines(old: &PlayerLevel, new: &PlayerLevel, critical: bool, path: usize) -> Vec<Line> {
    let ap = |v: u8| {
        if critical {
            (v as i32 * 3) >> 1
        } else {
            v as i32
        }
    };
    let mut lines = Vec::new();
    for (i, (a, b)) in [
        (old.strength as i32, new.strength as i32),
        (old.magic as i32, new.magic as i32),
        (old.defense as i32, new.defense as i32),
        (ap(old.ap), ap(new.ap)),
    ]
    .into_iter()
    .enumerate()
    {
        if b - a > 0 {
            lines.push(Line::Stat(i));
        }
    }
    if let Some(&ability) = new.abilities.get(path)
        && ability != 0
    {
        lines.push(Line::Ability(ability));
    }
    lines
}

/// The panels' entries with their text: the first two lines show
/// (`FUN_1403eda60` passes `list[0]` and `list[1]`).
pub fn entries(src: &drive_room::FormSources, panels: &[Panel]) -> Vec<Entry> {
    let text = |line: &Line| -> Option<Vec<u8>> {
        match *line {
            Line::Stat(i) => Some(src.growth.stat[i].clone()),
            Line::Bonus(i) => Some(src.growth.bonus[i].clone()),
            // `{icon}<name>!` (msg 0xae8); the icon glyph is not drawn [I].
            Line::Ability(item) => src.item_names.get(&item).map(|n| {
                let mut v = n.clone();
                v.push(0x48);
                v
            }),
        }
    };
    panels
        .iter()
        .filter_map(|p| {
            let mut lines = p.lines.iter().filter_map(text);
            let line1 = lines.next()?;
            Some(Entry {
                kind: p.kind,
                form: p.character,
                level: p.level,
                title: match p.character {
                    0 => src.sora_title.clone(),
                    i @ (1 | 2) => src.growth.names[i as usize - 1].clone(),
                    _ => Vec::new(),
                },
                line1,
                line2: lines.next(),
            })
        })
        .collect()
}

/// The character index of a `bons` row's character id.
fn character_index(id: u8) -> u8 {
    match id {
        1..=3 => id - 1,
        _ => 3,
    }
}

/// `FUN_1403edca0`: takes bonus `id` once. Sora's row raises his maximum
/// HP / MP (healing the same amount), the Drive gauge (to nine bars) and
/// teaches its abilities; every row shows a bonus panel. Donald's and
/// Goofy's rows only show theirs (no party stats yet). Returns whether the
/// bonus was new.
pub fn apply_bonus(
    rt: &mut EnemyRuntime,
    sora: &mut Fighter,
    bons: &[u8],
    id: u8,
) -> Result<bool, std::io::Error> {
    if rt.bonus_taken.contains(&id) {
        return Ok(false);
    }
    let rows = battle_tables::bonus_rows(bons, id)?;
    rt.bonus_taken.push(id);
    let count = rt.bonus_taken.len() as u8;
    let critical = rt.difficulty == combat::Difficulty::Critical;
    let Fighter { player, vitals, .. } = sora;
    for row in rows {
        // The party's HP and MP gains halve on Critical (FUN_1403edf90).
        let halve = critical && matches!(row.character, 1..=3 | 14);
        let (hp, mp) = if halve {
            (row.hp >> 1, row.mp >> 1)
        } else {
            (row.hp, row.mp)
        };
        if row.character == 1 {
            vitals.target.max_hp += hp as i32;
            vitals.hp = (vitals.hp + hp as i32).min(vitals.target.max_hp);
            player.mp.max += mp as i32;
            player.mp.cur = (player.mp.cur + mp as i32).min(player.mp.max);
            player.drive.max = (player.drive.max + row.drive).min(MAX_DRIVE);
            rt.learned
                .extend(row.abilities.iter().copied().filter(|&a| a != 0));
        }
        let mut lines = Vec::new();
        for (i, v) in [
            hp,
            mp,
            row.drive,
            row.item_slots,
            row.accessory_slots,
            row.armor_slots,
        ]
        .into_iter()
        .enumerate()
        {
            if v != 0 {
                lines.push(Line::Bonus(i));
            }
        }
        lines.extend(
            row.abilities
                .iter()
                .filter(|&&a| a != 0)
                .map(|&a| Line::Ability(a)),
        );
        rt.panels.push(Panel {
            kind: Kind::Bonus,
            character: character_index(row.character),
            level: count,
            lines,
        });
    }
    Ok(true)
}

/// The tour's scripted gains.
pub fn tour(game: &mut Game) {
    let tick = game.tick;
    let Game {
        enemy,
        sora,
        drive,
        log,
        ..
    } = game;
    let Some(rt) = enemy.as_mut() else { return };
    if tick == 100 {
        let exp = rt.sora_levels.get(6).map_or(0, |r| r.exp);
        prize_room::grant_exp(rt, &mut sora.vitals, exp, log, tick);
    }
    if tick == 420
        && let Some(d) = drive.as_ref()
    {
        let bons = d.src.growth.bons.clone();
        for id in [3, 51] {
            let taken = apply_bonus(rt, sora, &bons, id).unwrap_or(false);
            let Fighter { player, vitals, .. } = &*sora;
            prize_room::log_event(
                log,
                serde_json::json!({"event":"bonus_level","tick":tick,"id":id,"taken":taken,
                "max_hp":vitals.target.max_hp,"hp":vitals.hp,"max_mp":player.mp.max,
                "mp":player.mp.cur,"drive_max":player.drive.max,"learned":rt.learned}),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(strength: u8, magic: u8, defense: u8, ap: u8, ability: u16) -> PlayerLevel {
        PlayerLevel {
            exp: 0,
            strength,
            magic,
            defense,
            ap,
            abilities: [ability, 0, 0],
        }
    }

    #[test]
    fn lines_list_the_raised_stats_then_the_ability() {
        let (a, b) = (row(2, 6, 3, 0, 0), row(4, 6, 3, 2, 0x186));
        assert_eq!(
            level_lines(&a, &b, false, 0),
            vec![Line::Stat(0), Line::Stat(3), Line::Ability(0x186)]
        );
        // The path picks the ability; nothing raised gives no lines.
        let c = row(4, 6, 3, 2, 0);
        assert!(level_lines(&c, &c, false, 1).is_empty());
    }

    #[test]
    fn critical_ap_counts_one_and_a_half_times() {
        // 1 -> 2 AP is 1 -> 3 on Critical.
        let (a, b) = (row(0, 0, 0, 1, 0), row(0, 0, 0, 2, 0));
        assert_eq!(level_lines(&a, &b, true, 0), vec![Line::Stat(3)]);
        assert_eq!(level_lines(&a, &a, true, 0), vec![]);
    }

    #[test]
    fn character_ids_map_to_panel_groups() {
        assert_eq!(character_index(1), 0);
        assert_eq!(character_index(3), 2);
        assert_eq!(character_index(14), 3);
    }
}
