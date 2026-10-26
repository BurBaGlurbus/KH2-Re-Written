//! The installed Birth by Sleep package the LAN duel reads its Mirage Arena
//! from, validated as the KH2 source is: a full scan (whole-file and chunk
//! digests) writes a private manifest beside the KH2 one; each session
//! rehashes the index whole and checks the package's size and time against
//! it, and every package byte read is checked by its chunk.
//!
//! Only `bbs_third` is read: in this build it is the one package holding
//! the arena's archive [installed-build observation, T248].

use crate::probe_source::SourceReader;
use crate::{HedEntry, IndexInventory, original_payload, path_lookup::md5, read_hed};
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

/// The package's index and the package.
pub const INDEX: &str = "Image/dt/bbs_third.hed";
pub const PACKAGE: &str = "Image/dt/bbs_third.pkg";
const ROLE: &str = "bbs_package_candidate";
/// The largest index read whole.
const INDEX_LIMIT: u64 = 8 << 20;

/// An admitted session's reader of the BBS package.
pub struct BbsSource {
    inventory: IndexInventory,
    reader: SourceReader,
}

/// The BBS manifest kept beside a KH2 manifest.
pub fn manifest_beside(kh2_manifest: &Path) -> PathBuf {
    let stem = kh2_manifest.file_stem().map_or_else(
        || "source-manifest".into(),
        |s| s.to_string_lossy().into_owned(),
    );
    kh2_manifest.with_file_name(format!("{stem}.bbs.json"))
}

fn list() -> Vec<(String, &'static str)> {
    vec![(INDEX.into(), ROLE), (PACKAGE.into(), ROLE)]
}

fn modified(path: &Path) -> Result<u128> {
    Ok(fs::metadata(path)?
        .modified()?
        .duration_since(UNIX_EPOCH)?
        .as_nanos())
}

/// The bound manifest, if it still matches the files' sizes and times.
fn current(root: &Path, manifest: &Path) -> Option<Vec<validation::SourceFile>> {
    let bytes = fs::read(manifest).ok()?;
    let files: Vec<validation::SourceFile> = serde_json::from_slice(&bytes).ok()?;
    let ok = files.len() == 2
        && files.iter().all(|f| {
            list().iter().any(|(p, _)| *p == f.relative_path)
                && fs::metadata(root.join(&f.relative_path)).is_ok_and(|m| m.len() == f.size)
                && modified(&root.join(&f.relative_path)).is_ok_and(|t| t == f.modified_unix_nanos)
                && (f.relative_path != PACKAGE
                    || f.chunks.len() as u64 == f.size.div_ceil(validation::CHUNK_SIZE))
        });
    ok.then_some(files)
}

impl BbsSource {
    /// Binds the install's BBS package to the manifest at `manifest`
    /// (scanning in full and writing a new one when there is none or the
    /// files changed), then admits the session. `Err` when the install has
    /// no BBS package or it does not match.
    pub fn open(root: &Path, manifest: &Path) -> Result<Self> {
        // The validation scan reads only inside the canonical install root.
        let root = &fs::canonicalize(root)?;
        let files = match current(root, manifest) {
            Some(f) => f,
            None => {
                let fresh = validation::inspect_files(root, &list(), true)?;
                let tmp = manifest.with_extension("json.tmp");
                fs::write(&tmp, serde_json::to_vec(&fresh)?)?;
                fs::rename(&tmp, manifest)?;
                fresh
            }
        };
        let expect = |p: &str| {
            files
                .iter()
                .find(|f| f.relative_path == p)
                .ok_or_else(|| format!("{p} absent from the BBS manifest"))
        };
        // Admission: the index whole against its digest.
        let index_file = expect(INDEX)?;
        if index_file.size > INDEX_LIMIT {
            return Err("the BBS index is larger than an index can be".into());
        }
        let fresh = validation::inspect_files(root, &[(INDEX.into(), ROLE)], false)?;
        if fresh[0].sha256 != index_file.sha256 || fresh[0].size != index_file.size {
            return Err("the BBS index differs from its manifest; source rejected".into());
        }
        let package = expect(PACKAGE)?;
        let path = root.join(PACKAGE);
        if fs::metadata(&path)?.len() != package.size
            || modified(&path)? != package.modified_unix_nanos
        {
            return Err("the BBS package's size or time differs from its manifest".into());
        }
        let index = fs::read(root.join(INDEX))?;
        let inventory = read_hed(&index, package.size)?;
        let reader = SourceReader::new(File::open(&path)?, package.size, &package.chunks)?;
        Ok(Self { inventory, reader })
    }

    fn entry(&self, name: &str) -> Option<HedEntry> {
        let hash = md5(name.as_bytes());
        self.inventory
            .entries
            .iter()
            .rev()
            .find(|e| e.name_hash == hash)
            .cloned()
    }

    /// The original bytes of `name` (up to `limit`).
    pub fn read(&mut self, name: &str, limit: u32) -> Result<Vec<u8>> {
        let entry = self
            .entry(name)
            .ok_or_else(|| format!("{name} is not in the installed BBS package"))?;
        Ok(original_payload::read_original_limited(&mut self.reader, &entry, limit)?.bytes)
    }
}
