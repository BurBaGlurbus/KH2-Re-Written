//! The container file on disk. The original keeps one per Steam account:
//! `<Documents>\My Games\KINGDOM HEARTS HD 1.5+2.5 ReMIX\Steam\<id>\KHIIFM_WW.png`
//! (FUN_140103e40 builds the folder, FUN_1400fef80 the name from `KHFM.png`).
//! The rewrite writes its own copy under its private data folder and only
//! ever reads the original's file.

use crate::{
    cipher::{Sbox, key_from_steam_id},
    container::{self, Container},
};
use std::{
    fs, io,
    path::{Path, PathBuf},
};

/// The container's file name for Kingdom Hearts II Final Mix.
pub const FILE_NAME: &str = "KHIIFM_WW.png";
/// The original's save folder under "My Games".
pub const ORIGINAL_FOLDER: &str = "KINGDOM HEARTS HD 1.5+2.5 ReMIX";

#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    Container(container::Error),
    /// The decrypted header does not carry the key: wrong Steam id.
    WrongKey,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "{e}"),
            Self::Container(e) => write!(f, "{e}"),
            Self::WrongKey => f.write_str("the save file was written for another Steam account"),
        }
    }
}

impl std::error::Error for Error {}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<container::Error> for Error {
    fn from(e: container::Error) -> Self {
        Self::Container(e)
    }
}

/// A container file and the key that opens it.
pub struct Store {
    path: PathBuf,
    key: [u8; 16],
    sbox: Sbox,
}

impl Store {
    pub fn new(path: PathBuf, steam_id: u64, sbox: Sbox) -> Self {
        Self {
            path,
            key: key_from_steam_id(steam_id),
            sbox,
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The container: empty when the file does not exist yet.
    pub fn load(&self) -> Result<Container, Error> {
        let bytes = match fs::read(&self.path) {
            Ok(b) => b,
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                return Ok(Container::empty(&self.key));
            }
            Err(e) => return Err(e.into()),
        };
        let container = Container::open(&bytes, &self.key, &self.sbox)?;
        if container.key_matches(&self.key) {
            Ok(container)
        } else {
            Err(Error::WrongKey)
        }
    }

    /// Writes the container through a temporary file so a failure keeps the
    /// old one.
    pub fn save(&self, container: &Container) -> Result<(), Error> {
        if let Some(dir) = self.path.parent() {
            fs::create_dir_all(dir)?;
        }
        let tmp = self.path.with_extension("png.new");
        fs::write(&tmp, container.to_png(&self.key, &self.sbox))?;
        fs::rename(&tmp, &self.path)?;
        Ok(())
    }
}

/// Where the original game keeps the saves of the Steam accounts that
/// played it, if a "My Games" folder holding them exists; `other_documents`
/// are searched after the Windows profile's (Proton's, on Linux).
pub fn original_folder(other_documents: &[PathBuf]) -> Option<PathBuf> {
    let mut roots = Vec::new();
    if let Some(p) = std::env::var_os("USERPROFILE") {
        roots.push(PathBuf::from(p).join("Documents"));
    }
    for var in ["OneDrive", "OneDriveConsumer", "OneDriveCommercial"] {
        if let Some(p) = std::env::var_os(var) {
            roots.push(PathBuf::from(p).join("Documents"));
        }
    }
    roots.extend_from_slice(other_documents);
    roots
        .into_iter()
        .map(|r| r.join("My Games").join(ORIGINAL_FOLDER).join("Steam"))
        .find(|p| p.is_dir())
}

/// The Steam ids that have a save folder under `steam_folder`
/// (`<id>\KHIIFM_WW.png`), by folder name.
pub fn steam_ids(steam_folder: &Path) -> Vec<u64> {
    let Ok(entries) = fs::read_dir(steam_folder) else {
        return Vec::new();
    };
    let mut ids: Vec<u64> = entries
        .filter_map(Result::ok)
        .filter(|e| e.path().join(FILE_NAME).is_file())
        .filter_map(|e| e.file_name().to_str()?.parse().ok())
        .collect();
    ids.sort_unstable();
    ids
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cipher::aes_sbox;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT: AtomicU64 = AtomicU64::new(0);

    fn temp() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "kh2-save-synthetic-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn missing_file_is_an_empty_container_and_save_round_trips() {
        let dir = temp();
        let store = Store::new(dir.join("a").join(FILE_NAME), 42, aes_sbox());
        let mut c = store.load().unwrap();
        assert!(c.entries().is_empty());
        c.write("BISLPM-66675FM-00", &[7, 7], 5).unwrap();
        store.save(&c).unwrap();
        let back = store.load().unwrap();
        assert_eq!(back.read("BISLPM-66675FM-00").unwrap().1, [7, 7]);
        // Another account's key is refused.
        let other = Store::new(store.path().to_path_buf(), 43, aes_sbox());
        assert!(matches!(other.load(), Err(Error::WrongKey)));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn steam_ids_come_from_the_folder_names() {
        let dir = temp();
        for id in ["76561198000000001", "notanid", "76561198000000002"] {
            fs::create_dir_all(dir.join(id)).unwrap();
        }
        fs::write(dir.join("76561198000000001").join(FILE_NAME), b"x").unwrap();
        fs::write(dir.join("notanid").join(FILE_NAME), b"x").unwrap();
        assert_eq!(steam_ids(&dir), [76561198000000001]);
        fs::remove_dir_all(dir).unwrap();
    }
}
