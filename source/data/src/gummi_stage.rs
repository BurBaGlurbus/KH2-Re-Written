//! A Gummi flight's stage file (`gumibattle/gm<route><level>.gbx`,
//! FUN_1401f6f70): "GBX_", u32 0x3c, the rail and the script sections as
//! {u32 offset, u32 bytes}, and the stage map's name at +0x40 (e.g.
//! `gumibattle/gm00.map`, FUN_1401e7d10). The rail (FUN_1401e8070) is 28-byte
//! points: position, rotation in radians and the distance along the rail so
//! far. The script section (FUN_1401efb50) starts with seven
//! {u32 offset, u32 count} table entries, offsets from the section's start.

use std::io;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RailPoint {
    pub position: [f32; 3],
    pub rotation: [f32; 3],
    /// Cumulative distance along the rail.
    pub distance: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Stage {
    /// The stage map's path in the packages.
    pub map: String,
    pub rail: Vec<RailPoint>,
    /// The script section's bytes and its seven tables (offset from the
    /// section's start, record count).
    pub script: Vec<u8>,
    pub tables: [(u32, u32); 7],
}

fn invalid(m: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, m)
}

fn word(b: &[u8], at: usize) -> io::Result<u32> {
    b.get(at..at + 4)
        .map(|w| u32::from_le_bytes([w[0], w[1], w[2], w[3]]))
        .ok_or_else(|| invalid("gbx truncated"))
}

fn float(b: &[u8], at: usize) -> io::Result<f32> {
    word(b, at).map(f32::from_bits)
}

/// The record size of a rail point.
pub const RAIL_POINT: usize = 28;

pub fn read(bytes: &[u8]) -> io::Result<Stage> {
    if bytes.get(..4) != Some(b"GBX_") {
        return Err(invalid("not a gbx"));
    }
    let section = |k: usize| -> io::Result<&[u8]> {
        let at = word(bytes, 8 + 8 * k)? as usize;
        let len = word(bytes, 12 + 8 * k)? as usize;
        bytes
            .get(
                at..at
                    .checked_add(len)
                    .ok_or_else(|| invalid("gbx section overflows"))?,
            )
            .ok_or_else(|| invalid("gbx section outside the file"))
    };
    let rail_bytes = section(0)?;
    if rail_bytes.len() % RAIL_POINT != 0 {
        return Err(invalid("gbx rail is not whole points"));
    }
    let rail = rail_bytes
        .as_chunks::<RAIL_POINT>()
        .0
        .iter()
        .map(|p| {
            Ok(RailPoint {
                position: [float(p, 0)?, float(p, 4)?, float(p, 8)?],
                rotation: [float(p, 12)?, float(p, 16)?, float(p, 20)?],
                distance: float(p, 24)?,
            })
        })
        .collect::<io::Result<Vec<_>>>()?;
    let script = section(1)?.to_vec();
    let mut tables = [(0, 0); 7];
    for (k, t) in tables.iter_mut().enumerate() {
        *t = (word(&script, 8 * k)?, word(&script, 8 * k + 4)?);
    }
    let name = bytes
        .get(0x40..0x60)
        .ok_or_else(|| invalid("gbx truncated"))?;
    let end = name.iter().position(|&b| b == 0).unwrap_or(name.len());
    Ok(Stage {
        map: String::from_utf8_lossy(&name[..end]).into_owned(),
        rail,
        script,
        tables,
    })
}

/// The stage file of a route 0..=8 and level 1..=3 (DAT_1405b4ca1: "abc").
pub fn path(route: u8, level: u8) -> String {
    let letter = ["a", "b", "c"][(level.clamp(1, 3) - 1) as usize];
    format!("gumibattle/gm{route:02}{letter}.gbx")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_generated_stage() {
        let mut b = vec![0u8; 0x60];
        b[..4].copy_from_slice(b"GBX_");
        b[4..8].copy_from_slice(&0x3cu32.to_le_bytes());
        let rail_at = 0x60u32;
        b[8..12].copy_from_slice(&rail_at.to_le_bytes());
        b[12..16].copy_from_slice(&(2 * RAIL_POINT as u32).to_le_bytes());
        b[0x40..0x53].copy_from_slice(b"gumibattle/gm00.map");
        for k in 0..2 {
            for v in [1. + k as f32, 2., 3., 0., 0.5, 0., 10. * k as f32] {
                b.extend(v.to_le_bytes());
            }
        }
        let script_at = b.len() as u32;
        let mut script = Vec::new();
        for k in 0..7u32 {
            script.extend((0x38 + k).to_le_bytes());
            script.extend(k.to_le_bytes());
        }
        b[16..20].copy_from_slice(&script_at.to_le_bytes());
        b[20..24].copy_from_slice(&(script.len() as u32).to_le_bytes());
        b.extend(&script);
        let s = read(&b).unwrap();
        assert_eq!(s.map, "gumibattle/gm00.map");
        assert_eq!(s.rail.len(), 2);
        assert_eq!(s.rail[1].position, [2., 2., 3.]);
        assert_eq!(s.rail[1].distance, 10.);
        assert_eq!(s.tables[6], (0x3e, 6));
        assert!(read(&b[..0x70]).is_err());
        assert_eq!(path(0, 1), "gumibattle/gm00a.gbx");
        assert_eq!(path(8, 3), "gumibattle/gm08c.gbx");
    }
}
