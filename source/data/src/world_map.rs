//! World map tables: the `00progress.bin` entries `wmky`, `wldf`, `epsd`,
//! `wlds`, `link` and `dark`, the `land` entry of `70landing.bar` and
//! `00worldpoint.bin`. Each 00progress / landing table is a `u32` version
//! and a `u32` record count followed by fixed records; the exe's readers
//! (FUN_14038d070, FUN_1403df3c0, FUN_1403f0910, FUN_1403e3a00,
//! FUN_1403f0ff0, FUN_1403f0c90, FUN_1403f14f0) only swap them on
//! big-endian data, and the PC files are little-endian.
use std::io;

fn invalid(what: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, what)
}

fn u16_at(b: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([b[at], b[at + 1]])
}

fn u32_at(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(b[at..at + 4].try_into().unwrap())
}

/// The `{u32 version, u32 count}` header and `count` records of `size`
/// bytes from +8.
fn records<'a>(bytes: &'a [u8], size: usize, what: &'static str) -> io::Result<Vec<&'a [u8]>> {
    let count = bytes.get(4..8).map(|w| u32_at(w, 0)).ok_or(invalid(what))? as usize;
    let body = bytes
        .get(8..8 + count.checked_mul(size).ok_or(invalid(what))?)
        .ok_or(invalid(what))?;
    Ok(body.chunks_exact(size).collect())
}

/// A `wmky` record (0x10, "1i 6s"): a world and three `{flagA, flagB}`
/// story flag pairs; the world info panel counts the pairs whose first flag
/// is set (FUN_14038d330) and those with both set (FUN_14038d2c0).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VisitKeys {
    pub world: i32,
    pub pairs: [[u16; 2]; 3],
}

pub fn read_visit_keys(bytes: &[u8]) -> io::Result<Vec<VisitKeys>> {
    Ok(records(bytes, 0x10, "wmky truncated")?
        .into_iter()
        .map(|r| VisitKeys {
            world: u32_at(r, 0) as i32,
            pairs: [0, 1, 2].map(|k| [u16_at(r, 4 + k * 4), u16_at(r, 6 + k * 4)]),
        })
        .collect())
}

/// One world event script operation: `u16 op`, `u16 n`, `n` `u32`
/// arguments (stepped by FUN_1403fc730; FUN_1403deb40 uses each argument's
/// low half).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScriptOp {
    pub op: u16,
    pub args: Vec<u32>,
}

/// A `wldf` record (0x10, "4c 3i"): a world event.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorldEvent {
    /// +0: event id (bit index of the save's world-event bits).
    pub id: u8,
    /// +1: 1 = not queued while story flag 0x102b is set (FUN_14039ac80).
    pub kind: u8,
    /// +2: order in which queued events play (FUN_14038da10).
    pub priority: u8,
    /// +4: the story flag that queues it (FUN_1403df1b0).
    pub trigger: u32,
    /// +8: the world map event played for it (0: none).
    pub event: u32,
    /// +0xc: script offset from the table start (0: none), decoded.
    pub script: Vec<ScriptOp>,
}

fn read_script(bytes: &[u8], mut at: usize) -> io::Result<Vec<ScriptOp>> {
    let mut ops = Vec::new();
    loop {
        let head = bytes
            .get(at..at + 4)
            .ok_or(invalid("wldf script truncated"))?;
        let op = u16_at(head, 0);
        if op == 0 {
            return Ok(ops);
        }
        let n = u16_at(head, 2) as usize;
        let args = bytes
            .get(at + 4..at + 4 + n * 4)
            .ok_or(invalid("wldf script truncated"))?
            .as_chunks::<4>()
            .0
            .iter()
            .map(|w| u32::from_le_bytes(*w))
            .collect();
        ops.push(ScriptOp { op, args });
        at += 4 + n * 4;
    }
}

