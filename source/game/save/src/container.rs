//! The save container: a PNG file whose private `sqEX` chunk holds a
//! virtual memory card. Layout read from FUN_1400fef80 (open), FUN_140100810
//! (flush), FUN_140101590 (write a file), FUN_1401004d0 (delete) and
//! FUN_140100e50 (lookup), with the sizes passed by FUN_140199440
//! (`FUN_140101000(0x10fc0, 100)`):
//!
//! - bytes 0..16: the key (MD5 of the Steam id text);
//! - 100 entries of 0x158 bytes: name (NUL-terminated, first 0x40 bytes),
//!   creation time (+0x40, `time_t`), modification time (+0x48), size (+0x50);
//! - 100 data slots of 0x10fc0 bytes, in entry order;
//! - the first 0x100 bytes are XOR-masked ([`crate::cipher`]).
//!
//! The chunk is `((0x10fc0 + 0x158) * 100 + 0x40f) & !0x3ff` bytes long.

use crate::cipher::{self, Sbox};

pub const SLOT_BYTES: usize = 0x10fc0;
pub const ENTRY_COUNT: usize = 100;
pub const ENTRY_BYTES: usize = 0x158;
pub const PAYLOAD_BYTES: usize = ((SLOT_BYTES + ENTRY_BYTES) * ENTRY_COUNT + 0x40f) & !0x3ff;
const TABLE: usize = 0x10;
const DATA: usize = TABLE + ENTRY_COUNT * ENTRY_BYTES;
const NAME_BYTES: usize = 0x40;
const CHUNK: &[u8; 4] = b"sqEX";
const PNG_SIGNATURE: [u8; 8] = [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a];
/// The text chunk every file the original writes carries (FUN_140144520).
const AUTHOR: &[u8] = b"Author\0SQUARE CO.,LTD. / created by Somnium";

#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    NotPng,
    NoCardChunk,
    WrongChunkSize(usize),
    Full,
    TooLarge(usize),
    NameTooLong,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotPng => f.write_str("the file is not a PNG"),
            Self::NoCardChunk => f.write_str("the PNG has no sqEX save chunk"),
            Self::WrongChunkSize(n) => {
                write!(f, "the sqEX chunk is {n} bytes, not {PAYLOAD_BYTES}")
            }
            Self::Full => f.write_str("the save container has no free entry"),
            Self::TooLarge(n) => write!(f, "{n} bytes do not fit a {SLOT_BYTES}-byte slot"),
            Self::NameTooLong => f.write_str("the file name does not fit an entry"),
        }
    }
}

impl std::error::Error for Error {}

/// One entry of the table.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub name: String,
    pub created: i64,
    pub modified: i64,
    pub size: u32,
}

/// An opened container. `payload` is the decrypted `sqEX` data.
#[derive(Clone)]
pub struct Container {
    before: Vec<u8>,
    after: Vec<u8>,
    payload: Vec<u8>,
}

fn crc32(chunk_type_and_data: &[&[u8]]) -> u32 {
    let mut table = [0u32; 256];
    for (i, t) in table.iter_mut().enumerate() {
        let mut c = i as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 {
                0xedb88320 ^ (c >> 1)
            } else {
                c >> 1
            };
        }
        *t = c;
    }
    let mut crc = 0xffff_ffffu32;
    for part in chunk_type_and_data {
        for &b in *part {
            crc = table[((crc ^ b as u32) & 0xff) as usize] ^ (crc >> 8);
        }
    }
    !crc
}

fn push_chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    out.extend_from_slice(&crc32(&[kind, data]).to_be_bytes());
}

fn adler32(data: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for &x in data {
        a = (a + x as u32) % 65521;
        b = (b + a) % 65521;
    }
    (b << 16) | a
}

/// The picture a fresh container carries: 320x176 black RGB, stored in
/// uncompressed deflate blocks. (The original starts from a picture shipped
/// with the game, FUN_140106870; any picture reads back the same.)
fn blank_picture() -> (Vec<u8>, Vec<u8>) {
    const W: usize = 0x140;
    const H: usize = 0xb0;
    let mut before = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&(W as u32).to_be_bytes());
    ihdr.extend_from_slice(&(H as u32).to_be_bytes());
    ihdr.extend_from_slice(&[8, 2, 0, 0, 0]);
    push_chunk(&mut before, b"IHDR", &ihdr);
    push_chunk(&mut before, b"tEXt", AUTHOR);
    let raw = vec![0u8; H * (1 + W * 3)];
    let mut z = vec![0x78, 0x01];
    let mut blocks = raw.chunks(0xffff).peekable();
    while let Some(block) = blocks.next() {
        z.push(u8::from(blocks.peek().is_none()));
        z.extend_from_slice(&(block.len() as u16).to_le_bytes());
        z.extend_from_slice(&(!(block.len() as u16)).to_le_bytes());
        z.extend_from_slice(block);
    }
    z.extend_from_slice(&adler32(&raw).to_be_bytes());
    let mut after = Vec::new();
    for part in z.chunks(8192) {
        push_chunk(&mut after, b"IDAT", part);
    }
    push_chunk(&mut after, b"IEND", &[]);
    (before, after)
}

