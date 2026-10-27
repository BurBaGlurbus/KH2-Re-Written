//! Research tool: prints the collision table (BAR type 23) of each named
//! object model, one entry per line, with its default group mask.
//!
//! cargo run -p data --release --example collision_dump --locked -- <Steam-root> <manifest> <model>...
use data::{bar, object_collision, path_lookup::Library, probe_source::ProbeSource};
use std::path::PathBuf;

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() < 3 {
        return Err("usage: collision_dump <Steam-root> <manifest> <model>...".into());
    }
    let source = ProbeSource::bind_session(&PathBuf::from(&args[0]), &PathBuf::from(&args[1]))?;
    source.verify_full_source()?;
    let mut library = Library::open(&source)?;
    for model in &args[2..] {
        let bytes = library.read(&format!("obj/{model}.mdlx"))?;
        let table = bar::read_bar(&bytes)?;
        for e in table.entries.iter().filter(|e| e.entry_type == 23) {
            let start = usize::try_from(e.offset)?;
            let payload = bytes
                .get(start..start + e.size as usize)
                .ok_or("collision entry outside the archive")?;
            let groups = object_collision::default_groups(payload)?;
            println!("{model}: default groups {groups:#x}");
            for (k, c) in object_collision::read(payload)?.iter().enumerate() {
                println!(
                    "  {k:3} kind {} group {} parts {} arg {} shape {} bone {} at {:?} h {} r {} height {}",
                    c.kind,
                    c.group,
                    c.parts,
                    c.argument,
                    c.shape,
                    c.bone,
                    c.position,
                    c.position_height,
                    c.radius,
                    c.height
                );
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