pub fn read_world_events(bytes: &[u8]) -> io::Result<Vec<WorldEvent>> {
    records(bytes, 0x10, "wldf truncated")?
        .into_iter()
        .map(|r| {
            let offset = u32_at(r, 0xc) as usize;
            Ok(WorldEvent {
                id: r[0],
                kind: r[1],
                priority: r[2],
                trigger: u32_at(r, 4),
                event: u32_at(r, 8),
                script: if offset == 0 {
                    Vec::new()
                } else {
                    read_script(bytes, offset)?
                },
            })
        })
        .collect()
}

/// An `epsd` record (8, "1s 2c 2s"): an entry of a world's landing-menu
/// list, open while its bit is set in the save's episode bits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Episode {
    pub bit: u16,
    pub world: u8,
    pub index: u8,
    /// System message ids: the row text and its help line (FUN_140392680).
    pub title: u16,
    pub help: u16,
}

pub fn read_episodes(bytes: &[u8]) -> io::Result<Vec<Episode>> {
    Ok(records(bytes, 8, "epsd truncated")?
        .into_iter()
        .map(|r| Episode {
            bit: u16_at(r, 0),
            world: r[2],
            index: r[3],
            title: u16_at(r, 4),
            help: u16_at(r, 6),
        })
        .collect())
}

/// A `wlds` record (4): where landing on a world without a touched world
/// point arrives, for the world record's landing set (FUN_1403e4190).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DefaultLanding {
    pub world: u8,
    pub set: u8,
    /// An area, or (≥ 0x33) a spawn id resolved by the world's spawn table
    /// (FUN_14039fbc0).
    pub area: u8,
    pub entrance: u8,
}

pub fn read_default_landings(bytes: &[u8]) -> io::Result<Vec<DefaultLanding>> {
    Ok(records(bytes, 4, "wlds truncated")?
        .into_iter()
        .map(|r| DefaultLanding {
            world: r[0],
            set: r[1],
            area: r[2],
            entrance: r[3],
        })
        .collect())
}

/// A condition of a `link` record: `kind` 1 story flag, 2 world-event bit,
/// 3 menu flag; 0 never holds.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Condition {
    pub kind: u16,
    pub id: u16,
}

/// A `link` record (0x3c, "2s 26s 1i"): `result` is set once `need` of the
/// 13 conditions hold (FUN_1403f0ea0).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Link {
    pub result: Condition,
    pub conditions: [Condition; 13],
    pub need: i32,
}

pub fn read_links(bytes: &[u8]) -> io::Result<Vec<Link>> {
    let condition = |r: &[u8], at: usize| Condition {
        kind: u16_at(r, at),
        id: u16_at(r, at + 2),
    };
    Ok(records(bytes, 0x3c, "link truncated")?
        .into_iter()
        .map(|r| Link {
            result: condition(r, 0),
            conditions: std::array::from_fn(|k| condition(r, 4 + k * 4)),
            need: u32_at(r, 0x38) as i32,
        })
        .collect())
}

/// A `dark` record (0x18, "2i 2d"): map draw groups switched on and off
/// for a draw-group set id (FUN_1403f0c00).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DrawGroupSet {
    pub id: i32,
    pub on: u64,
    pub off: u64,
}

pub fn read_draw_group_sets(bytes: &[u8]) -> io::Result<Vec<DrawGroupSet>> {
    Ok(records(bytes, 0x18, "dark truncated")?
        .into_iter()
        .map(|r| DrawGroupSet {
            id: u32_at(r, 0) as i32,
            on: u64::from_le_bytes(r[8..16].try_into().unwrap()),
            off: u64::from_le_bytes(r[16..24].try_into().unwrap()),
        })
        .collect())
}

/// A `land` record (4) of `70landing.bar`: a world point's cell in its
/// world's landing grid (FUN_140394550).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LandingCell {
    pub point: u8,
    pub world: u8,
    /// 1-based grid cell.
    pub cell: u8,
    /// Index of the thumbnail picture.
    pub picture: u8,
}

