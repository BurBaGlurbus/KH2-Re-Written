//! Explicit sequential MotionTrigger constructor profile (OpenKH pin in NOTICE).
//! Raw event IDs/frames/parameters do not establish installed event semantics.
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::io;

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct RangeTrigger {
    pub start_frame: i16,
    pub end_frame: i16,
    pub trigger: u8,
    pub parameters: Vec<i16>,
}
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct FrameTrigger {
    pub frame: i16,
    pub trigger: u8,
    pub parameters: Vec<i16>,
}
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct MotionTriggers {
    pub declared_frame_offset: i16,
    pub sequential_frame_offset: usize,
    pub ranges: Vec<RangeTrigger>,
    pub frames: Vec<FrameTrigger>,
    pub trailing_bytes: Vec<u8>,
}
fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
fn take<'a>(bytes: &'a [u8], cursor: &mut usize, length: usize) -> io::Result<&'a [u8]> {
    let end = cursor
        .checked_add(length)
        .ok_or_else(|| invalid("motion trigger range overflow"))?;
    let result = bytes
        .get(*cursor..end)
        .ok_or_else(|| invalid("truncated motion trigger record"))?;
    *cursor = end;
    Ok(result)
}
fn parameters(bytes: &[u8], cursor: &mut usize, count: u8) -> io::Result<Vec<i16>> {
    if count > 4 {
        return Err(invalid(
            "trigger parameter count outside sequential profile",
        ));
    }
    Ok(take(bytes, cursor, count as usize * 2)?
        .as_chunks::<2>()
        .0
        .iter()
        .map(|word| i16::from_le_bytes(*word))
        .collect())
}
pub fn read(bytes: &[u8]) -> io::Result<MotionTriggers> {
    if bytes.len() > 65_536 {
        return Err(invalid("motion trigger payload exceeds inspection bound"));
    }
    let mut cursor = 0;
    let header = take(bytes, &mut cursor, 4)?;
    let mut ranges = Vec::new();
    for _ in 0..header[0] {
        let record = take(bytes, &mut cursor, 6)?;
        ranges.push(RangeTrigger {
            start_frame: i16::from_le_bytes(record[..2].try_into().unwrap()),
            end_frame: i16::from_le_bytes(record[2..4].try_into().unwrap()),
            trigger: record[4],
            parameters: parameters(bytes, &mut cursor, record[5])?,
        });
    }
    // Public constructor retains the declared offset but does not seek to it.
    let sequential_frame_offset = cursor;
    let mut frames = Vec::new();
    for _ in 0..header[1] {
        let record = take(bytes, &mut cursor, 4)?;
        frames.push(FrameTrigger {
            frame: i16::from_le_bytes(record[..2].try_into().unwrap()),
            trigger: record[2],
            parameters: parameters(bytes, &mut cursor, record[3])?,
        });
    }
    Ok(MotionTriggers {
        declared_frame_offset: i16::from_le_bytes(header[2..4].try_into().unwrap()),
        sequential_frame_offset,
        ranges,
        frames,
        trailing_bytes: bytes[cursor..].to_vec(),
    })
}

#[derive(Clone, Debug, Serialize)]
pub struct SelectedTriggers {
    pub bar_ordinal: usize,
    pub sha256: String,
    pub records: MotionTriggers,
}
pub fn inspect_anb(bytes: &[u8]) -> io::Result<Vec<SelectedTriggers>> {
    let table = crate::bar::read_bar(bytes)?;
    let mut selected = Vec::new();
    for (bar_ordinal, entry) in table.entries.iter().enumerate() {
        if entry.entry_type != 16 || entry.offset < 0 || entry.size == 0 {
            continue;
        }
        if selected.len() == 8 {
            return Err(invalid("too many selected ANB trigger entries"));
        }
        let start = entry.offset as usize;
        let data = &bytes[start..start + entry.size as usize];
        selected.push(SelectedTriggers {
            bar_ordinal,
            sha256: format!("{:x}", Sha256::digest(data)),
            records: read(data)?,
        });
    }
    Ok(selected)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preserves_signed_unsorted_unknown_records_and_ignores_declared_seek() {
        let mut bytes = vec![1, 2, 255, 255];
        bytes.extend(i16::MIN.to_le_bytes());
        bytes.extend((-1i16).to_le_bytes());
        bytes.extend([255, 4]);
        for value in [i16::MIN, -1, 0, i16::MAX] {
            bytes.extend(value.to_le_bytes());
        }
        for frame in [7i16, -8] {
            bytes.extend(frame.to_le_bytes());
            bytes.extend([254, 0]);
        }
        bytes.extend([11, 22]);
        let result = read(&bytes).unwrap();
        assert_eq!(result.declared_frame_offset, -1);
        assert_eq!(result.sequential_frame_offset, 18);
        assert_eq!(result.ranges[0].start_frame, i16::MIN);
        assert_eq!(result.ranges[0].end_frame, -1);
        assert_eq!(result.ranges[0].trigger, 255);
        assert_eq!(result.ranges[0].parameters, [i16::MIN, -1, 0, i16::MAX]);
        assert_eq!(
            result.frames.iter().map(|f| f.frame).collect::<Vec<_>>(),
            [7, -8]
        );
        assert_eq!(result.trailing_bytes, [11, 22]);
    }
    #[test]
    fn rejects_truncation_and_unsupported_parameter_width_without_truncating() {
        let bytes = [1, 1, 10, 0, 0, 0, 255, 255, 0, 0, 9, 0, 2, 1, 255, 255];
        assert!(read(&bytes).is_ok());
        for end in 0..bytes.len() {
            assert!(read(&bytes[..end]).is_err());
        }
        for index in [9, 13] {
            let mut altered = bytes;
            altered[index] = 5;
            assert!(read(&altered).is_err());
        }
        assert!(read(&vec![0; 65_537]).is_err());
    }
    #[test]
    fn bounds_selected_entries_and_retains_alias_ordinals() {
        let archive = |count: u32| {
            let mut bytes = b"BAR\x01".to_vec();
            bytes.extend(count.to_le_bytes());
            bytes.extend([0; 8]);
            for i in 0..count {
                bytes.extend(16u16.to_le_bytes());
                bytes.extend((i as i16).to_le_bytes());
                bytes.extend(*b"test");
                bytes.extend((16 + 16 * count).to_le_bytes());
                bytes.extend(4u32.to_le_bytes());
            }
            bytes.extend([0, 0, 4, 0]);
            bytes
        };
        let entries = inspect_anb(&archive(8)).unwrap();
        assert_eq!(entries.len(), 8);
        for (i, entry) in entries.iter().enumerate() {
            assert_eq!(entry.bar_ordinal, i);
            assert_eq!(entry.sha256, entries[0].sha256);
            assert!(entry.records.frames.is_empty());
        }
        assert!(inspect_anb(&archive(9)).is_err());
        assert!(inspect_anb(&[]).is_err());
    }
}
