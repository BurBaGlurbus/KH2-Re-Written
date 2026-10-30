//! Reading the player's own KINGDOM HEARTS III install: the mod's KH3 option
//! needs it, and nothing from it is kept in the repository or shipped. The
//! paks are opened with the key in the player's own executable, and the
//! packages are read as the duel needs them.
//!
//! The formats follow StrawHatPro's `kingdom-hearts-x-elden-ring` documents
//! (MIT, see CREDITS.md).
pub mod anim;
pub mod package;
pub mod pak;
pub mod props;
pub mod skeletal;
pub mod texture;

use package::Package;
use sha2::{Digest, Sha256};
use std::error::Error;
use std::path::{Path, PathBuf};

pub type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

pub(crate) fn bad(why: impl Into<String>) -> Box<dyn Error + Send + Sync> {
    why.into().into()
}

const GAME_DIR: &str = "KINGDOM HEARTS III";
const EXECUTABLE: &str = "Binaries/Win64/KINGDOM HEARTS III.exe";
const PAKS: &str = "Content/Paks";
const STEAM_APP: &str = "2552450";
/// A complete install's regular paks (base and patch); fewer is a partial copy.
const MIN_PAKS: usize = 100;

/// A folder that passed the checks. `root` holds `Binaries` and `Content`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Install {
    pub root: PathBuf,
    /// Identifies the executable and every pak's index: both PCs of a KH3
    /// match have to agree on it.
    pub identity: [u8; 32],
    pub pak_count: usize,
    pub exe_size: u64,
}

impl Install {
    pub fn exe(&self) -> PathBuf {
        self.root.join(EXECUTABLE)
    }

    pub fn pak_dir(&self) -> PathBuf {
        self.root.join(PAKS)
    }

    pub fn identity_hex(&self) -> String {
        self.identity.iter().map(|b| format!("{b:02x}")).collect()
    }
}

/// The game folder inside `folder`: itself, `KINGDOM HEARTS III` below it (the
/// Steam folder holds another of that name), or above a picked sub-folder.
pub fn locate(folder: &Path) -> Option<PathBuf> {
    let has = |p: &Path| p.join(EXECUTABLE).is_file();
    if has(folder) {
        return Some(folder.to_path_buf());
    }
    let inner = folder.join(GAME_DIR);
    if has(&inner) {
        return Some(inner);
    }
    let inner2 = inner.join(GAME_DIR);
    if has(&inner2) {
        return Some(inner2);
    }
    folder
        .ancestors()
        .skip(1)
        .take(4)
        .find(|p| has(p))
        .map(Path::to_path_buf)
}

/// Checks a folder: the executable and paks are there, every regular pak's
/// footer is one this reader knows, and the identity is computed from the
/// executable's size and time-stamp header and the paks' index digests.
pub fn admit(folder: &Path) -> Result<Install> {
    let root = locate(folder).ok_or_else(|| bad("KINGDOM HEARTS III.exe is not in this folder"))?;
    let exe = root.join(EXECUTABLE);
    let exe_size = std::fs::metadata(&exe)?.len();
    let pak_dir = root.join(PAKS);
    let paks = pak::list_paks(&pak_dir).map_err(|_| bad("this copy has no Content/Paks folder"))?;
    if paks.len() < MIN_PAKS {
        return Err(bad(format!(
            "this copy has only {} paks: it looks incomplete",
            paks.len()
        )));
    }
    let mut hash = Sha256::new();
    hash.update(exe_size.to_le_bytes());
    for p in &paks {
        let footer = pak::read_footer(p).map_err(|e| bad(format!("{}: {e}", p.display())))?;
        if footer.version != 4 {
            return Err(bad(format!(
                "{}: an unknown pak version ({})",
                p.display(),
                footer.version
            )));
        }
        hash.update(footer.index_sha1);
    }
    Ok(Install {
        root,
        identity: hash.finalize().into(),
        pak_count: paks.len(),
        exe_size,
    })
}

/// Steam library folders' KH3 installs (`common/KINGDOM HEARTS III`) and the
/// usual Epic folders, those that exist.
pub fn detect(libraries: &[PathBuf]) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = Vec::new();
    let mut add = |p: PathBuf| {
        if let Some(root) = locate(&p)
            && !found.contains(&root)
        {
            found.push(root);
        }
    };
    for lib in libraries {
        let apps = lib.join("steamapps");
        if apps.join(format!("appmanifest_{STEAM_APP}.acf")).is_file()
            || apps.join("common").join(GAME_DIR).is_dir()
        {
            add(apps.join("common").join(GAME_DIR));
        }
    }
    for var in ["ProgramFiles", "ProgramFiles(x86)"] {
        if let Some(base) = std::env::var_os(var) {
            let epic = PathBuf::from(base).join("Epic Games");
            add(epic.join("KINGDOMHEARTS3"));
            add(epic.join(GAME_DIR));
        }
    }
    found
}

