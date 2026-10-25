//! A party limit's file, `limit/<lang>/<name>.bar` (the 00battle `limt`
//! row's file name; loaded by FUN_1403d96d0): the scene program (BDX, type
//! 3), its motion set (type 20, 'LIMI'), its effects (PAX, type 18), the
//! cameras Trinity names (type 22) and the gauge sheets (types 24/25).
//!
//! The motion set holds one nested motion set per limit motion id (the id
//! `limit_motion_start` passes, e.g. 252). Inside it each participant's
//! motion is named `"%03d0"` of its objentry +7 byte (0x1405c5d48: "0010"
//! Sora, "0020" Donald, "0030" Goofy) and its weapons' motions end in 1 and
//! 2 instead of 0 (FUN_1403c64a0, FUN_1403c65e0).
use crate::bar;
use std::io;

const BDX: u16 = 3;
const MOTION: u16 = 17;
const PAX: u16 = 18;
const MOTION_SET: u16 = 20;
const CAMERA: u16 = 22;
const IMGD: u16 = 24;
const SEQD: u16 = 25;

/// One motion of a participant: the ANB archive bytes (a BAR holding the
/// motion and its triggers, as a motion set slot holds it).
pub type Anb = Vec<u8>;

#[derive(Clone, Debug, Default)]
pub struct LimitFile {
    /// The scene program (BAR type 3).
    pub program: Vec<u8>,
    /// Nested motion sets by limit motion id.
    pub motions: Vec<(u16, Vec<u8>)>,
    /// The effect container (PAX), when present.
    pub pax: Option<Vec<u8>>,
    /// Where the PAX starts in the file (its remaster pages are keyed to
    /// file offsets).
    pub pax_offset: usize,
    /// Camera events by entry name (type 22).
    pub cameras: Vec<([u8; 4], Vec<u8>)>,
    /// Image and sequence pairs by entry name (the gauge and combo sheets).
    pub sheets: Vec<([u8; 4], Vec<u8>, Vec<u8>)>,
}

fn slice(bytes: &[u8], e: &bar::BarEntry) -> io::Result<Vec<u8>> {
    let start = usize::try_from(e.offset)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "negative BAR offset"))?;
    bytes
        .get(start..start + e.size as usize)
        .map(<[u8]>::to_vec)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "BAR entry exceeds file"))
}

/// The motion key of a participant (FUN_1403c64a0): `"%03d0"` of its
/// objentry +7 byte, the last digit raised by one per weapon slot.
pub fn motion_key(objentry_byte7: u8, weapon_slot: Option<u8>) -> [u8; 4] {
    let s = format!("{objentry_byte7:03}0");
    let mut key: [u8; 4] = s.as_bytes()[s.len() - 4..].try_into().expect("four digits");
    if let Some(w) = weapon_slot {
        key[3] += w + 1;
    }
    key
}

pub fn read(bytes: &[u8]) -> io::Result<LimitFile> {
    let file = read_parts(bytes)?;
    if file.program.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "limit file without a program",
        ));
    }
    Ok(file)
}

/// A summon's limit file (`limt` rows whose second participant is a
/// summon): its motions and effects; the program is the summon's own
/// script (FUN_1403d7fa0), so the file carries none.
pub fn read_summon(bytes: &[u8]) -> io::Result<LimitFile> {
    read_parts(bytes)
}

fn read_parts(bytes: &[u8]) -> io::Result<LimitFile> {
    let table = bar::read_bar(bytes)?;
    let mut file = LimitFile::default();
    let mut images = Vec::new();
    for e in table.entries.iter().filter(|e| e.offset > 0 && e.size > 0) {
        match e.entry_type {
            BDX if file.program.is_empty() => file.program = slice(bytes, e)?,
            PAX if file.pax.is_none() => {
                file.pax = Some(slice(bytes, e)?);
                file.pax_offset = e.offset as usize;
            }
            MOTION_SET if file.motions.is_empty() => {
                let set = slice(bytes, e)?;
                let nested = bar::read_bar(&set)?;
                for (id, n) in nested.entries.iter().enumerate() {
                    if n.offset > 0 && n.size > 0 && n.entry_type == MOTION_SET {
                        file.motions.push((id as u16, slice(&set, n)?));
                    }
                }
            }
            CAMERA => file.cameras.push((e.name_bytes, slice(bytes, e)?)),
            IMGD => images.push((e.name_bytes, slice(bytes, e)?)),
            SEQD => {
                if let Some(at) = images.iter().position(|(n, _)| *n == e.name_bytes) {
                    let (name, image) = images.remove(at);
                    file.sheets.push((name, image, slice(bytes, e)?));
                }
            }
            _ => {}
        }
    }
    Ok(file)
}

