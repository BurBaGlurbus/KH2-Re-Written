//! Research tool: lists, for every English ARD in the installed packages, the
//! `B_*` object models its spawn sets place, so a boss fight can be traced to
//! the area that holds it. Prints names only; nothing is written.
//!
//! cargo run -p data --release --example boss_placements --locked -- <Steam-root> <manifest> [model prefix]
use data::{ard, bar, objentry, path_lookup::Library, probe_source::ProbeSource};
use std::{collections::BTreeSet, path::PathBuf};

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() < 2 {
        return Err("usage: boss_placements <Steam-root> <manifest> [model prefix]".into());
    }
    let prefix = args
        .get(2)
        .and_then(|a| a.to_str())
        .unwrap_or("B_")
        .to_string();
    let source = ProbeSource::bind_session(&PathBuf::from(&args[0]), &PathBuf::from(&args[1]))?;
    source.verify_full_source()?;
    let mut library = Library::open(&source)?;
    let objects = objentry::read(&library.read("00objentry.bin")?)?;
    let worlds = [
        "al", "bb", "ca", "dc", "eh", "es", "ex", "gm", "hb", "he", "lk", "lm", "mi", "mu", "nm",
        "po", "tr", "tt", "wi", "zz", "tz", "wm", "ze", "sp", "fn",
    ];
    for world in worlds {
        for area in 0..100 {
            let name = format!("ard/us/{world}{area:02}.ard");
            if library.find(&name).is_none() {
                continue;
            }
            let bytes = library.read(&name)?;
            let table = bar::read_bar(&bytes)?;
            let mut seen = BTreeSet::new();
            for entry in &table.entries {
                if entry.entry_type != 12 || entry.offset < 0 {
                    continue;
                }
                let start = entry.offset as usize;
                let Ok(set) = ard::spawn_set(&bytes[start..start + entry.size as usize]) else {
                    continue;
                };
                for point in &set.points {
                    for entity in &point.entities {
                        if let Some(o) = objects.iter().find(|o| o.object_id == entity.object) {
                            let model = String::from_utf8_lossy(&o.model_name).to_string();
                            if model.starts_with(&prefix) {
                                seen.insert(format!(
                                    "{} (obj 0x{:x}, type {})",
                                    model, o.object_id, o.object_type
                                ));
                            }
                        }
                    }
                }
            }
            if !seen.is_empty() {
                println!(
                    "{name}: {}",
                    seen.into_iter().collect::<Vec<_>>().join(", ")
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
