//! KH3's SEAD sounds: an `SQEXSEADSound` or `SQEXSEADSoundBank` export holds a
//! `sabf` bank inline, or points to one with `ReferenceBank` + `SoundIndex`.
//! Layout from StrawHatPro's `docs/formats/sead_audio.md` and `tools/lib/sead*.js`
//! (MIT). The materials' XOR table is the player's own, found in KH3's executable.
use super::hca::{self, Header, Tables};
use super::package::{Package, Rd};
use super::props::{Props, read_props};
use super::{Paks, Result, bad};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// One track's playback: its material and what its commands do once.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Note {
    pub delay_ms: u32,
    pub material: u16,
    pub volume: f32,
    /// A random span on the volume and on the pitch, and how each applies
    /// (1 add, 2 multiply).
    pub volume_random: Option<(f32, f32, u8)>,
    pub pitch_random: Option<(f32, f32, u8)>,
    pub looped: bool,
}

/// Sound type byte.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Kind {
    #[default]
    Empty,
    Sound,
    Random,
    Order,
    Cycle,
    Switch,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Sequence {
    pub weight: u8,
    pub group: u8,
    pub notes: Vec<Note>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Sound {
    pub kind: Kind,
    pub category: u8,
    pub priority: u8,
    pub number: u16,
    pub start_macro: u8,
    pub volume: f32,
    pub sequences: Vec<Sequence>,
    /// Cycle's interval, ms.
    pub cycle: (u32, u32),
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Material {
    pub channels: u8,
    pub codec: u8,
    pub rate: u32,
    pub seed: u8,
    pub extra: Vec<u8>,
    pub stream: Vec<u8>,
}

/// A parsed bank.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Bank {
    pub number: u16,
    pub sounds: Vec<Option<Sound>>,
    pub materials: Vec<Material>,
}

fn u16_at(b: &[u8], o: usize) -> Result<u16> {
    b.get(o..o + 2)
        .map(|s| u16::from_le_bytes([s[0], s[1]]))
        .ok_or_else(|| bad("bank too short"))
}

fn u32_at(b: &[u8], o: usize) -> Result<u32> {
    b.get(o..o + 4)
        .map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
        .ok_or_else(|| bad("bank too short"))
}

fn f32_at(b: &[u8], o: usize) -> Result<f32> {
    Ok(f32::from_bits(u32_at(b, o)?))
}

fn u8_at(b: &[u8], o: usize) -> Result<u8> {
    b.get(o).copied().ok_or_else(|| bad("bank too short"))
}

/// A section: entries' absolute offsets.
fn section(b: &[u8], at: usize) -> Result<Vec<usize>> {
    let header = usize::from(u16_at(b, at + 2)?);
    let count = usize::from(u16_at(b, at + 4)?);
    let mut out = Vec::with_capacity(count);
    for i in 0..count {
        let o = u32_at(b, at + header + i * 4)? as usize;
        out.push(at + o);
    }
    Ok(out)
}

fn track_notes(b: &[u8], tracks: &[usize], index: usize) -> Result<Option<Note>> {
    let Some(&at) = tracks.get(index) else {
        return Ok(None);
    };
    if u8_at(b, at + 1)? != 1 {
        return Ok(None);
    }
    let mut note = Note {
        material: u16_at(b, at + 4)?,
        volume: 1.,
        ..Note::default()
    };
    let mut c = at + usize::from(u16_at(b, at + 0x0e)?);
    for _ in 0..256 {
        let op = u8_at(b, c + 2)?;
        let (head, body) = (usize::from(u8_at(b, c + 1)?), usize::from(u8_at(b, c + 3)?));
        match op {
            1 => break,
            3 => note.volume = f32_at(b, c + 4)?,
            7 => {
                note.volume_random = Some((f32_at(b, c + 4)?, f32_at(b, c + 8)?, u8_at(b, c + 12)?))
            }
            8 => {
                note.pitch_random = Some((f32_at(b, c + 4)?, f32_at(b, c + 8)?, u8_at(b, c + 12)?))
            }
            _ => {}
        }
        c += head + body;
    }
    Ok(Some(note))
}

fn sequence_notes(b: &[u8], at: usize, tracks: &[usize]) -> Result<Vec<Note>> {
    let mut out = Vec::new();
    let mut c = at + usize::from(u16_at(b, at + 6)?);
    let mut clock = 0u32;
    for _ in 0..256 {
        let op = u8_at(b, c + 2)?;
        let (head, body) = (usize::from(u8_at(b, c + 1)?), usize::from(u8_at(b, c + 3)?));
        match op {
            1 => break,
            2 => {
                let track = u32_at(b, c + 4)? as usize;
                let looped = u8_at(b, c + 8)? != 0;
                if let Some(mut n) = track_notes(b, tracks, track)? {
                    n.delay_ms = clock;
                    n.looped = looped;
                    out.push(n);
                }
            }
            3 => clock += u32_at(b, c + 4)?,
            5 => clock += u32_at(b, c + 8)?,
            _ => {}
        }
        c += head + body;
    }
    Ok(out)
}

fn read_sound(b: &[u8], at: usize, sequences: &[usize], tracks: &[usize]) -> Result<Sound> {
    let kind = match u8_at(b, at + 4)? {
        1 => Kind::Sound,
        2 => Kind::Random,
        3 => Kind::Order,
        4 => Kind::Cycle,
        5 => Kind::Switch,
        _ => Kind::Empty,
    };
    let mut s = Sound {
        kind,
        category: u8_at(b, at + 6)?,
        priority: u8_at(b, at + 7)?,
        number: u16_at(b, at + 8)?,
        start_macro: u8_at(b, at + 0x0a)?,
        volume: f32_at(b, at + 0x0c)?,
        cycle: (u32_at(b, at + 0x10)?, u32_at(b, at + 0x14)?),
        ..Sound::default()
    };
    let count = usize::from(u8_at(b, at + 5)?);
    let info = at + usize::from(u16_at(b, at + 0x1a)?);
    for i in 0..count {
        let index = usize::from(u16_at(b, info + i * 16 + 4)?);
        let Some(&seq) = sequences.get(index) else {
            continue;
        };
        s.sequences.push(Sequence {
            weight: u8_at(b, seq + 10)?,
            group: u8_at(b, seq + 11)?,
            notes: sequence_notes(b, seq, tracks)?,
        });
    }
    Ok(s)
}

impl Bank {
    pub fn parse(b: &[u8]) -> Result<Self> {
        if b.get(..4) != Some(b"sabf") {
            return Err(bad("not a sound bank"));
        }
        let name_len = usize::from(u8_at(b, 9)?);
        let number = u16_at(b, 10)?;
        let table = usize::from(u16_at(b, 6)?) + ((name_len + 0x10) & 0xf0);
        let sections = usize::from(u8_at(b, 8)?);
        let mut places: HashMap<[u8; 4], usize> = HashMap::new();
        for i in 0..sections {
            let at = table + i * 16;
            let tag = b.get(at..at + 4).ok_or_else(|| bad("bank too short"))?;
            let mut t = [0; 4];
            t.copy_from_slice(tag);
            places.insert(t, u32_at(b, at + 8)? as usize);
        }
        let place = |t: &[u8; 4]| -> Result<usize> {
            places
                .get(t)
                .copied()
                .ok_or_else(|| bad("bank without a section"))
        };
        let sounds = section(b, place(b"snd ")?)?;
        let sequences = section(b, place(b"seq ")?)?;
        let tracks = section(b, place(b"trk ")?)?;
        let materials = section(b, place(b"mtrl")?)?;
        let mut out = Bank {
            number,
            ..Bank::default()
        };
        for &at in &sounds {
            let sound = read_sound(b, at, &sequences, &tracks)?;
            out.sounds
                .push((sound.kind != Kind::Empty).then_some(sound));
        }
        for &at in &materials {
            let channels = u8_at(b, at + 4)?;
            let codec = u8_at(b, at + 5)?;
            let extra_size = u32_at(b, at + 20)? as usize;
            let stream_size = u32_at(b, at + 24)? as usize;
            let head = usize::from(u16_at(b, at + 2)?);
            let extra_at = at + head;
            let stream_at = extra_at + extra_size;
            let extra = b
                .get(extra_at..stream_at)
                .ok_or_else(|| bad("material too short"))?
                .to_vec();
            let stream = b
                .get(stream_at..stream_at + stream_size)
                .ok_or_else(|| bad("material too short"))?
                .to_vec();
            out.materials.push(Material {
                channels,
                codec,
                rate: u32_at(b, at + 8)?,
                seed: u8_at(b, at + 28)?,
                extra,
                stream,
            });
        }
        Ok(out)
    }
}

/// The bank bytes in a sound or bank export.
fn embedded_bank(p: &Package, i: usize, sound: bool) -> Result<Option<(Props, Vec<u8>)>> {
    let bytes = p.export_bytes(i)?;
    let mut r = Rd::new(bytes);
    let props = read_props(&mut r, &p.names)?;
    r.i32()?;
    if sound {
        r.skip(8 + 16)?;
        if r.i32()? == 0 {
            return Ok(Some((props, Vec::new())));
        }
    }
    let version = r.i32()?;
    let size = usize::try_from(r.i32()?).map_err(|_| bad("bank size"))?;
    r.skip(if version == 0 { 16 } else { 20 })?;
    r.skip(4 + 4 + 4 + 8)?;
    let bank = r.take(size)?.to_vec();
    Ok(Some((props, bank)))
}

/// The player's audio tables, and the XOR table of the materials.
pub struct Keys {
    pub tables: Tables,
    pub sead_key: [u8; 256],
}

/// A material's HCA parts, the XOR undone with a key table.
fn hca_parts(m: &Material, key: &[u8; 256]) -> Result<(Header, Vec<u8>)> {
    let obfuscated = m.extra.get(13).copied().unwrap_or(0) != 0;
    let head = u16_at_be_size(&m.extra)?;
    let header_bytes = m.extra.get(16..16 + head).unwrap_or(&[]);
    let header_in_stream = header_bytes.is_empty();
    let (header, body) = if header_in_stream {
        let h = Header::parse(&m.stream)?;
        let size = usize::from(h.size);
        (h, m.stream.get(size..).unwrap_or(&[]).to_vec())
    } else {
        (Header::parse(header_bytes)?, m.stream.clone())
    };
    let mut body = body;
    if obfuscated {
        for (i, x) in body.iter_mut().enumerate() {
            *x ^= key[(usize::from(m.seed) + i) & 255];
        }
    }
    Ok((header, body))
}

fn u16_at_be_size(extra: &[u8]) -> Result<usize> {
    Ok(usize::from(u16_at(extra, 2)?))
}

/// A material to PCM.
pub fn decode_material(m: &Material, keys: &Keys) -> Result<hca::Decoded> {
    if m.codec != 7 {
        return Err(bad(format!("material codec {}", m.codec)));
    }
    let (header, body) = hca_parts(m, &keys.sead_key)?;
    hca::decode(&keys.tables, &header, &body)
}

/// Finds the XOR table: the place where an obfuscated frame of a bank turns
/// into a frame that passes its CRC.
pub fn find_key(image: &[u8], tables: &Tables, bank: &Bank) -> Result<[u8; 256]> {
    let m = bank
        .materials
        .iter()
        .find(|m| m.codec == 7 && m.extra.get(13).copied().unwrap_or(0) != 0)
        .ok_or_else(|| bad("no obfuscated material to find the sound key with"))?;
    let frame_size = usize::from(u16_at(&m.extra, 4)?);
    let frame = m
        .stream
        .get(..frame_size)
        .ok_or_else(|| bad("material too short"))?;
    let seed = usize::from(m.seed);
    let (want0, want1) = (frame[0] ^ 0xff, frame[1] ^ 0xff);
    let mut plain = vec![0u8; frame_size];
    for at in 0..image.len().saturating_sub(256) {
        if image[at + (seed & 255)] != want0 || image[at + ((seed + 1) & 255)] != want1 {
            continue;
        }
        let table = &image[at..at + 256];
        for (i, (out, x)) in plain.iter_mut().zip(frame).enumerate() {
            *out = x ^ table[(seed + i) & 255];
        }
        if tables.crc(&plain) == 0 {
            let mut key = [0; 256];
            key.copy_from_slice(table);
            return Ok(key);
        }
    }
    Err(bad("the sound key is not in this executable"))
}

/// A sound asset resolved: its bank and slot, its asset volume.
#[derive(Clone, Debug)]
pub struct Resolved {
    pub bank: Arc<Bank>,
    pub slot: usize,
    pub volume: f32,
    /// A 3D sound (it has `AttenuationSettings`).
    pub spatial: bool,
}

/// Banks by path, loaded once.
#[derive(Default)]
pub struct Banks {
    banks: Mutex<HashMap<String, Option<Arc<Bank>>>>,
}

impl Banks {
    fn bank_of(&self, paks: &Paks, path: &str) -> Option<Arc<Bank>> {
        let package = path.split('.').next().unwrap_or(path).to_owned();
        if let Ok(guard) = self.banks.lock()
            && let Some(hit) = guard.get(&package)
        {
            return hit.clone();
        }
        let bank = (|| -> Result<Arc<Bank>> {
            let p = paks
                .game_package(&package)?
                .ok_or_else(|| bad("bank package is missing"))?;
            let i = p
                .export_of_classes(&["SQEXSEADSoundBank", "SQEXSEADSound"])
                .ok_or_else(|| bad("no bank export"))?;
            let sound = p.class_of(i) == "SQEXSEADSound";
            let (_, bytes) = embedded_bank(&p, i, sound)?.ok_or_else(|| bad("no bank bytes"))?;
            Ok(Arc::new(Bank::parse(&bytes)?))
        })()
        .ok();
        if let Ok(mut guard) = self.banks.lock() {
            guard.insert(package, bank.clone());
        }
        bank
    }

    /// A sound asset (`/Game/Sound/...`) to its bank and slot.
    pub fn resolve(&self, paks: &Paks, path: &str) -> Option<Resolved> {
        let package = path.split('.').next().unwrap_or(path);
        let p = paks.game_package(package).ok().flatten()?;
        let i = p.export_of_class("SQEXSEADSound")?;
        let (props, bytes) = embedded_bank(&p, i, true).ok().flatten()?;
        let volume = props.f32("Volume").unwrap_or(1.);
        let spatial = props.object("AttenuationSettings").is_some_and(|o| o != 0);
        let reference = props
            .object("ReferenceBank")
            .filter(|o| *o != 0)
            .and_then(|o| p.import_package(o).map(str::to_owned));
        let slot = props.int("SoundIndex").unwrap_or(0).max(0) as usize;
        let bank = match reference {
            Some(r) => self.bank_of(paks, &r)?,
            None => Arc::new(Bank::parse(&bytes).ok()?),
        };
        Some(Resolved {
            bank,
            slot,
            volume,
            spatial,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A bank with one Sound, one sequence, one track and one empty material.
    fn synthetic_bank() -> Vec<u8> {
        let mut b = vec![0u8; 0x200];
        b[..4].copy_from_slice(b"sabf");
        b[4] = 2;
        b[6..8].copy_from_slice(&16u16.to_le_bytes());
        b[8] = 4;
        b[9] = 2;
        b[10..12].copy_from_slice(&2001u16.to_le_bytes());
        b[16..18].copy_from_slice(b"ab");
        // Table at 16 + (2 + 16) & 0xf0 = 32.
        let places = [
            (b"snd ", 0x80u32),
            (b"seq ", 0xa0),
            (b"trk ", 0xc0),
            (b"mtrl", 0xe0),
        ];
        for (i, (tag, at)) in places.iter().enumerate() {
            let o = 32 + i * 16;
            b[o..o + 4].copy_from_slice(*tag);
            b[o + 8..o + 12].copy_from_slice(&at.to_le_bytes());
        }
        let sect = |b: &mut Vec<u8>, at: usize, entry: u32| {
            b[at + 2..at + 4].copy_from_slice(&8u16.to_le_bytes());
            b[at + 4..at + 6].copy_from_slice(&1u16.to_le_bytes());
            b[at + 8..at + 12].copy_from_slice(&entry.to_le_bytes());
        };
        sect(&mut b, 0x80, 0x100);
        sect(&mut b, 0xa0, 0x100);
        sect(&mut b, 0xc0, 0x100);
        sect(&mut b, 0xe0, 0x100);
        // The sound at 0x180.
        let s = 0x180;
        b[s + 4] = 1;
        b[s + 5] = 1;
        b[s + 7] = 64;
        b[s + 8..s + 10].copy_from_slice(&7u16.to_le_bytes());
        b[s + 0x0c..s + 0x10].copy_from_slice(&0.5f32.to_le_bytes());
        // Its entry list at 0x1f0 (left zero): sequence 0.
        b[s + 0x1a..s + 0x1c].copy_from_slice(&0x70u16.to_le_bytes());
        // The sequence at 0x1a0: its commands at +0x10; keyOn track 0 then end.
        let q = 0x1a0;
        b[q + 6..q + 8].copy_from_slice(&0x10u16.to_le_bytes());
        b[q + 10] = 1;
        let c = q + 0x10;
        b[c + 1] = 4;
        b[c + 2] = 2;
        b[c + 3] = 12;
        b[c + 16 + 1] = 4;
        b[c + 16 + 2] = 1;
        // The track at 0x1c0: material 3; commands at +0x10: end.
        let t = 0x1c0;
        b[t + 1] = 1;
        b[t + 4..t + 6].copy_from_slice(&3u16.to_le_bytes());
        b[t + 0x0e..t + 0x10].copy_from_slice(&0x10u16.to_le_bytes());
        b[t + 0x10 + 1] = 4;
        b[t + 0x10 + 2] = 1;
        // The material at 0x1e0.
        let m = 0x1e0;
        b[m + 2..m + 4].copy_from_slice(&32u16.to_le_bytes());
        b
    }

    #[test]
    fn a_bank_reads() {
        let mut b = synthetic_bank();
        b.truncate(0x200);
        let bank = Bank::parse(&b).unwrap();
        assert_eq!(bank.number, 2001);
        let s = bank.sounds[0].as_ref().unwrap();
        assert_eq!(s.kind, Kind::Sound);
        assert_eq!(s.number, 7);
        assert_eq!(s.volume, 0.5);
        assert_eq!(s.sequences[0].notes[0].material, 3);
        assert_eq!(bank.materials.len(), 1);
    }

    #[test]
    fn other_bytes_are_not_a_bank() {
        assert!(Bank::parse(&[0; 64]).is_err());
    }
}
