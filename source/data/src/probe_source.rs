//! Common source binding for developer probes; it never qualifies gameplay.
use crate::{IndexInventory, MAX_INDEX_BYTES, read_hed};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, Metadata},
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};
use validation::SourceManifest;

type ProbeResult<T> = Result<T, Box<dyn std::error::Error>>;

const MANIFEST_LIMIT: u64 = 8 * 1024 * 1024;

pub struct ProbeSource {
    root: PathBuf,
    manifest: SourceManifest,
    // Set only by `bind_session`: one admission (full scan, or the session
    // check with per-chunk verification of every read) serves the session.
    session: Option<std::sync::OnceLock<()>>,
}

pub struct CheckedIndex {
    pub inventory: IndexInventory,
    pub reader: SourceReader,
    package: PathBuf,
    before: Metadata,
}

/// A package reader. With chunk digests (a session on a schema 2 manifest)
/// every byte served comes from a CHUNK_SIZE chunk that was just read and
/// matched against the full-scan record; a mismatch fails the read.
pub struct SourceReader {
    file: File,
    len: u64,
    pos: u64,
    expected: Vec<[u8; 32]>,
    /// Verified chunks kept for reuse (index, bytes), most recent last.
    cache: Vec<(u64, Vec<u8>)>,
}

const CACHED_CHUNKS: usize = 32;

fn digest_bytes(hex: &str) -> Option<[u8; 32]> {
    let mut out = [0; 32];
    if hex.len() != 64 {
        return None;
    }
    for (i, b) in out.iter_mut().enumerate() {
        *b = u8::from_str_radix(hex.get(2 * i..2 * i + 2)?, 16).ok()?;
    }
    Some(out)
}

impl SourceReader {
    pub(crate) fn new(file: File, len: u64, chunks: &[String]) -> std::io::Result<Self> {
        let expected = chunks
            .iter()
            .map(|h| {
                digest_bytes(h).ok_or_else(|| {
                    std::io::Error::new(std::io::ErrorKind::InvalidData, "chunk digest")
                })
            })
            .collect::<std::io::Result<_>>()?;
        Ok(Self {
            file,
            len,
            pos: 0,
            expected,
            cache: Vec::new(),
        })
    }

    pub fn metadata(&self) -> std::io::Result<Metadata> {
        self.file.metadata()
    }

    /// Chunk `k`, read whole and checked against its recorded digest.
    fn chunk(&mut self, k: u64) -> std::io::Result<usize> {
        if let Some(i) = self.cache.iter().position(|(c, _)| *c == k) {
            let hit = self.cache.remove(i);
            self.cache.push(hit);
            return Ok(self.cache.len() - 1);
        }
        let start = k * validation::CHUNK_SIZE;
        let size = (self.len - start).min(validation::CHUNK_SIZE) as usize;
        let mut bytes = vec![0; size];
        self.file.seek(SeekFrom::Start(start))?;
        self.file.read_exact(&mut bytes)?;
        let digest: [u8; 32] = Sha256::digest(&bytes).into();
        if self.expected.get(k as usize) != Some(&digest) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "package bytes differ from the full-scan record; source rejected",
            ));
        }
        if self.cache.len() == CACHED_CHUNKS {
            self.cache.remove(0);
        }
        self.cache.push((k, bytes));
        Ok(self.cache.len() - 1)
    }
}

impl Read for SourceReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.expected.is_empty() {
            // Schema 1 (research probes): admitted by a full rescan.
            self.file.seek(SeekFrom::Start(self.pos))?;
            let n = self.file.read(buf)?;
            self.pos += n as u64;
            return Ok(n);
        }
        if self.pos >= self.len || buf.is_empty() {
            return Ok(0);
        }
        let k = self.pos / validation::CHUNK_SIZE;
        let at = (self.pos % validation::CHUNK_SIZE) as usize;
        let i = self.chunk(k)?;
        let data = &self.cache[i].1[at..];
        let n = data.len().min(buf.len());
        buf[..n].copy_from_slice(&data[..n]);
        self.pos += n as u64;
        Ok(n)
    }
}

