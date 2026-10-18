//! Research tool: prints the raw encoded bytes of the given message ids from
//! one installed `msg/<lang>/<world>.bar`, with a best-effort letter mapping
//! (digits 0x21, capitals 0x2e, lower case 0x9a) so a subtitle can be recognised by eye.
//!
//! cargo run -p data --release --example message_dump --locked -- <Steam-root> <manifest> <msg name> <id>...
use data::{bar, path_lookup::Library, probe_source::ProbeSource};
use std::path::PathBuf;

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() < 4 {
        return Err("usage: message_dump <Steam-root> <manifest> <msg name> <id>...".into());
    }
    let source = ProbeSource::bind_session(&PathBuf::from(&args[0]), &PathBuf::from(&args[1]))?;
    source.verify_full_source()?;
    let mut library = Library::open(&source)?;
    let archive = library.read(&args[2])?;
    let table = bar::read_bar(&archive)?;
    let entry = table
        .entries
        .iter()
        .find(|e| e.entry_type == 2)
        .ok_or("message table absent")?;
    let d = &archive[entry.offset as usize..entry.offset as usize + entry.size as usize];
    let count = u32::from_le_bytes(d[4..8].try_into()?) as usize;
    for id in args[3..].iter().map(|a| a.parse::<i32>()) {
        let id = id?;
        for k in 0..count {
            let at = 8 + k * 8;
            if i32::from_le_bytes(d[at..at + 4].try_into()?) != id {
                continue;
            }
            let off = u32::from_le_bytes(d[at + 4..at + 8].try_into()?) as usize;
            let bytes = &d[off..];
            let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
            let text: String = bytes[..end]
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
                .collect();
            println!("{id}: {text}   {:02x?}", &bytes[..end]);
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
