//! The launcher's non-visual half: where its private files live, finding
//! the Steam install, binding a folder to a source manifest (a fast size and
//! time check against an existing one, or a full scan that writes a new
//! one), and turning a test into the program arguments it runs with.
//!
//! The launcher's check only decides what to offer; every program still
//! admits its source itself (`ProbeSource::bind_session` +
//! `verify_session`) when it starts.
use super::test_configs::TEST_CONFIGS;
use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicU64;
use std::time::UNIX_EPOCH;
use validation::{KH2_EXECUTABLE, SourceManifest};

const APP_ID: &str = "2552430";
const SETTINGS: &str = "launcher.json";
/// Schema 2 manifests list ~16k chunk digests (about 1.2 MB).
const MANIFEST_LIMIT: u64 = 8 << 20;
const CONFIG_LIMIT: u64 = 1 << 20;

/// The private folder for launch files, configs, manifests and the
/// launcher's settings: `.local/test-room` beside the checkout the
/// executable was built in, else `%LOCALAPPDATA%\KH2FinalRewrite`.
pub fn data_dir() -> PathBuf {
    if let Ok(exe) = std::env::current_exe() {
        for up in exe.ancestors().skip(1).take(4) {
            let dir = up.join(".local/test-room");
            if dir.is_dir() {
                return dir;
            }
        }
    }
    std::env::var_os("LOCALAPPDATA")
        .map(|local| PathBuf::from(local).join("KH2FinalRewrite"))
        .unwrap_or_else(|| PathBuf::from(".local/test-room"))
}

fn read_limited(path: &Path, limit: u64) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    File::open(path)?.take(limit + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{} is larger than expected", path.display()),
        ));
    }
    Ok(bytes)
}

/// What the launcher remembers between runs.
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    pub game_folder: Option<PathBuf>,
    pub manifest: Option<PathBuf>,
    /// The user's Skate 3 Xbox 360 ISO (or extracted `default.xex`).
    #[serde(default)]
    pub skate3_iso: Option<PathBuf>,
    /// The quick start guide has been shown once.
    #[serde(default)]
    pub guide_seen: bool,
}

pub fn load_settings(dir: &Path) -> Settings {
    read_limited(&dir.join(SETTINGS), CONFIG_LIMIT)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

/// Written beside and renamed over, so a crash never leaves half a file.
pub fn save_settings(dir: &Path, settings: &Settings) -> io::Result<()> {
    fs::create_dir_all(dir)?;
    let next = dir.join(format!("{SETTINGS}.new"));
    fs::write(&next, serde_json::to_vec_pretty(settings)?)?;
    fs::rename(next, dir.join(SETTINGS))
}

pub fn has_executable(folder: &Path) -> bool {
    folder.join(KH2_EXECUTABLE).is_file()
}

/// The game folder at most three levels below `folder` (a Steam library,
/// `steamapps` or `common` picked by mistake), when exactly one is there.
pub fn find_game_below(folder: &Path) -> Option<PathBuf> {
    let mut found = Vec::new();
    let mut level = vec![folder.to_path_buf()];
    for _ in 0..3 {
        let mut next = Vec::new();
        for dir in &level {
            for entry in fs::read_dir(dir).into_iter().flatten().flatten().take(400) {
                if !entry.file_type().is_ok_and(|t| t.is_dir()) {
                    continue;
                }
                let path = entry.path();
                if has_executable(&path) {
                    found.push(path);
                } else {
                    next.push(path);
                }
            }
        }
        if !found.is_empty() || next.len() > 2000 {
            break;
        }
        level = next;
    }
    (found.len() == 1).then(|| found.remove(0))
}

/// Every `"key" "value"` pair named `key` in a Steam VDF text.
fn vdf_values(text: &str, key: &str) -> Vec<String> {
    text.lines()
        .filter_map(|line| {
            let fields: Vec<_> = line.split('"').collect();
            (fields.len() >= 5 && fields[1] == key).then(|| fields[3].replace("\\\\", "\\"))
        })
        .collect()
}

/// Steam library folders: the default install locations and every library
/// they list in `steamapps/libraryfolders.vdf`.
fn steam_libraries() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    for var in ["ProgramFiles(x86)", "ProgramFiles"] {
        if let Some(p) = std::env::var_os(var) {
            roots.push(PathBuf::from(p).join("Steam"));
        }
    }
    roots.push(PathBuf::from(r"C:\Program Files (x86)\Steam"));
    let mut libraries = Vec::new();
    for root in roots {
        if !root.is_dir() {
            continue;
        }
        libraries.push(root.clone());
        if let Ok(bytes) = read_limited(&root.join("steamapps/libraryfolders.vdf"), CONFIG_LIMIT) {
            let text = String::from_utf8_lossy(&bytes);
            libraries.extend(vdf_values(&text, "path").into_iter().map(PathBuf::from));
        }
    }
    libraries
}

