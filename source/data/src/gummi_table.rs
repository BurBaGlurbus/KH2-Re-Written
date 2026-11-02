//! The Gummi flight's rank and treasure tables (`gumibattle/rank.bin`,
//! `gumibattle/treasure.bin`). Each is u32 version, u32 count and the
//! records from +8 (FUN_14014c780 reads them with the exe's own format
//! strings, "4c 16i 60c" and "2i 4c 1i 16c"). Also the Gummi menu's item
//! records (`gumimenu/menuentry.bar`) and its layout transitions
//! (`gumimenu/<lang>/gumimenu.bar` entry `0004`).

/// A rank.bin record (0x80 bytes): the thresholds of one route and level.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RankRow {
    /// 0..=8.
    pub route: u8,
    /// 1..=3.
    pub level: u8,
    /// What is ranked (FUN_140207400): 0 the medal value, 1 ships
    /// destroyed, 2 the score.
    pub kind: u8,
    /// The value that reaches rank n + 1 is `thresholds[n]`.
    pub thresholds: [i32; 16],
}

/// A treasure.bin record (0x20 bytes).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Treasure {
    /// The obtained bit (save +0x10f60, a bitset of u32 words).
    pub bit: u32,
    pub item: u32,
    pub route: u8,
    /// 1..=3, plus 100 for EX.
    pub level: u8,
    /// 0 for a rank prize, else the treasure's place in the stage.
    pub stage: u8,
    /// The prize slot of a rank prize, 0..=6.
    pub slot: u8,
    pub flag: u32,
}

fn records(bytes: &[u8], size: usize) -> Option<impl Iterator<Item = &[u8]>> {
    let count = u32::from_le_bytes(bytes.get(4..8)?.try_into().ok()?) as usize;
    let body = bytes.get(8..8 + count.checked_mul(size)?)?;
    Some(body.chunks_exact(size))
}

pub fn read_rank(bytes: &[u8]) -> Option<Vec<RankRow>> {
    records(bytes, 0x80)?
        .map(|r| {
            let mut thresholds = [0i32; 16];
            for (k, t) in thresholds.iter_mut().enumerate() {
                *t = i32::from_le_bytes(r[4 + k * 4..8 + k * 4].try_into().ok()?);
            }
            Some(RankRow {
                route: r[0],
                level: r[1],
                kind: r[2],
                thresholds,
            })
        })
        .collect()
}

pub fn read_treasure(bytes: &[u8]) -> Option<Vec<Treasure>> {
    records(bytes, 0x20)?
        .map(|r| {
            let word = |at: usize| u32::from_le_bytes(r[at..at + 4].try_into().unwrap());
            Some(Treasure {
                bit: word(0),
                item: word(4),
                route: r[8],
                level: r[9],
                stage: r[10],
                slot: r[11],
                flag: word(12),
            })
        })
        .collect()
}

/// A `menuentry.bar` record (0x100 bytes, one BAR entry per item id): what
/// one 2D menu item plays (gm::MENUITEM, FUN_140270e80).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MenuEntry {
    pub name: String,
    /// The 2LD it plays from (+0x40), e.g. "gummi_menu_lay".
    pub layout: String,
    /// The layout's sequence and texture (+0x60, +0x64).
    pub sequence: i32,
    pub texture: i32,
    /// Appear, loop and close while selected (+0x68..).
    pub selected: [i32; 3],
    /// Appear, loop and close otherwise (+0x74..).
    pub normal: [i32; 3],
    /// The message drawn at the loop group's IS_ACTIVE anchor (+0x80); -1
    /// none.
    pub text: i32,
}

fn name(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).into_owned()
}

fn int(r: &[u8], at: usize) -> i32 {
    i32::from_le_bytes(r[at..at + 4].try_into().unwrap())
}

impl MenuEntry {
    pub fn read(r: &[u8]) -> Option<Self> {
        let r = r.get(..0x84)?;
        Some(Self {
            name: name(&r[..0x40]),
            layout: name(&r[0x40..0x60]),
            sequence: int(r, 0x60),
            texture: int(r, 0x64),
            selected: [int(r, 0x68), int(r, 0x6c), int(r, 0x70)],
            normal: [int(r, 0x74), int(r, 0x78), int(r, 0x7c)],
            text: int(r, 0x80),
        })
    }
}

/// `menuentry.bar`: the records in BAR order, which is their id.
pub fn read_menu_entries(bar: &[u8]) -> Option<Vec<MenuEntry>> {
    let table = crate::bar::read_bar(bar).ok()?;
    table
        .entries
        .iter()
        .map(|e| {
            let at = usize::try_from(e.offset).ok()?;
            MenuEntry::read(bar.get(at..at + e.size as usize)?)
        })
        .collect()
}

/// A layout transition (FUN_140252e50): going from one menu screen to
/// another plays `scenes[0]`, then the last scene once it ends, and turns the
/// 3D background by `yaw` (clamped to ±2.05 rad) at `speed` a frame.
#[derive(Clone, Debug, PartialEq)]
pub struct Transition {
    pub layout: String,
    /// Screen names ("none", "top", "mission", ...).
    pub from: String,
    pub to: String,
    pub yaw: f32,
    pub speed: f32,
    pub scenes: Vec<i32>,
}

