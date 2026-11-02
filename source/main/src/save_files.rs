//! The file menu's host side shared by the programs that open it: the
//! `save.2ld` art, the texts, and a backend over the save container (the
//! rewrite's own file; the original's file is only ever read).

use data::{path_lookup::Library, probe_source::ProbeSource};
use save::{
    container::Container,
    data::SaveData,
    menu::{Backend, Input},
    slots::{self, Slot},
    store::{self, Store},
};
use std::path::{Path, PathBuf};
use visual::{
    file_menu::{self, Texts},
    hud,
    menu_screen::{self, Menu2ld},
};

/// The image one program hands the next (the title's Load to the room, the
/// room to the world map and back): a save file of its own under the data
/// folder, kept beside the container.
pub fn handoff_path() -> PathBuf {
    crate::launcher::data_dir()
        .join("saves")
        .join("handoff.sav")
}

/// A test of the chain between programs: with `KH2RW_CHAIN=1` an unattended
/// run hands over as a person's would.
pub fn chain_test() -> bool {
    std::env::var_os("KH2RW_CHAIN").is_some()
}

/// What such a test adds to the next run of `program`
/// (`KH2RW_CHAIN_ARGS_<PROGRAM>`, split on `|`).
pub fn chain_args(program: &str) -> Vec<String> {
    let name = format!("KH2RW_CHAIN_ARGS_{}", program.to_uppercase());
    std::env::var(name)
        .map(|v| v.split('|').map(str::to_owned).collect())
        .unwrap_or_default()
}

/// Set on a program the launcher started.
pub const UNDER_LAUNCHER: &str = "KH2RW_UNDER_LAUNCHER";
/// The line that asks the launcher for the next program, followed by its
/// arguments as a JSON list.
pub const NEXT_PROGRAM: &str = "kh2fm-next ";

/// Starts the next program of a chain (room to map to field, event to room).
/// The launcher waits on its one child and shows itself when that ends, so
/// under it the launcher starts the next one.
pub fn start_next(args: Vec<String>) -> bool {
    if std::env::var_os(UNDER_LAUNCHER).is_some() {
        println!("{NEXT_PROGRAM}{}", serde_json::json!(args));
        return true;
    }
    let extra = args.first().map(|p| chain_args(p)).unwrap_or_default();
    std::env::current_exe()
        .and_then(|exe| {
            std::process::Command::new(exe)
                .args(&args)
                .args(extra)
                .spawn()
        })
        .inspect_err(|e| eprintln!("The next program could not start: {e}"))
        .is_ok()
}

/// The world the handed-over image is in.
pub fn handoff_world() -> Option<u8> {
    let bytes = std::fs::read(handoff_path()).ok()?;
    bytes.get(slots::field::WORLD).copied()
}

pub fn write_handoff(image: &SaveData) -> std::io::Result<()> {
    let path = handoff_path();
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, image.bytes())
}

/// The save container the menu works on, written back after each change.
pub struct FileBackend {
    container: Container,
    store: Option<Store>,
    /// What Save writes: the host's picture of the running game.
    snapshot: Option<SaveData>,
}

impl FileBackend {
    /// A container kept in memory only (unattended runs use generated data).
    pub fn in_memory(container: Container) -> Self {
        Self {
            container,
            store: None,
            snapshot: None,
        }
    }

    /// A save container file of any account, read without its Steam id and
    /// kept in memory (unattended checks with a supplied file).
    pub fn from_test_file(path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let png = std::fs::read(path)?;
        let mask = Container::recover_mask(&png).ok_or("no header mask found in the file")?;
        Ok(Self::in_memory(Container::open_masked(&png, &mask)?))
    }

    /// The rewrite's own container under `data_dir/saves`. When it does not
    /// exist yet, the original game's file for the same Steam account is
    /// read (never written) as the starting point.
    pub fn open(data_dir: &Path, root: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let exe = std::fs::read(root.join("KINGDOM HEARTS II FINAL MIX.exe"))?;
        let sbox = save::cipher::Sbox::from_executable(&exe)?;
        let original = store::original_folder(&crate::launcher::install::game_documents());
        let id = original
            .as_deref()
            .and_then(|f| store::steam_ids(f).first().copied())
            .unwrap_or(0);
        let path: PathBuf = data_dir.join("saves").join(store::FILE_NAME);
        let own = Store::new(path.clone(), id, sbox.clone());
        let container = if path.is_file() {
            own.load()?
        } else if let Some(folder) = original.filter(|_| id != 0) {
            let theirs = Store::new(folder.join(id.to_string()).join(store::FILE_NAME), id, sbox);
            theirs
                .load()
                .unwrap_or_else(|_| Container::empty(&save::cipher::key_from_steam_id(id)))
        } else {
            own.load()?
        };
        Ok(Self {
            container,
            store: Some(own),
            snapshot: None,
        })
    }

    /// The state the next Save writes (the host captures it when the menu
    /// opens).
    pub fn set_snapshot(&mut self, data: SaveData) {
        self.snapshot = Some(data);
    }

    fn persist(&self) -> bool {
        self.store
            .as_ref()
            .is_none_or(|s| s.save(&self.container).is_ok())
    }
}

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}

impl Backend for FileBackend {
    fn slots(&self) -> Vec<Slot> {
        slots::list(&self.container)
    }

    fn load(&mut self, slot: usize) -> Option<SaveData> {
        slots::load(&self.container, slot)
    }

