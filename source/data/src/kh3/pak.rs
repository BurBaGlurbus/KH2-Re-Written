//! Kingdom Hearts III's `.pak` archives (UE 4.17, footer version 4): the
//! footer, the AES-256-ECB index, the entries and their zlib blocks. The key
//! is found in the player's own executable (`find_key`) and kept in memory
//! only. Layout from StrawHatPro's `docs/formats/pak.md` (MIT; see CREDITS.md).
use super::{Result, bad};
use aes::Aes256;
use aes::cipher::{BlockDecrypt, KeyInit, generic_array::GenericArray};
use sha1::{Digest, Sha1};
use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

const MAGIC: u32 = 0x5a6f_12e1;
const FOOTER: usize = 45;
const MAX_INDEX: u64 = 256 << 20;
/// The largest single file read whole (the biggest movie is left alone).
const MAX_ENTRY: u64 = 1 << 30;

/// The pak index key, from the player's executable. Never written anywhere.
#[derive(Clone)]
pub struct PakKey(Aes256);

impl PakKey {
    pub fn new(key: &[u8; 32]) -> Self {
        Self(Aes256::new(GenericArray::from_slice(key)))
    }

    fn decrypt(&self, bytes: &mut [u8]) {
        for block in bytes.chunks_exact_mut(16) {
            self.0.decrypt_block(GenericArray::from_mut_slice(block));
        }
    }
}

/// A pak's last 45 bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Footer {
    pub encrypted_index: bool,
    pub version: u32,
    pub index_offset: u64,
    pub index_size: u64,
    pub index_sha1: [u8; 20],
}

pub fn parse_footer(bytes: &[u8]) -> Result<Footer> {
    let b = bytes
        .get(bytes.len().saturating_sub(FOOTER)..)
        .unwrap_or(&[]);
    if b.len() != FOOTER || b[1..5] != MAGIC.to_le_bytes() {
        return Err(bad("not a pak: bad footer"));
    }
    let u32_at = |o: usize| u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]]);
    let u64_at = |o: usize| u64::from(u32_at(o)) | (u64::from(u32_at(o + 4)) << 32);
    let mut index_sha1 = [0; 20];
    index_sha1.copy_from_slice(&b[25..45]);
    Ok(Footer {
        encrypted_index: b[0] != 0,
        version: u32_at(5),
        index_offset: u64_at(9),
        index_size: u64_at(17),
        index_sha1,
    })
}

pub fn read_footer(path: &Path) -> Result<Footer> {
    let mut file = File::open(path)?;
    let len = file.metadata()?.len();
    if len < FOOTER as u64 {
        return Err(bad("not a pak: too short"));
    }
    file.seek(SeekFrom::Start(len - FOOTER as u64))?;
    let mut b = [0; FOOTER];
    file.read_exact(&mut b)?;
    parse_footer(&b)
}

fn fstring_at(b: &[u8], o: &mut usize) -> Result<String> {
    let len = i32_at(b, *o)?;
    *o += 4;
    let out = if len >= 0 {
        let n = len as usize;
        let s = b
            .get(*o..*o + n)
            .ok_or_else(|| bad("name past the index"))?;
        *o += n;
        s[..n.saturating_sub(1)]
            .iter()
            .map(|&c| c as char)
            .collect()
    } else {
        let n = len.unsigned_abs() as usize * 2;
        let s = b
            .get(*o..*o + n)
            .ok_or_else(|| bad("name past the index"))?;
        *o += n;
        let units: Vec<u16> = s
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        String::from_utf16_lossy(&units[..units.len().saturating_sub(1)])
    };
    Ok(out)
}

