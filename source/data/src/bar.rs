//! BAR table inspection, based on the pinned OpenKH reader; no entry type is guessed.
use serde::Serialize;
use std::io;

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct BarEntry {
    pub entry_type: u16,
    pub index: i16,
    pub name_bytes: [u8; 4],
    pub offset: i32,
    pub size: u32,
}

#[derive(Debug, Serialize)]
pub struct BarInventory {
    pub header_field_8: i32,
    pub header_field_12: i32,
    pub absent_offsets: usize,
    pub table_overlapping_entries: usize,
    pub entries: Vec<BarEntry>,
}

pub fn read_bar(bytes: &[u8]) -> io::Result<BarInventory> {
    let invalid = |message| io::Error::new(io::ErrorKind::InvalidData, message);
    if bytes.len() < 16 || !bytes.starts_with(b"BAR\x01") {
        return Err(invalid("invalid BAR signature/header"));
    }
    let count = i32::from_le_bytes(bytes[4..8].try_into().unwrap());
    if count < 0 {
        return Err(invalid("negative BAR entry count"));
    }
    let table_end = 16u64 + count as u64 * 16;
    if table_end > bytes.len() as u64 {
        return Err(invalid("truncated BAR table"));
    }
    let mut entries = Vec::with_capacity(count as usize);
    let mut absent_offsets = 0;
    let mut table_overlapping_entries = 0;
    for record in bytes[16..table_end as usize].as_chunks::<16>().0 {
        let offset = i32::from_le_bytes(record[8..12].try_into().unwrap());
        let size = i32::from_le_bytes(record[12..16].try_into().unwrap());
        if offset < -1 || size < 0 {
            return Err(invalid("invalid signed BAR range"));
        }
        if offset == -1 {
            absent_offsets += 1;
        } else {
            if offset as u64 + size as u64 > bytes.len() as u64 {
                return Err(invalid("BAR payload range exceeds original"));
            }
            if size != 0 && (offset as u64) < table_end {
                table_overlapping_entries += 1;
            }
        }
        entries.push(BarEntry {
            entry_type: u16::from_le_bytes(record[..2].try_into().unwrap()),
            index: i16::from_le_bytes(record[2..4].try_into().unwrap()),
            name_bytes: record[4..8].try_into().unwrap(),
            offset,
            size: size as u32,
        });
    }
    Ok(BarInventory {
        header_field_8: i32::from_le_bytes(bytes[8..12].try_into().unwrap()),
        header_field_12: i32::from_le_bytes(bytes[12..16].try_into().unwrap()),
        absent_offsets,
        table_overlapping_entries,
        entries,
    })
}

/// One BAR record with its payload, as `write_bar` lays it out again.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BarItem {
    pub entry_type: u16,
    pub index: i16,
    pub name_bytes: [u8; 4],
    pub data: BarData,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BarData {
    /// Offset -1; the recorded size is kept as it is.
    Absent {
        size: u32,
    },
    Bytes(Vec<u8>),
}

/// Where an original placed its payloads, and the bytes no record covers.
/// Installed BARs do not follow one packing rule: some payloads are
/// unaligned or share an offset, some files end unpadded, and padding and
/// unreferenced tails are not always zero. Keeping them lets an unedited
/// BAR be written back byte for byte.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BarLayout {
    pub len: usize,
    /// Each record's offset, in table order (-1 for absent ones).
    pub offsets: Vec<i32>,
    /// Runs of bytes after the table that no payload covers.
    pub filler: Vec<(usize, Vec<u8>)>,
}

/// A whole BAR: the header words, every record's payload and, when it was
/// read from a file, that file's layout.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bar {
    pub header_field_8: i32,
    pub header_field_12: i32,
    pub items: Vec<BarItem>,
    pub layout: Option<BarLayout>,
}

pub fn parse_bar(bytes: &[u8]) -> io::Result<Bar> {
    let table = read_bar(bytes)?;
    let table_end = 16 + 16 * table.entries.len();
    let mut covered = vec![false; bytes.len()];
    for e in table.entries.iter().filter(|e| e.offset >= 0) {
        let start = (e.offset as usize).max(table_end);
        let end = e.offset as usize + e.size as usize;
        if start < end {
            covered[start..end].fill(true);
        }
    }
    let mut filler = Vec::new();
    let mut at = table_end;
    while at < bytes.len() {
        let run = covered[at..].iter().take_while(|c| !**c).count();
        if run > 0 {
            filler.push((at, bytes[at..at + run].to_vec()));
        }
        at += run;
        at += covered[at..].iter().take_while(|c| **c).count();
    }
    let offsets = table.entries.iter().map(|e| e.offset).collect();
    let items = table
        .entries
        .into_iter()
        .map(|e| BarItem {
            entry_type: e.entry_type,
            index: e.index,
            name_bytes: e.name_bytes,
            data: if e.offset == -1 {
                BarData::Absent { size: e.size }
            } else {
                let start = e.offset as usize;
                BarData::Bytes(bytes[start..start + e.size as usize].to_vec())
            },
        })
        .collect();
    Ok(Bar {
        header_field_8: table.header_field_8,
        header_field_12: table.header_field_12,
        items,
        layout: Some(BarLayout {
            len: bytes.len(),
            offsets,
            filler,
        }),
    })
}