    fn save(&mut self, slot: usize, data: &SaveData) -> bool {
        slots::store(&mut self.container, slot, data, now()).is_ok() && self.persist()
    }

    fn delete(&mut self, slot: usize) -> bool {
        slots::delete(&mut self.container, slot);
        self.persist()
    }

    fn capture(&mut self) -> SaveData {
        self.snapshot.clone().unwrap_or_else(SaveData::blank)
    }
}

/// Generated saves for unattended runs: a few worlds, levels and
/// backgrounds, one damaged file.
pub fn synthetic_container() -> Container {
    use save::slots::field;
    let mut c = Container::empty(&save::cipher::key_from_steam_id(1));
    /// slot, world, area, level, frames, munny, difficulty, background.
    type Row = (usize, u8, u8, u8, u32, u32, u32, u8);
    let rows: [Row; 6] = [
        (0, 2, 8, 12, 216_000 * 3 + 3600 * 25, 12_345, 1, 0),
        (1, 4, 3, 40, 216_000 * 21, 880_000, 2, 3),
        (2, 6, 1, 55, 216_000 * 40 + 3600 * 5, 999_999, 3, 5),
        (4, 14, 2, 25, 216_000 * 9, 5_000, 0, 6),
        (5, 18, 4, 99, 216_000 * 98 + 3600 * 59, 0, 1, 7),
        (8, 15, 0, 7, 3600 * 3, 120, 1, 2),
    ];
    for (slot, world, area, level, frames, munny, difficulty, background) in rows {
        let mut d = SaveData::blank();
        d.set_u8(field::WORLD, world);
        d.set_u8(field::AREA, area);
        d.set_u8(field::PARTY + field::RECORD_LEVEL, level);
        d.set_u32(field::PLAY_FRAMES, frames);
        d.set_u32(field::MUNNY, munny);
        d.set_u32(field::DIFFICULTY, difficulty);
        d.set_u8(field::BACKGROUND, background);
        d.seal();
        let _ = slots::store(&mut c, slot, &d, 1_700_000_000 + slot as i64 * 1000);
    }
    let mut broken = SaveData::blank();
    broken.set_u32(field::MUNNY, 7);
    let _ = c.write(&save::slot_name(3), broken.bytes(), 1_700_000_500);
    c
}

/// The art, texts and backend of one open file menu.
pub struct Files {
    pub menu: Menu2ld,
    pub sheets: Vec<usize>,
    pub texts: Texts,
    pub backend: FileBackend,
    pub ui: Option<file_menu::FileMenu>,
    pub last_slot: Option<usize>,
}

impl Files {
    /// Loads `menu/us/save.2ld`, the menu texts and the place names.
    pub fn load(
        source: &ProbeSource,
        sys: (&str, usize, [u8; 16]),
        backend: FileBackend,
        layout: usize,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let mut library = Library::open(source)?;
        let found = library
            .find("menu/us/save.2ld")
            .ok_or("menu/us/save.2ld is not installed")?;
        let menu =
            menu_screen::load_layout(source, found.index, found.ordinal, found.name_hash, layout)?;
        let ids = file_menu::message_ids();
        let loaded = hud::load_messages_optional(source, sys.0, sys.1, sys.2, &ids)?;
        let mut texts = Texts::default();
        for (id, m) in ids.iter().zip(loaded) {
            if let Some(m) = m {
                texts.messages.insert(id & 0x7fff, m);
            }
        }
        let place = library.read("msg/us/place.bin")?;
        let table = data::bar::read_bar(&place)?;
        let mut keys = Vec::new();
        let mut place_ids = Vec::new();
        for (world, code) in area::progress::WORLDS.iter().enumerate() {
            let Some(e) = table
                .entries
                .iter()
                .find(|e| e.entry_type == 2 && e.name_bytes[..2] == *code.as_bytes())
            else {
                continue;
            };
            let rows = &place[e.offset as usize..e.offset as usize + e.size as usize];
            for (area, r) in rows.as_chunks::<4>().0.iter().enumerate() {
                keys.push((world as u8, area as u8));
                place_ids.push(u16::from_le_bytes([r[0], r[1]]) as u32);
            }
        }
        place_ids.push(0x82b8);
        let loaded = hud::load_messages_optional(source, sys.0, sys.1, sys.2, &place_ids)?;
        for (&key, t) in keys.iter().zip(&loaded) {
            if let Some(t) = t {
                texts.places.insert(key, t.clone());
            }
        }
        texts.unknown_place = loaded.last().cloned().flatten().unwrap_or_default();
        Ok(Self {
            menu,
            sheets: Vec::new(),
            texts,
            backend,
            ui: None,
            last_slot: None,
        })
    }
}

/// The presses a file menu reads from one frame's logical pad bits (pad
/// mode 7, as on the title): up and down repeat, the rest are presses.
pub fn inputs(pressed: u64, repeat: u64) -> Vec<Input> {
    let mut out = Vec::new();
    if repeat & 0x10 != 0 {
        out.push(Input::Up);
    }
    if repeat & 0x20 != 0 {
        out.push(Input::Down);
    }
    for (bit, input) in [
        (0x40, Input::Left),
        (0x80, Input::Right),
        (0x8000, Input::PageUp),
        (0x4000, Input::PageDown),
        (0x4, Input::Decide),
        (0x8, Input::Back),
        (0x1000, Input::Delete),
    ] {
        if pressed & bit != 0 {
            out.push(input);
        }
    }
    out
}
