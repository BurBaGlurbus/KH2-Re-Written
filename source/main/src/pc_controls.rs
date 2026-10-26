//! The PC version's own key bindings for this player (T256): its
//! `config1525.dat` (the collection's launcher writes it under Documents\My
//! Games\KINGDOM HEARTS HD 1.5+2.5 ReMIX\<store>\<account>\), read only and
//! only for its bindings; the exe's defaults when there is none. The
//! newest one wins when several accounts have one.

use std::path::{Path, PathBuf};
use visual::input::{Assign, Bindings, Controls, Devices};

const GAME_FOLDER: &str = "My Games/KINGDOM HEARTS HD 1.5+2.5 ReMIX";
const FILE: &str = "config1525.dat";
/// The file is 0x1fc bytes; nothing larger is read.
const FILE_LIMIT: u64 = 0x1000;

/// The Documents folders to look in (OneDrive may hold it).
fn documents() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Some(home) = std::env::var_os("USERPROFILE").map(PathBuf::from) {
        out.push(home.join("Documents"));
        out.push(home.join("OneDrive").join("Documents"));
    }
    if let Some(one) = std::env::var_os("OneDrive").map(PathBuf::from) {
        out.push(one.join("Documents"));
    }
    out.dedup();
    out
}

/// The newest `config1525.dat` under `documents`.
fn find(documents: &[PathBuf]) -> Option<PathBuf> {
    let mut best: Option<(std::time::SystemTime, PathBuf)> = None;
    for d in documents {
        let Ok(stores) = std::fs::read_dir(d.join(GAME_FOLDER)) else {
            continue;
        };
        for store in stores.flatten() {
            let Ok(accounts) = std::fs::read_dir(store.path()) else {
                continue;
            };
            for account in accounts.flatten() {
                let path = account.path().join(FILE);
                let Ok(meta) = std::fs::metadata(&path) else {
                    continue;
                };
                if !meta.is_file() || meta.len() > FILE_LIMIT {
                    continue;
                }
                let at = meta.modified().unwrap_or(std::time::UNIX_EPOCH);
                if best.as_ref().is_none_or(|(t, _)| at > *t) {
                    best = Some((at, path));
                }
            }
        }
    }
    best.map(|(_, p)| p)
}

fn read(path: &Path) -> Option<Bindings> {
    Bindings::from_config(&std::fs::read(path).ok()?)
}

/// The controls for an instance asking for `wanted`; `defaults` keeps the
/// exe's defaults (unattended runs, so they do not depend on this PC).
pub fn load(wanted: Assign, defaults: bool) -> Controls {
    let bindings = if defaults {
        None
    } else {
        find(&documents()).and_then(|p| read(&p))
    }
    .unwrap_or_default();
    let devices = match wanted {
        Assign::Auto => Devices::Both,
        Assign::Fixed(d) => d,
    };
    eprintln!(
        "Controls: the PC bindings ({}), mouse sensitivity {}, reading {}.",
        bindings.source,
        bindings.sensitivity,
        devices.label()
    );
    Controls {
        bindings,
        wanted,
        devices,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_newest_config_is_found_and_others_are_left() {
        let base = std::env::temp_dir().join(format!("kh2-controls-{}", std::process::id()));
        let docs = base.join("Documents");
        let a = docs.join(GAME_FOLDER).join("Steam").join("1");
        let b = docs.join(GAME_FOLDER).join("Epic").join("2");
        std::fs::create_dir_all(&a).unwrap();
        std::fs::create_dir_all(&b).unwrap();
        std::fs::write(a.join(FILE), b"old").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
        std::fs::write(b.join(FILE), b"new").unwrap();
        // Not the file: ignored.
        std::fs::write(a.join("other.dat"), b"x").unwrap();
        assert_eq!(find(std::slice::from_ref(&docs)), Some(b.join(FILE)));
        assert_eq!(find(&[base.join("none")]), None);
        // Not a config: the defaults.
        assert_eq!(read(&b.join(FILE)), None);
        std::fs::remove_dir_all(&base).unwrap();
    }
}
