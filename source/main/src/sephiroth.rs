//! `kh2fm sephiroth <Steam-root> <manifest> <config> [room options]`: the
//! Sephiroth encounter of Hollow Bastion in the order the area script plays
//! it: the meeting (event 601), the fight in area 1 under map program 262 and
//! battle program 75, then the aftermath (event 602).
//!
//! Each part is its own program, so the chain starts this executable again
//! for every part. The fight reports a win through a result file; closing
//! any part early ends the chain.
use std::{
    ffi::OsString,
    path::PathBuf,
    process::{Command, ExitCode},
};

/// Area 1's background group mask under these programs (the area script's
/// `DAT_142a0f7a8`: groups 0 and 1).
const BG_MASK: &str = "3";

fn part(exe: &std::path::Path, args: &[OsString]) -> Result<bool, String> {
    Command::new(exe)
        .args(args)
        .status()
        .map(|s| s.success())
        .map_err(|e| format!("could not start {}: {e}", exe.display()))
}

fn event(steam: &OsString, manifest: &OsString, name: &str, load_only: bool) -> Vec<OsString> {
    let mut args: Vec<OsString> = vec![
        "event".into(),
        steam.clone(),
        manifest.clone(),
        "--event".into(),
        format!("hb:1:{name}").into(),
        "--bg-mask".into(),
        BG_MASK.into(),
    ];
    if load_only {
        args.push("--load-only".into());
    }
    args
}

fn run(args: Vec<PathBuf>) -> Result<(), String> {
    if args.len() < 3 {
        return Err(
            "usage: kh2fm sephiroth <Steam-root> <manifest> <config> [--load-only] [room options]"
                .into(),
        );
    }
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let [steam, manifest, config] = [0, 1, 2].map(|i| args[i].clone().into_os_string());
    let mut rest: Vec<OsString> = args[3..]
        .iter()
        .map(|a| a.clone().into_os_string())
        .collect();
    let load_only = rest.iter().any(|a| a == "--load-only");
    rest.retain(|a| a != "--load-only");
    eprintln!("Sephiroth: the meeting (event 601).");
    if !part(&exe, &event(&steam, &manifest, "601", load_only))? {
        return Err("the meeting did not finish".into());
    }
    if load_only {
        return part(&exe, &event(&steam, &manifest, "602", true))?
            .then_some(())
            .ok_or_else(|| "the aftermath did not load".into());
    }
    let result = std::env::temp_dir().join(format!("kh2rw-sephiroth-{}.txt", std::process::id()));
    let _ = std::fs::remove_file(&result);
    eprintln!("Sephiroth: the fight.");
    let mut fight: Vec<OsString> = vec![
        "room".into(),
        steam.clone(),
        manifest.clone(),
        config,
        "--sephiroth".into(),
        "--sephiroth-result".into(),
        result.clone().into_os_string(),
    ];
    fight.extend(rest);
    part(&exe, &fight)?;
    let won = std::fs::read(&result).is_ok_and(|b| b == b"won");
    let _ = std::fs::remove_file(&result);
    if !won {
        eprintln!("Sephiroth: the fight did not end in a win; the chain stops here.");
        return Ok(());
    }
    eprintln!("Sephiroth: the aftermath (event 602).");
    part(&exe, &event(&steam, &manifest, "602", false)).map(|_| ())
}

pub fn main(args: Vec<PathBuf>, _interactive: bool) -> ExitCode {
    match run(args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("Sephiroth rejected: {e}");
            ExitCode::FAILURE
        }
    }
}