impl Container {
    /// A container with no files, keyed for `key`.
    pub fn empty(key: &[u8; 16]) -> Self {
        let (before, after) = blank_picture();
        let mut payload = vec![0u8; PAYLOAD_BYTES];
        payload[..16].copy_from_slice(key);
        Self {
            before,
            after,
            payload,
        }
    }

    /// Reads a container file, decrypting its header with `key`.
    pub fn open(png: &[u8], key: &[u8; 16], sbox: &Sbox) -> Result<Self, Error> {
        Self::open_masked(png, &cipher::mask(key, sbox))
    }

    /// The header mask of a container file written for any account, found
    /// without its Steam id: the unused parts of the first entry are zeros,
    /// so the most common 16-byte block of the masked head is the mask.
    pub fn recover_mask(png: &[u8]) -> Option<[u8; 16]> {
        let payload = Self::chunk(png).ok()?.2;
        let mut best: Option<([u8; 16], usize)> = None;
        for i in 2..16 {
            let block: [u8; 16] = payload[i * 16..i * 16 + 16].try_into().ok()?;
            let n = (2..16)
                .filter(|&j| payload[j * 16..j * 16 + 16] == block)
                .count();
            if best.is_none_or(|(_, c)| n > c) {
                best = Some((block, n));
            }
        }
        best.filter(|&(_, n)| n >= 4).map(|(b, _)| b)
    }

    /// The `sqEX` chunk: where it starts and ends in the file and its data.
    fn chunk(png: &[u8]) -> Result<(usize, usize, &[u8]), Error> {
        if png.get(..8) != Some(&PNG_SIGNATURE[..]) {
            return Err(Error::NotPng);
        }
        let mut at = 8;
        while at + 12 <= png.len() {
            let len = u32::from_be_bytes([png[at], png[at + 1], png[at + 2], png[at + 3]]) as usize;
            let end = at + 12 + len;
            if end > png.len() {
                return Err(Error::NotPng);
            }
            if &png[at + 4..at + 8] == CHUNK {
                if len != PAYLOAD_BYTES {
                    return Err(Error::WrongChunkSize(len));
                }
                return Ok((at, end, &png[at + 8..at + 8 + len]));
            }
            at = end;
        }
        Err(Error::NoCardChunk)
    }

    /// Reads a container file with its header mask already known.
    pub fn open_masked(png: &[u8], mask: &[u8; 16]) -> Result<Self, Error> {
        let (at, end, data) = Self::chunk(png)?;
        let mut payload = data.to_vec();
        cipher::apply(&mut payload, mask);
        Ok(Self {
            before: png[..at].to_vec(),
            after: png[end..].to_vec(),
            payload,
        })
    }

    /// The key stored in the decrypted first block (the account's key).
    pub fn stored_key(&self) -> [u8; 16] {
        self.payload[..16].try_into().expect("16 bytes")
    }

    /// Whether the decrypted first block equals the key: false means the
    /// Steam id (or the table) was wrong and every entry is garbage.
    pub fn key_matches(&self, key: &[u8; 16]) -> bool {
        self.payload[..16] == key[..]
    }

    /// The file bytes: the picture, the masked card chunk, the rest.
    pub fn to_png(&self, key: &[u8; 16], sbox: &Sbox) -> Vec<u8> {
        let mut data = self.payload.clone();
        data[..16].copy_from_slice(key);
        cipher::apply(&mut data, &cipher::mask(key, sbox));
        let mut out = self.before.clone();
        push_chunk(&mut out, CHUNK, &data);
        out.extend_from_slice(&self.after);
        out
    }

    fn entry_at(&self, i: usize) -> &[u8] {
        &self.payload[TABLE + i * ENTRY_BYTES..TABLE + (i + 1) * ENTRY_BYTES]
    }

