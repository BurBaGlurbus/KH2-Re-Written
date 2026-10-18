//! One launcher for the rewrite's programs. Started without arguments (a
//! double-click) it opens the launcher window: choose the game folder, then
//! a test. `kh2fm title|room|talk|play|tt|eh [arguments]` runs one directly
//! with the program's own arguments (none = its launch file; `play` is the
//! room with both the Shadow and Axel, `tt` Twilight Town, `eh` The World
//! That Never Was).
//!
//! A Windows-subsystem program, so no console window opens behind the
//! launcher; output still reaches a pipe or redirect (the launcher reads
//! its tests' output that way).
#![cfg_attr(windows, windows_subsystem = "windows")]
mod area;
mod cutscene;
mod keyblade_board;
mod launcher;
mod npcs;
mod save_files;
mod sephiroth;
mod skate;
#[cfg(feature = "skate3")]
mod skate_hud;
mod skate_rig;
mod test_room;
mod title;
mod world_map;

use std::path::PathBuf;
use std::process::ExitCode;

fn run(program: &str, args: Vec<PathBuf>) -> ExitCode {
    match program {
        "title" => title::main(args, false),
        "room" => test_room::main(args, false),
        // The talk room's launch file, unless arguments are given.
        "talk" if args.is_empty() => test_room::main(vec!["talk".into()], false),
        "talk" => test_room::main(args, false),
        "play" if args.is_empty() => match launcher::play_args() {
            Ok(args) => test_room::main(args, false),
            Err(e) => {
                eprintln!("Test room rejected: {e}");
                ExitCode::FAILURE
            }
        },
        "play" => test_room::main(args, false),
        "tt" | "eh" if args.is_empty() => match launcher::field_args(program) {
            Ok(args) => test_room::main(args, false),
            Err(e) => {
                eprintln!("Field world {program} rejected: {e}");
                ExitCode::FAILURE
            }
        },
        "tt" | "eh" => test_room::main(args, false),
        "event" => cutscene::main(args, false),
        "sephiroth" => sephiroth::main(args, false),
        "wm" => world_map::main(args, false),
        "--version" => {
            println!("KH2RW {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        _ => {
            eprintln!(
                "usage: kh2fm [title|room|talk|play|tt|eh|event|sephiroth|wm|--version] [program arguments]"
            );
            ExitCode::FAILURE
        }
    }
}

fn main() -> ExitCode {
    let mut args: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    if args.is_empty() {
        return launcher::main();
    }
    let program = args.remove(0);
    run(&program.to_string_lossy(), args)
}