impl Seek for SourceReader {
    fn seek(&mut self, to: SeekFrom) -> std::io::Result<u64> {
        let next = match to {
            SeekFrom::Start(p) => Some(p),
            SeekFrom::End(d) => self.len.checked_add_signed(d),
            SeekFrom::Current(d) => self.pos.checked_add_signed(d),
        };
        self.pos = next.ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "seek before start")
        })?;
        Ok(self.pos)
    }
}

impl ProbeSource {
    pub fn bind(root: &Path, manifest_path: &Path) -> ProbeResult<Self> {
        let root = fs::canonicalize(root)?;
        let mut bytes = Vec::new();
        // Schema 2 lists ~16k chunk digests for the whole collection.
        File::open(manifest_path)?
            .take(MANIFEST_LIMIT + 1)
            .read_to_end(&mut bytes)?;
        if bytes.len() as u64 > MANIFEST_LIMIT {
            return Err("source manifest exceeds inspection limit".into());
        }
        let manifest: SourceManifest = serde_json::from_slice(&bytes)?;
        let required = validation::required_files();
        let chunks_ok = manifest.schema_version == 1
            || (manifest.schema_version == validation::SCHEMA
                && validation::chunk_identity(&manifest.files) == manifest.chunk_identity
                && manifest
                    .files
                    .iter()
                    .all(|f| f.chunks.len() as u64 == f.size.div_ceil(validation::CHUNK_SIZE)));
        if !chunks_ok
            || manifest.canonical_source_path != root
            || manifest.qualification != "quarantined_unqualified"
            || validation::content_identity(&manifest.files) != manifest.content_identity
            || manifest.files.len() != required.len()
            || required.iter().any(|(path, _)| {
                manifest
                    .files
                    .iter()
                    .filter(|f| f.relative_path == *path)
                    .count()
                    != 1
            })
        {
            return Err(
                "source manifest identity/path/schema/candidate layout is inconsistent".into(),
            );
        }
        Ok(Self {
            root,
            manifest,
            session: None,
        })
    }

    /// Binds one interactive session. The first `verify_full_source` still
    /// rehashes every candidate file; later admissions in the same process
    /// reuse that result, and per-index metadata checks still run.
    pub fn bind_session(root: &Path, manifest_path: &Path) -> ProbeResult<Self> {
        let mut source = Self::bind(root, manifest_path)?;
        source.session = Some(std::sync::OnceLock::new());
        Ok(source)
    }

    pub fn content_identity(&self) -> &str {
        &self.manifest.content_identity
    }

    /// Rehash the selected candidate, retaining quarantine. Research viewers
    /// must call this at startup; matching cached metadata is insufficient.
    pub fn verify_full_source(&self) -> ProbeResult<()> {
        if self.session.as_ref().is_some_and(|s| s.get().is_some()) {
            return Ok(());
        }
        let fresh = validation::scan_with(&self.root, false)?;
        if fresh.content_identity != self.manifest.content_identity
            || fresh.files.iter().any(|observed| {
                self.manifest
                    .files
                    .iter()
                    .find(|expected| expected.relative_path == observed.relative_path)
                    .is_none_or(|expected| {
                        observed.sha256 != expected.sha256
                            || observed.size != expected.size
                            || observed.modified_unix_nanos != expected.modified_unix_nanos
                    })
            })
        {
            return Err(
                "selected source differs from the bound manifest; full scan rejected".into(),
            );
        }
        if let Some(session) = &self.session {
            let _ = session.set(());
        }
        Ok(())
    }

