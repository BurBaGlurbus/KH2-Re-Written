//! Research tool: prints one installed ARD's spawn sets (entities with their
//! object model names) and the raw type-13 records (`map`, `btl`, `evt`).
//!
//! cargo run -p data --release --example ard_sets --locked -- <Steam-root> <manifest> <ard name>
use data::{ard, bar, objentry, path_lookup::Library, probe_source::ProbeSource};
use std::path::PathBuf;

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 3 {
        return Err("usage: ard_sets <Steam-root> <manifest> <ard name>".into());
    }
    let source = ProbeSource::bind_session(&PathBuf::from(&args[0]), &PathBuf::from(&args[1]))?;
    source.verify_full_source()?;
    let mut library = Library::open(&source)?;
    let objects = objentry::read(&library.read("00objentry.bin")?)?;
    let ard_bytes = library.read(&args[2])?;
    let table = bar::read_bar(&ard_bytes)?;
    for (i, e) in table.entries.iter().enumerate() {
        if e.offset < 0 {
            continue;
        }
        let name = String::from_utf8_lossy(&e.name_bytes)
            .trim_end_matches('\0')
            .to_string();
        let bytes = &ard_bytes[e.offset as usize..e.offset as usize + e.size as usize];
        match e.entry_type {
            12 => {
                let Ok(set) = ard::spawn_set(bytes) else {
                    println!("#{i} set {name}: undecodable");
                    continue;
                };
                println!(
                    "#{i} set {name} version {} points {}",
                    set.version,
                    set.points.len()
                );
                for p in &set.points {
                    println!(
                        "  point id {} kind {} flags {:#x} header {:02x?}",
                        p.id, p.kind, p.flags, p.header
                    );
                    for en in &p.entities {
                        let model = objects
                            .iter()
                            .find(|o| o.object_id == en.object)
                            .map_or(String::from("?"), |o| {
                                String::from_utf8_lossy(&o.model_name).into_owned()
                            });
                        println!(
                            "    entity obj {} {model} type {} arg {} serial {} a1 {:#x} a2 {:#x} pos {:?} talk {} react {} radius {}",
                            en.object,
                            en.spawn_type,
                            en.spawn_arg,
                            en.serial,
                            en.arg1,
                            en.arg2,
                            en.position,
                            en.talk,
                            en.reaction,
                            en.radius
                        );
                    }
                    println!(
                        "    activators {} signals {}",
                        p.activators.len(),
                        p.signals.len()
                    );
                }
            }
            13 => println!("#{i} record {name} {} bytes: {:02x?}", e.size, bytes),
            t => println!("#{i} type {t} {name} {} bytes", e.size),
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