/// Installs of the collection (Steam app 2552430) that hold the KH2FM
/// executable, without duplicates.
pub fn detect_installs() -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = Vec::new();
    for library in steam_libraries() {
        let apps = library.join("steamapps");
        let Ok(bytes) = read_limited(
            &apps.join(format!("appmanifest_{APP_ID}.acf")),
            CONFIG_LIMIT,
        ) else {
            continue;
        };
        let text = String::from_utf8_lossy(&bytes);
        let Some(dir) = validation::vdf_value(&text, "installdir") else {
            continue;
        };
        let folder = apps.join("common").join(dir);
        if !has_executable(&folder) {
            continue;
        }
        let folder = fs::canonicalize(&folder).unwrap_or(folder);
        if !found.contains(&folder) {
            found.push(folder);
        }
    }
    found
}

/// A folder bound to a manifest, as the launcher shows it.
#[derive(Debug, Clone, PartialEq)]
pub struct Bound {
    pub folder: PathBuf,
    pub manifest: PathBuf,
    pub identity: String,
    pub build_id: Option<String>,
    pub language: Option<String>,
}

/// Why a folder was not bound.
#[derive(Debug, Clone, PartialEq)]
pub enum CheckError {
    /// No manifest of this folder matches what is on disk (a new folder, or
    /// Steam changed files since the last scan): a full scan is needed.
    NeedsScan,
    /// Not usable; the text says why.
    Rejected(String),
}

fn modified_nanos(meta: &fs::Metadata) -> Option<u128> {
    Some(
        meta.modified()
            .ok()?
            .duration_since(UNIX_EPOCH)
            .ok()?
            .as_nanos(),
    )
}

fn bound(folder: PathBuf, manifest_path: PathBuf, m: SourceManifest) -> Bound {
    Bound {
        folder,
        manifest: manifest_path,
        identity: m.content_identity,
        build_id: m.observed_build_id,
        language: m.observed_steam_language,
    }
}

/// The fast check: the manifest is for this folder and self-consistent,
/// and every required file has the size and time it was hashed at. No
/// file is read.
fn quick_match(folder: &Path, manifest_path: &Path) -> Option<Bound> {
    let bytes = read_limited(manifest_path, MANIFEST_LIMIT).ok()?;
    let m: SourceManifest = serde_json::from_slice(&bytes).ok()?;
    let required = validation::required_files();
    if m.canonical_source_path != folder
        || m.qualification != "quarantined_unqualified"
        || validation::content_identity(&m.files) != m.content_identity
        || m.files.len() != required.len()
    {
        return None;
    }
    for (relative, _) in &required {
        let expected = m.files.iter().find(|f| &f.relative_path == relative)?;
        let meta = fs::metadata(folder.join(relative)).ok()?;
        if meta.len() != expected.size || modified_nanos(&meta)? != expected.modified_unix_nanos {
            return None;
        }
    }
    Some(bound(folder.to_path_buf(), manifest_path.to_path_buf(), m))
}

/// `source-manifest-NN.json` files in `dir`, newest number first.
fn manifests_in(dir: &Path) -> Vec<PathBuf> {
    let mut list: Vec<PathBuf> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("source-manifest-") && n.ends_with(".json"))
        })
        .collect();
    list.sort();
    list.reverse();
    list
}

/// Binds `folder` to an existing manifest (`hint` first, then the data
/// folder's) without hashing.
pub fn quick_check(dir: &Path, folder: &Path, hint: Option<&Path>) -> Result<Bound, CheckError> {
    let folder = fs::canonicalize(folder)
        .map_err(|e| CheckError::Rejected(format!("This folder can't be opened ({e}).")))?;
    if !has_executable(&folder) {
        return Err(CheckError::Rejected(format!(
            "There is no \"{KH2_EXECUTABLE}\" in this folder. Choose the \"KINGDOM HEARTS -HD 1.5+2.5 ReMIX-\" folder Steam installed. To find it, right-click the game in Steam and choose Manage > Browse local files."
        )));
    }
    hint.map(Path::to_path_buf)
        .into_iter()
        .chain(manifests_in(dir))
        .find_map(|path| quick_match(&folder, &path))
        .ok_or(CheckError::NeedsScan)
}

/// Hashes the folder in full and writes a new manifest into `dir`.
pub fn full_scan(dir: &Path, folder: &Path, progress: &AtomicU64) -> Result<Bound, String> {
    let folder = fs::canonicalize(folder).map_err(|e| e.to_string())?;
    let manifest =
        validation::scan_with_progress(&folder, true, progress).map_err(|e| e.to_string())?;
    fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let dir = fs::canonicalize(dir).map_err(|e| e.to_string())?;
    if dir.starts_with(&folder) {
        return Err("The launcher's files cannot live inside the game folder.".into());
    }
    let bytes = serde_json::to_vec_pretty(&manifest).map_err(|e| e.to_string())?;
    for n in 1..100 {
        let path = dir.join(format!("source-manifest-{n:02}.json"));
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(mut file) => {
                file.write_all(&bytes)
                    .and_then(|()| file.sync_all())
                    .map_err(|e| e.to_string())?;
                return Ok(bound(folder, path, manifest));
            }
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e.to_string()),
        }
    }
    Err("No free manifest name in the launcher's folder.".into())
}

/// A program launch file (`launch*.json`): the program's config and extra
/// options. Its source paths are replaced by the chosen folder.
#[derive(Debug, Deserialize)]
struct LaunchFile {
    steam_root: Option<PathBuf>,
    manifest: Option<PathBuf>,
    config: PathBuf,
    #[serde(default)]
    options: Vec<String>,
}