    /// Admission of a play session on a schema 2 manifest: the executable
    /// and every index are rehashed whole, every package's size and time
    /// must match, and each package byte the session reads is verified by
    /// its chunk (`SourceReader`) against the full-scan record. A schema 1
    /// manifest falls back to the full rescan.
    pub fn verify_session(&self) -> ProbeResult<()> {
        if self.session.as_ref().is_some_and(|s| s.get().is_some()) {
            return Ok(());
        }
        if self.manifest.schema_version < validation::SCHEMA {
            return self.verify_full_source();
        }
        let whole: Vec<(String, &str)> = validation::required_files()
            .into_iter()
            .filter(|(p, _)| !p.ends_with(".pkg"))
            .collect();
        let fresh = validation::inspect_files(&self.root, &whole, false)?;
        for observed in &fresh {
            let expected = self.expected(&observed.relative_path)?;
            if observed.sha256 != expected.sha256
                || observed.size != expected.size
                || observed.modified_unix_nanos != expected.modified_unix_nanos
            {
                return Err(format!(
                    "{} differs from the bound manifest; source rejected",
                    observed.relative_path
                )
                .into());
            }
        }
        for (relative, _) in validation::required_files() {
            if !relative.ends_with(".pkg") {
                continue;
            }
            let expected = self.expected(&relative)?;
            let meta = fs::metadata(self.root.join(&relative))?;
            if meta.len() != expected.size
                || meta.modified()?.duration_since(UNIX_EPOCH)?.as_nanos()
                    != expected.modified_unix_nanos
            {
                return Err(format!(
                    "{relative} size/time differs from the bound manifest; rescan fully"
                )
                .into());
            }
        }
        if let Some(session) = &self.session {
            let _ = session.set(());
        }
        Ok(())
    }

    fn expected(&self, relative: &str) -> ProbeResult<&validation::SourceFile> {
        self.manifest
            .files
            .iter()
            .find(|f| f.relative_path == relative)
            .ok_or_else(|| format!("{relative} absent from the bound manifest").into())
    }

    pub fn output_path(&self, path: &Path) -> std::io::Result<PathBuf> {
        validation::safe_output(&self.root, path)
    }

    pub fn open_index(&self, relative: &str) -> ProbeResult<CheckedIndex> {
        if self.session.as_ref().is_some_and(|s| s.get().is_none()) {
            return Err("session source not admitted yet (verify_session)".into());
        }
        if !validation::required_files()
            .iter()
            .any(|(name, _)| name == relative && name.ends_with(".hed"))
        {
            return Err("index is outside the inspected KH2 candidate set".into());
        }
        let expected = self
            .manifest
            .files
            .iter()
            .find(|f| f.relative_path == relative)
            .ok_or("missing bound index")?;
        let hed = fs::canonicalize(self.root.join(relative))?;
        if !hed.starts_with(&self.root) {
            return Err("index escapes source".into());
        }
        if expected.size > MAX_INDEX_BYTES as u64
            || fs::metadata(&hed)?.len() > MAX_INDEX_BYTES as u64
        {
            return Err("index exceeds inspection limit before hashing".into());
        }
        let observed = validation::inspect_file(&self.root, relative, "index_check")?;
        if observed.sha256 != expected.sha256
            || observed.size != expected.size
            || observed.modified_unix_nanos != expected.modified_unix_nanos
        {
            return Err("index differs from bound source manifest".into());
        }
        let package_relative = relative
            .strip_suffix(".hed")
            .ok_or("index extension")?
            .to_owned()
            + ".pkg";
        // Pair by the manifest's logical path even when the HED is an in-root link.
        let package = fs::canonicalize(self.root.join(&package_relative))?;
        if !package.starts_with(&self.root) {
            return Err("package escapes source".into());
        }
        let expected = self
            .manifest
            .files
            .iter()
            .find(|f| f.relative_path == package_relative)
            .ok_or("missing bound package")?;
        let file = File::open(&package)?;
        let before = file.metadata()?;
        let reader = SourceReader::new(file, expected.size, &expected.chunks)?;
        if !before.is_file()
            || before.len() != expected.size
            || before.modified()?.duration_since(UNIX_EPOCH)?.as_nanos()
                != expected.modified_unix_nanos
        {
            return Err("package metadata differs from source manifest; rescan fully".into());
        }
        let mut bytes = Vec::new();
        File::open(hed)?
            .take((MAX_INDEX_BYTES + 1) as u64)
            .read_to_end(&mut bytes)?;
        let inventory = read_hed(&bytes, before.len())?;
        Ok(CheckedIndex {
            inventory,
            reader,
            package,
            before,
        })
    }
}

