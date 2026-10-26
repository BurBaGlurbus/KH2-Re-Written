//! The camp (pause) menu and Jiminy's Journal for the world map program
//! (`kh2fm wm`). START opens the camp there as in the field (FUN_1401572b0
//! has no world map test); the camp task's world map branch
//! (FUN_14031d3c0 with the world map's heap loaded) loads the journal's
//! files itself (FUN_1402f5a40) and opens the journal as kind 0
//! (FUN_1402e4930(0)), so it starts on the world select page
//! (FUN_1402f3d70). The landing grid's Triangle opens that page directly
//! (FUN_14038e050 → FUN_1402e4bc0(0, …) → FUN_1402e4950(4, 0)).
//!
//! The camp's sources come from the field configuration the launcher
//! passes the world map (`--field`); the camp keeps the field's starting
//! state [test-room values: no save is decoded].
use super::*;

/// The journal kind the world map opens (FUN_1402e4930(0)) and the world
/// the map is (DAT_140717008 = 15).
const WORLD_MAP_KIND: u8 = 0;
const WORLD_MAP_WORLD: i32 = 15;

pub(crate) struct WorldMapCamp {
    c: CampRes,
    settings: settings_room::SettingsRes,
    /// The camp's pages count frames as the room does.
    tick: u64,
    /// Sora's HP and MP at the start (00battle 'plrp', as the field's HUD).
    stats: (i32, i32),
    /// This tick's records for the program's log.
    pub log: Vec<serde_json::Value>,
}

/// Loads the camp and journal sources named by the field configuration at
/// `config_path` (its `camp`, `menu`, `journal`, `system` and `battle`).
/// `unattended` runs keep the default settings and save none.
pub(crate) fn load(
    source: Arc<ProbeSource>,
    config_path: &Path,
    unattended: bool,
) -> Result<WorldMapCamp, Box<dyn std::error::Error>> {
    let config: Config = serde_json::from_slice(&read_limited(config_path)?)?;
    if config.schema_version != 1 || config.source_content_identity != source.content_identity() {
        return Err("field configuration identity/schema invalid".into());
    }
    let (Some(l), Some(m)) = (&config.camp, &config.menu) else {
        return Err("the field configuration has no camp".into());
    };
    let system = read_archive(&source, &config.system)?;
    let battle = read_archive(&source, &config.battle)?;
    let camp = load_camp_source(&source, l, m, &battle, &system)?;
    let journal = match &config.journal {
        Some(j) => Some(load_journal(&source, j, &m.messages, &system)?),
        None => None,
    };
    // The room's equipment, abilities and AP (as the field starts them).
    let item = bar_entry(&system, b"item")?;
    let table = data::item_table::read(item)?;
    let owned = gear::owned(&table);
    let inventory = ability_inventory::load(&table, &battle)?;
    let stats = hud::starting_stats(bar_entry(&battle, b"plrp")?)?;
    let mut camp = camp;
    camp.numbers.ap = camp.numbers.ap.max(inventory.ap);
    let settings = settings_room::SettingsRes::new(
        source.clone(),
        (!unattended).then(|| {
            config_path
                .parent()
                .unwrap_or(Path::new("."))
                .join("kh2fm-settings.json")
        }),
        None,
    )?;
    let lang = settings.loaded_or("us");
    Ok(WorldMapCamp {
        c: CampRes {
            source: camp,
            open: None,
            memory: pause_menu::Memory::default(),
            config_flags: 0x40b,
            game_mode: 1,
            pad: Pad::default(),
            art: [None; 4],
            font_sheet: 0,
            icon_sheet: 0,
            glyph: 0,
            keyboard_last: true,
            stock: gear::with_owned_equipment(room_stock(), &owned),
            starting: gear::with_owned_equipment(room_stock(), &owned)
                .into_keys()
                .collect(),
            seen: Vec::new(),
            shortcuts: ROOM_SHORTCUTS,
            no_charge: 0,
            equipment: room_equipment(),
            logged_equipment: None,
            ability_slots: inventory.slots.clone(),
            journal_source: journal,
            journal: None,
            curtain: None,
            journal_sheets: JournalSheets::default(),
            picture_source: Some(source),
            seen_chars: Vec::new(),
            seen_albums: Vec::new(),
            seen_limits: Vec::new(),
            seen_quests: Vec::new(),
            seen_minigames: Vec::new(),
            seen_story: Vec::new(),
            seen_synthesis: Vec::new(),
            picture_sheets: HashMap::new(),
            notice_sheets: HashMap::new(),
            lang,
            // The world map has no save point menu here.
            files: None,
            steam_root: std::path::PathBuf::new(),
        },
        settings,
        tick: 0,
        stats,
        log: Vec::new(),
    })
}

impl WorldMapCamp {
    /// The camp's and the journal's images as 2D sheets.
    pub fn upload(&mut self, frame: &mut Ui2dFrame, images: &mut Assets<Image>) {
        upload_camp_sheets(&mut self.c, frame, images);
    }