fn launch_file(dir: &Path, name: &str) -> Result<LaunchFile, String> {
    let path = dir.join(format!("{name}.json"));
    let bytes = read_limited(&path, CONFIG_LIMIT)
        .map_err(|_| format!("The test's launch file {} is missing.", path.display()))?;
    serde_json::from_slice(&bytes).map_err(|e| format!("{}: {e}", path.display()))
}

fn config_identity(path: &Path) -> Option<String> {
    let bytes = read_limited(path, CONFIG_LIMIT).ok()?;
    let v: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    Some(v.get("source_content_identity")?.as_str()?.to_owned())
}

/// Where a named file is: index, ordinal and the name's MD5 digest.
type FindFile<'a> = &'a dyn Fn(&str) -> Option<(String, usize, [u8; 16])>;

/// Replaces every `{"$file": name}` with where `name` is: the index, the
/// ordinal and the name's MD5 digest (the HED key).
fn fill_files(v: &mut serde_json::Value, find: FindFile) -> Result<(), String> {
    match v {
        serde_json::Value::Object(o) => {
            if let Some(name) = o.remove("$file") {
                let name = name
                    .as_str()
                    .ok_or("A file name in the test configs is not text.")?;
                let (index, ordinal, hash) = find(name)
                    .ok_or_else(|| format!("{name} is not in the installed game files."))?;
                o.insert("index".into(), index.into());
                o.insert("ordinal".into(), ordinal.into());
                o.insert("name_hash".into(), hash.to_vec().into());
            } else {
                for x in o.values_mut() {
                    fill_files(x, find)?;
                }
            }
        }
        serde_json::Value::Array(a) => {
            for x in a {
                fill_files(x, find)?;
            }
        }
        _ => {}
    }
    Ok(())
}

/// Writes every config in TEST_CONFIGS for build `identity` as
/// `auto-<key>.json` in `dir`.
fn write_made_configs(dir: &Path, identity: &str, find: FindFile) -> Result<(), String> {
    let mut all: serde_json::Map<String, serde_json::Value> =
        serde_json::from_str(TEST_CONFIGS).map_err(|e| format!("test configs: {e}"))?;
    fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    for (key, config) in &mut all {
        fill_files(config, find)?;
        config
            .as_object_mut()
            .ok_or("A test config is not an object.")?
            .insert("source_content_identity".into(), identity.into());
        let path = dir.join(format!("auto-{key}.json"));
        let bytes = serde_json::to_vec_pretty(config).map_err(|e| e.to_string())?;
        fs::write(&path, bytes).map_err(|e| format!("{}: {e}", path.display()))?;
    }
    Ok(())
}

/// The made config `key` for `b`, written first if missing or for another
/// build. The names are looked up in the bound folder's checked indexes.
fn made_config(dir: &Path, b: &Bound, key: &str) -> Result<PathBuf, String> {
    let path = dir.join(format!("auto-{key}.json"));
    if config_identity(&path).as_deref() == Some(b.identity.as_str()) {
        return Ok(path);
    }
    let fail = |e: Box<dyn std::error::Error>| format!("The game files could not be read: {e}");
    let source = data::probe_source::ProbeSource::bind(&b.folder, &b.manifest).map_err(fail)?;
    let library = data::path_lookup::Library::open(&source).map_err(fail)?;
    // The first package holding the name, as in the configs these were
    // made from (se/se000 is also in the sixth package).
    let find = |name: &str| {
        library
            .find_all(name)
            .into_iter()
            .next()
            .map(|f| (f.index.to_owned(), f.ordinal, f.name_hash))
    };
    write_made_configs(dir, &b.identity, &find)?;
    Ok(path)
}

/// One entry of the test list.
pub struct Test {
    pub name: &'static str,
    pub blurb: &'static str,
    /// The `kh2fm` command it runs.
    pub program: &'static str,
}

pub const TESTS: [Test; 6] = [
    Test {
        name: "Title screen",
        blurb: "Disney and Square Enix logos, intro, title menu",
        program: "title",
    },
    Test {
        name: "Test room",
        blurb: "Large arena, Shadow combat, enemy animation summons (F3)",
        program: "play",
    },
    Test {
        name: "Twilight Town",
        blurb: "Station Plaza and the areas its exits lead to",
        program: "tt",
    },
    Test {
        name: "The World That Never Was",
        blurb: "Alley to Between and the areas past it",
        program: "eh",
    },
    Test {
        name: "Cutscene: Roxas and Axel",
        blurb: "Betwixt and Between, before the fight (event 613)",
        program: "event",
    },
    Test {
        name: "World map",
        blurb: "Fly the cursor between the world symbols",
        program: "wm",
    },
];

/// Where the Twilight Town test starts: Station Plaza (tt08) entrance 0,
/// with the story flags 0x800..=0x895 set (Sora's first visit; area-01 §6).
const TT_START: (u8, u8) = (8, 0);
const TT_STORY: [u32; 2] = [0x800, 0x895];
/// Where The World That Never Was test starts: Alley to Between (eh01)
/// entrance 0, where Betwixt and Between's exit leads (exit-01 §0).
const EH_START: (u8, u8) = (1, 0);

/// The start rooms chosen in the debug menu, by world code
/// (`{"tt": [area, entrance], ...}`), in the launcher's data folder.
const START_FILE: &str = "start-area.json";

