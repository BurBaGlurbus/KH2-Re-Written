//! Where this system keeps things: private data and Steam. These are places to
//! look, not evidence: whatever is found still has to pass source validation.

use std::path::{Path, PathBuf};

/// The folder private per-user program data goes under: `%LOCALAPPDATA%` on
/// Windows, the XDG data folder (`~/.local/share`) elsewhere.
pub fn local_data_root() -> Option<PathBuf> {
    if cfg!(windows) {
        std::env::var_os("LOCALAPPDATA").map(PathBuf::from)
    } else {
        unix_data_root(xdg_data_home(), std::env::home_dir())
    }
}

/// Steam's own folders, wherever a default install puts them. Not all exist;
/// the same install can appear under several names.
pub fn steam_roots() -> Vec<PathBuf> {
    if cfg!(windows) {
        let mut roots: Vec<PathBuf> = ["ProgramFiles(x86)", "ProgramFiles"]
            .into_iter()
            .filter_map(std::env::var_os)
            .map(|p| PathBuf::from(p).join("Steam"))
            .collect();
        roots.push(PathBuf::from(r"C:\Program Files (x86)\Steam"));
        roots
    } else {
        std::env::home_dir()
            .map(|home| unix_steam_roots(&home, xdg_data_home().as_deref()))
            .unwrap_or_default()
    }
}

/// `XDG_DATA_HOME` when usable; the XDG spec says a relative value is ignored.
fn xdg_data_home() -> Option<PathBuf> {
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
}

fn unix_data_root(xdg: Option<PathBuf>, home: Option<PathBuf>) -> Option<PathBuf> {
    xdg.or_else(|| home.map(|h| h.join(".local/share")))
}

/// The native package's links and folder, then the Flatpak's and the Snap's.
fn unix_steam_roots(home: &Path, xdg: Option<&Path>) -> Vec<PathBuf> {
    let mut roots = vec![
        home.join(".steam/steam"),
        home.join(".steam/root"),
        home.join(".local/share/Steam"),
    ];
    roots.extend(xdg.map(|data| data.join("Steam")));
    roots.extend([
        home.join(".var/app/com.valvesoftware.Steam/.local/share/Steam"),
        home.join(".var/app/com.valvesoftware.Steam/data/Steam"),
        home.join("snap/steam/common/.local/share/Steam"),
    ]);
    roots
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unix_data_prefers_xdg_then_home() {
        let (xdg, home) = (PathBuf::from("/x/data"), PathBuf::from("/h"));
        assert_eq!(
            unix_data_root(Some(xdg.clone()), Some(home.clone())),
            Some(xdg)
        );
        assert_eq!(
            unix_data_root(None, Some(home.clone())),
            Some(home.join(".local/share"))
        );
        assert_eq!(unix_data_root(None, None), None);
    }

    #[test]
    fn unix_steam_covers_native_flatpak_snap_and_xdg() {
        let home = Path::new("/h");
        let roots = unix_steam_roots(home, Some(Path::new("/x/data")));
        for expected in [
            home.join(".steam/steam"),
            home.join(".local/share/Steam"),
            Path::new("/x/data").join("Steam"),
            home.join(".var/app/com.valvesoftware.Steam/.local/share/Steam"),
            home.join("snap/steam/common/.local/share/Steam"),
        ] {
            assert!(roots.contains(&expected), "{}", expected.display());
        }
    }
}