fn i32_at(b: &[u8], o: usize) -> Result<i32> {
    let s = b.get(o..o + 4).ok_or_else(|| bad("past the index"))?;
    Ok(i32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

fn u64_at(b: &[u8], o: usize) -> Result<u64> {
    let s = b.get(o..o + 8).ok_or_else(|| bad("past the index"))?;
    let mut a = [0; 8];
    a.copy_from_slice(s);
    Ok(u64::from_le_bytes(a))
}

/// One FPakEntry, in the index or in front of its payload.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Entry {
    pub offset: u64,
    pub size: u64,
    pub uncompressed: u64,
    pub compression: i32,
    pub blocks: Vec<(u64, u64)>,
    pub encrypted: bool,
    pub block_size: u32,
    pub header_size: u32,
}

pub fn parse_entry(b: &[u8], start: usize) -> Result<Entry> {
    let mut o = start;
    let mut e = Entry {
        offset: u64_at(b, o)?,
        size: u64_at(b, o + 8)?,
        uncompressed: u64_at(b, o + 16)?,
        compression: i32_at(b, o + 24)?,
        ..Entry::default()
    };
    o += 28 + 20;
    if e.compression != 0 {
        let n = i32_at(b, o)?;
        o += 4;
        if n < 0 || n as usize > b.len() / 16 {
            return Err(bad("bad block count"));
        }
        e.blocks.reserve(n as usize);
        for _ in 0..n {
            e.blocks.push((u64_at(b, o)?, u64_at(b, o + 8)?));
            o += 16;
        }
    }
    e.encrypted = *b.get(o).ok_or_else(|| bad("past the index"))? != 0;
    e.block_size = i32_at(b, o + 1)? as u32;
    o += 5;
    e.header_size = (o - start) as u32;
    Ok(e)
}

/// A decoded pak index.
#[derive(Debug, Default)]
pub struct PakIndex {
    pub mount: String,
    pub entries: Vec<(String, Entry)>,
}

pub fn parse_index(b: &[u8]) -> Result<PakIndex> {
    let mut o = 0;
    let mount = fstring_at(b, &mut o)?;
    let count = i32_at(b, o)?;
    o += 4;
    if count < 0 || count as usize > b.len() / 50 {
        return Err(bad("bad entry count"));
    }
    let mut entries = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let name = fstring_at(b, &mut o)?;
        let e = parse_entry(b, o)?;
        o += e.header_size as usize;
        entries.push((name, e));
    }
    Ok(PakIndex { mount, entries })
}

/// Reads and decrypts one pak's index, checked against the footer's SHA-1.
pub fn read_index(path: &Path, key: &PakKey) -> Result<PakIndex> {
    let footer = read_footer(path)?;
    if footer.version != 4 {
        return Err(bad(format!(
            "{}: pak version {}",
            path.display(),
            footer.version
        )));
    }
    if footer.index_size > MAX_INDEX {
        return Err(bad("pak index too large"));
    }
    let mut file = File::open(path)?;
    file.seek(SeekFrom::Start(footer.index_offset))?;
    let mut bytes = vec![0; footer.index_size as usize];
    file.read_exact(&mut bytes)?;
    if footer.encrypted_index {
        key.decrypt(&mut bytes);
    }
    if Sha1::digest(&bytes).as_slice() != footer.index_sha1 {
        return Err(bad(format!(
            "{}: the index does not match the key",
            path.display()
        )));
    }
    parse_index(&bytes)
}

/// The mounted path of a pak entry, lower case, without `../../../` or a
/// leading slash. The paks spell the same folders in one case each, but a
/// lookup should not depend on it.
pub fn mounted_path(mount: &str, name: &str) -> String {
    let joined = format!("{mount}{name}");
    let mut s = joined.as_str();
    while let Some(rest) = s.strip_prefix("../") {
        s = rest;
    }
    s.trim_start_matches('/').to_ascii_lowercase()
}