/// The field worlds the launcher starts, and their default start rooms.
pub fn field_start(world: &str) -> Option<(u8, u8)> {
    match world {
        "tt" => Some(TT_START),
        "eh" => Some(EH_START),
        _ => None,
    }
}

/// The start room saved for `world` in `dir`, if any.
pub fn saved_start(dir: &Path, world: &str) -> Option<(u8, u8)> {
    let bytes = read_limited(&dir.join(START_FILE), CONFIG_LIMIT).ok()?;
    let v: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    let pair = v.get(world)?.as_array()?;
    let n = |i: usize| pair.get(i)?.as_u64().and_then(|n| u8::try_from(n).ok());
    Some((n(0)?, n(1)?))
}

/// Saves the room `world`'s test starts in from now on (the other worlds'
/// choices are kept).
pub fn save_start(world: &str, area: u8, entrance: u8) -> Result<PathBuf, String> {
    save_start_in(&data_dir(), world, area, entrance)
}

fn save_start_in(dir: &Path, world: &str, area: u8, entrance: u8) -> Result<PathBuf, String> {
    let path = dir.join(START_FILE);
    let mut v = read_limited(&path, CONFIG_LIMIT)
        .ok()
        .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
        .filter(serde_json::Value::is_object)
        .unwrap_or_else(|| serde_json::json!({}));
    v[world] = serde_json::json!([area, entrance]);
    fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let next = dir.join(format!("{START_FILE}.new"));
    let bytes = serde_json::to_vec_pretty(&v).map_err(|e| e.to_string())?;
    fs::write(&next, bytes)
        .and_then(|()| fs::rename(&next, &path))
        .map_err(|e| e.to_string())?;
    Ok(path)
}

/// A field world's config (`tt` or `eh`): the arena config's actor, HUD,
/// menu and sound sources with a field area in place of the arena and its
/// enemy. `saved` is the start room chosen in the debug menu.
pub fn field_room_config(
    arena: &Path,
    world: &str,
    saved: Option<(u8, u8)>,
) -> Result<PathBuf, String> {
    let default = field_start(world).ok_or_else(|| format!("no field test for world {world}"))?;
    // KH2FM_TT_STORY=<last flag, e.g. 0x860> sets tt flags 0x800..=last
    // (default: the whole first-visit range); KH2FM_TT_AREA=<area>[:<entrance>]
    // picks Twilight Town's starting area over the saved one.
    let story = std::env::var("KH2FM_TT_STORY")
        .ok()
        .and_then(|v| {
            let v = v.trim();
            let v = v.strip_prefix("0x").map_or_else(
                || v.parse::<u32>().ok(),
                |h| u32::from_str_radix(h, 16).ok(),
            )?;
            (0x800..0x900).contains(&v).then_some([0x800, v])
        })
        .unwrap_or(TT_STORY);
    let start = std::env::var("KH2FM_TT_AREA")
        .ok()
        .filter(|_| world == "tt")
        .and_then(|v| {
            let mut p = v.split(':');
            let area = p.next()?.trim().parse::<u8>().ok()?;
            let entrance = p.next().map_or(Some(0), |e| e.trim().parse::<u8>().ok())?;
            Some((area, entrance))
        })
        .or(saved)
        .unwrap_or(default);
    write_field_config(
        arena,
        world,
        start,
        None,
        &[story],
        &format!("test-room-{world}"),
    )
}

/// The field config the world map's landing jumps to (FUN_1403946d0 →
/// FUN_140152990): the arena config's sources with `world`'s `area` at
/// `entrance`, the program overrides a jump by program id sets
/// (FUN_14039fbc0) and the world map's story ranges.
pub fn landing_room_config(
    arena: &Path,
    world: &str,
    area: u8,
    entrance: u8,
    programs: Option<[i16; 3]>,
    story: &[[u32; 2]],
) -> Result<PathBuf, String> {
    write_field_config(
        arena,
        world,
        (area, entrance),
        programs,
        story,
        "test-room-wm-landing",
    )
}

/// Writes `<name>.json` beside the arena config: its actor, HUD, menu and
/// sound sources with a field area in place of the arena and its enemy.
fn write_field_config(
    arena: &Path,
    world: &str,
    start: (u8, u8),
    programs: Option<[i16; 3]>,
    story: &[[u32; 2]],
    name: &str,
) -> Result<PathBuf, String> {
    let bytes =
        read_limited(arena, CONFIG_LIMIT).map_err(|e| format!("{}: {e}", arena.display()))?;
    let mut room: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|e| format!("{}: {e}", arena.display()))?;
    let o = room
        .as_object_mut()
        .ok_or("The arena config is not an object.")?;
    for key in ["arena", "enemy", "spawn", "npc", "room_selection"] {
        o.remove(key);
    }
    let mut area =
        serde_json::json!({"world":world,"area":start.0,"entrance":start.1,"story":story});
    if let Some(p) = programs {
        area["programs"] = serde_json::json!(p);
    }
    o.insert("area".into(), area);
    let dir = arena.parent().unwrap_or(Path::new("."));
    let (out, next) = (
        dir.join(format!("{name}.json")),
        dir.join(format!("{name}.json.new")),
    );
    let bytes = serde_json::to_vec_pretty(&room).map_err(|e| e.to_string())?;
    fs::write(&next, bytes)
        .and_then(|()| fs::rename(&next, &out))
        .map_err(|e| e.to_string())?;
    Ok(out)
}