    fn name_at(&self, i: usize) -> &[u8] {
        let e = &self.entry_at(i)[..NAME_BYTES];
        &e[..e.iter().position(|&b| b == 0).unwrap_or(NAME_BYTES)]
    }

    /// FUN_140100e50: the first entry that is empty or whose name contains
    /// `name` (the original uses `strstr`).
    fn find(&self, name: &str) -> Option<usize> {
        let query = name.as_bytes();
        (0..ENTRY_COUNT).find(|&i| {
            let have = self.name_at(i);
            have.is_empty() || have.windows(query.len().max(1)).any(|w| w == query)
        })
    }

    /// The entries that hold a file.
    pub fn entries(&self) -> Vec<Entry> {
        (0..ENTRY_COUNT)
            .filter(|&i| !self.name_at(i).is_empty())
            .map(|i| self.entry(i))
            .collect()
    }

    fn entry(&self, i: usize) -> Entry {
        let e = self.entry_at(i);
        Entry {
            name: String::from_utf8_lossy(self.name_at(i)).into_owned(),
            created: i64::from_le_bytes(e[0x40..0x48].try_into().expect("8 bytes")),
            modified: i64::from_le_bytes(e[0x48..0x50].try_into().expect("8 bytes")),
            size: u32::from_le_bytes(e[0x50..0x54].try_into().expect("4 bytes")),
        }
    }

    /// The file called `name`, if there is one.
    pub fn read(&self, name: &str) -> Option<(Entry, &[u8])> {
        let i = self.find(name).filter(|&i| !self.name_at(i).is_empty())?;
        let entry = self.entry(i);
        let start = DATA + i * SLOT_BYTES;
        let size = (entry.size as usize).min(SLOT_BYTES);
        Some((entry, &self.payload[start..start + size]))
    }

    /// FUN_140101590: stores `data` under `name` at time `now`.
    pub fn write(&mut self, name: &str, data: &[u8], now: i64) -> Result<(), Error> {
        if data.len() > SLOT_BYTES {
            return Err(Error::TooLarge(data.len()));
        }
        if name.len() >= NAME_BYTES {
            return Err(Error::NameTooLong);
        }
        let i = self.find(name).ok_or(Error::Full)?;
        let e = TABLE + i * ENTRY_BYTES;
        self.payload[e..e + NAME_BYTES].fill(0);
        self.payload[e..e + name.len()].copy_from_slice(name.as_bytes());
        if self.payload[e + 0x40..e + 0x48] == [0; 8] {
            self.payload[e + 0x40..e + 0x48].copy_from_slice(&now.to_le_bytes());
        }
        self.payload[e + 0x48..e + 0x50].copy_from_slice(&now.to_le_bytes());
        self.payload[e + 0x50..e + 0x54].copy_from_slice(&(data.len() as u32).to_le_bytes());
        let start = DATA + i * SLOT_BYTES;
        self.payload[start..start + data.len()].copy_from_slice(data);
        Ok(())
    }