impl CheckedIndex {
    pub fn verify_unchanged(&self) -> ProbeResult<()> {
        for after in [self.reader.metadata()?, fs::metadata(&self.package)?] {
            if after.len() != self.before.len() || after.modified()? != self.before.modified()? {
                return Err("package changed during source inspection".into());
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        sync::atomic::{AtomicU64, Ordering},
        time::SystemTime,
    };

    static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

    struct Fixture {
        base: PathBuf,
        root: PathBuf,
        manifest: PathBuf,
    }
    impl Fixture {
        fn new() -> Self {
            Self::with_stamp(
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos(),
            )
        }

        fn with_stamp(stamp: u128) -> Self {
            let base = std::env::temp_dir().join(format!(
                "kh2-probe-{}-{}-{}",
                std::process::id(),
                stamp,
                NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed),
            ));
            let root = base.join("source");
            fs::create_dir_all(root.join("Image/dt")).unwrap();
            for (relative, _) in validation::required_files() {
                let data = if relative.ends_with(".hed") {
                    let mut record = vec![0; 24];
                    record.extend(16i32.to_le_bytes());
                    record.extend(0i32.to_le_bytes());
                    record
                } else {
                    vec![0; 16]
                };
                fs::write(root.join(relative), data).unwrap();
            }
            let manifest = base.join("manifest.json");
            fs::write(
                &manifest,
                serde_json::to_vec(&validation::scan(&root).unwrap()).unwrap(),
            )
            .unwrap();
            Self {
                base,
                root,
                manifest,
            }
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.base).unwrap();
        }
    }

    #[test]
    fn fixtures_remain_isolated_when_clock_values_repeat() {
        let first = Fixture::with_stamp(42);
        let second = Fixture::with_stamp(42);
        assert_ne!(first.base, second.base);
        drop(first);
        ProbeSource::bind(&second.root, &second.manifest)
            .unwrap()
            .verify_full_source()
            .unwrap();
    }

    #[test]
    fn binding_rejects_changed_indexes_packages_and_unlisted_inputs() {
        let fixture = Fixture::new();
        let source = ProbeSource::bind(&fixture.root, &fixture.manifest).unwrap();
        let checked = source.open_index("Image/dt/kh2_first.hed").unwrap();
        assert_eq!(checked.inventory.record_count, 1);
        checked.verify_unchanged().unwrap();
        assert!(source.open_index("../outside.hed").is_err());
        fs::write(fixture.root.join("Image/dt/kh2_first.pkg"), [0; 17]).unwrap();
        assert!(checked.verify_unchanged().is_err());
        assert!(source.open_index("Image/dt/kh2_first.hed").is_err());
        let hed = fixture.root.join("Image/dt/kh2_second.hed");
        let mut bytes = fs::read(&hed).unwrap();
        bytes[0] = 1;
        fs::write(hed, bytes).unwrap();
        assert!(source.open_index("Image/dt/kh2_second.hed").is_err());
        File::create(fixture.root.join("Image/dt/kh2_third.hed"))
            .unwrap()
            .set_len(MAX_INDEX_BYTES as u64 + 1)
            .unwrap();
        assert!(
            source
                .open_index("Image/dt/kh2_third.hed")
                .err()
                .unwrap()
                .to_string()
                .contains("before hashing")
        );
    }

    #[test]
    fn binding_rejects_incomplete_recomputed_manifest_and_wrong_root() {
        let fixture = Fixture::new();
        assert!(ProbeSource::bind(&fixture.base, &fixture.manifest).is_err());
        let mut manifest: SourceManifest =
            serde_json::from_slice(&fs::read(&fixture.manifest).unwrap()).unwrap();
        manifest.files.pop();
        manifest.content_identity = validation::content_identity(&manifest.files);
        fs::write(&fixture.manifest, serde_json::to_vec(&manifest).unwrap()).unwrap();
        assert!(ProbeSource::bind(&fixture.root, &fixture.manifest).is_err());
    }

