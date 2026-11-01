//! The camp's Status rows for the Drive forms and the Summons (camp-04 §2.2,
//! §2.4: FUN_140353020, FUN_140352350, FUN_140354340) and the numbers Sora's
//! own row shows only once a form is obtained.
use super::*;
use pause_menu::abilities::{FormAbilities, MemberAbilities, new_marks};
use player::forms::{self, FormLevel};

/// Message tables at 0x1405bc0c0, 0x1405bc0a0 and 0x1405bbf88 and following.
const FORM_NAMES: [u32; 5] = [0x845d, 0xb353, 0xce9f, 0x845f, 0x8461];
const FORM_HELPS: [u32; 5] = [0xcb8f, 0xcb90, 0xcea0, 0xcb91, 0xcb92];
const FORM_STAT_HELPS: [[u32; 4]; 5] = [
    [0xc57e, 0xc584, 0xc585, 0xc586],
    [0xc57f, 0xc587, 0xc588, 0xc589],
    [0xcea1, 0xce9c, 0xce9d, 0xce9e],
    [0xc580, 0xc58a, 0xc58b, 0xc58c],
    [0xc581, 0xc58d, 0xc58e, 0xc58f],
];
/// The forms' rows 0..3 (0x1405bbf90): label and style.
const FORM_LABELS: [(u32, usize); 4] = [(0xc569, 0), (0xc55a, 0), (0xc55b, 0), (0xc583, 1)];
/// Rows 10..16, the same for every form (0x1405bbfcc): label, help, style.
const COMBAT_ROWS: [(u32, u32, usize); 7] = [
    (0xc560, 0xc575, 2),
    (0xc561, 0xc576, 2),
    (0xc562, 0xc577, 2),
    (0xc563, 0xc578, 3),
    (0xc564, 0xc579, 3),
    (0xc565, 0xc57a, 3),
    (0xc566, 0xc57b, 3),
];
/// Lion's row on the Abilities page (FUN_140361fc0).
const LION_NAME: u32 = 0xb2ec;
const SUMMON_NAME: u32 = 0xc5b1;
const SUMMON_HELP: u32 = 0xc5b0;
/// The Summon rows 0..3 and 10..13 (0x1405bc010, 0x1405bc04c).
const SUMMON_ROWS: [(u32, u32, usize); 4] = [
    (0xc568, 0xc57d, 0),
    (0xc55a, 0xc595, 0),
    (0xc55b, 0xc596, 0),
    (0xc590, 0xc597, 1),
];
const SUMMON_ITEM_ROWS: [(u32, u32); 4] = [
    (0xc591, 0xc598),
    (0xc592, 0xc599),
    (0xc593, 0xc59a),
    (0xc594, 0xc59b),
];
/// Sora's rows that need a form (0x1405bbf10 / 0x1405bbf4c): bit, label, help,
/// style, the form they ask for (`VALOR` also for the Drive Gauge).
const SORA_ROWS: [(usize, u32, u32, usize, u8); 6] = [
    (6, 0xc55f, 0xc574, 1, forms::VALOR),
    (8, 0xc5ab, 0xc57f, 4, forms::WISDOM),
    (9, 0xc5ac, 0xc580, 4, forms::MASTER),
    (17, 0xc5aa, 0xc57e, 4, forms::VALOR),
    (18, 0xce9b, 0xcea1, 4, forms::LIMIT),
    (19, 0xc5ad, 0xc581, 4, forms::FINAL),
];
/// FUN_140369520's Summon items (0x1405bc588).
pub const SUMMON_ITEMS: [u16; 4] = [0x19, 0x9f, 0xa0, 0x17f];
/// Summon level and EXP in the save (FUN_1403e88f0).
const SUMMON_LEVEL: usize = 0x3526;
const SUMMON_EXP: usize = 0x36e4;
/// A fresh room's forms come with its development grant: never NEW!.
pub const ALL_SEEN: u16 = 0x1ff;
/// Pride Lands and Atlantica hide the forms (0x1405bac00 [I: worlds]).
const NO_FORMS: [u8; 2] = [10, 11];
/// FUN_140364920: Lion's row in the Pride Lands.
const LION_WORLD: u8 = 10;

