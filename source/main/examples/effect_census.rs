//! Private research tool: steps every effect of the installed game's
//! containers (objects, maps, system, magic) in the effect VM and reports
//! the programs it lacks, per container and in total.
//!
//! cargo run -p main --example effect_census --release --locked -- <Steam-root> <manifest> <out.tsv>
//!
//! Output holds only program numbers, counts and installed file names (no
//! game bytes); keep it outside the repository.
use data::{bar, object_effect, objentry, path_lookup::Library, probe_source::ProbeSource};
use std::{collections::BTreeMap, sync::Arc};

const WORLDS: [&str; 19] = [
    "zz", "es", "tt", "di", "hb", "bb", "he", "al", "mu", "po", "lk", "lm", "dc", "wi", "nm", "wm",
    "ca", "tr", "eh",
];

fn pax_of(bytes: &[u8]) -> Vec<object_effect::Pax> {
    let Ok(table) = bar::read_bar(bytes) else {
        return Vec::new();
    };
    table
        .entries
        .iter()
        .filter(|e| e.entry_type == 18 && e.size > 0)
        .filter_map(|e| {
            bytes
                .get(e.offset as usize..e.offset as usize + e.size as usize)
                .and_then(|b| object_effect::read_pax(b).ok())
        })
        .collect()
}

/// Steps each element's effect 240 frames; returns the unsupported kinds.
fn unsupported(pax: &object_effect::Pax) -> BTreeMap<u32, Vec<u16>> {
    let mut out: BTreeMap<u32, Vec<u16>> = BTreeMap::new();
    let view = effect::math::IDENTITY;
    let mut rng = effect::Rng::default();
    for element in &pax.elements {
        let Some(dpx) = pax.effect(element.effect_number) else {
            continue;
        };
        let world = effect::instance_matrix(element.translation, element.rotation, element.scale);
        let Some(mut instance) = effect::Instance::new(
            Arc::clone(&pax.dpds[dpx.dpd]),
            dpx.pdata,
            world,
            element.start_wait,
        ) else {
            continue;
        };
        for _ in 0..240 {
            instance.frame(&view, &mut rng);
            if instance.finished() {
                break;
            }
        }
        for &k in &instance.unsupported {
            out.entry(k).or_default().push(element.id);
        }
    }
    out
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let source = ProbeSource::bind_session(
        std::path::Path::new(&args[0]),
        std::path::Path::new(&args[1]),
    )?;
    source.verify_session()?;
    let mut library = Library::open(&source)?;
    let objects = objentry::read(&library.read("00objentry.bin")?)?;
    let mut containers: Vec<(String, Vec<object_effect::Pax>)> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for o in &objects {
        let name = String::from_utf8_lossy(&o.model_name)
            .trim_end_matches('\0')
            .to_string();
        if name.is_empty() || !seen.insert(name.clone()) {
            continue;
        }
        if let Ok(bytes) = library.read(&format!("obj/{name}.a.us")) {
            let p = pax_of(&bytes);
            if !p.is_empty() {
                containers.push((format!("obj/{name}"), p));
            }
        }
    }
    for w in WORLDS {
        for n in 0..100 {
            let name = format!("map/{w}{n:02}.map");
            if let Ok(bytes) = library.read(&name) {
                let p = pax_of(&bytes);
                if !p.is_empty() {
                    containers.push((name, p));
                }
            }
        }
    }
    if let Ok(bytes) = library.read("00effect.bar") {
        containers.push(("00effect.bar".into(), pax_of(&bytes)));
    }
    let mut totals: BTreeMap<u32, (usize, Vec<String>)> = BTreeMap::new();
    let mut lines = vec!["container\tpax\tkind\telements".to_string()];
    for (name, paxes) in &containers {
        for pax in paxes {
            for (kind, ids) in unsupported(pax) {
                let t = totals.entry(kind).or_default();
                t.0 += ids.len();
                if !t.1.contains(name) {
                    t.1.push(name.clone());
                }
                lines.push(format!("{name}\t{}\t{kind}\t{ids:?}", pax.name));
            }
        }
    }
    // FUN_1403d10c0's rows: the attacker's own guard spark.
    lines.push(String::new());
    lines.push("GUARD SPARKS container\tpax\telement\tcategory".into());
    for (name, paxes) in &containers {
        for pax in paxes {
            for e in &pax.elements {
                if e.category & 0x10 != 0 && e.category >> 32 != 0 {
                    lines.push(format!("{name}\t{}\t{}\t{:#x}", pax.name, e.id, e.category));
                }
            }
        }
    }
    lines.push(String::new());
    lines.push("TOTALS kind\telements\tcontainers".into());
    for (kind, (n, c)) in &totals {
        lines.push(format!("{kind}\t{n}\t{}", c.len()));
    }
    std::fs::write(&args[2], lines.join("\n"))?;
    println!(
        "{} containers, {} unsupported kinds",
        containers.len(),
        totals.len()
    );
    Ok(())
}
