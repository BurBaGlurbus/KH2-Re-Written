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
use visual::gummi_menu::GummiData;
use visual::{
    hud::{self, SystemFont},
    menu_screen::{self, Menu2ld, Texture},
    world_map_menu::{self, MenuTexts},
};
use world_map::gummi::{self, Mission};
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

/// libretto-wm.bar's type-2 table {u32, i32 count, count × {u16 id, u16,
/// u32 offset}} and the scripts it points at (FUN_1403ec420).
fn read_events(library: &mut Library) -> HashMap<u16, Vec<world_map::events::Op>> {
    let mut out = HashMap::new();
    let Ok(bytes) = library.read("libretto-wm.bar") else {
        return out;
    };
    let Some(entry) = bar::read_bar(&bytes).ok().and_then(|t| {
        let e = t.entries.iter().find(|e| e.entry_type == 2)?;
        bytes.get(e.offset as usize..e.offset as usize + e.size as usize)
    }) else {
        return out;
    };
    let count = entry.get(4..8).map_or(0, |w| {
        i32::from_le_bytes(w.try_into().unwrap()).max(0) as usize
    });
    for r in 0..count {
        let Some(rec) = entry.get(8 + r * 8..16 + r * 8) else {
            break;
        };
        let id = u16::from_le_bytes([rec[0], rec[1]]);
        let off = u32::from_le_bytes(rec[4..8].try_into().unwrap()) as usize;
        if let Some(script) = entry.get(off..) {
            out.insert(id, world_map::events::read_script(script));
        }
    }
    out
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

/// What one Gummi route's info panel and menu show.
pub struct PreparedRoute {
    pub texts: MenuTexts,
    pub name: Vec<u8>,
    /// The route's normal and EX mission records, from the loaded save
    /// (all unplayed without one).
    pub missions: [[Mission; 3]; 2],
}

pub struct Ui {
    pub layout: Menu2ld,
    pub font: SystemFont,
    pub icons: SystemFont,
    pub pictures: Vec<Texture>,
    pub ws_i: Option<sprite_sequence::Sequence>,
    pub worlds: HashMap<u8, Prepared>,
    pub routes: HashMap<u8, PreparedRoute>,
    /// libretto-wm.bar (FUN_1403ec4d0: "libretto-%s.bar" for the world):
    /// the world map event scripts by event id.
    pub events: HashMap<u16, Vec<world_map::events::Op>>,
    /// The message windows' SEQD and art (fontinfo.bar, wm.bar 'md_m'),
    /// the event font and the window text colours (03system 'ftst' ids 5
    /// and 6 for world 15).
    pub window: Option<hud::Sheet>,
    pub event_font: Option<SystemFont>,
    pub colours: [u32; 2],
    /// The loaded message tables in search order, sys.bar second (the
    /// 0xadc fallback's table).
    tables: [HashMap<u32, Vec<u8>>; 3],
    /// The field command menu's layout for the world map
    /// (`field2d/<lang>/wm<n>command.2dd` 'comw', FUN_140153ee0) and its
    /// name, and the field 2D sheet with the lock-on reticle ('fiel').
    pub command: Option<(String, hud::Sheet)>,
    pub field: Option<(String, hud::Sheet)>,
    /// The Gummi menu's layout (`gumimenu/us/gumimenu.bar` `0000`) and its
    /// records, transitions and texts.
    pub gummi: Option<(Menu2ld, GummiData)>,
    /// The Gummi flight's sprites (`gumibattle/sprite/us/sprite.bar`), for
    /// the result panel.
    pub gummi_sprites: Option<Menu2ld>,
    /// The flight's system menus (`file/us/gm_ps.2ld`, menu file 5).
    pub gummi_lists: Option<Menu2ld>,
}

/// The Gummi menu's files (the menu mode's loads, FUN_140243bc0).
fn load_gummi(source: &ProbeSource, library: &mut Library) -> Result<(Menu2ld, GummiData)> {
    let (i, o, h) = locate(library, "gumimenu/us/gumimenu.bar")?;
    let layout = menu_screen::load_entry_layout(source, i, o, h, *b"0000")?;
    let outer = library.read("gumimenu/us/gumimenu.bar")?;
    let table = bar::read_bar(&outer)?;
    let transitions = table
        .entries
        .iter()
        .find(|e| &e.name_bytes == b"0004")
        .and_then(|e| outer.get(e.offset as usize..e.offset as usize + e.size as usize))
        .and_then(data::gummi_table::read_transitions)
        .ok_or("gumimenu.bar 0004 unreadable")?;
    let digits = table
        .entries
        .iter()
        .find(|e| &e.name_bytes == b"0005")
        .and_then(|e| outer.get(e.offset as usize..e.offset as usize + e.size as usize))
        .and_then(data::gummi_table::read_digit_styles)
        .ok_or("gumimenu.bar 0005 unreadable")?;
    let entries = data::gummi_table::read_menu_entries(&library.read("gumimenu/menuentry.bar")?)
        .ok_or("menuentry.bar unreadable")?;
    let texts = message_table(&library.read("msg/us/gumi.bar")?)?;
    let treasures = data::gummi_table::read_treasure(&library.read("gumibattle/treasure.bin")?)
        .ok_or("treasure.bin unreadable")?
        .iter()
        .map(|t| (t.route, t.level, t.bit))
        .collect();
    Ok((
        layout,
        GummiData {
            entries,
            transitions,
            digits,
            treasures,
            texts,
        },
    ))
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
        routes: &[u8],
        save: Option<&[u8]>,
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
        // FUN_1403ee560: the bits, not the flags: a cleared flag still counts.
        let levels = Some(src.progress().battle_sums());
        let mut ui = Self {
            layout,
            font,
            icons,
            pictures,
            ws_i,
            worlds: HashMap::new(),
            routes: HashMap::new(),
            events: read_events(library),
            window: hud::load_window_sheet(
                source,
                locate(library, "msg/us/fontinfo.bar")?,
                locate(library, "msg/us/wm.bar")?,
            )
            .map_err(|e| eprintln!("Message window art unavailable: {e}"))
            .ok(),
            event_font: hud::load_font(
                source,
                locate(library, "msg/us/fontimage.bar")?,
                locate(library, "msg/us/fontinfo.bar")?,
                b"evt\0",
            )
            .map_err(|e| eprintln!("Event font unavailable: {e}"))
            .ok(),
            colours: {
                let system = library.read("03system.bin")?;
                [
                    hud::ftst_colour(&system, visual::dialogue::WINDOW_COLOUR_ID, 15)
                        .unwrap_or(0x80f0f0f0),
                    hud::ftst_colour(&system, 6, 15).unwrap_or(0x80f0f0f0),
                ]
            },
            tables: [area, sys, journal],
            command: None,
            field: None,
            gummi: load_gummi(source, library)
                .map_err(|e| eprintln!("Gummi menu unavailable: {e}"))
                .ok(),
            gummi_lists: locate(library, "file/us/gm_ps.2ld")
                .and_then(|(i, o, h)| menu_screen::load(source, i, o, h))
                .map_err(|e| eprintln!("Gummi menus unavailable: {e}"))
                .ok(),
            gummi_sprites: locate(library, "gumibattle/sprite/us/sprite.bar")
                .and_then(|(i, o, h)| menu_screen::load(source, i, o, h))
                .map_err(|e| eprintln!("Gummi sprites unavailable: {e}"))
                .ok(),
        };
        // The world map is area 0 of world 15 (FUN_140154e20).
        let name = src.command_layout(15, 0, "us", false);
        ui.command = locate(library, &name)
            .and_then(|(i, o, h)| hud::load(source, i, o, h, b"comw"))
            .map_err(|e| eprintln!("{name}: command layout unavailable: {e}"))
            .ok()
            .map(|sheet| (name, sheet));
        ui.field = ["field2d/us/wm0field.2dd", "field2d/us/zz0field.2dd"]
            .into_iter()
            .find_map(|name| {
                let (i, o, h) = locate(library, name).ok()?;
                let sheet = hud::load(source, i, o, h, b"fiel").ok()?;
                Some((name.to_owned(), sheet))
            });
        for &r in routes {
            let texts = MenuTexts {
                rows: [
                    ui.message(gummi::route_row(r)),
                    ui.message(landing::CANCEL_ROW),
                ],
                ..MenuTexts::default()
            };
            let name = ui.message(gummi::route_name(r));
            ui.routes.insert(
                r,
                PreparedRoute {
                    texts,
                    name,
                    missions: save.map_or([[Mission::default(); 3]; 2], |s| {
                        gummi::route_missions(s, r)
                    }),
                },
            );
        }
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