/// A developer viewer over one private selection file.
#[derive(Debug, Clone, PartialEq)]
pub struct Viewer {
    pub label: String,
    /// The sibling executable it runs.
    pub program: &'static str,
    pub selection: PathBuf,
}

/// Viewers for the model and image selections in `dir`, sorted by label.
pub fn viewers(dir: &Path) -> Vec<Viewer> {
    let mut list: Vec<Viewer> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter_map(|path| {
            let name = path.file_name()?.to_str()?.to_owned();
            let (who, _) = name.strip_suffix(".json")?.split_once("-selection-")?;
            let v: serde_json::Value =
                serde_json::from_slice(&read_limited(&path, CONFIG_LIMIT).ok()?).ok()?;
            let flag = |k: &str| v.get(k).and_then(serde_json::Value::as_bool) == Some(true);
            let (kind, program) = if flag("inspect_model_geometry") {
                ("Model viewer", "model_inspector")
            } else if flag("inspect_images") {
                ("Image viewer", "image_inspector")
            } else {
                return None;
            };
            // Names for the selections in use [launcher labels].
            let what = match who {
                "actor" => "Sora".to_owned(),
                "weapon" => "Kingdom Key".to_owned(),
                other => {
                    let mut c = other.chars();
                    c.next()
                        .map(|f| f.to_uppercase().chain(c).collect())
                        .unwrap_or_default()
                }
            };
            Some(Viewer {
                label: format!("{kind}: {what}"),
                program,
                selection: path,
            })
        })
        .collect();
    list.sort_by(|a, b| a.label.cmp(&b.label));
    list
}

/// A new private log path for a viewer: outside any checkout, under
/// `%LOCALAPPDATA%\KH2FinalRewrite\logs`.
pub fn viewer_log(program: &str) -> Result<PathBuf, String> {
    let local = std::env::var_os("LOCALAPPDATA").ok_or("LOCALAPPDATA is not set")?;
    let logs = PathBuf::from(local).join("KH2FinalRewrite").join("logs");
    fs::create_dir_all(&logs).map_err(|e| e.to_string())?;
    let stamp = std::time::SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis());
    Ok(logs.join(format!("{program}-{stamp}.jsonl")))
}

/// The content identity the tests' configs were made for (the title
/// config's), if the configs are present.
pub fn supported_identity(dir: &Path) -> Option<String> {
    let launch = launch_file(dir, "launch-title").ok()?;
    config_identity(&dir.join(launch.config))
}

/// Where Axel stands in the combined room (from Sora's spawn; the Shadow
/// is straight ahead) and his facing [launcher choice].
const COMBINED_NPC_OFFSET: [f32; 3] = [-450.0, 0.0, 650.0];
const COMBINED_NPC_YAW: f32 = 2.5;

/// The combined room's config: the arena config with the talk config's
/// NPC added, written beside the arena config (its selections are relative
/// to it). Rebuilt on every launch so it follows both.
pub fn combined_room_config(arena: &Path, talk: &Path) -> Result<PathBuf, String> {
    let read = |path: &Path| -> Result<serde_json::Value, String> {
        let bytes =
            read_limited(path, CONFIG_LIMIT).map_err(|e| format!("{}: {e}", path.display()))?;
        serde_json::from_slice(&bytes).map_err(|e| format!("{}: {e}", path.display()))
    };
    let (mut room, talk_config) = (read(arena)?, read(talk)?);
    if room.get("source_content_identity") != talk_config.get("source_content_identity") {
        return Err("The arena and talk configs are for different builds.".into());
    }
    let mut npc = talk_config
        .get("npc")
        .cloned()
        .ok_or("The talk config has no NPC.")?;
    let (arena_dir, talk_dir) = (
        arena.parent().unwrap_or(Path::new(".")),
        talk.parent().unwrap_or(Path::new(".")),
    );
    if arena_dir != talk_dir
        && let Some(sel) = npc.get_mut("model_selection")
        && let Some(name) = sel.as_str()
    {
        *sel = talk_dir.join(name).to_string_lossy().into_owned().into();
    }
    npc["offset"] = serde_json::json!(COMBINED_NPC_OFFSET);
    npc["yaw"] = serde_json::json!(COMBINED_NPC_YAW);
    room.as_object_mut()
        .ok_or("The arena config is not an object.")?
        .insert("npc".into(), npc);
    let out = arena_dir.join("test-room-combined.json");
    let next = arena_dir.join("test-room-combined.json.new");
    let bytes = serde_json::to_vec_pretty(&room).map_err(|e| e.to_string())?;
    fs::write(&next, bytes)
        .and_then(|()| fs::rename(&next, &out))
        .map_err(|e| e.to_string())?;
    Ok(out)
}