/// `gumimenu.bar` `0004`: u32 count, then 0x80-byte records.
pub fn read_transitions(bytes: &[u8]) -> Option<Vec<Transition>> {
    let count = u32::from_le_bytes(bytes.get(..4)?.try_into().ok()?) as usize;
    let body = bytes.get(4..4 + count.checked_mul(0x80)?)?;
    Some(
        body.as_chunks::<0x80>()
            .0
            .iter()
            .map(|r| Transition {
                layout: name(&r[..0x20]),
                from: name(&r[0x20..0x30]),
                to: name(&r[0x30..0x40]),
                yaw: f32::from_le_bytes(r[0x44..0x48].try_into().unwrap()),
                speed: f32::from_le_bytes(r[0x4c..0x50].try_into().unwrap()),
                scenes: r[0x50..]
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .map(|w| i32::from_le_bytes(*w))
                    .take_while(|&v| v >= 0)
                    .collect(),
            })
            .collect(),
    )
}

/// A digit font (`gumimenu.bar` `0005`, a BAR of six 0x80-byte "MENU"
/// records; FUN_14026aed0): the groups of the digits 0 to 9 on a sequence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DigitStyle {
    pub layout: String,
    pub sequence: i16,
    pub texture: i16,
    pub digits: [i16; 10],
    /// +0x38 [I: what the width is read from].
    pub extra: i16,
}

pub fn read_digit_styles(bar: &[u8]) -> Option<Vec<DigitStyle>> {
    let table = crate::bar::read_bar(bar).ok()?;
    table
        .entries
        .iter()
        .map(|e| {
            let at = usize::try_from(e.offset).ok()?;
            let r = bar.get(at..at + 0x3a)?;
            let h = |o: usize| i16::from_le_bytes([r[o], r[o + 1]]);
            Some(DigitStyle {
                layout: name(&r[..0x20]),
                sequence: h(0x20),
                texture: h(0x22),
                digits: std::array::from_fn(|k| h(0x24 + 2 * k)),
                extra: h(0x38),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn menu_records_from_generated_bytes() {
        let mut r = vec![0u8; 0x100];
        r[..9].copy_from_slice(b"TOP_START");
        r[0x40..0x4e].copy_from_slice(b"gummi_menu_lay");
        for (k, v) in [5i32, 5, 63, 64, 62, 66, 67, 65, 0x18ff].iter().enumerate() {
            r[0x60 + k * 4..0x64 + k * 4].copy_from_slice(&v.to_le_bytes());
        }
        assert_eq!(
            MenuEntry::read(&r),
            Some(MenuEntry {
                name: "TOP_START".into(),
                layout: "gummi_menu_lay".into(),
                sequence: 5,
                texture: 5,
                selected: [63, 64, 62],
                normal: [66, 67, 65],
                text: 0x18ff,
            })
        );
        assert_eq!(MenuEntry::read(&r[..0x80]), None);

        let mut t = 2u32.to_le_bytes().to_vec();
        for (from, to, yaw, scenes) in [
            ("none", "top", 0f32, [34i32, 35]),
            ("top", "mission", -2.26, [16, 17]),
        ] {
            let mut rec = vec![0u8; 0x80];
            rec[..14].copy_from_slice(b"gummi_menu_lay");
            rec[0x20..0x20 + from.len()].copy_from_slice(from.as_bytes());
            rec[0x30..0x30 + to.len()].copy_from_slice(to.as_bytes());
            rec[0x44..0x48].copy_from_slice(&yaw.to_le_bytes());
            rec[0x4c..0x50].copy_from_slice(&(1f32 / 60.).to_le_bytes());
            for (k, v) in scenes.iter().chain(&[-1]).enumerate() {
                rec[0x50 + k * 4..0x54 + k * 4].copy_from_slice(&v.to_le_bytes());
            }
            t.extend(rec);
        }
        let list = read_transitions(&t).unwrap();
        assert_eq!(
            (list[1].from.as_str(), list[1].to.as_str()),
            ("top", "mission")
        );
        assert_eq!((list[1].yaw, list[1].scenes.clone()), (-2.26, vec![16, 17]));
        assert!(read_transitions(&t[..t.len() - 1]).is_none());
    }

    #[test]
    fn rows_from_generated_bytes() {
        let mut rank = vec![1, 0, 0, 0, 2, 0, 0, 0];
        for (route, level) in [(4u8, 2u8), (8, 3)] {
            let mut r = vec![route, level, level - 1, 0];
            for k in 0..16i32 {
                r.extend((k * 10 + route as i32).to_le_bytes());
            }
            r.resize(0x80, 0);
            rank.extend(r);
        }
        let rows = read_rank(&rank).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!((rows[1].route, rows[1].level, rows[1].kind), (8, 3, 2));
        assert_eq!(rows[1].thresholds[15], 158);
        assert!(read_rank(&rank[..rank.len() - 1]).is_none());

        let mut treasure = vec![1, 0, 0, 0, 1, 0, 0, 0];
        treasure.extend(317u32.to_le_bytes());
        treasure.extend(59u32.to_le_bytes());
        treasure.extend([2, 103, 4, 0]);
        treasure.extend(7u32.to_le_bytes());
        treasure.resize(8 + 0x20, 0);
        assert_eq!(
            read_treasure(&treasure).unwrap(),
            vec![Treasure {
                bit: 317,
                item: 59,
                route: 2,
                level: 103,
                stage: 4,
                slot: 0,
                flag: 7,
            }]
        );
    }
}