/// FNV-1a; a lookup key for 500 000 paths without keeping their text.
fn path_key(path: &str) -> u64 {
    path.bytes().fold(0xcbf2_9ce4_8422_2325, |h, c| {
        (h ^ u64::from(c)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

#[derive(Clone, Copy)]
struct Place {
    pak: u16,
    offset: u64,
}

/// Every regular pak mounted: base paks, then `_P` patches, the last writer
/// of a path winning, as the game does. The voice overlays under `_DLC_`
/// duplicate regular files of the other language and stay out.
pub struct Mounted {
    key: PakKey,
    paks: Vec<Mutex<File>>,
    places: HashMap<u64, Place>,
    names: Vec<String>,
}

impl Mounted {
    /// `keep_names` also keeps every path's text (for the research probe).
    pub fn open(pak_dir: &Path, key: &PakKey, keep_names: bool) -> Result<Self> {
        let files = list_paks(pak_dir)?;
        let threads = std::thread::available_parallelism()
            .map_or(4, |n| n.get())
            .min(8);
        let next = std::sync::atomic::AtomicUsize::new(0);
        let results: Mutex<Vec<(usize, PakIndex)>> = Mutex::new(Vec::new());
        let failed: Mutex<Option<String>> = Mutex::new(None);
        std::thread::scope(|s| {
            for _ in 0..threads {
                s.spawn(|| {
                    loop {
                        let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        let Some(path) = files.get(i) else { break };
                        match read_index(path, key) {
                            Ok(index) => {
                                if let Ok(mut r) = results.lock() {
                                    r.push((i, index));
                                }
                            }
                            Err(e) => {
                                if let Ok(mut f) = failed.lock() {
                                    *f = Some(e.to_string());
                                }
                                break;
                            }
                        }
                    }
                });
            }
        });
        if let Some(why) = failed.into_inner().ok().flatten() {
            return Err(bad(why));
        }
        let mut indexes = results.into_inner().map_err(|_| bad("pak index lock"))?;
        indexes.sort_by_key(|(i, _)| *i);
        let mut places = HashMap::with_capacity(520_000);
        let mut names = Vec::new();
        for (i, index) in &indexes {
            for (name, e) in &index.entries {
                let path = mounted_path(&index.mount, name);
                places.insert(
                    path_key(&path),
                    Place {
                        pak: *i as u16,
                        offset: e.offset,
                    },
                );
                if keep_names {
                    names.push(path);
                }
            }
        }
        if keep_names {
            names.sort_unstable();
            names.dedup();
        }
        let paks = files
            .iter()
            .map(|p| File::open(p).map(Mutex::new))
            .collect::<std::io::Result<_>>()?;
        Ok(Self {
            key: key.clone(),
            paks,
            places,
            names,
        })
    }

    pub fn names(&self) -> &[String] {
        &self.names
    }

    pub fn len(&self) -> usize {
        self.places.len()
    }

    pub fn is_empty(&self) -> bool {
        self.places.is_empty()
    }

    pub fn contains(&self, path: &str) -> bool {
        self.places
            .contains_key(&path_key(&path.to_ascii_lowercase()))
    }

    /// The decompressed, decrypted bytes of a mounted file, if it is there.
    pub fn read(&self, path: &str) -> Result<Option<Vec<u8>>> {
        let Some(place) = self.places.get(&path_key(&path.to_ascii_lowercase())) else {
            return Ok(None);
        };
        let mut file = self.paks[place.pak as usize]
            .lock()
            .map_err(|_| bad("pak file lock"))?;
        extract(&mut file, place.offset, &self.key).map(Some)
    }
}

fn extract(file: &mut File, offset: u64, key: &PakKey) -> Result<Vec<u8>> {
    // The entry again, in front of its payload: 4 KB holds any block list
    // short of a 170 MB file, and a longer one is read again whole.
    let mut head = vec![0; 4096];
    file.seek(SeekFrom::Start(offset))?;
    let got = read_up_to(file, &mut head)?;
    head.truncate(got);
    let entry = match parse_entry(&head, 0) {
        Ok(e) => e,
        Err(_) => {
            let mut more = vec![0; 1 << 20];
            file.seek(SeekFrom::Start(offset))?;
            let got = read_up_to(file, &mut more)?;
            more.truncate(got);
            parse_entry(&more, 0)?
        }
    };
    if entry.uncompressed > MAX_ENTRY || entry.size > MAX_ENTRY {
        return Err(bad("pak entry too large"));
    }
    let align = |n: u64| (n + 15) & !15;
    if entry.compression == 0 {
        let stored = if entry.encrypted {
            align(entry.size)
        } else {
            entry.size
        };
        file.seek(SeekFrom::Start(offset + u64::from(entry.header_size)))?;
        let mut bytes = vec![0; stored as usize];
        file.read_exact(&mut bytes)?;
        if entry.encrypted {
            key.decrypt(&mut bytes);
        }
        bytes.truncate(entry.size as usize);
        return Ok(bytes);
    }
    if entry.compression != 1 {
        return Err(bad(format!("pak compression {}", entry.compression)));
    }
    let mut out = Vec::with_capacity(entry.uncompressed as usize);
    let mut stored = Vec::new();
    for &(start, end) in &entry.blocks {
        let len = end.checked_sub(start).ok_or_else(|| bad("bad block"))?;
        let read = if entry.encrypted { align(len) } else { len };
        stored.resize(read as usize, 0);
        file.seek(SeekFrom::Start(start))?;
        file.read_exact(&mut stored)?;
        if entry.encrypted {
            key.decrypt(&mut stored);
        }
        flate2::read::ZlibDecoder::new(&stored[..len as usize]).read_to_end(&mut out)?;
    }
    if out.len() as u64 != entry.uncompressed {
        return Err(bad("pak entry inflated to the wrong size"));
    }
    Ok(out)
}

fn read_up_to(file: &mut File, buf: &mut [u8]) -> Result<usize> {
    let mut got = 0;
    while got < buf.len() {
        let n = file.read(&mut buf[got..])?;
        if n == 0 {
            break;
        }
        got += n;
    }
    Ok(got)
}

/// The regular paks in mount order (base paks, then `_P`, each by name).
pub fn list_paks(pak_dir: &Path) -> Result<Vec<PathBuf>> {
    let mut paks: Vec<PathBuf> = std::fs::read_dir(pak_dir)?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("pak")))
        .collect();
    let patch = |p: &Path| {
        p.file_stem()
            .is_some_and(|s| s.to_string_lossy().to_ascii_lowercase().ends_with("_p"))
    };
    paks.sort_by_key(|p| (patch(p), p.file_name().map(|n| n.to_os_string())));
    Ok(paks)
}

/// A candidate key is any run of 32 printable bytes in `.rdata`: UE 4.17
/// defines the key as a string. It is right when a pak's first index block
/// decrypts to the mount point's `FString` (or, for the pak given, when the
/// whole index matches the footer's SHA-1).
pub fn find_key(exe: &Path, pak: &Path) -> Result<(PakKey, u64)> {
    let image = std::fs::read(exe)?;
    let footer = read_footer(pak)?;
    let mut file = File::open(pak)?;
    file.seek(SeekFrom::Start(footer.index_offset))?;
    let mut cipher = vec![0; footer.index_size.min(MAX_INDEX) as usize];
    file.read_exact(&mut cipher)?;
    if cipher.len() < 16 {
        return Err(bad("pak index too short"));
    }
    let mut first = [0; 16];
    first.copy_from_slice(&cipher[..16]);
    let test = |k: &[u8]| -> Option<PakKey> {
        let mut key = [0; 32];
        key.copy_from_slice(k);
        let key = PakKey::new(&key);
        let mut b = first;
        key.decrypt(&mut b);
        let len = i32::from_le_bytes([b[0], b[1], b[2], b[3]]);
        let plausible = (1..512).contains(&len) && (b[4..].starts_with(b"../") || b[4] == b'/');
        if !plausible {
            return None;
        }
        let mut whole = cipher.clone();
        key.decrypt(&mut whole);
        (Sha1::digest(&whole).as_slice() == footer.index_sha1).then_some(key)
    };
    let printable = |c: u8| (0x20..0x7f).contains(&c);
    let (from, to) = read_only_data(&image);
    let area = &image[from..to];
    // Runs of exactly 32 first (a literal between NULs), then any window.
    let mut run = 0usize;
    for (i, &c) in area.iter().enumerate() {
        if printable(c) {
            run += 1;
            continue;
        }
        if run == 32
            && let Some(key) = test(&area[i - 32..i])
        {
            return Ok((key, (from + i - 32) as u64));
        }
        run = 0;
    }
    run = 0;
    for (i, &c) in area.iter().enumerate() {
        run = if printable(c) { run + 1 } else { 0 };
        if run >= 32
            && let Some(key) = test(&area[i + 1 - 32..=i])
        {
            return Ok((key, (from + i + 1 - 32) as u64));
        }
    }
    Err(bad("no pak key in this executable"))
}

/// The key at a remembered file offset, checked the same way.
pub fn key_at(exe: &Path, offset: u64, pak: &Path) -> Result<PakKey> {
    let mut file = File::open(exe)?;
    file.seek(SeekFrom::Start(offset))?;
    let mut key = [0; 32];
    file.read_exact(&mut key)?;
    let key = PakKey::new(&key);
    read_index(pak, &key)?;
    Ok(key)
}

/// The byte range of `.rdata`, or the whole file when the PE header is odd.
fn read_only_data(image: &[u8]) -> (usize, usize) {
    let u16_at = |o: usize| {
        image
            .get(o..o + 2)
            .map(|s| usize::from(u16::from_le_bytes([s[0], s[1]])))
    };
    let u32_at = |o: usize| {
        image
            .get(o..o + 4)
            .map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]) as usize)
    };
    let found = (|| {
        let pe = u32_at(0x3c)?;
        let sections = u16_at(pe + 6)?;
        let optional = u16_at(pe + 20)?;
        let table = pe + 24 + optional;
        for i in 0..sections {
            let s = table + i * 40;
            if image.get(s..s + 6)? == b".rdata" {
                let raw = u32_at(s + 16)?;
                let at = u32_at(s + 20)?;
                return Some((at, (at + raw).min(image.len())));
            }
        }
        None
    })();
    found.unwrap_or((0, image.len()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use aes::cipher::BlockEncrypt;

    const KEY: [u8; 32] = *b"synthetic-test-key-0123456789abc";

    fn fstring(s: &str) -> Vec<u8> {
        let mut out = (s.len() as i32 + 1).to_le_bytes().to_vec();
        out.extend_from_slice(s.as_bytes());
        out.push(0);
        out
    }

    fn entry_bytes(
        offset: u64,
        size: u64,
        plain: u64,
        comp: i32,
        blocks: &[(u64, u64)],
    ) -> Vec<u8> {
        let mut e = Vec::new();
        e.extend_from_slice(&offset.to_le_bytes());
        e.extend_from_slice(&size.to_le_bytes());
        e.extend_from_slice(&plain.to_le_bytes());
        e.extend_from_slice(&comp.to_le_bytes());
        e.extend_from_slice(&[0; 20]);
        if comp != 0 {
            e.extend_from_slice(&(blocks.len() as i32).to_le_bytes());
            for (a, b) in blocks {
                e.extend_from_slice(&a.to_le_bytes());
                e.extend_from_slice(&b.to_le_bytes());
            }
        }
        e.push(0);
        e.extend_from_slice(&65536u32.to_le_bytes());
        e
    }

    /// A pak with one stored file and one zlib file.
    fn synthetic_pak() -> Vec<u8> {
        use flate2::{Compression, write::ZlibEncoder};
        use std::io::Write;
        let mut pak = Vec::new();
        let stored = b"hello";
        let head = entry_bytes(0, 5, 5, 0, &[]);
        let off_a = pak.len() as u64;
        pak.extend_from_slice(&head);
        pak.extend_from_slice(stored);
        let plain = b"zlib zlib zlib zlib";
        let mut z = ZlibEncoder::new(Vec::new(), Compression::default());
        z.write_all(plain).unwrap();
        let zipped = z.finish().unwrap();
        let off_b = pak.len() as u64;
        let head_len = entry_bytes(0, 0, 0, 1, &[(0, 0)]).len() as u64;
        let span = (off_b + head_len, off_b + head_len + zipped.len() as u64);
        pak.extend_from_slice(&entry_bytes(
            0,
            zipped.len() as u64,
            plain.len() as u64,
            1,
            &[span],
        ));
        pak.extend_from_slice(&zipped);
        let mut index = fstring("../../../Game/");
        index.extend_from_slice(&2i32.to_le_bytes());
        index.extend_from_slice(&fstring("A.txt"));
        index.extend_from_slice(&entry_bytes(off_a, 5, 5, 0, &[]));
        index.extend_from_slice(&fstring("Sub/B.txt"));
        index.extend_from_slice(&entry_bytes(
            off_b,
            zipped.len() as u64,
            plain.len() as u64,
            1,
            &[span],
        ));
        index.resize(index.len().div_ceil(16) * 16, 0);
        let sha = Sha1::digest(&index);
        let cipher = Aes256::new(GenericArray::from_slice(&KEY));
        for block in index.chunks_exact_mut(16) {
            cipher.encrypt_block(GenericArray::from_mut_slice(block));
        }
        let index_at = pak.len() as u64;
        pak.extend_from_slice(&index);
        pak.push(1);
        pak.extend_from_slice(&MAGIC.to_le_bytes());
        pak.extend_from_slice(&4u32.to_le_bytes());
        pak.extend_from_slice(&index_at.to_le_bytes());
        pak.extend_from_slice(&(index.len() as u64).to_le_bytes());
        pak.extend_from_slice(&sha);
        pak
    }

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("kh3-pak-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn footer_round_trip() {
        let f = parse_footer(&synthetic_pak()).unwrap();
        assert!(f.encrypted_index);
        assert_eq!(f.version, 4);
        assert!(parse_footer(&[0; 44]).is_err());
    }

    #[test]
    fn mounts_and_reads_stored_and_zlib() {
        let dir = temp_dir("mount");
        std::fs::write(dir.join("a.pak"), synthetic_pak()).unwrap();
        let m = Mounted::open(&dir, &PakKey::new(&KEY), true).unwrap();
        assert_eq!(m.len(), 2);
        assert_eq!(m.names(), ["game/a.txt", "game/sub/b.txt"]);
        assert_eq!(m.read("Game/A.txt").unwrap().unwrap(), b"hello");
        assert_eq!(
            m.read("game/sub/B.TXT").unwrap().unwrap(),
            b"zlib zlib zlib zlib"
        );
        assert!(m.read("game/none").unwrap().is_none());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn wrong_key_is_refused() {
        let dir = temp_dir("wrong");
        let path = dir.join("a.pak");
        std::fs::write(&path, synthetic_pak()).unwrap();
        assert!(read_index(&path, &PakKey::new(&[7; 32])).is_err());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn key_is_found_in_a_synthetic_executable() {
        let dir = temp_dir("key");
        let pak = dir.join("a.pak");
        std::fs::write(&pak, synthetic_pak()).unwrap();
        let mut exe = vec![0u8; 64];
        exe.extend_from_slice(b"just a long printable string that is not the key at all");
        exe.push(0);
        let at = exe.len() as u64;
        exe.extend_from_slice(&KEY);
        exe.push(0);
        let exe_path = dir.join("game.exe");
        std::fs::write(&exe_path, exe).unwrap();
        let (key, offset) = find_key(&exe_path, &pak).unwrap();
        assert_eq!(offset, at);
        assert!(read_index(&pak, &key).is_ok());
        assert!(key_at(&exe_path, offset, &pak).is_ok());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn mount_order_puts_patches_last() {
        let dir = temp_dir("order");
        for n in ["a_0_P.pak", "b.pak", "a.pak"] {
            std::fs::write(dir.join(n), []).unwrap();
        }
        let names: Vec<String> = list_paks(&dir)
            .unwrap()
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, ["a.pak", "b.pak", "a_0_P.pak"]);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn mounted_paths_lose_the_relative_prefix() {
        assert_eq!(
            mounted_path("../../../", "KH3/Content/A.uasset"),
            "kh3/content/a.uasset"
        );
        assert_eq!(mounted_path("/", "x"), "x");
    }
}