/// `kh2fm` arguments for a test on a bound folder: the program, the folder,
/// its manifest, the config and the launch file's options.
pub fn test_args(dir: &Path, program: &str, b: &Bound) -> Result<Vec<String>, String> {
    // The event player and the world map read everything by name from the
    // bound folder and check the source themselves; no private config.
    // A launch file of this build (a developer's own config) wins; without
    // one the configs are made from the file names in TEST_CONFIGS.
    let own = |name: &str| {
        launch_file(dir, name)
            .ok()
            .map(|l| (dir.join(&l.config), l.options))
            .filter(|(config, _)| config_identity(config).as_deref() == Some(b.identity.as_str()))
    };
    let made = |key: &str| made_config(dir, b, key);
    if program == "event" || program == "wm" {
        let mut args = vec![
            program.to_owned(),
            b.folder.to_string_lossy().into_owned(),
            b.manifest.to_string_lossy().into_owned(),
        ];
        // The world map's landing starts the field from the arena config's
        // sources when one is at hand.
        if program == "wm"
            && let Some(arena) = own("launch").map(|(a, _)| a).or_else(|| made("room").ok())
        {
            args.extend(["--field".to_owned(), arena.to_string_lossy().into_owned()]);
        }
        return Ok(args);
    }
    let (command, config, options) = match program {
        "title" => {
            let (config, options) = match own("launch-title") {
                Some(own) => own,
                None => (made("title")?, Vec::new()),
            };
            ("title", config, options)
        }
        "play" => {
            let (arena, talk, options) = match (own("launch"), own("launch-talk")) {
                (Some((arena, options)), Some((talk, _))) => (arena, talk, options),
                _ => (made("room")?, made("talk")?, Vec::new()),
            };
            ("room", combined_room_config(&arena, &talk)?, options)
        }
        "tt" | "eh" => {
            let (arena, options) = match own("launch") {
                Some(own) => own,
                None => (made("room")?, Vec::new()),
            };
            let saved = saved_start(dir, program);
            ("room", field_room_config(&arena, program, saved)?, options)
        }
        other => return Err(format!("unknown test {other}")),
    };
    if config_identity(&config).as_deref() != Some(b.identity.as_str()) {
        return Err(
            "This test's config was made for a different build than the chosen folder.".into(),
        );
    }
    let mut args = vec![
        command.to_owned(),
        b.folder.to_string_lossy().into_owned(),
        b.manifest.to_string_lossy().into_owned(),
        config.to_string_lossy().into_owned(),
    ];
    args.extend(options);
    Ok(args)
}

/// `kh2fm tt` or `kh2fm eh` without arguments: the world's field config
/// with the arena launch file's source paths.
pub fn field_args(world: &str) -> Result<Vec<PathBuf>, String> {
    let dir = data_dir();
    let arena = launch_file(&dir, "launch")?;
    let (Some(root), Some(manifest)) = (arena.steam_root, arena.manifest) else {
        return Err("launch.json names no steam_root/manifest".into());
    };
    let saved = saved_start(&dir, world);
    let config = field_room_config(&dir.join(arena.config), world, saved)?;
    let mut args = vec![root, manifest, config];
    args.extend(arena.options.into_iter().map(PathBuf::from));
    Ok(args)
}

