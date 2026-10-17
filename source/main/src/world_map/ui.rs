//! The world map's 2D side for `kh2fm wm`: the messages (FUN_1403ad9a0:
//! `id & 0x7fff` searched in the three loaded tables, sys 0xadc when none
//! has it; here the area's wm.bar, sys.bar and jm.bar [I: the journal
//! table as the third, since the story texts the menu shows are there]), the
//! wm_ws.2ld layout, the fonts, the world-point pictures, and per world
//! what the info panel and the landing menu show, prepared from the story
//! state at load.
use crate::area::AreaSource;
use ::area::world_state::{LandingGrid, Target};
use data::{bar, journal_table, path_lookup::Library, probe_source::ProbeSource, sprite_sequence};
use std::collections::HashMap;
use visual::{
    hud::{self, SystemFont},
    menu_screen::{self, Menu2ld, Texture},
    world_map_menu::{self, MenuTexts},
};
use world_map::landing::{self, EpisodeRow};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

/// Every message of a message archive (BAR type 2) by id & 0x7fff, up to
/// its terminating zero in code position (hud::load_messages_optional's
/// rule).
fn message_table(archive: &[u8]) -> Result<HashMap<u32, Vec<u8>>> {
    let table = bar::read_bar(archive)?;
    let e = table
        .entries
        .iter()
        .find(|e| e.entry_type == 2)
        .ok_or("message table absent")?;
    let d = archive
        .get(e.offset as usize..e.offset as usize + e.size as usize)
        .ok_or("message table exceeds archive")?;
    let word = |at: usize| -> Option<u32> {
        Some(u32::from_le_bytes(d.get(at..at + 4)?.try_into().ok()?))
    };
    if word(0) != Some(1) {
        return Err("message table header".into());
    }
    let mut out = HashMap::new();
    for k in 0..word(4).ok_or("message count")? as usize {
        let (Some(id), Some(start)) = (word(8 + k * 8), word(12 + k * 8)) else {
            break;
        };
        let Some(rest) = d.get(start as usize..) else {
            continue;
        };
        let mut end = 0;
        while let Some(&b) = rest.get(end) {
            if b == 0 {
                break;
            }
            end += if b < 0x20 {
                visual::dialogue::code_length(b)
            } else {
                1
            };
        }
        out.insert(id & 0x7fff, rest[..end.min(rest.len())].to_vec());
    }
    Ok(out)
}

/// What one world's info panel and landing menu show.
pub struct Prepared {
    pub grid: LandingGrid,
    pub default_target: Target,
    pub episodes: Vec<EpisodeRow>,
    pub texts: MenuTexts,
    /// Per grid cell, its picture (an IMGD index of 70landing.bar).
    pub pictures: Vec<Option<usize>>,
    pub name: Vec<u8>,
    pub visits: Option<(usize, usize)>,
    /// The world's battle level (FUN_1403ee470).
    pub battle_level: Option<u8>,
}

/// 00battle.bin 'btlv' (FUN_1403f4c30 → FUN_1403ee490): 0x20-byte records
/// {u32 bit, u32 story flag, 19 per-world levels from +8}. FUN_1403ee4a0
/// sets a record's bit when its story flag is set; FUN_1403ee1d0 sums the
/// levels of the records whose bit is set (byte sums).
fn battle_levels(bytes: &[u8], flag_set: impl Fn(u32) -> bool) -> Option<[u8; 19]> {
    let count = u32::from_le_bytes(bytes.get(4..8)?.try_into().ok()?) as usize;
    let mut sums = [0u8; 19];
    for k in 0..count {
        let r = bytes.get(8 + k * 0x20..8 + (k + 1) * 0x20)?;
        let flag = u32::from_le_bytes(r[4..8].try_into().ok()?);
        if flag_set(flag) {
            for (w, s) in sums.iter_mut().enumerate() {
                *s = s.wrapping_add(r[8 + w]);
            }
        }
    }
    Some(sums)
}

pub struct Ui {
    pub layout: Menu2ld,
    pub font: SystemFont,
    pub icons: SystemFont,
    pub pictures: Vec<Texture>,
    pub ws_i: Option<sprite_sequence::Sequence>,
    pub worlds: HashMap<u8, Prepared>,
    /// The loaded message tables in search order, sys.bar second (the
    /// 0xadc fallback's table).
    tables: [HashMap<u32, Vec<u8>>; 3],
}

fn locate(library: &Library, name: &str) -> Result<(&'static str, usize, [u8; 16])> {
    let f = library
        .find(name)
        .ok_or_else(|| format!("{name} is not in the installed packages"))?;
    Ok((f.index, f.ordinal, f.name_hash))
}

impl Ui {
    /// FUN_1403ad9a0.
    pub fn message(&self, id: u32) -> Vec<u8> {
        self.tables
            .iter()
            .find_map(|t| t.get(&(id & 0x7fff)))
            .or_else(|| self.tables[1].get(&0xadc))
            .cloned()
            .unwrap_or_default()
    }