    /// FUN_1401004d0: removes `name`, moving the later entries and slots up.
    pub fn remove(&mut self, name: &str) {
        let Some(i) = self.find(name).filter(|&i| !self.name_at(i).is_empty()) else {
            return;
        };
        let last = ENTRY_COUNT - 1;
        self.payload.copy_within(
            TABLE + (i + 1) * ENTRY_BYTES..TABLE + ENTRY_COUNT * ENTRY_BYTES,
            TABLE + i * ENTRY_BYTES,
        );
        self.payload.copy_within(
            DATA + (i + 1) * SLOT_BYTES..DATA + ENTRY_COUNT * SLOT_BYTES,
            DATA + i * SLOT_BYTES,
        );
        self.payload[TABLE + last * ENTRY_BYTES..TABLE + (last + 1) * ENTRY_BYTES].fill(0);
        self.payload[DATA + last * SLOT_BYTES..DATA + (last + 1) * SLOT_BYTES].fill(0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cipher::{aes_sbox, key_from_steam_id};

    #[test]
    fn size_matches_the_original_chunk() {
        // The chunk length of a container the game wrote.
        assert_eq!(PAYLOAD_BYTES, 6_991_872);
    }

    #[test]
    fn round_trips_files_and_hides_the_table_head() {
        let key = key_from_steam_id(76561198000000001);
        let sbox = aes_sbox();
        let mut c = Container::empty(&key);
        c.write("BISLPM-66675FM-00", &[1, 2, 3, 4], 1_700_000_000)
            .unwrap();
        c.write("BISLPM-66675FM-SYS", b"system", 1_700_000_100)
            .unwrap();
        let png = c.to_png(&key, &sbox);
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
        let back = Container::open(&png, &key, &sbox).unwrap();
        assert!(back.key_matches(&key));
        let (entry, data) = back.read("BISLPM-66675FM-00").unwrap();
        assert_eq!(data, [1, 2, 3, 4]);
        assert_eq!(
            (entry.created, entry.modified, entry.size),
            (1_700_000_000, 1_700_000_000, 4)
        );
        assert_eq!(back.read("BISLPM-66675FM-SYS").unwrap().1, b"system");
        assert!(back.read("BISLPM-66675FM-01").is_none());
        assert_eq!(back.entries().len(), 2);
        // The name of the first entry sits in the masked head, so it is not
        // readable in the file.
        assert!(!png.windows(17).any(|w| w == b"BISLPM-66675FM-00"));
        // A different Steam id reads back a different head.
        let wrong = key_from_steam_id(76561198000000002);
        assert!(
            !Container::open(&png, &wrong, &sbox)
                .unwrap()
                .key_matches(&wrong)
        );
    }

    #[test]
    fn an_opened_file_is_written_back_byte_for_byte() {
        let key = key_from_steam_id(76561198000000003);
        let sbox = aes_sbox();
        let mut c = Container::empty(&key);
        c.write("BISLPM-66675FM-04", &[5; 300], 1_700_000_000)
            .unwrap();
        // Chunks the reader does not know, before and after the card.
        c.before
            .extend_from_slice(b"\0\0\0\x03teXtabc\x01\x02\x03\x04");
        c.after.insert(0, 0x42);
        let png = c.to_png(&key, &sbox);
        let opened = Container::open(&png, &key, &sbox).unwrap();
        assert_eq!(opened.to_png(&key, &sbox), png);
        let mask = Container::recover_mask(&png).unwrap();
        let masked = Container::open_masked(&png, &mask).unwrap();
        assert_eq!(masked.to_png(&masked.stored_key(), &sbox), png);
    }

    #[test]
    fn the_mask_is_recovered_from_a_file_without_its_steam_id() {
        let key = key_from_steam_id(555);
        let sbox = aes_sbox();
        let mut c = Container::empty(&key);
        c.write("BISLPM-66675FM-00", &[1, 2, 3], 7).unwrap();
        let png = c.to_png(&key, &sbox);
        let mask = Container::recover_mask(&png).unwrap();
        assert_eq!(mask, cipher::mask(&key, &sbox));
        let back = Container::open_masked(&png, &mask).unwrap();
        assert_eq!(back.stored_key(), key);
        assert_eq!(back.read("BISLPM-66675FM-00").unwrap().1, [1, 2, 3]);
    }

    #[test]
    fn rewriting_keeps_the_creation_time_and_deleting_closes_the_gap() {
        let key = key_from_steam_id(1);
        let mut c = Container::empty(&key);
        for (n, t) in [("a", 10), ("b", 20), ("c", 30)] {
            c.write(n, &[t as u8], t).unwrap();
        }
        c.write("b", &[9, 9], 99).unwrap();
        let (e, d) = c.read("b").unwrap();
        assert_eq!((e.created, e.modified, d), (20, 99, &[9u8, 9][..]));
        c.remove("a");
        let names: Vec<_> = c.entries().into_iter().map(|e| e.name).collect();
        assert_eq!(names, ["b", "c"]);
        assert_eq!(c.read("c").unwrap().1, [30]);
    }

    #[test]
    fn bad_inputs_are_refused() {
        let key = key_from_steam_id(1);
        let mut c = Container::empty(&key);
        assert_eq!(
            c.write("x", &vec![0; SLOT_BYTES + 1], 0),
            Err(Error::TooLarge(SLOT_BYTES + 1))
        );
        for i in 0..ENTRY_COUNT {
            c.write(&format!("f{i:03}"), &[1], 0).unwrap();
        }
        assert_eq!(c.write("one-more", &[1], 0), Err(Error::Full));
        assert!(matches!(
            Container::open(b"nope", &key, &aes_sbox()),
            Err(Error::NotPng)
        ));
        let (before, after) = blank_picture();
        let mut plain = before;
        plain.extend_from_slice(&after);
        assert!(matches!(
            Container::open(&plain, &key, &aes_sbox()),
            Err(Error::NoCardChunk)
        ));
    }
}