/// The texts, read when the camp's sources load.
#[derive(Default)]
pub struct Texts(HashMap<u32, Vec<u8>>);

impl Texts {
    /// Every message id this module shows.
    pub fn ids() -> Vec<u32> {
        let mut ids = vec![SUMMON_NAME, SUMMON_HELP, LION_NAME];
        ids.extend(FORM_NAMES);
        ids.extend(FORM_HELPS);
        ids.extend(FORM_STAT_HELPS.iter().flatten());
        ids.extend(FORM_LABELS.iter().map(|r| r.0));
        ids.extend(COMBAT_ROWS.iter().flat_map(|r| [r.0, r.1]));
        ids.extend(SUMMON_ROWS.iter().flat_map(|r| [r.0, r.1]));
        ids.extend(SUMMON_ITEM_ROWS.iter().flat_map(|r| [r.0, r.1]));
        ids.extend(SORA_ROWS.iter().flat_map(|r| [r.1, r.2]));
        ids
    }

    pub fn new(ids: &[u32], texts: Vec<Vec<u8>>) -> Self {
        Self(ids.iter().copied().zip(texts).collect())
    }

    fn get(&self, id: u32) -> Vec<u8> {
        self.0.get(&id).cloned().unwrap_or_default()
    }

    fn stat(&self, label: u32, help: u32, style: usize, value: &str) -> Option<pause_menu::Stat> {
        Some(pause_menu::Stat {
            label: self.get(label),
            help: self.get(help),
            style,
            value: pause_menu::encode_number(value),
        })
    }
}

/// One form's numbers, Sora in that form included (status object [f]).
#[derive(Clone, Debug, PartialEq)]
pub struct FormNumbers {
    pub form: u8,
    pub level: u8,
    pub exp: u32,
    pub next: u32,
    pub strength: i32,
    pub magic: i32,
    pub defense: i32,
    pub resist: [u8; 4],
}

/// What the Status list reads from the room.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Numbers {
    pub forms: Vec<FormNumbers>,
    /// Summon level, EXP, next level and the summons owned.
    pub summon: Option<(u8, u32, u32, [bool; 4])>,
    pub exp: u32,
    pub bonus_levels: u8,
    /// Full bars and maximum.
    pub drive: (u8, u8),
    pub new: u16,
}

/// FUN_1403e87f0: a record's EXP with the levels below it counted.
pub fn total_exp(fmlv: &[FormLevel], form: u8, level: u8, exp: i32) -> u32 {
    let below: u32 = (1..level)
        .filter_map(|l| forms::level_row(fmlv, form << 4 | l))
        .map(|r| r.exp)
        .sum();
    below + exp.max(0) as u32
}

/// FUN_1403e8a00's threshold: 0 at the cap.
pub fn to_next(fmlv: &[FormLevel], form: u8, level: u8, exp: i32, cap: u8) -> u32 {
    if level >= cap {
        return 0;
    }
    forms::level_row(fmlv, form << 4 | level).map_or(0, |r| r.exp.saturating_sub(exp.max(0) as u32))
}

/// FUN_14040d680: the Summon level's cap.
fn summon_cap(owned: &[bool; 4]) -> u8 {
    let n = 2 + owned.iter().filter(|&&o| o).count() as u8;
    if n == 6 { 7 } else { n }
}

