//! Research tool: prints the ENMP rows of the installed 00battle.bin (id,
//! level, HP percent, damage cap/min, exp, prize) so an object's enemy
//! parameter row can be matched by eye.
//!
//! cargo run -p data --release --example enmp_dump --locked -- <Steam-root> <manifest>
use data::{bar, path_lookup::Library, probe_source::ProbeSource};
use std::path::PathBuf;

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: enmp_dump <Steam-root> <manifest>".into());
    }
    let source = ProbeSource::bind_session(&PathBuf::from(&args[0]), &PathBuf::from(&args[1]))?;
    source.verify_full_source()?;
    let mut library = Library::open(&source)?;
    let battle = library.read("00battle.bin")?;
    let table = bar::read_bar(&battle)?;
    let e = table
        .entries
        .iter()
        .find(|e| e.name_bytes == *b"enmp")
        .ok_or("enmp entry absent")?;
    let d = &battle[e.offset as usize..e.offset as usize + e.size as usize];
    let count = u32::from_le_bytes(d[4..8].try_into()?) as usize;
    println!("count {count}");
    for k in 0..count {
        let r = &d[8 + k * 0x5c..8 + (k + 1) * 0x5c];
        let w = |o: usize| u16::from_le_bytes([r[o], r[o + 1]]);
        println!(
            "id {} lvl {} hp% {} cap {} min {} exp {} prize {} raw {:02x?}",
            w(0),
            w(2),
            w(4),
            w(68),
            w(70),
            w(0x56),
            w(0x58),
            &r[..0x5c]
        );
    }
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
