//! `shop` in 03system.bin (magic TZSH): the shops, their goods and the watch
//! list. Read by FUN_1402fbdf0; a shop id resolves to its goods with
//! FUN_1402fba40, FUN_1402fbbb0 and FUN_1402fb7e0.
use std::io;

const MAGIC: &[u8; 4] = b"TZSH";
const HEADER: usize = 0x10;
const ROW: usize = 0x18;
/// The watch list's length (u16 ids).
const WATCH: usize = 0x80;

/// Row flags (+0xe).
pub const CATEGORY_MASK: u16 = 0x1f;
pub const TOWN: u16 = 0x80;
pub const BUY_SELL: u16 = 0x100;
pub const SYNTHESIZE: u16 = 0x200;

/// A row's stock record: its goods when `flag` is negative or set.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Record {
    pub flag: i16,
    pub items: Vec<u16>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    /// The signal that opens the shop (its id).
    pub signal: i16,
    /// Set when the shop opens; town shops read it.
    pub visited: i16,
    /// The shop's name message.
    pub name: u16,
    /// The objentry id in the model frame, and its placement.
    pub model: u16,
    pub x: i16,
    pub y: i16,
    pub height: i16,
    pub flags: u16,
    pub records: Vec<Record>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Table {
    pub rows: Vec<Row>,
    pub watch: Vec<u16>,
}

/// What FUN_1402fba40 fills.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Goods {
    pub items: Vec<u16>,
    /// Byte +0xc bit 0: the Buy and Sell rows may show.
    pub buy_sell: bool,
    /// Byte +0xc bit 1.
    pub synthesize: bool,
    /// Byte +0xd: not a town shop, and its first record has goods.
    pub own_goods: bool,
}

fn truncated() -> io::Error {
    io::Error::new(io::ErrorKind::UnexpectedEof, "shop table is truncated")
}

fn u16_at(b: &[u8], at: usize) -> io::Result<u16> {
    let s = b.get(at..at + 2).ok_or_else(truncated)?;
    Ok(u16::from_le_bytes([s[0], s[1]]))
}

fn i16_at(b: &[u8], at: usize) -> io::Result<i16> {
    Ok(u16_at(b, at)? as i16)
}

fn u32_at(b: &[u8], at: usize) -> io::Result<usize> {
    let s = b.get(at..at + 4).ok_or_else(truncated)?;
    Ok(u32::from_le_bytes([s[0], s[1], s[2], s[3]]) as usize)
}

pub fn read(b: &[u8]) -> io::Result<Table> {
    if b.get(..4) != Some(MAGIC) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "not a TZSH shop table",
        ));
    }
    let count = i16_at(b, 6)?.max(0) as usize;
    let watch_at = u32_at(b, 0xc)?;
    let mut rows = Vec::with_capacity(count);
    for k in 0..count {
        let r = HEADER + ROW * k;
        let record_count = u16_at(b, r + 0x10)? as usize;
        let records_at = u32_at(b, r + 0x14)?;
        let mut records = Vec::with_capacity(record_count);
        for n in 0..record_count {
            let at = records_at + 8 * n;
            let item_count = u16_at(b, at + 2)? as usize;
            let items_at = u32_at(b, at + 4)?;
            let items = (0..item_count)
                .map(|i| u16_at(b, items_at + 2 * i))
                .collect::<io::Result<_>>()?;
            records.push(Record {
                flag: i16_at(b, at)?,
                items,
            });
        }
        rows.push(Row {
            signal: i16_at(b, r)?,
            visited: i16_at(b, r + 2)?,
            name: u16_at(b, r + 4)?,
            model: u16_at(b, r + 6)?,
            x: i16_at(b, r + 8)?,
            y: i16_at(b, r + 0xa)?,
            height: i16_at(b, r + 0xc)?,
            flags: u16_at(b, r + 0xe)?,
            records,
        });
    }
    let watch = (0..WATCH)
        .map(|i| u16_at(b, watch_at + 2 * i))
        .collect::<io::Result<_>>()?;
    Ok(Table { rows, watch })
}

/// FUN_1402fb7e0's category bit of an item kind (0: never sold).
pub fn category(kind: u8) -> u16 {
    match kind {
        0 | 1 => 8,
        2..=0xd => 1,
        0xe => 2,
        0xf => 4,
        0x10 => 0x10,
        _ => 0,
    }
}

impl Table {
    pub fn row(&self, signal: i16) -> Option<&Row> {
        self.rows.iter().find(|r| r.signal == signal)
    }

    /// FUN_1402fb960: a good's bit in the seen field.
    pub fn watch_index(&self, item: u16) -> Option<usize> {
        self.watch.iter().position(|&w| w == item)
    }