/// The recorded offsets, if they still fit the items: absent records stay
/// absent and every payload ends inside the recorded length.
fn recorded_offsets(bar: &Bar) -> Option<(&BarLayout, Vec<i32>)> {
    let layout = bar.layout.as_ref()?;
    let fits = layout.offsets.len() == bar.items.len()
        && bar
            .items
            .iter()
            .zip(&layout.offsets)
            .all(|(item, &at)| match &item.data {
                BarData::Absent { .. } => at == -1,
                BarData::Bytes(data) => at >= 0 && at as usize + data.len() <= layout.len,
            })
        && layout
            .filler
            .iter()
            .all(|(at, run)| at + run.len() <= layout.len);
    fits.then(|| (layout, layout.offsets.clone()))
}

/// Writes `bar`. A layout read from a file that still fits the items is
/// reproduced exactly; otherwise payloads follow the table in order, each
/// at the next 16-byte boundary, with zero padding to a 16-byte length.
pub fn write_bar(bar: &Bar) -> Vec<u8> {
    let table_end = 16 + bar.items.len() * 16;
    let (mut out, offsets) = match recorded_offsets(bar) {
        Some((layout, offsets)) => {
            let mut out = vec![0; layout.len];
            for (at, run) in &layout.filler {
                out[*at..at + run.len()].copy_from_slice(run);
            }
            (out, offsets)
        }
        None => {
            let mut offset = table_end;
            let offsets = bar
                .items
                .iter()
                .map(|item| match &item.data {
                    BarData::Absent { .. } => -1,
                    BarData::Bytes(data) => {
                        let at = offset.next_multiple_of(16);
                        offset = at + data.len();
                        at as i32
                    }
                })
                .collect();
            (vec![0; offset.next_multiple_of(16)], offsets)
        }
    };
    out.resize(out.len().max(table_end), 0);
    for (item, &at) in bar.items.iter().zip(&offsets) {
        if let BarData::Bytes(data) = &item.data {
            out[at as usize..at as usize + data.len()].copy_from_slice(data);
        }
    }
    // The table goes last: a payload recorded over it holds the same bytes.
    let mut table = b"BAR\x01".to_vec();
    table.extend((bar.items.len() as i32).to_le_bytes());
    table.extend(bar.header_field_8.to_le_bytes());
    table.extend(bar.header_field_12.to_le_bytes());
    for (item, &at) in bar.items.iter().zip(&offsets) {
        let size = match &item.data {
            BarData::Absent { size } => *size,
            BarData::Bytes(data) => data.len() as u32,
        };
        table.extend(item.entry_type.to_le_bytes());
        table.extend(item.index.to_le_bytes());
        table.extend(item.name_bytes);
        table.extend(at.to_le_bytes());
        table.extend(size.to_le_bytes());
    }
    out[..table_end].copy_from_slice(&table);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture(offset: i32, size: i32) -> Vec<u8> {
        let mut bytes = b"BAR\x01".to_vec();
        bytes.extend(1i32.to_le_bytes());
        bytes.extend(7i32.to_le_bytes());
        bytes.extend(2i32.to_le_bytes());
        bytes.extend(65535u16.to_le_bytes());
        bytes.extend((-2i16).to_le_bytes());
        bytes.extend(b"test");
        bytes.extend(offset.to_le_bytes());
        bytes.extend(size.to_le_bytes());
        bytes.extend(b"DATA");
        bytes
    }
    #[test]
    fn preserves_unknown_types_signed_indices_and_absent_ranges() {
        let result = read_bar(&fixture(32, 4)).unwrap();
        assert_eq!(
            result.entries[0],
            BarEntry {
                entry_type: 65535,
                index: -2,
                name_bytes: *b"test",
                offset: 32,
                size: 4
            }
        );
        assert_eq!(result.header_field_8, 7);
        assert_eq!(result.header_field_12, 2);
        assert_eq!(read_bar(&fixture(-1, 4)).unwrap().absent_offsets, 1);
        assert_eq!(
            read_bar(&fixture(0, 4)).unwrap().table_overlapping_entries,
            1
        );
    }
    #[test]
    fn rejects_bad_signature_counts_and_ranges() {
        assert!(read_bar(&[]).is_err());
        let mut bytes = fixture(32, 4);
        bytes[0] = 0;
        assert!(read_bar(&bytes).is_err());
        for count in [-1, i32::MAX] {
            let mut bytes = fixture(32, 4);
            bytes[4..8].copy_from_slice(&count.to_le_bytes());
            assert!(read_bar(&bytes).is_err());
        }
        for (offset, size) in [(-2, 1), (32, -1), (33, 4), (i32::MAX, i32::MAX)] {
            assert!(read_bar(&fixture(offset, size)).is_err());
        }
    }

    /// A synthetic BAR from (type, name, offset, size) records and its body.
    fn build(
        records: &[(u16, &[u8; 4], i32, i32)],
        body: &[(usize, &[u8])],
        len: usize,
    ) -> Vec<u8> {
        let mut bytes = b"BAR\x01".to_vec();
        bytes.extend((records.len() as i32).to_le_bytes());
        bytes.extend(0i32.to_le_bytes());
        bytes.extend(5i32.to_le_bytes());
        for (i, (kind, name, offset, size)) in records.iter().enumerate() {
            bytes.extend(kind.to_le_bytes());
            bytes.extend((i as i16).to_le_bytes());
            bytes.extend(*name);
            bytes.extend(offset.to_le_bytes());
            bytes.extend(size.to_le_bytes());
        }
        bytes.resize(len, 0);
        for (at, run) in body {
            bytes[*at..at + run.len()].copy_from_slice(run);
        }
        bytes
    }

    fn round_trips(bytes: &[u8]) {
        assert_eq!(write_bar(&parse_bar(bytes).unwrap()), bytes);
    }

    #[test]
    fn writes_unedited_bars_back_byte_for_byte() {
        // Aligned payloads, zero padding, padded end.
        round_trips(&build(
            &[(4, b"a\0\0\0", 48, 5), (7, b"b\0\0\0", 64, 16)],
            &[(48, b"hello"), (64, &[9; 16])],
            80,
        ));
        // Unaligned offset, unpadded end and an absent record with a size.
        round_trips(&build(
            &[(4, b"a\0\0\0", 52, 3), (0, b"none", -1, 12)],
            &[(52, b"xyz")],
            55,
        ));
        // Two records sharing one payload, and an empty record at offset 0.
        round_trips(&build(
            &[
                (4, b"a\0\0\0", 64, 8),
                (4, b"a\0\0\0", 64, 8),
                (0, b"zero", 0, 0),
            ],
            &[(64, b"together")],
            72,
        ));
        // Non-zero padding and an unreferenced tail no record covers.
        round_trips(&build(
            &[(4, b"a\0\0\0", 32, 4)],
            &[(32, b"DATA"), (36, &[0xcd; 12]), (48, b"tail bytes")],
            58,
        ));
        // Records listed out of payload order, one overlapping another.
        round_trips(&build(
            &[(4, b"late", 80, 8), (4, b"soon", 64, 20)],
            &[(64, &[1; 16]), (80, &[2; 8])],
            96,
        ));
    }

    #[test]
    fn nested_bars_round_trip_inside_their_parent() {
        let inner = build(&[(1, b"in\0\0", 32, 3)], &[(32, b"abc")], 35);
        let outer = build(
            &[(10, b"out\0", 32, inner.len() as i32)],
            &[(32, &inner)],
            80,
        );
        round_trips(&outer);
        let parsed = parse_bar(&outer).unwrap();
        let BarData::Bytes(data) = &parsed.items[0].data else {
            panic!("payload expected");
        };
        round_trips(data);
    }

    #[test]
    fn new_bars_pack_each_payload_on_the_next_16_byte_boundary() {
        let item = |name: &[u8; 4], data: BarData| BarItem {
            entry_type: 4,
            index: 0,
            name_bytes: *name,
            data,
        };
        let bar = Bar {
            header_field_8: 0,
            header_field_12: 0,
            items: vec![
                item(b"one\0", BarData::Bytes(vec![1; 5])),
                item(b"gone", BarData::Absent { size: 7 }),
                item(b"two\0", BarData::Bytes(vec![2; 17])),
            ],
            layout: None,
        };
        let bytes = write_bar(&bar);
        let table = read_bar(&bytes).unwrap();
        let placed: Vec<_> = table.entries.iter().map(|e| (e.offset, e.size)).collect();
        assert_eq!(placed, [(64, 5), (-1, 7), (80, 17)]);
        assert_eq!(bytes.len(), 112);
        assert!(bytes[69..80].iter().chain(&bytes[97..]).all(|b| *b == 0));
        assert_eq!(parse_bar(&bytes).unwrap().items, bar.items);
    }

    #[test]
    fn a_resized_payload_is_packed_again_instead_of_using_its_old_place() {
        let original = build(
            &[(4, b"a\0\0\0", 48, 4), (4, b"b\0\0\0", 52, 4)],
            &[(48, b"AAAA"), (52, b"BBBB")],
            56,
        );
        round_trips(&original);
        let mut bar = parse_bar(&original).unwrap();
        bar.items[1].data = BarData::Bytes(vec![3; 40]);
        let bytes = write_bar(&bar);
        let table = read_bar(&bytes).unwrap();
        assert_eq!(table.entries[0].offset, 48);
        assert_eq!(table.entries[1].offset, 64);
        assert_eq!(parse_bar(&bytes).unwrap().items, bar.items);
    }
}
