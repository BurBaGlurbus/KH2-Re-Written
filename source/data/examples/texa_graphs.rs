//! Generate synthetic TEXA inputs for independent public-constructor checks.
use data::model_texture_animation::read_records;
use serde_json::json;
use std::{
    fs,
    io::{self, Write},
    path::Path,
};

fn write_new(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}
fn body(slots: &[i16], offsets: &[u32], length: usize, sprite: usize) -> Vec<u8> {
    let mut bytes = vec![0; length];
    let animation_table = (40 + slots.len() * 2).next_multiple_of(4);
    let u = [
        3u16,
        1,
        777,
        8,
        0,
        (slots.len() - 1) as u16,
        offsets.len() as u16,
        1,
        0,
        0,
        1,
        1,
    ];
    for (i, word) in u.into_iter().enumerate() {
        bytes[i * 2..i * 2 + 2].copy_from_slice(&word.to_le_bytes());
    }
    for (i, word) in [40i32, animation_table as i32, sprite as i32, -1]
        .into_iter()
        .enumerate()
    {
        bytes[24 + i * 4..28 + i * 4].copy_from_slice(&word.to_le_bytes());
    }
    for (i, word) in slots.iter().enumerate() {
        bytes[40 + i * 2..42 + i * 2].copy_from_slice(&word.to_le_bytes());
    }
    for (i, word) in offsets.iter().enumerate() {
        bytes[animation_table + i * 4..animation_table + i * 4 + 4]
            .copy_from_slice(&word.to_le_bytes());
    }
    bytes[sprite] = 0x12;
    bytes
}
fn frame(bytes: &mut [u8], offset: usize, words: [u16; 4]) {
    for (i, word) in words.into_iter().enumerate() {
        bytes[offset + i * 2..offset + i * 2 + 2].copy_from_slice(&word.to_le_bytes());
    }
}
fn texture(body: &[u8]) -> Vec<u8> {
    // Authored three-environment indexed layout; no captured/vendor bytes.
    let mut bytes = vec![0; 1064 + 4096];
    for (i, word) in [0i32, 1024, 1, 3, 36, 40, 328, 808, 1064]
        .into_iter()
        .enumerate()
    {
        bytes[i * 4..i * 4 + 4].copy_from_slice(&word.to_le_bytes());
    }
    bytes[76..80].copy_from_slice(&100u32.to_le_bytes());
    bytes[300..304].copy_from_slice(&808i32.to_le_bytes());
    for (ordinal, psm) in [20u64, 19, 20].into_iter().enumerate() {
        let tex0 = (psm << 20) | (4u64 << 26) | (4u64 << 30) | (100u64 << 37);
        let start = 328 + ordinal * 160 + 112;
        bytes[start..start + 8].copy_from_slice(&tex0.to_le_bytes());
    }
    bytes.extend(b"TEXA");
    bytes.extend((body.len() as i32).to_le_bytes());
    bytes.extend(body);
    bytes
}
fn cases() -> Vec<Vec<u8>> {
    let mut cases = vec![body(&[-1], &[], 64, 56)];
    let mut b = body(&[-1, -32767], &[56, 80], 128, 120);
    frame(&mut b, 56, [0, 1, 2, 0]);
    frame(&mut b, 64, [0xfff2, 0xffff, 3, 0xffff]);
    frame(&mut b, 80, [3, 7, 5, 2]);
    cases.push(b);
    let mut b = body(&[0, 1], &[80, 80], 128, 120);
    frame(&mut b, 80, [0xffe2, 1, 1, 0]);
    frame(&mut b, 64, [9, 5, 3, 60000]);
    frame(&mut b, 72, [0x0013, 0, 0, 0]);
    cases.push(b);
    let mut b = body(&[0], &[56, 16440], 16464, 16456);
    frame(&mut b, 56, [0x7ff2, 0, 0, 0]);
    frame(&mut b, 16432, [0x8013, 0, 0, 0]);
    frame(&mut b, 16440, [0x8002, 0, 0, 0]);
    cases.push(b);
    let offsets: Vec<_> = (0..16).map(|i| 160 + i * 16).collect();
    let slots: Vec<_> = (0..16).collect();
    let mut b = body(&slots, &offsets, 464, 448);
    for control in 0..16 {
        let data = if control == 2 || control == 3 {
            0x0010 | control
        } else {
            0xf000 | control
        };
        frame(&mut b, 160 + control as usize * 16, [data, 5, 3, 0xffff]);
        frame(&mut b, 168 + control as usize * 16, [3, 0, 0, 0]);
    }
    cases.push(b);
    let slots: Vec<_> = (0..65536).map(|i| i as u16 as i16).collect();
    cases.push(body(&slots, &[], 131136, 131128));
    cases.into_iter().map(|b| texture(&b)).collect()
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args_os()
        .nth(1)
        .ok_or("existing private output directory required")?;
    let directory = fs::canonicalize(path)?;
    let root = fs::canonicalize(
        validation::host::local_data_root()
            .ok_or("no local data folder is known")?
            .join("KH2FinalRewrite/research"),
    )?;
    if !directory.starts_with(&root) || directory.ancestors().any(|p| p.join(".git").exists()) {
        return Err("synthetic outputs require private research storage outside Git".into());
    }
    let mut entries = Vec::new();
    for (i, bytes) in cases().into_iter().enumerate() {
        let records = read_records(&bytes)?;
        let file = format!("case-{i:02}.texture");
        write_new(&directory.join(&file), &bytes)?;
        entries.push(json!({"file":file,"records":records}));
    }
    write_new(
        &directory.join("cases.json"),
        &serde_json::to_vec_pretty(
            &json!({"provenance":"generated_synthetic_bytes","cases":entries}),
        )?,
    )?;
    Ok(())
}
