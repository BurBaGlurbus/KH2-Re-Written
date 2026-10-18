//! Research tool: prints the 00objentry records whose model name starts with
//! the given prefix, parsed fields and the raw 0x60 bytes.
//!
//! cargo run -p data --release --example object_dump --locked -- <Steam-root> <manifest> <model prefix>...
use data::{objentry, path_lookup::Library, probe_source::ProbeSource};
use std::path::PathBuf;

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() < 3 {
        return Err("usage: object_dump <Steam-root> <manifest> <model prefix>...".into());
    }
    let source = ProbeSource::bind_session(&PathBuf::from(&args[0]), &PathBuf::from(&args[1]))?;
    source.verify_full_source()?;
    let mut library = Library::open(&source)?;
    let bytes = library.read("00objentry.bin")?;
    let objects = objentry::read(&bytes)?;
    for (index, o) in objects.iter().enumerate() {
        let model = String::from_utf8_lossy(&o.model_name).into_owned();
        if !args[2..].iter().any(|p| model.starts_with(p.as_str())) {
            continue;
        }
        let raw = &bytes[8 + index * objentry::ENTRY_SIZE..][..objentry::ENTRY_SIZE];
        println!(
            "obj {} type {} sub {} model {model} anim {} character {} moveset {}\n    {:02x?}",
            o.object_id,
            o.object_type,
            o.sub_type,
            String::from_utf8_lossy(&o.animation_name),
            o.character,
            o.neo_moveset,
            raw
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
