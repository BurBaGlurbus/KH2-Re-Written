//! Research tool: for every English ARD in the installed packages, the
//! ordinary enemies (objentry types 4 and 21) each map / battle program places
//! through its spawn sets (`script::run`, FUN_1403a2170), one line per model,
//! so an enemy can be run on an area where it spawns. Prints names only.
//!
//! cargo run -p area --release --example enemy_placements --locked -- <Steam-root> <manifest>
use area::script::{self, Location, ScriptRandom};
use data::{ard, bar, objentry, path_lookup::Library, probe_source::ProbeSource};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
};

/// World, area, script group, program and the map program it ran with.
type Place = (String, u8, &'static str, i16, i16);

struct NoProgress;
impl script::Game for NoProgress {
    fn flag_set(&self, _: u32) -> bool {
        false
    }
}

const WORLDS: [&str; 19] = [
    "zz", "es", "tt", "di", "hb", "bb", "he", "al", "mu", "po", "lk", "lm", "dc", "wi", "nm", "wm",
    "ca", "tr", "eh",
];

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: enemy_placements <Steam-root> <manifest>".into());
    }
    let source = ProbeSource::bind_session(&PathBuf::from(&args[0]), &PathBuf::from(&args[1]))?;
    source.verify_full_source()?;
    let mut library = Library::open(&source)?;
    let objects = objentry::read(&library.read("00objentry.bin")?)?;
    let enemy = |id: u32| {
        objects
            .iter()
            .find(|o| o.object_id == id && matches!(o.object_type, 4 | 21))
    };
    // model -> (world, area, group, program, map program) -> set names.
    let mut found: BTreeMap<String, BTreeSet<Place>> = BTreeMap::new();
    for (world_index, world) in WORLDS.iter().enumerate() {
        for area in 0..100u8 {
            let name = format!("ard/us/{world}{area:02}.ard");
            if library.find(&name).is_none() {
                continue;
            }
            let bytes = library.read(&name)?;
            let table = bar::read_bar(&bytes)?;
            let entry = |kind: u16, tag: u32| {
                table
                    .entries
                    .iter()
                    .find(|e| {
                        e.entry_type == kind
                            && e.offset >= 0
                            && e.name_bytes
                                .iter()
                                .enumerate()
                                .fold(0u32, |v, (i, &b)| v | u32::from(b) << (8 * i))
                                == tag
                    })
                    .and_then(|e| {
                        let start = e.offset as usize;
                        bytes.get(start..start + e.size as usize)
                    })
            };
            let map = entry(13, ard::tag("map"));
            let btl = entry(13, ard::tag("btl"));
            let map_ids = map
                .and_then(|m| ard::program_ids(m).ok())
                .unwrap_or_default();
            let btl_ids = btl
                .and_then(|b| ard::program_ids(b).ok())
                .unwrap_or_default();
            let first_map = map_ids.first().copied().unwrap_or(-1);
            let mut runs: Vec<(&str, i16, [i16; 3])> = Vec::new();
            runs.extend(btl_ids.iter().map(|&b| ("btl", b, [first_map, b, -1])));
            runs.extend(map_ids.iter().map(|&m| ("map", m, [m, -1, -1])));
            for (group, program, programs) in runs {
                let location = Location {
                    world: world_index as u8,
                    area,
                    entrance: 0,
                    programs,
                };
                let Ok(setup) = script::run(
                    &location,
                    [map, btl, None],
                    &mut ScriptRandom::default(),
                    &mut NoProgress,
                ) else {
                    continue;
                };
                for spawn in &setup.spawns {
                    let Some(data) = entry(12, spawn.name) else {
                        continue;
                    };
                    let Ok(set) = ard::spawn_set(data) else {
                        continue;
                    };
                    for entity in set.points.iter().flat_map(|p| &p.entities) {
                        if let Some(o) = enemy(entity.object) {
                            let model = String::from_utf8_lossy(&o.model_name)
                                .trim_end_matches('\0')
                                .to_string();
                            found.entry(model).or_default().insert((
                                world.to_string(),
                                area,
                                group,
                                program,
                                programs[0],
                            ));
                        }
                    }
                }
            }
        }
    }
    for (model, places) in found {
        let list: Vec<String> = places
            .iter()
            .map(|(w, a, g, p, m)| format!("{w}{a:02}:{g}{p}:map{m}"))
            .collect();
        println!("{model} {} {}", places.len(), list.join(" "));
    }
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
