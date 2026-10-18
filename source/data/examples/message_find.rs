//! Research tool: searches every installed English message archive for a
//! text (letters and spaces only) and prints the matching ids with their text.
//!
//! cargo run -p data --release --example message_find --locked -- <Steam-root> <manifest> <text>
use data::{bar, path_lookup::Library, probe_source::ProbeSource};
use std::path::PathBuf;

fn encode(text: &str) -> Vec<u8> {
    text.bytes()
        .map(|c| match c {
            b' ' => 0x01,
            b'A'..=b'Z' => 0x2e + (c - b'A'),
            b'a'..=b'z' => 0x9a + (c - b'a'),
            _ => 0xff,
        })
        .collect()
}

fn decode(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|&b| match b {
            0x01 => ' ',
            0x21..=0x2a => char::from(b'0' + (b - 0x21)),
            0x2e..=0x47 => char::from(b'A' + (b - 0x2e)),
            0x9a..=0xb3 => char::from(b'a' + (b - 0x9a)),
            0x48 => '!',
            0x49 => '?',
            0x4f => '.',
            0x66 => '-',
            0xec => '\'',
            _ => '*',
        })
        .collect()
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 3 {
        return Err("usage: message_find <Steam-root> <manifest> <text>".into());
    }
    let needle = encode(&args[2]);
    let source = ProbeSource::bind_session(&PathBuf::from(&args[0]), &PathBuf::from(&args[1]))?;
    source.verify_full_source()?;
    let mut library = Library::open(&source)?;
    let mut names: Vec<String> = [
        "sys", "jm", "tt", "hb", "al", "bb", "ca", "dc", "eh", "es", "ex", "he", "lk", "lm", "mu",
        "nm", "po", "tr", "wi", "zz", "gm", "mi", "hb2", "pause",
    ]
    .iter()
    .map(|w| format!("msg/us/{w}.bar"))
    .collect();
    names.retain(|n| library.find(n).is_some());
    for name in names {
        let archive = library.read(&name)?;
        let Ok(table) = bar::read_bar(&archive) else {
            continue;
        };
        for entry in table
            .entries
            .iter()
            .filter(|e| e.entry_type == 2 && e.offset >= 0)
        {
            let d = &archive[entry.offset as usize..entry.offset as usize + entry.size as usize];
            if d.len() < 8 {
                continue;
            }
            let count = u32::from_le_bytes(d[4..8].try_into()?) as usize;
            for k in 0..count {
                let at = 8 + k * 8;
                if at + 8 > d.len() {
                    break;
                }
                let id = i32::from_le_bytes(d[at..at + 4].try_into()?);
                let off = u32::from_le_bytes(d[at + 4..at + 8].try_into()?) as usize;
                let Some(bytes) = d.get(off..) else { continue };
                let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
                let record = &bytes[..end];
                if record.windows(needle.len()).any(|w| w == needle.as_slice()) {
                    println!("{name} {id}: {}", decode(record));
                }
            }
        }
    }
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