/// `kh2fm play` without arguments: the combined room with the arena launch
/// file's source paths (as `kh2fm room` uses them).
pub fn play_args() -> Result<Vec<PathBuf>, String> {
    let dir = data_dir();
    let (arena, talk) = (
        launch_file(&dir, "launch")?,
        launch_file(&dir, "launch-talk")?,
    );
    let (Some(root), Some(manifest)) = (arena.steam_root, arena.manifest) else {
        return Err("launch.json names no steam_root/manifest".into());
    };
    let config = combined_room_config(&dir.join(arena.config), &dir.join(talk.config))?;
    let mut args = vec![root, manifest, config];
    args.extend(arena.options.into_iter().map(PathBuf::from));
    Ok(args)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64 as Counter, Ordering};
    static NEXT: Counter = Counter::new(0);

    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "kh2-launcher-synthetic-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(&path).unwrap();
            Self(fs::canonicalize(path).unwrap())
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn populate(root: &Path) {
        for (file, _) in validation::required_files() {
            let path = root.join(file);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, b"synthetic fixture only").unwrap();
        }
    }

    #[test]
    fn start_rooms_are_saved_per_world() {
        let f = Fixture::new();
        assert_eq!(saved_start(&f.0, "eh"), None);
        save_start_in(&f.0, "eh", 4, 1).unwrap();
        save_start_in(&f.0, "tt", 10, 0).unwrap();
        save_start_in(&f.0, "eh", 5, 0).unwrap();
        assert_eq!(saved_start(&f.0, "eh"), Some((5, 0)));
        assert_eq!(saved_start(&f.0, "tt"), Some((10, 0)));
        // A field config starts in the saved room, else the world's default.
        let arena = f.0.join("arena.json");
        fs::write(&arena, br#"{"arena":{},"enemy":{}}"#).unwrap();
        let read = |p: PathBuf| -> serde_json::Value {
            serde_json::from_slice(&fs::read(p).unwrap()).unwrap()
        };
        let eh = read(field_room_config(&arena, "eh", saved_start(&f.0, "eh")).unwrap());
        assert_eq!(eh["area"]["world"], "eh");
        assert_eq!(
            (eh["area"]["area"].as_u64(), eh["area"]["entrance"].as_u64()),
            (Some(5), Some(0))
        );
        assert!(eh.get("arena").is_none());
        let eh = read(field_room_config(&arena, "eh", None).unwrap());
        assert_eq!(eh["area"]["area"].as_u64(), Some(EH_START.0 as u64));
        assert!(field_room_config(&arena, "al", None).is_err());
    }

    #[test]
    fn a_landing_config_names_the_target_and_story() {
        let f = Fixture::new();
        let arena = f.0.join("arena.json");
        fs::write(&arena, br#"{"arena":{},"enemy":{},"hud":"h"}"#).unwrap();
        let out = landing_room_config(&arena, "tt", 9, 0x63, None, &[[0x800, 0x895]]).unwrap();
        assert_eq!(out.file_name().unwrap(), "test-room-wm-landing.json");
        let v: serde_json::Value = serde_json::from_slice(&fs::read(out).unwrap()).unwrap();
        assert_eq!(
            v["area"],
            serde_json::json!({"world":"tt","area":9,"entrance":0x63,"story":[[0x800,0x895]]})
        );
        assert!(v.get("enemy").is_none() && v["hud"] == "h");
        // A jump by program id carries it as the three programs.
        let out = landing_room_config(&arena, "hb", 9, 0, Some([0x33; 3]), &[]).unwrap();
        let v: serde_json::Value = serde_json::from_slice(&fs::read(out).unwrap()).unwrap();
        assert_eq!(v["area"]["programs"], serde_json::json!([0x33, 0x33, 0x33]));
    }

    #[test]
    fn scan_then_quick_check_then_change_needs_scan() {
        let f = Fixture::new();
        let (game, data) = (f.0.join("game"), f.0.join("data"));
        populate(&game);
        assert_eq!(quick_check(&data, &game, None), Err(CheckError::NeedsScan));
        let progress = AtomicU64::new(0);
        let scanned = full_scan(&data, &game, &progress).unwrap();
        assert!(scanned.manifest.ends_with("source-manifest-01.json"));
        assert_eq!(quick_check(&data, &game, None).unwrap(), scanned);
        // A hint that is not a manifest of this folder is skipped.
        let other = quick_check(&data, &game, Some(&data.join("missing.json"))).unwrap();
        assert_eq!(other, scanned);
        fs::write(game.join(KH2_EXECUTABLE), b"a different synthetic exe").unwrap();
        assert_eq!(quick_check(&data, &game, None), Err(CheckError::NeedsScan));
        let rescanned = full_scan(&data, &game, &progress).unwrap();
        assert!(rescanned.manifest.ends_with("source-manifest-02.json"));
        assert_ne!(rescanned.identity, scanned.identity);
    }

    #[test]
    fn game_found_below_a_library_folder() {
        let f = Fixture::new();
        let game = f.0.join("steamapps").join("common").join("KH");
        fs::create_dir_all(&game).unwrap();
        fs::create_dir_all(f.0.join("steamapps").join("common").join("Other")).unwrap();
        assert_eq!(find_game_below(&f.0), None);
        fs::write(game.join(KH2_EXECUTABLE), b"synthetic").unwrap();
        assert_eq!(find_game_below(&f.0), Some(game.clone()));
        assert_eq!(find_game_below(&f.0.join("steamapps")), Some(game.clone()));
        // Two copies: the player has to choose.
        let second = f.0.join("steamapps").join("common").join("Other");
        fs::write(second.join(KH2_EXECUTABLE), b"synthetic").unwrap();
        assert_eq!(find_game_below(&f.0), None);
    }

    #[test]
    fn made_configs_name_every_file() {
        let f = Fixture::new();
        // Synthetic lookup: every name is found, keyed by its own digest.
        let find = |name: &str| {
            Some((
                "Image/dt/kh2_first.hed".to_owned(),
                name.len(),
                data::path_lookup::md5(name.as_bytes()),
            ))
        };
        write_made_configs(&f.0, "synthetic-identity", &find).unwrap();
        for key in ["title", "room", "talk"] {
            let path = f.0.join(format!("auto-{key}.json"));
            assert_eq!(
                config_identity(&path).as_deref(),
                Some("synthetic-identity")
            );
            let text = fs::read_to_string(&path).unwrap();
            assert!(!text.contains("$file"), "{key}");
            // Model selections the config names were written beside it.
            let v: serde_json::Value = serde_json::from_str(&text).unwrap();
            for sel in [
                &v["actor_selection"],
                &v["weapon_selection"],
                &v["enemy"]["model_selection"],
                &v["npc"]["model_selection"],
            ] {
                if let Some(name) = sel.as_str() {
                    assert!(f.0.join(name).is_file(), "{name}");
                }
            }
        }
        let title: serde_json::Value =
            serde_json::from_slice(&fs::read(f.0.join("auto-title.json")).unwrap()).unwrap();
        let hash: Vec<u8> = serde_json::from_value(title["music"]["name_hash"].clone()).unwrap();
        assert_eq!(hash, data::path_lookup::md5(b"vagstream/Title.win32.scd"));
        // A name the install lacks is reported by name.
        let err = write_made_configs(&f.0, "x", &|_: &str| None).unwrap_err();
        assert!(err.contains("is not in the installed game files"), "{err}");
        // The room and talk configs combine into the play config.
        let combined =
            combined_room_config(&f.0.join("auto-room.json"), &f.0.join("auto-talk.json"));
        assert!(combined.is_ok(), "{combined:?}");
    }

    #[test]
    fn folder_without_executable_is_rejected() {
        let f = Fixture::new();
        assert!(matches!(
            quick_check(&f.0, &f.0, None),
            Err(CheckError::Rejected(_))
        ));
    }

    #[test]
    fn settings_round_trip() {
        let f = Fixture::new();
        assert_eq!(load_settings(&f.0), Settings::default());
        let s = Settings {
            game_folder: Some(f.0.join("game")),
            manifest: Some(f.0.join("m.json")),
            skate3_iso: Some(f.0.join("skate.iso")),
            guide_seen: true,
        };
        save_settings(&f.0, &s).unwrap();
        assert_eq!(load_settings(&f.0), s);
        // Settings written before the Skate 3 field still load.
        fs::write(
            f.0.join(SETTINGS),
            r#"{"game_folder":null,"manifest":null}"#,
        )
        .unwrap();
        assert_eq!(load_settings(&f.0), Settings::default());
    }

    #[test]
    fn viewers_come_from_model_and_image_selections() {
        let f = Fixture::new();
        let write = |name: &str, v: serde_json::Value| {
            fs::write(f.0.join(name), v.to_string()).unwrap();
        };
        write(
            "actor-selection-01.json",
            serde_json::json!({"inspect_model_geometry": true}),
        );
        write(
            "shadow-selection-01.json",
            serde_json::json!({"inspect_model_geometry": true}),
        );
        write(
            "tex-selection-01.json",
            serde_json::json!({"inspect_images": true}),
        );
        write("other-selection-01.json", serde_json::json!({}));
        write(
            "launch.json",
            serde_json::json!({"inspect_model_geometry": true}),
        );
        let labels: Vec<_> = viewers(&f.0)
            .into_iter()
            .map(|v| (v.label, v.program))
            .collect();
        assert_eq!(
            labels,
            vec![
                ("Image viewer: Tex".to_owned(), "image_inspector"),
                ("Model viewer: Shadow".to_owned(), "model_inspector"),
                ("Model viewer: Sora".to_owned(), "model_inspector"),
            ]
        );
    }

    #[test]
    fn library_paths_unescape() {
        let text =
            "\"libraryfolders\"\n{\n\t\"0\"\n\t{\n\t\t\"path\"\t\t\"D:\\\\SteamLibrary\"\n\t}\n}";
        assert_eq!(
            vdf_values(text, "path"),
            vec![r"D:\SteamLibrary".to_owned()]
        );
    }

    #[test]
    fn combined_config_adds_the_npc_and_checks_identity() {
        let f = Fixture::new();
        let id = "a".repeat(64);
        let arena = f.0.join("arena.json");
        let talk = f.0.join("talk.json");
        fs::write(
            &arena,
            serde_json::json!({"source_content_identity": id, "enemy": {"offset": [0, 0, 250]}})
                .to_string(),
        )
        .unwrap();
        fs::write(
            &talk,
            serde_json::json!({"source_content_identity": id,
                "npc": {"model_selection": "npc.json", "offset": [0, 0, 180], "yaw": 3.0}})
            .to_string(),
        )
        .unwrap();
        let out = combined_room_config(&arena, &talk).unwrap();
        let v: serde_json::Value = serde_json::from_slice(&fs::read(&out).unwrap()).unwrap();
        assert!(v.get("enemy").is_some());
        assert_eq!(v["npc"]["model_selection"], "npc.json");
        assert_eq!(v["npc"]["offset"], serde_json::json!(COMBINED_NPC_OFFSET));
        fs::write(
            &talk,
            serde_json::json!({"source_content_identity": "b", "npc": {}}).to_string(),
        )
        .unwrap();
        assert!(combined_room_config(&arena, &talk).is_err());
    }

    /// Starts every test of the list from an empty data folder against the
    /// installed game (`KH2_TEST_FOLDER` = the Steam game folder): scans
    /// it, makes the configs, then runs each program of the built
    /// `KH2RW` (short unattended runs; the event player only loads).
    /// `cargo build -p main` first; run with `-- --ignored`.
    #[test]
    #[ignore = "needs the installed game: set KH2_TEST_FOLDER"]
    fn every_test_starts_from_an_empty_data_folder() {
        let Some(folder) = std::env::var_os("KH2_TEST_FOLDER") else {
            return;
        };
        let f = Fixture::new();
        let data = f.0.join("data");
        let bound = full_scan(&data, Path::new(&folder), &AtomicU64::new(0)).unwrap();
        let exe = std::env::current_exe()
            .unwrap()
            .parent()
            .and_then(Path::parent)
            .unwrap()
            .join(if cfg!(windows) { "KH2RW.exe" } else { "KH2RW" });
        assert!(
            exe.is_file(),
            "build the main binary first: {}",
            exe.display()
        );
        for t in TESTS.iter() {
            let mut args = test_args(&data, t.program, &bound).unwrap();
            if t.program == "event" || t.program == "wm" {
                args.push("--load-only".into());
            } else {
                args.extend(["--frames".into(), "240".into()]);
            }
            let status = std::process::Command::new(&exe)
                .args(&args)
                .status()
                .unwrap();
            assert!(status.success(), "{} exited with {status}", t.name);
        }
    }
}
