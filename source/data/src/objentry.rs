//! Raw 00objentry records, following pinned OpenKH Objentry.cs
//! (7a3b945c538d32c6a285128c98aefba093f52ceb): a two-word header, then
//! 0x60-byte entries. Only fields used by the test room are exposed.
use serde::Serialize;
use std::io;

pub const ENTRY_SIZE: usize = 0x60;

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct ObjectEntry {
    pub object_id: u32,
    pub object_type: u8,
    pub sub_type: u8,
    pub draw_priority: u8,
    pub weapon_joint: u8,
    pub model_name: Vec<u8>,
    pub animation_name: Vec<u8>,
    pub neo_moveset: u16,
    /// +0x48: the flag word (bit 3 makes FUN_140411700 give the body
    /// collision class 2 instead of 1).
    pub flags: u16,
    /// +0x4c: character id (FUN_1403b5b20; 1 Sora, 2 Donald, 3 Goofy).
    pub character: u16,
    /// +0x57: Drive form index (FUN_1403e0cc0's form record for weapon
    /// slot 1; 1..=6 Valor..Antiform).
    pub form: u8,
    /// +0x50: the weight (f32 bits; FUN_1403b5e00), which a vacuum's pull
    /// scales with and 0 of which makes the object immovable.
    pub weight_bits: u32,
    /// +0x54: what one living object of this entry counts toward a spawn
    /// point's limit before its next wave comes (FUN_1403feb50).
    pub spawn_limit: u8,
}

impl ObjectEntry {
    pub fn weight(&self) -> f32 {
        f32::from_bits(self.weight_bits)
    }
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
fn name(bytes: &[u8]) -> Vec<u8> {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    bytes[..end].to_vec()
}

pub fn read(bytes: &[u8]) -> io::Result<Vec<ObjectEntry>> {
    if bytes.len() < 8 || bytes.len() > 4 * 1024 * 1024 {
        return Err(invalid("object entry table outside inspection bounds"));
    }
    let count = u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize;
    let body = bytes
        .get(
            8..8 + count
                .checked_mul(ENTRY_SIZE)
                .ok_or_else(|| invalid("count overflow"))?,
        )
        .ok_or_else(|| invalid("object entry table exceeds payload"))?;
    Ok(body
        .as_chunks::<ENTRY_SIZE>()
        .0
        .iter()
        .map(|e| ObjectEntry {
            object_id: u32::from_le_bytes(e[0..4].try_into().unwrap()),
            object_type: e[4],
            sub_type: e[5],
            draw_priority: e[6],
            weapon_joint: e[7],
            model_name: name(&e[8..40]),
            animation_name: name(&e[40..72]),
            neo_moveset: u16::from_le_bytes(e[78..80].try_into().unwrap()),
            flags: u16::from_le_bytes(e[0x48..0x4a].try_into().unwrap()),
            character: u16::from_le_bytes(e[76..78].try_into().unwrap()),
            form: e[0x57],
            weight_bits: u32::from_le_bytes(e[0x50..0x54].try_into().unwrap()),
            spawn_limit: e[0x54],
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_names_and_weapon_joint() {
        let mut bytes = vec![0; 8 + 2 * ENTRY_SIZE];
        bytes[0..4].copy_from_slice(&3u32.to_le_bytes());
        bytes[4..8].copy_from_slice(&2u32.to_le_bytes());
        let second = 8 + ENTRY_SIZE;
        bytes[second..second + 4].copy_from_slice(&0x54u32.to_le_bytes());
        bytes[second + 7] = 178;
        bytes[second + 8..second + 15].copy_from_slice(b"P_TEST1");
        bytes[second + 40..second + 44].copy_from_slice(b"ANIM");
        bytes[second + 78..second + 80].copy_from_slice(&9u16.to_le_bytes());
        bytes[second + 76] = 1;
        bytes[second + 0x57] = 3;
        let entries = read(&bytes).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[1].object_id, 0x54);
        assert_eq!(entries[1].weapon_joint, 178);
        assert_eq!(entries[1].model_name, b"P_TEST1");
        assert_eq!(entries[1].animation_name, b"ANIM");
        assert_eq!(entries[1].neo_moveset, 9);
        assert_eq!((entries[1].character, entries[1].form), (1, 3));
        assert!(entries[0].model_name.is_empty());
    }

    #[test]
    fn rejects_truncated_tables() {
        let mut bytes = vec![0; 8 + ENTRY_SIZE];
        bytes[4..8].copy_from_slice(&2u32.to_le_bytes());
        assert!(read(&bytes).is_err());
        assert!(read(&bytes[..4]).is_err());
    }
}