/// The room's numbers when the camp opens. FUN_140369520's new bits: an owned
/// form (bits 0..4) or summon (5..8) whose seen bit is clear.
pub fn numbers(game: &Game, c: &CampRes, world: u8) -> Numbers {
    let stock = game.items.as_ref().map_or(&c.stock, |i| &i.stock);
    let base = &c.source.numbers;
    let mut out = Numbers {
        exp: game.enemy.as_ref().map_or(0, |rt| rt.exp_total),
        bonus_levels: game
            .enemy
            .as_ref()
            .map_or(0, |rt| rt.bonus_taken.len() as u8),
        drive: (game.sora.player.drive.level, game.sora.player.drive.max),
        ..Numbers::default()
    };
    let Some(d) = game.drive.as_ref() else {
        return out;
    };
    let fmlv = &d.src.fmlv;
    if !NO_FORMS.contains(&world) {
        for form in [
            forms::VALOR,
            forms::WISDOM,
            forms::LIMIT,
            forms::MASTER,
            forms::FINAL,
        ] {
            let (true, Some(r)) = (d.forms.owned[form as usize], d.forms.record(form)) else {
                continue;
            };
            // FUN_1403c03f0: the status with the form's record linked.
            let weapon = matches!(form, forms::VALOR | forms::MASTER | forms::FINAL)
                .then_some(r.keyblade)
                .filter(|&k| k != 0);
            let gear = game.ability_source.gear(&game.equipment, weapon);
            out.forms.push(FormNumbers {
                form,
                level: r.level,
                exp: total_exp(fmlv, form, r.level, r.exp),
                next: to_next(fmlv, form, r.level, r.exp, d.forms.level_cap()),
                strength: base.strength + i32::from(gear.strength),
                magic: base.magic + i32::from(gear.magic),
                defense: base.defense + i32::from(gear.defense),
                resist: [
                    gear.resist[1],
                    gear.resist[2],
                    gear.resist[3],
                    gear.resist[4],
                ],
            });
            out.new |= 1 << (form - 1);
        }
    }
    let owned = SUMMON_ITEMS.map(|i| stock.get(&i).is_some_and(|&n| n > 0));
    if owned.contains(&true) {
        let image = &game.save.base;
        let level = image.u8_at(SUMMON_LEVEL).max(1);
        let exp = image.u32_at(SUMMON_EXP) as i32;
        out.summon = Some((
            level,
            total_exp(fmlv, 0, level, exp),
            to_next(fmlv, 0, level, exp, summon_cap(&owned)),
            owned,
        ));
        for (k, o) in owned.iter().enumerate() {
            if *o {
                out.new |= 1 << (5 + k);
            }
        }
    }
    out
}

/// The Abilities page's form rows when the camp opens: the owned forms
/// outside the worlds that hide them (FUN_1402fc500), then Lion in the Pride
/// Lands (FUN_140361fc0), with FUN_140368ed0's marks over each record's slots.
pub fn ability_forms(game: &Game, c: &CampRes, world: u8) -> Vec<FormAbilities> {
    let Some(d) = game.drive.as_ref() else {
        return Vec::new();
    };
    let mut rows = Vec::new();
    if !NO_FORMS.contains(&world) {
        for form in [
            forms::VALOR,
            forms::WISDOM,
            forms::LIMIT,
            forms::MASTER,
            forms::FINAL,
        ] {
            if let (true, Some(r)) = (d.forms.owned[form as usize], d.forms.record(form)) {
                // FUN_1402fc860: these three hold a Keyblade of their own.
                let weapon = matches!(form, forms::VALOR | forms::MASTER | forms::FINAL)
                    .then(|| game.ability_source.equipment_abilities.get(&r.keyblade))
                    .flatten();
                rows.push((form, r.abilities.to_vec(), weapon.copied().unwrap_or(0)));
            }
        }
    }
    if world == LION_WORLD {
        let at = save::state::FORMS + 6 * save::state::FORM_BYTES + 8;
        let image = &game.save.base;
        let slots = (0..save::state::FORM_ABILITY_SLOTS)
            .map(|k| image.u16_at(at + k * 2))
            .collect();
        rows.push((7, slots, 0));
    }
    let texts = &c.source.status_texts;
    rows.into_iter()
        .map(|(bit, slots, weapon_ability)| {
            let name = if bit == 7 {
                texts.get(LION_NAME)
            } else {
                texts.get(FORM_NAMES[bit as usize - 1])
            };
            let (mut order, mut seen) = c.form_marks.get(&bit).cloned().unwrap_or_default();
            let new = new_marks(&mut order, &mut seen, &slots, 48);
            FormAbilities {
                bit,
                name,
                own: MemberAbilities {
                    slots,
                    weapon_ability,
                    order,
                    new,
                    seen,
                    ..MemberAbilities::default()
                },
            }
        })
        .collect()
}

