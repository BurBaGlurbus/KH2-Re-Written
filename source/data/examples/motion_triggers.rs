//! Research tool: prints the range and frame triggers (BAR type 16) of the
//! given slots of an installed motion set, as the motion player reads them.
//!
//! cargo run -p data --release --example motion_triggers --locked -- <Steam-root> <manifest> <model> <slot>...
use data::{bar, motion_trigger, path_lookup::Library, probe_source::ProbeSource};
use std::path::PathBuf;

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() < 4 {
        return Err("usage: motion_triggers <Steam-root> <manifest> <model> <slot>...".into());
    }
    let source = ProbeSource::bind_session(&PathBuf::from(&args[0]), &PathBuf::from(&args[1]))?;
    source.verify_full_source()?;
    let mut library = Library::open(&source)?;
    let bytes = library.read(&format!("obj/{}.mset", args[2]))?;
    let table = bar::read_bar(&bytes)?;
    println!(
        "{}: {} entries, variants {}",
        args[2],
        table.entries.len(),
        table.header_field_12 & 1 != 0
    );
    for slot in &args[3..] {
        let slot: usize = slot.parse()?;
        let Some(e) = table.entries.get(slot) else {
            println!("slot {slot}: absent");
            continue;
        };
        let start = usize::try_from(e.offset)?;
        let Some(anb) = bytes
            .get(start..start + e.size as usize)
            .filter(|b| !b.is_empty())
        else {
            println!("slot {slot}: empty");
            continue;
        };
        for t in motion_trigger::inspect_anb(anb)? {
            for r in &t.records.ranges {
                println!(
                    "slot {slot} range {}..{} kind {} {:?}",
                    r.start_frame, r.end_frame, r.trigger, r.parameters
                );
            }
            for f in &t.records.frames {
                println!(
                    "slot {slot} frame {} kind {} {:?}",
                    f.frame, f.trigger, f.parameters
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