/// The install's mounted paks with their key.
pub struct Paks {
    pub mounted: pak::Mounted,
    /// The key's place in the executable, for a faster next session.
    pub key_offset: u64,
}

impl Paks {
    /// Opens every pak. `hint`: a key offset remembered from an earlier
    /// session (checked; the executable is scanned when it is wrong).
    pub fn open(install: &Install, hint: Option<u64>, keep_names: bool) -> Result<Self> {
        let paks = pak::list_paks(&install.pak_dir())?;
        // A pak of real size: an empty patch pak has an index too short to test.
        let probe = paks
            .iter()
            .find(|p| std::fs::metadata(p).is_ok_and(|m| m.len() > 1 << 20))
            .ok_or_else(|| bad("no pak to test the key with"))?;
        let exe = install.exe();
        let remembered = hint.and_then(|o| pak::key_at(&exe, o, probe).ok().map(|k| (k, o)));
        let (key, key_offset) = match remembered {
            Some(found) => found,
            None => pak::find_key(&exe, probe)?,
        };
        Ok(Self {
            mounted: pak::Mounted::open(&install.pak_dir(), &key, keep_names)?,
            key_offset,
        })
    }

    /// A package by its file path without extension (`.../foo` for
    /// `foo.uasset`, `foo.uexp`, `foo.ubulk`); `None` when it is not there.
    pub fn package(&self, path: &str) -> Result<Option<Package>> {
        let Some(head) = self.mounted.read(&format!("{path}.uasset"))? else {
            return Ok(None);
        };
        let uexp = self
            .mounted
            .read(&format!("{path}.uexp"))?
            .unwrap_or_default();
        let ubulk = self
            .mounted
            .read(&format!("{path}.ubulk"))?
            .unwrap_or_default();
        Package::parse(&head, uexp, ubulk).map(Some)
    }

    /// A package by `/Game/...` path.
    pub fn game_package(&self, game_path: &str) -> Result<Option<Package>> {
        self.package(&package::mounted_of_game_path(game_path))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("kh3-admit-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn footer(index_byte: u8) -> Vec<u8> {
        let mut f = vec![1];
        f.extend(0x5a6f_12e1u32.to_le_bytes());
        f.extend(4u32.to_le_bytes());
        f.extend([0; 16]);
        f.extend([index_byte; 20]);
        f
    }

    fn fake_install(paks: usize, byte: u8) -> PathBuf {
        let dir = temp(&format!("{paks}-{byte}"));
        let root = dir.join("KINGDOM HEARTS III").join("KINGDOM HEARTS III");
        std::fs::create_dir_all(root.join("Binaries/Win64")).unwrap();
        std::fs::create_dir_all(root.join("Content/Paks")).unwrap();
        std::fs::write(root.join(EXECUTABLE), [1, 2, 3]).unwrap();
        for i in 0..paks {
            std::fs::write(root.join(format!("Content/Paks/p{i}.pak")), footer(byte)).unwrap();
        }
        dir
    }

    #[test]
    fn a_nested_steam_folder_is_found_and_admitted() {
        let dir = fake_install(MIN_PAKS, 1);
        let install = admit(&dir).unwrap();
        assert!(
            install
                .root
                .ends_with("KINGDOM HEARTS III/KINGDOM HEARTS III")
        );
        assert_eq!(install.pak_count, MIN_PAKS);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_partial_copy_is_refused() {
        let dir = fake_install(3, 1);
        assert!(admit(&dir).unwrap_err().to_string().contains("incomplete"));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_folder_without_the_game_is_refused() {
        let dir = temp("none");
        assert!(admit(&dir).is_err());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn identity_follows_the_paks() {
        let (a, b) = (fake_install(MIN_PAKS, 2), fake_install(MIN_PAKS, 3));
        assert_ne!(admit(&a).unwrap().identity, admit(&b).unwrap().identity);
        std::fs::remove_dir_all(a).unwrap();
        std::fs::remove_dir_all(b).unwrap();
    }
}