/// The rows of `bits` 1..=6 in list order.
pub fn entries(t: &Texts, n: &Numbers) -> Vec<pause_menu::StatusEntry> {
    let mut rows = Vec::new();
    for f in &n.forms {
        let i = f.form as usize - 1;
        let mut stats = vec![None; 20];
        let helps = FORM_STAT_HELPS[i];
        let values = [
            format!("{}", f.level),
            format!("{}", f.exp),
            format!("{}", f.next),
            format!("{}", f.level + 2),
        ];
        for (k, value) in values.iter().enumerate() {
            let (label, style) = FORM_LABELS[k];
            stats[k] = t.stat(label, helps[k], style, value);
        }
        for (k, (label, help, style)) in COMBAT_ROWS.iter().enumerate() {
            let value = match k {
                0 => format!("{}", f.strength),
                1 => format!("{}", f.magic),
                2 => format!("{}", f.defense),
                r => format!("{}%", 100 - i32::from(f.resist[r - 3])),
            };
            stats[10 + k] = t.stat(*label, *help, *style, &value);
        }
        rows.push(pause_menu::StatusEntry {
            bit: f.form,
            label: t.get(FORM_NAMES[i]),
            help: t.get(FORM_HELPS[i]),
            stats,
        });
    }
    if let Some((level, exp, next, owned)) = n.summon {
        let mut stats = vec![None; 20];
        let values = [
            format!("{level}"),
            format!("{exp}"),
            format!("{next}"),
            format!("{}", level + 2),
        ];
        for (k, value) in values.iter().enumerate() {
            let (label, help, style) = SUMMON_ROWS[k];
            stats[k] = t.stat(label, help, style, value);
        }
        for (k, (label, help)) in SUMMON_ITEM_ROWS.iter().enumerate() {
            if owned[k] {
                stats[10 + k] = t.stat(*label, *help, 4, "");
            }
        }
        rows.push(pause_menu::StatusEntry {
            bit: 6,
            label: t.get(SUMMON_NAME),
            help: t.get(SUMMON_HELP),
            stats,
        });
    }
    rows
}

/// Sora's rows that wait on a form (camp-04 §2.4).
pub fn sora_rows(t: &Texts, n: &Numbers, stats: &mut [Option<pause_menu::Stat>]) {
    for (bit, label, help, style, form) in SORA_ROWS {
        let Some(f) = n.forms.iter().find(|f| f.form == form) else {
            continue;
        };
        let value = if bit == 6 {
            format!("{}/{}", n.drive.0, n.drive.1)
        } else {
            format!("{}", f.level)
        };
        stats[bit] = t.stat(label, help, style, &value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(id: u8, exp: u32) -> FormLevel {
        FormLevel {
            id,
            growth: 0,
            ability: 0,
            exp,
        }
    }

    #[test]
    fn exp_counts_the_levels_below_and_next_stops_at_the_cap() {
        let fmlv = [row(0x11, 10), row(0x12, 20), row(0x13, 40), row(0x14, 80)];
        // FUN_1403e87f0: level 3 with 5 EXP is 10 + 20 + 5.
        assert_eq!(total_exp(&fmlv, 1, 3, 5), 35);
        assert_eq!(total_exp(&fmlv, 1, 1, 5), 5);
        assert_eq!(to_next(&fmlv, 1, 3, 5, 7), 35);
        assert_eq!(to_next(&fmlv, 1, 3, 5, 3), 0);
    }

    #[test]
    fn the_summon_cap_skips_six() {
        assert_eq!(summon_cap(&[true, false, false, false]), 3);
        assert_eq!(summon_cap(&[true, true, true, true]), 7);
    }

    #[test]
    fn rows_list_the_forms_then_the_summon() {
        let t = Texts::default();
        let f = |form| FormNumbers {
            form,
            level: 2,
            exp: 1,
            next: 1,
            strength: 1,
            magic: 1,
            defense: 1,
            resist: [100; 4],
        };
        let n = Numbers {
            forms: vec![f(1), f(4)],
            summon: Some((1, 0, 0, [true, false, false, true])),
            ..Numbers::default()
        };
        let rows = entries(&t, &n);
        assert_eq!(rows.iter().map(|r| r.bit).collect::<Vec<_>>(), [1, 4, 6]);
        // Rows 0..3 and 10..16 for a form; only the owned summons' for Summon.
        assert_eq!(rows[0].stats.iter().flatten().count(), 11);
        assert_eq!(rows[2].stats.iter().flatten().count(), 6);
        assert!(rows[2].stats[10].is_some() && rows[2].stats[11].is_none());
    }
}