pub fn read_landing_cells(bytes: &[u8]) -> io::Result<Vec<LandingCell>> {
    Ok(records(bytes, 4, "land truncated")?
        .into_iter()
        .map(|r| LandingCell {
            point: r[0],
            world: r[1],
            cell: r[2],
            picture: r[3],
        })
        .collect())
}

/// A `00worldpoint.bin` record (4): a world point (save point) and where
/// landing on it arrives (FUN_1403f2070). The file has no header; the exe
/// searches 0x36 records (FUN_1403f2030).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WorldPoint {
    pub world: u8,
    pub area: u8,
    pub entrance: u8,
}

/// Records searched by FUN_1403f2030.
pub const WORLD_POINTS: usize = 0x36;

pub fn read_world_points(bytes: &[u8]) -> io::Result<Vec<WorldPoint>> {
    let body = bytes
        .get(..WORLD_POINTS * 4)
        .ok_or(invalid("00worldpoint truncated"))?;
    Ok(body
        .as_chunks::<4>()
        .0
        .iter()
        .map(|r| WorldPoint {
            world: r[0],
            area: r[1],
            entrance: r[2],
        })
        .collect())
}

/// `07localset.bin` (FUN_14039fc50 → `DAT_1429fb4a0`): per world (its n-th
/// BAR type-2 entry) the `{u16 program, u16 area}` records sorted by program
/// that FUN_14039fbc0 bsearches to place a jump by program id.
pub fn read_program_areas(bytes: &[u8]) -> io::Result<Vec<Vec<(u16, u16)>>> {
    let table = crate::bar::read_bar(bytes)?;
    let mut out = Vec::new();
    for e in table.entries.iter().filter(|e| e.entry_type == 2) {
        let body = bytes
            .get(e.offset as usize..e.offset as usize + e.size as usize)
            .ok_or(invalid("07localset entry outside the file"))?;
        let count = body
            .get(4..8)
            .map(|c| i32::from_le_bytes(c.try_into().unwrap()).max(0) as usize)
            .ok_or(invalid("07localset entry truncated"))?;
        let records = body
            .get(8..8 + count * 4)
            .ok_or(invalid("07localset records truncated"))?;
        out.push(
            records
                .as_chunks::<4>()
                .0
                .iter()
                .map(|r| {
                    (
                        u16::from_le_bytes([r[0], r[1]]),
                        u16::from_le_bytes([r[2], r[3]]),
                    )
                })
                .collect(),
        );
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn program_areas_read_per_world() {
        let body: Vec<u8> = [1u32, 2]
            .iter()
            .flat_map(|w| w.to_le_bytes())
            .chain([0x33u16, 9, 0x40, 4].iter().flat_map(|h| h.to_le_bytes()))
            .collect();
        let mut bytes = b"BAR\x01".to_vec();
        bytes.extend(1u32.to_le_bytes());
        bytes.extend([0; 8]);
        bytes.extend(2u16.to_le_bytes());
        bytes.extend(0u16.to_le_bytes());
        bytes.extend(*b"loca");
        bytes.extend(0x20u32.to_le_bytes());
        bytes.extend((body.len() as u32).to_le_bytes());
        bytes.extend(&body);
        assert_eq!(
            read_program_areas(&bytes).unwrap(),
            vec![vec![(0x33, 9), (0x40, 4)]]
        );
    }

    fn table(records: &[&[u8]]) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend(1u32.to_le_bytes());
        out.extend((records.len() as u32).to_le_bytes());
        for r in records {
            out.extend(*r);
        }
        out
    }

    #[test]
    fn world_events_decode_their_scripts() {
        // One event with a script after the table: op 3 on worlds 2 and 4,
        // op 15 set 2, end.
        let mut record = vec![5, 1, 9, 0];
        record.extend(0x105eu32.to_le_bytes());
        record.extend(0x1f7u32.to_le_bytes());
        record.extend(0x18u32.to_le_bytes());
        let mut bytes = table(&[&record]);
        assert_eq!(bytes.len(), 0x18);
        for w in [3u16, 2] {
            bytes.extend(w.to_le_bytes());
        }
        bytes.extend(2u32.to_le_bytes());
        bytes.extend(4u32.to_le_bytes());
        for w in [15u16, 1] {
            bytes.extend(w.to_le_bytes());
        }
        bytes.extend(2u32.to_le_bytes());
        bytes.extend([0, 0, 0, 0]);
        let events = read_world_events(&bytes).unwrap();
        assert_eq!(events.len(), 1);
        let e = &events[0];
        assert_eq!(
            (e.id, e.kind, e.priority, e.trigger, e.event),
            (5, 1, 9, 0x105e, 0x1f7)
        );
        assert_eq!(
            e.script,
            vec![
                ScriptOp {
                    op: 3,
                    args: vec![2, 4]
                },
                ScriptOp {
                    op: 15,
                    args: vec![2]
                }
            ]
        );
        // A truncated script is an error, not a silent stop.
        assert!(read_world_events(&bytes[..bytes.len() - 6]).is_err());
    }

    #[test]
    fn link_records_hold_thirteen_conditions_and_a_count() {
        let mut r = Vec::new();
        for w in [1u16, 0x1834, 1, 0x200b, 2, 7] {
            r.extend(w.to_le_bytes());
        }
        r.resize(0x38, 0);
        r.extend(2i32.to_le_bytes());
        let links = read_links(&table(&[&r])).unwrap();
        assert_eq!(
            links[0].result,
            Condition {
                kind: 1,
                id: 0x1834
            }
        );
        assert_eq!(
            links[0].conditions[0],
            Condition {
                kind: 1,
                id: 0x200b
            }
        );
        assert_eq!(links[0].conditions[1], Condition { kind: 2, id: 7 });
        assert_eq!(links[0].conditions[12], Condition::default());
        assert_eq!(links[0].need, 2);
    }

    #[test]
    fn small_tables() {
        let keys = read_visit_keys(&table(&[&[
            4, 0, 0, 0, 0x5c, 0x10, 0x5e, 0x10, 0x76, 0x10, 0x77, 0x10, 0, 0, 0, 0,
        ]]))
        .unwrap();
        assert_eq!(keys[0].world, 4);
        assert_eq!(keys[0].pairs, [[0x105c, 0x105e], [0x1076, 0x1077], [0, 0]]);
        let eps = read_episodes(&table(&[&[0x20, 0, 2, 1, 0x3b, 0x4c, 0x3c, 0x4c]])).unwrap();
        assert_eq!(
            eps[0],
            Episode {
                bit: 0x20,
                world: 2,
                index: 1,
                title: 0x4c3b,
                help: 0x4c3c
            }
        );
        let lands = read_default_landings(&table(&[&[6, 2, 3, 0]])).unwrap();
        assert_eq!(lands[0].area, 3);
        let mut set = 2i32.to_le_bytes().to_vec();
        set.extend([0; 4]);
        set.extend(0xaa9004u64.to_le_bytes());
        set.extend(0x6860554fe8u64.to_le_bytes());
        let sets = read_draw_group_sets(&table(&[&set])).unwrap();
        assert_eq!(
            (sets[0].id, sets[0].on, sets[0].off),
            (2, 0xaa9004, 0x6860554fe8)
        );
        let cells = read_landing_cells(&table(&[&[47, 2, 1, 46]])).unwrap();
        assert_eq!(cells[0].cell, 1);
        let mut points = vec![0u8; WORLD_POINTS * 4];
        points[..4].copy_from_slice(&[7, 2, 0x63, 0]);
        let points = read_world_points(&points).unwrap();
        assert_eq!(points.len(), WORLD_POINTS);
        assert_eq!(
            points[0],
            WorldPoint {
                world: 7,
                area: 2,
                entrance: 0x63
            }
        );
        assert!(read_world_points(&[0; 8]).is_err());
        assert!(read_episodes(&table(&[&[1, 2, 3]])).is_err());
    }
}