    #[test]
    fn sessions_verify_each_package_byte_read_against_its_chunk() {
        let fixture = Fixture::new();
        let source = ProbeSource::bind_session(&fixture.root, &fixture.manifest).unwrap();
        // Nothing opens before the session check.
        assert!(source.open_index("Image/dt/kh2_first.hed").is_err());
        source.verify_session().unwrap();
        let mut checked = source.open_index("Image/dt/kh2_first.hed").unwrap();
        let mut bytes = Vec::new();
        checked.reader.read_to_end(&mut bytes).unwrap();
        assert_eq!(bytes, vec![0; 16]);
        // Same size and time, different bytes: the read fails.
        let pkg = fixture.root.join("Image/dt/kh2_first.pkg");
        let time = fs::metadata(&pkg).unwrap().modified().unwrap();
        fs::write(&pkg, [7; 16]).unwrap();
        File::options()
            .write(true)
            .open(&pkg)
            .unwrap()
            .set_modified(time)
            .unwrap();
        let mut checked = source.open_index("Image/dt/kh2_first.hed").unwrap();
        checked.reader.seek(SeekFrom::Start(4)).unwrap();
        let mut one = [0; 1];
        assert!(checked.reader.read(&mut one).is_err());
        // A changed executable fails a fresh session.
        let source = ProbeSource::bind_session(&fixture.root, &fixture.manifest).unwrap();
        fs::write(fixture.root.join(validation::KH2_EXECUTABLE), [1; 16]).unwrap();
        assert!(source.verify_session().is_err());
    }

    #[test]
    fn full_validation_checks_the_executable_as_bytes_without_loading_it() {
        let fixture = Fixture::new();
        let source = ProbeSource::bind(&fixture.root, &fixture.manifest).unwrap();
        source.verify_full_source().unwrap();
        fs::write(fixture.root.join(validation::KH2_EXECUTABLE), [1; 16]).unwrap();
        assert!(source.verify_full_source().is_err());
    }

    #[test]
    fn model_inspection_rejects_stale_unselected_executable_before_decoding() {
        let fixture = Fixture::new();
        let mut plain_empty_header = [0u8; 16];
        plain_empty_header[8..12].copy_from_slice(&(-2i32).to_le_bytes());
        fs::write(
            fixture.root.join("Image/dt/kh2_first.pkg"),
            plain_empty_header,
        )
        .unwrap();
        fs::write(
            &fixture.manifest,
            serde_json::to_vec(&validation::scan(&fixture.root).unwrap()).unwrap(),
        )
        .unwrap();
        let source = ProbeSource::bind(&fixture.root, &fixture.manifest).unwrap();
        let path = fixture.base.join("selection.json");
        let mut selection = serde_json::json!({"schema_version":1,
            "source_content_identity":source.content_identity(),"inspect_model_geometry":true,
            "model_pose_profiles":["openkh_legacy_bind_fma_v1"],
            "selections":[{"index":"Image/dt/kh2_first.hed","ordinal":0,"name_hash":vec![0u8;16]}]});
        fs::write(&path, serde_json::to_vec(&selection).unwrap()).unwrap();
        let decoding_error = crate::model_inspection::read_selected_models(&source, &path)
            .err()
            .unwrap()
            .to_string();
        assert!(decoding_error.contains("invalid BAR"), "{decoding_error}");
        fs::write(fixture.root.join(validation::KH2_EXECUTABLE), [1; 16]).unwrap();
        assert!(
            crate::model_inspection::read_selected_models(&source, &path)
                .err()
                .unwrap()
                .to_string()
                .contains("full scan rejected")
        );
        selection["model_pose_profiles"] = serde_json::json!([]);
        fs::write(&path, serde_json::to_vec(&selection).unwrap()).unwrap();
        assert!(
            crate::model_inspection::read_selected_models(&source, &path)
                .err()
                .unwrap()
                .to_string()
                .contains("explicit profiles invalid")
        );
    }
}