impl LimitFile {
    /// The ANB a participant plays for limit motion `id` (FUN_1403c65e0:
    /// the first of the four variants with the key's name).
    pub fn anb(&self, id: u16, key: [u8; 4]) -> Option<Anb> {
        let (_, set) = self.motions.iter().find(|(m, _)| *m == id)?;
        let table = bar::read_bar(set).ok()?;
        let e = table
            .entries
            .iter()
            .find(|e| e.entry_type == MOTION && e.name_bytes == key && e.size > 0)?;
        slice(set, e).ok()
    }

    /// The participant keys limit motion `id` has (`"%03d0"` names).
    pub fn keys(&self, id: u16) -> Vec<[u8; 4]> {
        let Some((_, set)) = self.motions.iter().find(|(m, _)| *m == id) else {
            return Vec::new();
        };
        bar::read_bar(set).map_or_else(
            |_| Vec::new(),
            |t| {
                t.entries
                    .iter()
                    .filter(|e| e.entry_type == MOTION && e.size > 0)
                    .map(|e| e.name_bytes)
                    .collect()
            },
        )
    }

    /// Every limit motion id the set holds.
    pub fn motion_ids(&self) -> Vec<u16> {
        self.motions.iter().map(|(id, _)| *id).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bar_of(entries: &[(u16, [u8; 4], Vec<u8>)]) -> Vec<u8> {
        let mut out = b"BAR\x01".to_vec();
        out.extend((entries.len() as u32).to_le_bytes());
        out.extend([0u8; 8]);
        let mut offset = 16 + 16 * entries.len();
        let mut payload: Vec<u8> = Vec::new();
        for (t, name, bytes) in entries {
            out.extend(t.to_le_bytes());
            out.extend(0u16.to_le_bytes());
            out.extend(name);
            out.extend((offset as u32).to_le_bytes());
            out.extend((bytes.len() as u32).to_le_bytes());
            offset += bytes.len();
            payload.extend(bytes);
        }
        out.extend(payload);
        out
    }

    #[test]
    fn keys_follow_the_objentry_byte_and_the_weapon_slot() {
        assert_eq!(&motion_key(1, None), b"0010");
        assert_eq!(&motion_key(3, Some(0)), b"0031");
        assert_eq!(&motion_key(23, None), b"0230");
    }

    #[test]
    fn reads_the_program_motions_and_sheets() {
        let inner = bar_of(&[(17, *b"0010", vec![1, 2, 3]), (17, *b"0030", vec![4, 5])]);
        // The outer set: slot 0 empty, slot 1 the nested set for id 1.
        let mut set = b"BAR\x01".to_vec();
        set.extend(2u32.to_le_bytes());
        set.extend([0u8; 8]);
        set.extend([0u8; 16]);
        set.extend(20u16.to_le_bytes());
        set.extend(0u16.to_le_bytes());
        set.extend(*b"A930");
        set.extend(48u32.to_le_bytes());
        set.extend((inner.len() as u32).to_le_bytes());
        set.extend(&inner);
        let file = bar_of(&[
            (3, *b"limi", vec![9; 8]),
            (20, *b"LIMI", set),
            (24, *b"gage", vec![7]),
            (25, *b"gage", vec![8]),
        ]);
        let f = read(&file).unwrap();
        assert_eq!(f.program, vec![9; 8]);
        assert_eq!(f.motion_ids(), vec![1]);
        assert_eq!(f.anb(1, *b"0030"), Some(vec![4, 5]));
        assert_eq!(f.anb(1, *b"0020"), None);
        assert_eq!(f.sheets.len(), 1);
    }
}