    /// A menu (camp, curtain or journal) holds the screen and the pad.
    pub fn menu_up(&self) -> bool {
        self.c.menu_up()
    }

    /// The camp's state when it opens: Sora at his starting HP and MP with
    /// the room's numbers [test-room values]; the play time is the save's
    /// (FUN_140157390).
    fn data(&self, play_time: u32) -> pause_menu::Data {
        let c = &self.c;
        let (hp, mp) = self.stats;
        let panels = pause_menu::Panels {
            munny: 0,
            time_frames: play_time,
            members: vec![pause_menu::Member {
                name: c.source.name.clone(),
                level: 1,
                hp,
                max_hp: hp,
                mp,
                max_mp: mp,
            }],
            labels: c.source.labels.clone(),
        };
        let side = CampSide {
            gear: player::equipment_stats::Totals::start(0, 0, 0, 0),
            form_slots: Vec::new(),
            form_abilities: Vec::new(),
            form_weapon: None,
            hp,
            max_hp: hp,
            mp,
            max_mp: mp,
            party: Vec::new(),
        };
        camp_data(c, panels, None, &self.settings, &side)
    }

    /// FUN_14038e050 → FUN_1402e4bc0(0, journal index): the journal's page
    /// opens straight over the landing menu, as kind 0 on the world select
    /// page. Returns its sounds.
    pub fn open_journal(&mut self) -> Vec<u32> {
        let Some(j) = &self.c.journal_source else {
            return Vec::new();
        };
        let data = journal_data(&self.c, j, WORLD_MAP_KIND, WORLD_MAP_WORLD, false);
        let (opened, sounds) = Journal::open(&j.book.layout, data);
        self.log
            .push(serde_json::json!({"event":"journal_open","tick":self.tick,
            "kind":WORLD_MAP_KIND,"state":opened.state,"from":"landing"}));
        self.c.journal = Some(opened);
        // The landing's Triangle is still down: the journal starts from it
        // held, not pressed.
        self.c.pad.update_raw(0, controller::MODE_TITLE, 1.);
        sounds
    }

    /// One frame of the camp chain: `raw` PADDATA bits, the left stick, the
    /// input devices last used (keyboard, pad glyph) and whether START may
    /// open the camp now. Returns the se000 sounds.
    #[allow(clippy::too_many_arguments)]
    pub fn tick(
        &mut self,
        raw: u64,
        stick: [f32; 3],
        devices: (bool, Option<u8>),
        can_open: bool,
        play_time: u32,
        frame: &mut Ui2dFrame,
        images: &mut Assets<Image>,
    ) -> Vec<u32> {
        let (keyboard, glyph) = devices;
        if keyboard {
            self.c.keyboard_last = true;
        } else if glyph.is_some() {
            self.c.keyboard_last = false;
        }
        let g = if self.c.keyboard_last {
            0
        } else {
            glyph.map_or(self.c.glyph, usize::from)
        };
        if self.c.art[g].is_some() {
            self.c.glyph = g;
        }
        let mode = if self.c.menu_up() {
            controller::MODE_TITLE
        } else {
            controller::MODE_FIELD
        };
        self.c.pad.update_raw(raw, mode, 1.);
        let tick = self.tick;
        let mut sounds = Vec::new();
        let data = self.data(play_time);
        let c = &mut self.c;
        let layout = &c.source.menu.layout;
        if let Some(menu) = c.open.as_mut() {
            menu.pictures.loaded = c
                .picture_sheets
                .iter()
                .filter(|(_, s)| s.is_some())
                .map(|(&id, _)| id)
                .collect();
            sounds = menu.frame(layout, c.pad.pressed, c.pad.repeat, play_time);
            self.tick += 1;
            if let Some(s) = &menu.data.settings {
                self.settings.current.clone_from(s);
            }
            if menu.journal.is_some()
                && c.curtain.is_none()
                && let Some(j) = &c.journal_source
            {
                c.curtain = Some(Curtain::cover(&j.curtain.layout));
            }
            if menu.journal.is_some()
                && !menu.closed
                && c.curtain.as_ref().is_some_and(Curtain::covered)
            {
                menu.kill_all();
            }
            if menu.closed {
                self.settings.save_if_changed();
                c.memory = menu.memory;
                c.config_flags = menu.data.config_flags;
                c.stock = menu.data.items.counts.clone();
                c.seen = menu.data.items.seen.clone();
                c.shortcuts = menu.data.customize.shortcuts;
                if let Some(e) = menu.data.items.equipment.first() {
                    c.equipment = e.clone();
                }
                c.no_charge = menu.data.customize.no_charge;
                if let Some(m) = menu.data.abilities.members.first() {
                    c.ability_slots = m.slots.clone();
                }
                // The camp's journal on the world map is kind 0
                // (FUN_14031d3c0's world map branch).
                let kind = menu.journal.map(|_| WORLD_MAP_KIND);
                c.open = None;
                self.log
                    .push(serde_json::json!({"event":"camp_close","tick":tick}));
                if let (Some(kind), Some(j)) = (kind, &c.journal_source) {
                    let data = journal_data(c, j, kind, WORLD_MAP_WORLD, false);
                    let (opened, s) = Journal::open(&j.book.layout, data);
                    self.log
                        .push(serde_json::json!({"event":"journal_open","tick":tick,
                        "kind":kind,"state":opened.state,"from":"camp"}));
                    c.journal = Some(opened);
                    sounds.extend(s);
                }
            }
        } else if let Some(open) = c.journal.as_mut() {
            let j = c.journal_source.as_ref().expect("journal source");
            sounds = open.frame(
                &j.book.layout,
                c.pad.pressed,
                c.pad.repeat,
                c.pad.held,
                stick,
            );
            self.tick += 1;
            if open.closed {
                (c.seen_chars, c.seen_albums) = open.seen();
                c.seen_limits = open.seen_limits();
                (c.seen_quests, c.seen_minigames) = open.seen_lists();
                c.seen_story = open.seen_story();
                c.seen_synthesis = open.seen_synthesis();
                c.journal = None;
                self.log
                    .push(serde_json::json!({"event":"journal_close","tick":tick}));
                // From the camp the camp comes back as the curtain lifts
                // (FUN_140304ea0, FUN_14031a690); from the landing grid the
                // menu is there again.
                if let Some(cur) = c.curtain.as_mut() {
                    let (menu, s) = Camp::open(layout, data, c.memory);
                    c.open = Some(menu);
                    sounds.extend(s);
                    cur.uncover(&j.curtain.layout);
                    self.log
                        .push(serde_json::json!({"event":"camp_open","tick":tick}));
                }
            }
        } else if can_open && c.curtain.is_none() && c.pad.pressed & 0x2 != 0 {
            // START (logical 0x2): FUN_1401572b0 → FUN_1402e4950(0).
            let (menu, s) = Camp::open(layout, data, c.memory);
            c.open = Some(menu);
            sounds = s;
            self.tick += 1;
            self.log
                .push(serde_json::json!({"event":"camp_open","tick":tick}));
        }
        load_wanted_picture(c, frame, images);
        load_wanted_notice(c, frame, images);
        load_wanted_journal(c, frame, images);
        if let (Some(cur), Some(j)) = (c.curtain.as_mut(), c.journal_source.as_ref()) {
            cur.update(&j.curtain.layout);
            if cur.dead {
                c.curtain = None;
            }
        }
        sounds
    }

