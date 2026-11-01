//! Read-only source identity. Detection is evidence, not build qualification.

pub mod host;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File},
    io::{self, Read},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

pub const KH2_EXECUTABLE: &str = "KINGDOM HEARTS II FINAL MIX.exe";
const PACKAGES: [&str; 6] = ["first", "second", "third", "fourth", "fifth", "sixth"];
/// Schema 2 also records the SHA-256 of every CHUNK_SIZE-byte chunk, so a
/// session can verify exactly the bytes it reads against the full scan.
pub const SCHEMA: u32 = 2;
pub const CHUNK_SIZE: u64 = 1 << 20;

#[derive(Debug, Serialize, Deserialize)]
pub struct SourceFile {
    pub relative_path: String,
    pub size: u64,
    pub modified_unix_nanos: u128,
    pub sha256: String,
    pub role: String,
    /// SHA-256 (hex) of each CHUNK_SIZE chunk in order (schema 2).
    #[serde(default)]
    pub chunks: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SourceManifest {
    pub schema_version: u32,
    /// Binds every file's chunk digests (schema 2); see `chunk_identity`.
    #[serde(default)]
    pub chunk_identity: String,
    pub canonical_source_path: PathBuf,
    pub product: String,
    pub observed_app_id: Option<String>,
    pub observed_build_id: Option<String>,
    pub observed_steam_language: Option<String>,
    pub qualification: String,
    pub edition_confidence: String,
    pub content_identity: String,
    pub files: Vec<SourceFile>,
    pub unknowns: Vec<String>,
}

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

fn timestamp(time: SystemTime) -> io::Result<u128> {
    Ok(time
        .duration_since(UNIX_EPOCH)
        .map_err(|_| invalid("timestamp predates Unix epoch"))?
        .as_nanos())
}

/// Hash a regular file using bounded memory, rejecting metadata changes during reading.
pub fn inspect_file(root: &Path, relative: &str, role: &str) -> io::Result<SourceFile> {
    inspect_file_with(root, relative, role, true)
}

/// `inspect_file`; `chunks` = false skips the per-chunk digests (a check
/// against an existing manifest needs only the whole-file hash).
pub fn inspect_file_with(
    root: &Path,
    relative: &str,
    role: &str,
    with_chunks: bool,
) -> io::Result<SourceFile> {
    inspect_file_counted(root, relative, role, with_chunks, None)
}

/// `inspect_file_with`, adding each hashed chunk's length to `progress`.
fn inspect_file_counted(
    root: &Path,
    relative: &str,
    role: &str,
    with_chunks: bool,
    progress: Option<&AtomicU64>,
) -> io::Result<SourceFile> {
    let path = root.join(relative);
    let resolved = fs::canonicalize(&path)?;
    if !resolved.starts_with(root) {
        return Err(invalid("source link escapes the selected installation"));
    }
    let before = fs::metadata(&resolved)?;
    if !before.is_file() {
        return Err(invalid(format!("expected a file: {relative}")));
    }
    let mut file = File::open(&resolved)?;
    let mut hash = Sha256::new();
    let mut buffer = vec![0; CHUNK_SIZE as usize];
    let mut bytes = 0u64;
    let mut chunks = Vec::new();
    loop {
        // Fill a whole chunk (short reads are legal) so chunk bounds are
        // fixed multiples of CHUNK_SIZE.
        let mut filled = 0;
        while filled < buffer.len() {
            let count = file.read(&mut buffer[filled..])?;
            if count == 0 {
                break;
            }
            filled += count;
        }
        if filled == 0 {
            break;
        }
        hash.update(&buffer[..filled]);
        if with_chunks {
            chunks.push(format!("{:x}", Sha256::digest(&buffer[..filled])));
        }
        bytes += filled as u64;
        if let Some(progress) = progress {
            progress.fetch_add(filled as u64, Ordering::Relaxed);
        }
        if filled < buffer.len() {
            break;
        }
    }
    let after = file.metadata()?;
    let path_after = fs::metadata(&resolved)?;
    if bytes != before.len()
        || after.len() != before.len()
        || after.modified()? != before.modified()?
        || path_after.len() != before.len()
        || path_after.modified()? != before.modified()?
    {
        return Err(invalid(format!(
            "source changed during scan: {relative}; finish Steam updates and retry"
        )));
    }
    Ok(SourceFile {
        relative_path: relative.replace('\\', "/"),
        size: bytes,
        modified_unix_nanos: timestamp(before.modified()?)?,
        sha256: format!("{:x}", hash.finalize()),
        role: role.into(),
        chunks,
    })
}

/// SHA-256 over every file's chunk digests in path order, so the chunk
/// records cannot drift from the manifest they came with.
pub fn chunk_identity(files: &[SourceFile]) -> String {
    let mut sorted: Vec<_> = files.iter().collect();
    sorted.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
    let mut hash = Sha256::new();
    hash.update(b"kh2-source-chunks-v1\0");
    hash.update(CHUNK_SIZE.to_le_bytes());
    for file in sorted {
        hash.update((file.relative_path.len() as u64).to_le_bytes());
        hash.update(file.relative_path.as_bytes());
        hash.update((file.chunks.len() as u64).to_le_bytes());
        for c in &file.chunks {
            hash.update(c.as_bytes());
        }
    }
    format!("{:x}", hash.finalize())
}

/// `inspect_file` for every listed file, several at a time.
pub fn inspect_files(
    root: &Path,
    list: &[(String, &str)],
    with_chunks: bool,
) -> io::Result<Vec<SourceFile>> {
    inspect_files_counted(root, list, with_chunks, None)
}

fn inspect_files_counted(
    root: &Path,
    list: &[(String, &str)],
    with_chunks: bool,
    progress: Option<&AtomicU64>,
) -> io::Result<Vec<SourceFile>> {
    let workers = std::thread::available_parallelism()
        .map_or(4, |n| n.get())
        .min(list.len().max(1));
    let next = std::sync::atomic::AtomicUsize::new(0);
    let results: Vec<std::sync::Mutex<Option<io::Result<SourceFile>>>> =
        list.iter().map(|_| std::sync::Mutex::new(None)).collect();
    std::thread::scope(|s| {
        for _ in 0..workers {
            s.spawn(|| {
                loop {
                    let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    let Some((relative, role)) = list.get(i) else {
                        break;
                    };
                    let r = inspect_file_counted(root, relative, role, with_chunks, progress);
                    *results[i].lock().unwrap() = Some(r);
                }
            });
        }
    });
    results
        .into_iter()
        .zip(list)
        .map(|(r, (relative, _))| {
            r.into_inner()
                .unwrap()
                .unwrap_or_else(|| Err(invalid("scan worker stopped")))
                .map_err(|e| invalid(format!("cannot inspect {relative}: {e}; select the collection install root and finish/verify the installation in Steam")))
        })
        .collect()
}

/// Identity excludes machine-specific root paths and timestamps; names and content remain binding.
pub fn content_identity(files: &[SourceFile]) -> String {
    let mut sorted: Vec<_> = files.iter().collect();
    sorted.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
    let mut hash = Sha256::new();
    hash.update(b"kh2-source-identity-v1\0");
    for file in sorted {
        hash.update((file.relative_path.len() as u64).to_le_bytes());
        hash.update(file.relative_path.as_bytes());
        hash.update(file.size.to_le_bytes());
        hash.update(file.sha256.as_bytes());
    }
    format!("{:x}", hash.finalize())
}

pub fn required_files() -> Vec<(String, &'static str)> {
    let mut files = vec![(KH2_EXECUTABLE.into(), "kh2_executable_candidate")];
    for package in PACKAGES {
        for extension in ["hed", "pkg"] {
            files.push((
                format!("Image/dt/kh2_{package}.{extension}"),
                "kh2_package_candidate",
            ));
        }
    }
    files
}

/// Scan the observed local layout, fail on missing files, and leave unsupported sources quarantined.
pub fn scan(root: &Path) -> io::Result<SourceManifest> {
    scan_with(root, true)
}

/// `scan`; without chunk digests when only whole-file hashes are compared.
pub fn scan_with(root: &Path, with_chunks: bool) -> io::Result<SourceManifest> {
    scan_counted(root, with_chunks, None)
}

/// Total size of the required files, for `scan_with_progress` (missing
/// files count as 0; the scan itself reports them).
pub fn required_bytes(root: &Path) -> u64 {
    required_files()
        .iter()
        .filter_map(|(relative, _)| fs::metadata(root.join(relative)).ok())
        .map(|m| m.len())
        .sum()
}

/// `scan_with`, adding hashed bytes to `progress` as the scan runs (compare
/// with `required_bytes`).
pub fn scan_with_progress(
    root: &Path,
    with_chunks: bool,
    progress: &AtomicU64,
) -> io::Result<SourceManifest> {
    scan_counted(root, with_chunks, Some(progress))
}

fn scan_counted(
    root: &Path,
    with_chunks: bool,
    progress: Option<&AtomicU64>,
) -> io::Result<SourceManifest> {
    let root = fs::canonicalize(root)?;
    if !root.is_dir() {
        return Err(invalid(
            "select the installed game folder, not a file or cache",
        ));
    }
    let files = inspect_files_counted(&root, &required_files(), with_chunks, progress)?;
    if let Some(empty) = files.iter().find(|f| f.size == 0) {
        return Err(invalid(format!(
            "empty required candidate file: {}; finish/verify the Steam installation",
            empty.relative_path
        )));
    }
    let acf = root
        .parent()
        .and_then(Path::parent)
        .map(|p| p.join("appmanifest_2552430.acf"));
    let metadata = match acf.filter(|p| p.is_file()) {
        Some(path) => fs::read_to_string(path)?,
        None => String::new(),
    };
    let steam_matches = vdf_value(&metadata, "appid").as_deref() == Some("2552430")
        && vdf_value(&metadata, "installdir").is_some_and(|dir| {
            root.file_name()
                .is_some_and(|name| name == std::ffi::OsStr::new(&dir))
        });
    Ok(SourceManifest {
        schema_version: SCHEMA,
        chunk_identity: chunk_identity(&files),
        canonical_source_path: root,
        product: "KINGDOM HEARTS II FINAL MIX (Steam collection candidate)".into(),
        observed_app_id: steam_matches
            .then(|| vdf_value(&metadata, "appid"))
            .flatten(),
        observed_build_id: steam_matches
            .then(|| vdf_value(&metadata, "buildid"))
            .flatten(),
        observed_steam_language: steam_matches
            .then(|| vdf_value(&metadata, "language"))
            .flatten(),
        qualification: "quarantined_unqualified".into(),
        edition_confidence:
            "Steam language and package names are clues; edition not independently verified".into(),
        content_identity: content_identity(&files),
        files,
        unknowns: vec![
            "No independently qualified supported-build profile yet".into(),
            "Clean/modded status not established by hashes alone".into(),
            "Shared launcher, language, font, movie and audio dependency coverage incomplete"
                .into(),
            "English/Japanese content and edition equivalence unverified".into(),
        ],
    })
}

// Deliberately bounded: extracts simple manifest scalar clues, not a full VDF parser.
pub fn vdf_value(text: &str, key: &str) -> Option<String> {
    text.lines().find_map(|line| {
        let fields: Vec<_> = line.split('"').collect();
        (fields.len() >= 5 && fields[1] == key).then(|| fields[3].to_owned())
    })
}

/// Resolve an output directory first, to prevent scanner output mutating the source tree.
pub fn safe_output(root: &Path, output: &Path) -> io::Result<PathBuf> {
    let root = fs::canonicalize(root)?;
    let parent = output
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let parent = fs::canonicalize(parent)?;
    if parent.starts_with(&root) {
        return Err(invalid(
            "manifest output must be outside the Steam installation",
        ));
    }
    if parent.ancestors().any(|dir| dir.join(".git").exists()) {
        return Err(invalid(
            "private source records must be outside Git checkouts",
        ));
    }
    let name = output
        .file_name()
        .ok_or_else(|| invalid("output must name a new file"))?;
    let output = parent.join(name);
    if output.exists() {
        return Err(invalid("output already exists; choose a new manifest path"));
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "kh2-synthetic-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }
    fn populate(path: &Path) {
        for (file, _) in required_files() {
            let path = path.join(file);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, b"synthetic fixture only").unwrap();
        }
    }
    #[test]
    fn identity_survives_relocation_but_detects_damage_and_missing_files() {
        let fixture = Fixture::new();
        let a = fixture.0.join("Steam A/日本語 KH2");
        let b = fixture.0.join("Steam B/renamed");
        populate(&a);
        populate(&b);
        let first = scan(&a).unwrap();
        assert_eq!(first.content_identity, scan(&a).unwrap().content_identity);
        assert_eq!(first.content_identity, scan(&b).unwrap().content_identity);
        assert_eq!(first.qualification, "quarantined_unqualified");
        fs::write(b.join(KH2_EXECUTABLE), b"changed synthetic edition").unwrap();
        assert_ne!(first.content_identity, scan(&b).unwrap().content_identity);
        fs::remove_file(b.join("Image/dt/kh2_first.hed")).unwrap();
        assert!(scan(&b).is_err());
        assert!(scan(&fixture.0).is_err());
    }
    #[test]
    fn hash_is_sha256_and_read_does_not_modify_source() {
        let fixture = Fixture::new();
        fs::write(fixture.0.join("sample"), b"abc").unwrap();
        let root = fs::canonicalize(&fixture.0).unwrap();
        let before = fs::metadata(root.join("sample"))
            .unwrap()
            .modified()
            .unwrap();
        let file = inspect_file(&root, "sample", "synthetic").unwrap();
        assert_eq!(
            file.sha256,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            before,
            fs::metadata(root.join("sample"))
                .unwrap()
                .modified()
                .unwrap()
        );
    }
    #[test]
    fn output_rejects_source_tree_and_existing_files() {
        let fixture = Fixture::new();
        populate(&fixture.0);
        assert!(safe_output(&fixture.0, &fixture.0.join("manifest.json")).is_err());
        assert!(safe_output(&fixture.0, &fixture.0.join("Image/manifest.json")).is_err());
        let other = Fixture::new();
        assert!(safe_output(&fixture.0, &other.0.join("new.json")).is_ok());
        fs::write(other.0.join("old.json"), b"preserve").unwrap();
        assert!(safe_output(&fixture.0, &other.0.join("old.json")).is_err());
        fs::create_dir(other.0.join(".git")).unwrap();
        assert!(safe_output(&fixture.0, &other.0.join("private-manifest.json")).is_err());
    }
    #[test]
    fn steam_scalar_clues_are_bounded() {
        let text = "\"AppState\"\n{\n\t\"appid\" \"2552430\"\n\t\"language\" \"english\"\n}";
        assert_eq!(vdf_value(text, "appid").as_deref(), Some("2552430"));
        assert_eq!(vdf_value(text, "language").as_deref(), Some("english"));
        assert_eq!(vdf_value(text, "buildid"), None);
    }

    #[test]
    fn progress_counts_every_hashed_byte() {
        let fixture = Fixture::new();
        populate(&fixture.0);
        let progress = AtomicU64::new(0);
        let manifest = scan_with_progress(&fixture.0, true, &progress).unwrap();
        let total = required_bytes(&fixture.0);
        assert_eq!(total, manifest.files.iter().map(|f| f.size).sum::<u64>());
        assert_eq!(progress.load(Ordering::Relaxed), total);
        assert_eq!(
            manifest.content_identity,
            scan(&fixture.0).unwrap().content_identity
        );
    }

    #[test]
    fn empty_install_file_is_not_a_complete_candidate() {
        let fixture = Fixture::new();
        populate(&fixture.0);
        fs::write(fixture.0.join(KH2_EXECUTABLE), b"").unwrap();
        assert!(scan(&fixture.0).is_err());
    }

    #[test]
    fn supports_long_selected_path() {
        let fixture = Fixture::new();
        let mut path = fs::canonicalize(&fixture.0).unwrap();
        for _ in 0..10 {
            path = path.join("synthetic-long-path-component");
        }
        populate(&path);
        assert_eq!(scan(&path).unwrap().files.len(), 13);
    }

    #[test]
    fn steam_metadata_must_name_the_selected_install() {
        let fixture = Fixture::new();
        let root = fixture.0.join("steamapps/common/selected");
        populate(&root);
        let acf = fixture.0.join("steamapps/appmanifest_2552430.acf");
        fs::write(&acf,"\"appid\" \"2552430\"\n\"installdir\" \"other-edition\"\n\"buildid\" \"1\"\n\"language\" \"english\"").unwrap();
        assert_eq!(scan(&root).unwrap().observed_build_id, None);
        fs::write(&acf,"\"appid\" \"2552430\"\n\"installdir\" \"selected\"\n\"buildid\" \"1\"\n\"language\" \"english\"").unwrap();
        assert_eq!(scan(&root).unwrap().observed_build_id.as_deref(), Some("1"));
    }
}