    pub fn load(
        source: &ProbeSource,
        library: &mut Library,
        src: &AreaSource,
        worlds: &[u8],
    ) -> Result<Self> {
        let (i, o, h) = locate(library, "file/us/wm_ws.2ld")?;
        let layout = menu_screen::load(source, i, o, h)?;
        let font = hud::load_font(
            source,
            locate(library, "msg/us/fontimage.bar")?,
            locate(library, "msg/us/fontinfo.bar")?,
            b"sys\0",
        )?;
        let icons = hud::load_font(
            source,
            locate(library, "msg/us/fontimage.bar")?,
            locate(library, "msg/us/fontinfo.bar")?,
            b"icon",
        )?;
        let sys = message_table(&library.read("msg/us/sys.bar")?)?;
        let area = message_table(&library.read("msg/us/wm.bar")?)?;
        let journal = library
            .read("msg/us/jm.bar")
            .ok()
            .and_then(|b| message_table(&b).ok())
            .unwrap_or_default();
        let (i, o, h) = locate(library, "70landing.bar")?;
        let pictures = match menu_screen::load_images(source, i, o, h) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("World-point pictures unavailable: {e}");
                Vec::new()
            }
        };
        let landing_bar = library.read("70landing.bar")?;
        let ws_i = bar::read_bar(&landing_bar)?
            .entries
            .iter()
            .find(|e| e.entry_type == 25)
            .and_then(|e| landing_bar.get(e.offset as usize..e.offset as usize + e.size as usize))
            .map(sprite_sequence::read)
            .transpose()?;
        let stories = library
            .read("menu/us/jiminy.bar")
            .ok()
            .and_then(|b| {
                let t = bar::read_bar(&b).ok()?;
                let e = t.entries.iter().find(|e| &e.name_bytes == b"stor")?;
                let s = b.get(e.offset as usize..e.offset as usize + e.size as usize)?;
                journal_table::read_stories(s).ok()
            })
            .unwrap_or_default();
        let levels = library.read("00battle.bin").ok().and_then(|b| {
            let t = bar::read_bar(&b).ok()?;
            let e = t.entries.iter().find(|e| &e.name_bytes == b"btlv")?;
            let s = b.get(e.offset as usize..e.offset as usize + e.size as usize)?;
            battle_levels(s, |f| src.progress().test(f))
        });
        let mut ui = Self {
            layout,
            font,
            icons,
            pictures,
            ws_i,
            worlds: HashMap::new(),
            tables: [area, sys, journal],
        };
        for &w in worlds {
            let mut prepared = ui.prepare(src, w, &stories);
            prepared.battle_level = levels.map(|l| l[w as usize]);
            ui.worlds.insert(w, prepared);
        }
        Ok(ui)
    }

    /// The panel's and menu's contents for `world` from the story state.
    fn prepare(&self, src: &AreaSource, world: u8, stories: &[journal_table::Story]) -> Prepared {
        let progress = src.progress();
        let tables = progress.world_tables();
        let grid = tables.map_or_else(LandingGrid::default, |t| progress.landing_grid(t, world));
        let default_target = tables.map_or(Target::NONE, |t| progress.default_landing(t, world));
        let episodes: Vec<EpisodeRow> = tables
            .map(|t| {
                progress
                    .world
                    .open_episodes(t, world)
                    .map(|e| EpisodeRow {
                        title: e.title,
                        help: e.help,
                        new: progress.world.episode_new(e),
                    })
                    .collect()
            })
            .unwrap_or_default();
        let visits = tables.and_then(|t| progress.visits(t, world));
        let name = self.message(landing::world_name(world, progress.test(0x1077)));
        // FUN_1402f23d0 then FUN_140344660: the journal's last reached
        // story step of the world.
        let journal = match world {
            1 | 6 => Some(3),
            2 => Some(0),
            4 => Some(1),
            5 => Some(2),
            7..=14 => Some(world as i8 - 3),
            16..=18 => Some(world as i8 - 4),
            _ => None,
        };
        let mut story = (0x8adc, 0x8adc);
        for s in stories {
            if Some(s.world) == journal && progress.test(s.flag as u32) {
                story = (s.summary as u32, s.objective as u32);
            }
        }
        let cell_names = grid
            .cells
            .iter()
            .map(|c| {
                c.and_then(|c| src.place_name(c.target.world, c.target.area).cloned())
                    .unwrap_or_default()
            })
            .collect();
        let pictures = grid
            .cells
            .iter()
            .map(|c| c.map(|c| c.picture as usize))
            .collect();
        let texts = MenuTexts {
            rows: [
                self.message(*landing::LAND_ROW.get(world as usize).unwrap_or(&0)),
                self.message(landing::CANCEL_ROW),
            ],
            cell_names,
            labels: world_map_menu::label_messages()
                .into_iter()
                .map(|id| self.message(id))
                .collect(),
            story: [self.message(story.0), self.message(story.1)],
            notice: self.message(landing::BLOCKED_NOTICE),
            heading: self.message(landing::EPISODE_HEADING),
            episodes: episodes
                .iter()
                .map(|e| (self.message(e.title as u32), self.message(e.help as u32)))
                .collect(),
        };
        Prepared {
            grid,
            default_target,
            episodes,
            texts,
            pictures,
            name,
            visits,
            battle_level: None,
        }
    }
}