    /// FUN_1402fba40 without setting the visited flag (the caller's first
    /// step): a shop's own records, and for a town shop what FUN_1402fbbb0 adds
    /// from each other visited shop's always-on records, of this shop's
    /// categories.
    pub fn goods(
        &self,
        signal: i16,
        set: impl Fn(i16) -> bool,
        kind: impl Fn(u16) -> Option<u8>,
    ) -> Option<Goods> {
        let row = self.row(signal)?;
        let mut items = Vec::new();
        let mut add = |list: &[u16], mask: u16| {
            for &id in list {
                let bit = kind(id).map_or(0, category);
                if !items.contains(&id) && mask & bit != 0 {
                    items.push(id);
                }
            }
        };
        for r in &row.records {
            if r.flag < 0 || set(r.flag) {
                add(&r.items, CATEGORY_MASK);
            }
        }
        if row.flags & TOWN != 0 {
            for other in &self.rows {
                if other.signal == signal || other.flags & TOWN != 0 || !set(other.visited) {
                    continue;
                }
                for r in other.records.iter().filter(|r| r.flag < 0) {
                    add(&r.items, row.flags);
                }
            }
        }
        Some(Goods {
            items,
            buy_sell: row.flags & BUY_SELL != 0,
            synthesize: row.flags & SYNTHESIZE != 0,
            own_goods: row.flags & TOWN == 0
                && row.records.first().is_some_and(|r| !r.items.is_empty()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two rows: a town weapon shop 0x67 and a Moogle 0x70, with the watch list.
    fn synthetic() -> Vec<u8> {
        let mut b = vec![0u8; 0x400];
        b[..4].copy_from_slice(MAGIC);
        b[6..8].copy_from_slice(&2i16.to_le_bytes());
        let watch = 0x300usize;
        b[0xc..0x10].copy_from_slice(&(watch as u32).to_le_bytes());
        let row = |b: &mut Vec<u8>,
                   k: usize,
                   signal: i16,
                   visited: i16,
                   flags: u16,
                   records: usize,
                   at: usize| {
            let r = HEADER + ROW * k;
            b[r..r + 2].copy_from_slice(&signal.to_le_bytes());
            b[r + 2..r + 4].copy_from_slice(&visited.to_le_bytes());
            b[r + 4..r + 6].copy_from_slice(&0x1234u16.to_le_bytes());
            b[r + 6..r + 8].copy_from_slice(&0x487u16.to_le_bytes());
            b[r + 0xc..r + 0xe].copy_from_slice(&(-576i16).to_le_bytes());
            b[r + 0xe..r + 0x10].copy_from_slice(&flags.to_le_bytes());
            b[r + 0x10..r + 0x12].copy_from_slice(&(records as u16).to_le_bytes());
            b[r + 0x14..r + 0x18].copy_from_slice(&(at as u32).to_le_bytes());
        };
        let record = |b: &mut Vec<u8>, at: usize, flag: i16, items: &[u16], items_at: usize| {
            b[at..at + 2].copy_from_slice(&flag.to_le_bytes());
            b[at + 2..at + 4].copy_from_slice(&(items.len() as u16).to_le_bytes());
            b[at + 4..at + 8].copy_from_slice(&(items_at as u32).to_le_bytes());
            for (i, id) in items.iter().enumerate() {
                let p = items_at + 2 * i;
                b[p..p + 2].copy_from_slice(&id.to_le_bytes());
            }
        };
        row(&mut b, 0, 0x67, 41, TOWN | BUY_SELL | 1, 1, 0x100);
        record(&mut b, 0x100, -1, &[10], 0x180);
        row(&mut b, 1, 0x70, 50, 0x31f, 2, 0x110);
        // Its goods: a weapon, a potion; a material behind flag 73.
        record(&mut b, 0x110, -1, &[20, 30], 0x190);
        record(&mut b, 0x118, 73, &[40], 0x1a0);
        for (i, id) in [10u16, 20, 30, 40].iter().enumerate() {
            let p = watch + 2 * i;
            b[p..p + 2].copy_from_slice(&id.to_le_bytes());
        }
        b
    }

    fn kind(id: u16) -> Option<u8> {
        match id {
            10 | 20 => Some(2),
            30 => Some(0),
            40 => Some(0x10),
            _ => None,
        }
    }

    #[test]
    fn it_reads_rows_records_and_the_watch_list() {
        let t = read(&synthetic()).unwrap();
        assert_eq!(t.rows.len(), 2);
        assert_eq!(t.rows[0].signal, 0x67);
        assert_eq!((t.rows[0].model, t.rows[0].height), (0x487, -576));
        assert_eq!(t.rows[1].records[1].flag, 73);
        assert_eq!(t.rows[1].records[0].items, vec![20, 30]);
        assert_eq!(t.watch_index(30), Some(2));
        assert_eq!(t.watch_index(99), None);
    }

    #[test]
    fn a_town_shop_gains_its_categories_from_visited_shops_only() {
        let t = read(&synthetic()).unwrap();
        let none = t.goods(0x67, |_| false, kind).unwrap();
        assert_eq!(none.items, vec![10]);
        assert!(none.buy_sell && !none.synthesize && !none.own_goods);
        // Mog visited: the weapon comes, not the potion or the gated material.
        let visited = t.goods(0x67, |f| f == 50, kind).unwrap();
        assert_eq!(visited.items, vec![10, 20]);
    }

    #[test]
    fn a_moogle_sells_every_category_and_materials_behind_their_flag() {
        let t = read(&synthetic()).unwrap();
        let g = t.goods(0x70, |_| false, kind).unwrap();
        assert_eq!(g.items, vec![20, 30]);
        assert!(g.buy_sell && g.synthesize && g.own_goods);
        let g = t.goods(0x70, |f| f == 73, kind).unwrap();
        assert_eq!(g.items, vec![20, 30, 40]);
    }

    #[test]
    fn it_refuses_other_files() {
        assert!(read(b"NOPE").is_err());
    }
}