    /// This frame's camp, curtain and journal draws (no member model is
    /// rendered on the world map).
    pub fn draw(&mut self, frame: &mut Ui2dFrame) {
        let c = &mut self.c;
        let font = field_command_menu::Font {
            widths: &c.source.font.widths,
            pad: c.glyph as u8,
        };
        if let Some(menu) = c.open.as_mut() {
            let Some(art) = c.art[c.glyph].or(c.art[0]) else {
                return;
            };
            for (sheet, quads) in menu.draw(&c.source.menu.layout, &font) {
                let slot = match sheet {
                    pause_menu::Sheet::Art => art,
                    pause_menu::Sheet::Font => c.font_sheet,
                    pause_menu::Sheet::Icon => c.icon_sheet,
                    pause_menu::Sheet::Model(_) => continue,
                    pause_menu::Sheet::Picture(id) => match c.picture_sheets.get(&id) {
                        Some(&Some(s)) => s,
                        _ => continue,
                    },
                    pause_menu::Sheet::Notice(name) => match c.notice_sheets.get(&name) {
                        Some(&s) => s,
                        None => continue,
                    },
                };
                frame.draws.push((slot, quads));
            }
        }
        let Some(j) = &c.journal_source else { return };
        let s = &c.journal_sheets;
        if let Some(cur) = &c.curtain {
            frame.draws.push((s.curtain, cur.quads(&j.curtain.layout)));
        }
        if let Some(open) = &c.journal {
            let top = open.top.as_ref().and_then(|code| s.tops.get(code));
            for (sheet, quads) in open.draw(&j.book.layout, &font) {
                let slot = match sheet {
                    journal::Sheet::Art => Some(s.book),
                    journal::Sheet::Font => Some(c.font_sheet),
                    journal::Sheet::Icon => Some(c.icon_sheet),
                    journal::Sheet::World(i) => s.worlds.get(i).copied(),
                    journal::Sheet::Top(i) => top.and_then(|t| t.get(i).copied()),
                    journal::Sheet::Face(k, i) => {
                        s.faces.get(&k).and_then(|v| v.get(i as usize).copied())
                    }
                    journal::Sheet::Photo(r) => s.photos.get(&r).copied(),
                };
                if let Some(slot) = slot {
                    frame.draws.push((slot, quads));
                }
            }
        }
    }
}
