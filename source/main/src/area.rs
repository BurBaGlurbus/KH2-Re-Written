//! Field areas for the test room: an area's map, ARD scripts, story
//! programs and spawn points loaded by name from the selected install, as
//! the exe's area loader does (FUN_140152680 → FUN_14039c860 →
//! FUN_1403a2170; map names FUN_140161c50, ARD FUN_1403a2060).
use crate::npcs::{FieldNpc, Lcg, NpcKind};
use ::area::{
    Area, PlayerProbe, Request, Spawn,
    progress::{self, Progress},
    script::{self, Location, ScriptRandom},
};
use bevy::prelude::Vec3;
use collision::{Body, Query};
use data::{
    ard::{self, tag},
    bar, objentry,
    path_lookup::Library,
    probe_source::ProbeSource,
    room_collision,
};
use std::{collections::HashMap, sync::Arc};
use visual::{
    dialogue::{self, Op},
    hud,
    room::{LoadedRoom, load_room_archive},
};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

/// Sora's map body: P_EX100.mdlx BAR type 23 entry 0 (kind 4, r 90, h 90,
/// ph 115), collision class 0 (FUN_1403d5840) [collide-01 §2].
pub const SORA_BODY: Body = Body {
    radius: 90.,
    height: 90.,
    offset: 115.,
    mode: 1,
    class: 0,
};

/// Debug-menu names of the worlds in `progress::WORLDS` order (rewrite
/// labels, not game text); the code where none is known.
pub fn world_name(world: u8) -> &'static str {
    const NAMES: [&str; 19] = [
        "zz",
        "es",
        "Twilight Town",
        "Destiny Islands",
        "Hollow Bastion",
        "Beast's Castle",
        "Olympus Coliseum",
        "Agrabah",
        "The Land of Dragons",
        "100 Acre Wood",
        "Pride Lands",
        "Atlantica",
        "Disney Castle",
        "Timeless River",
        "Halloween Town",
        "World Map",
        "Port Royal",
        "Space Paranoids",
        "The World That Never Was",
    ];
    NAMES.get(world as usize).copied().unwrap_or("?")
}

/// The installed files an area needs, opened once per session.
pub struct AreaSource {
    source: Arc<ProbeSource>,
    library: Library,
    progress: Progress,
    /// 00objentry records and their index by object id.
    objects: Vec<objentry::ObjectEntry>,
    object_index: HashMap<u32, usize>,
    /// Loaded townsperson models (kept across area changes) by objentry id.
    pub kinds: Vec<NpcKind>,
    kind_index: HashMap<u32, usize>,
    /// Index of `kinds[0]` (nonzero in a fork, whose new kinds follow the
    /// ones its parent already holds).
    kind_base: usize,
    rng: ScriptRandom,
    /// Place names by (world, area) (msg/us/place.bin → sys.bar) and the
    /// "???" shown for places not visited yet (sys 0x82b8).
    place_names: HashMap<(u8, u8), Vec<u8>>,
    /// Per world (index as in `progress::WORLDS`), the areas whose map is
    /// installed.
    rooms: Vec<Vec<u8>>,
    /// 03system 'arif' (per-room table) and the BGM pair last started.
    arif: Vec<u8>,
    last_pair: Option<[u16; 2]>,
    pub unknown_place: Vec<u8>,
    /// Disable-reason texts by 'dsa' message word.
    reasons: HashMap<u16, Vec<u8>>,
    /// The ARD language directory (`*0x1408ec5d0`; "us" in this build).
    language: &'static str,
}

/// One loaded area.
pub struct LoadedArea {
    pub world: u8,
    pub area: u8,
    pub programs: [i16; 3],
    /// Taken by the field when the area becomes current.
    collision: Option<collision::World>,
    pub state: Area,
    /// Entities the scripts created.
    #[allow(dead_code)]
    pub spawns: Vec<Spawn>,
    /// Music changes for this entry.
    pub music: MusicChange,
    /// Map objects (lamps, signs, clock) and their placements.
    pub bobs: visual::scenery::LoadedBobs,
    /// The townspeople of this area (objentry type 15, spawn types 0/1/3,
    /// then zone-spawned ones as Sora enters their zones).
    pub npcs: Vec<FieldNpc>,
    /// Spawn-type-2 townspeople waiting for their zone.
    pub deferred: Vec<Deferred>,
    /// Libretto scripts by talk id and their message texts.
    pub scripts: HashMap<u16, Vec<Op>>,
    pub lines: HashMap<u32, Vec<u8>>,
    /// Player spawn in scene coordinates (Y up) and scene yaw.
    pub player: (Vec3, f32),
    /// The map's own effects (its first BAR entry of type 18, FUN_1401613c0).
    pub pax: Option<Arc<data::object_effect::Pax>>,
}

/// What the field and battle music slots must do on entering an area
/// (FUN_140155840): `None` keeps the slot, `Some(None)` silences it,
/// `Some(Some(bytes))` starts the new track.
#[derive(Default)]
pub struct MusicChange {
    pub slots: [Option<Option<Vec<u8>>>; 2],
}

/// A zone-spawned townsperson (spawn type 2).
pub struct Deferred {
    pub spawn: Spawn,
    pub kind: usize,
    /// The NPC slot once spawned.
    pub npc: Option<usize>,
    /// Ticks since Sora left every zone of its point.
    pub away: u32,
}

impl AreaSource {
    /// Opens the indexes and sets story flags `first..=last` of the world in
    /// ascending order (FUN_14039afc0 each), which fixes every area's
    /// programs for that point of the story.
    pub fn open(source: Arc<ProbeSource>, story: &[(u32, u32)]) -> Result<Self> {
        let mut library = Library::open(&source)?;
        let progress_bytes = library.read("00progress.bin")?;
        let table = bar::read_bar(&progress_bytes)?;
        let scripts = progress::WORLDS
            .iter()
            .map(|code| {
                let mut name = [0u8; 4];
                name[..2].copy_from_slice(code.as_bytes());
                table
                    .entries
                    .iter()
                    .find(|e| e.name_bytes == name)
                    .and_then(|e| {
                        progress_bytes
                            .get(e.offset as usize..e.offset as usize + e.size as usize)
                            .map(<[u8]>::to_vec)
                    })
            })
            .collect();
        let mut progress = Progress::new(scripts);
        if let Some(e) = table.entries.iter().find(|e| e.name_bytes[..3] == *b"dsa")
            && let Some(b) =
                progress_bytes.get(e.offset as usize..e.offset as usize + e.size as usize)
        {
            progress.set_dsa(b);
        }
        for &(first, last) in story {
            for flag in first..=last {
                if !progress.test(flag) {
                    progress.set(flag);
                }
            }
        }
        // Every area is reachable: no story-blocked destination.
        progress.unblock_all();
        let objects = objentry::read(&library.read("00objentry.bin")?)?;
        let object_index = objects
            .iter()
            .enumerate()
            .map(|(i, o)| (o.object_id, i))
            .collect();
        // place.bin: per world (type-2 entry named by the world code) a
        // table of {u16 message, u16 offset}, one record per area; the text
        // is a sys.bar message.
        let mut place_names = HashMap::new();
        let mut unknown_place = Vec::new();
        if let Ok(place) = library.read("msg/us/place.bin")
            && let Ok(table) = bar::read_bar(&place)
            && let Some(sys) = library.find("msg/us/sys.bar")
        {
            let mut keys = Vec::new();
            let mut ids = Vec::new();
            for (world, code) in progress::WORLDS.iter().enumerate() {
                let Some(rows) = table
                    .entries
                    .iter()
                    .find(|e| e.entry_type == 2 && e.name_bytes[..2] == *code.as_bytes())
                    .and_then(|e| {
                        place.get(e.offset as usize..e.offset as usize + e.size as usize)
                    })
                else {
                    continue;
                };
                for (area, r) in rows.as_chunks::<4>().0.iter().enumerate() {
                    keys.push((world as u8, area as u8));
                    ids.push(u16::from_le_bytes([r[0], r[1]]) as u32);
                }
            }
            ids.push(0x82b8);
            if let Ok(texts) =
                hud::load_messages_optional(&source, sys.index, sys.ordinal, sys.name_hash, &ids)
            {
                for (&key, t) in keys.iter().zip(&texts) {
                    if let Some(t) = t {
                        place_names.insert(key, t.clone());
                    }
                }
                unknown_place = texts.last().cloned().flatten().unwrap_or_default();
            }
        }
        let rooms = progress::WORLDS
            .iter()
            .map(|code| {
                (0..progress::AREAS_PER_WORLD as u8)
                    .filter(|n| library.find(&format!("map/{code}{n:02}.map")).is_some())
                    .collect()
            })
            .collect();
        let arif = library
            .read("03system.bin")
            .ok()
            .and_then(|sys| {
                let table = bar::read_bar(&sys).ok()?;
                let e = table.entries.iter().find(|e| e.name_bytes == *b"arif")?;
                sys.get(e.offset as usize..e.offset as usize + e.size as usize)
                    .map(<[u8]>::to_vec)
            })
            .unwrap_or_default();
        Ok(Self {
            source,
            library,
            progress,
            arif,
            last_pair: None,
            place_names,
            rooms,
            unknown_place,
            reasons: HashMap::new(),
            objects,
            object_index,
            kinds: Vec::new(),
            kind_index: HashMap::new(),
            kind_base: 0,
            rng: ScriptRandom::default(),
            language: "us",
        })
    }

    /// A copy that loads the next area off the main thread: its own open
    /// indexes and the same story, random and music state. `adopt` takes
    /// the copy's state back once its load is done.
    pub fn fork(&self) -> Result<Self> {
        Ok(Self {
            source: Arc::clone(&self.source),
            library: Library::open(&self.source)?,
            progress: self.progress.clone(),
            objects: self.objects.clone(),
            object_index: self.object_index.clone(),
            kinds: Vec::new(),
            kind_index: self.kind_index.clone(),
            kind_base: self.kind_base + self.kinds.len(),
            rng: self.rng,
            place_names: HashMap::new(),
            rooms: Vec::new(),
            arif: self.arif.clone(),
            last_pair: self.last_pair,
            unknown_place: Vec::new(),
            reasons: HashMap::new(),
            language: self.language,
        })
    }

    /// Takes back a fork's state after it loaded an area: the townsperson
    /// models it added, the script random state and the music pair.
    pub fn adopt(&mut self, fork: Self) {
        debug_assert_eq!(fork.kind_base, self.kind_base + self.kinds.len());
        self.kinds.extend(fork.kinds);
        self.kind_index = fork.kind_index;
        self.rng = fork.rng;
        self.last_pair = fork.last_pair;
    }

    /// FUN_140152be0 (programs from the save table) then the area loader.
    /// `request` overrides the save table's programs where it is not −1
    /// (FUN_140152be0).
    pub fn load(
        &mut self,
        world: u8,
        area: u8,
        entrance: u8,
        request: [i16; 3],
    ) -> Result<(LoadedRoom, LoadedArea)> {
        let code = progress::WORLDS
            .get(world as usize)
            .ok_or("world index outside the world table")?;
        let name = format!("{code}{area:02}");
        let map = self.library.read(&format!("map/{name}.map"))?;
        let ard_bytes = self
            .library
            .read(&format!("ard/{}/{name}.ard", self.language))?;
        let ard_bar = bar::read_bar(&ard_bytes)?;
        let entry = |kind: u16, name: u32| -> Option<&[u8]> {
            ard_bar
                .entries
                .iter()
                .find(|e| e.entry_type == kind && u32::from_le_bytes(e.name_bytes) == name)
                .and_then(|e| ard_bytes.get(e.offset as usize..e.offset as usize + e.size as usize))
        };
        let mut programs = self.progress.programs(world, area);
        for (p, r) in programs.iter_mut().zip(request) {
            if r != -1 {
                *p = r;
            }
        }
        let location = Location {
            world,
            area,
            entrance,
            programs,
        };
        let scripts = ["map", "btl", "evt"].map(|n| entry(13, tag(n)));
        let types: HashMap<u32, u8> = self
            .objects
            .iter()
            .map(|o| (o.object_id, o.object_type))
            .collect();
        // The story point's map program; when it places no player at this
        // entrance (an area its story state has not opened) the area's other
        // map programs are tried in file order so every area can be entered.
        let mut candidates = vec![location.programs[0]];
        if let Some(group) = scripts[0]
            && let Ok(ids) = ard::program_ids(group)
        {
            candidates.extend(ids.into_iter().filter(|&i| i != location.programs[0]));
        }
        let mut chosen = None;
        for (n, &candidate) in candidates.iter().enumerate() {
            let mut location = Location {
                world,
                area,
                entrance,
                programs: location.programs,
            };
            location.programs[0] = candidate;
            let mut game = Flags(&self.progress);
            let Ok(setup) = script::run(&location, scripts, &mut self.rng, &mut game) else {
                continue;
            };
            let mut state = Area::new(world, area, entrance, setup, |n| {
                entry(12, n).and_then(|d| ard::spawn_set(d).ok())
            });
            let spawns = state.activate(|id| types.get(&id).copied()).to_vec();
            if Area::player_spawn(&spawns).is_some() || n + 1 == candidates.len() {
                chosen = Some((state, spawns, location));
                break;
            }
        }
        let (state, spawns, location) = chosen.ok_or("area scripts did not run")?;
        let programs = location.programs;
        let (npcs, deferred, scripts, lines) =
            self.townspeople(code, area, &state, &spawns, &types);
        let music = self.music_change(world, area, state.setup.bgm);
        let room = load_room_archive(&map, None)?;
        let bobs = visual::scenery::load(&map).unwrap_or_else(|e| {
            eprintln!("{name}: map objects not loaded: {e}");
            visual::scenery::LoadedBobs {
                models: Vec::new(),
                placements: Vec::new(),
            }
        });
        let collision = map_collision(&map, state.setup.bg_mask)?;
        let pax = map_pax(&map).map(Arc::new);
        let player = Area::player_spawn(&spawns)
            .map(|s| {
                let [x, y, z] = s.entity.position;
                // Engine yaw Y faces (sin Y, 0, cos Y); in the scene (x, −y,
                // −z) that is (sin Y, −cos Y) = (sin φ, cos φ), φ = π − Y.
                (Vec3::new(x, -y, -z), std::f32::consts::PI - s.yaw)
            })
            .ok_or_else(|| format!("{name} has no player spawn for entrance {entrance}"))?;
        Ok((
            room,
            LoadedArea {
                world,
                area,
                programs,
                collision: Some(collision),
                state,
                spawns,
                music,
                bobs,
                npcs,
                deferred,
                scripts,
                lines,
                player,
                pax,
            },
        ))
    }

    /// The BGM pair of a room: the ARD override, else the arif row's pair
    /// for the world's set index (FUN_1403a5960; bgm-input §A4).
    fn bgm_pair(&self, world: u8, area: u8, ard: Option<[u16; 2]>) -> [u16; 2] {
        if let Some(p) = ard {
            return p;
        }
        let word = |o: usize| {
            self.arif
                .get(o..o + 2)
                .map(|b| u16::from_le_bytes([b[0], b[1]]))
        };
        let (count, off) = (word(8 + world as usize * 4), word(10 + world as usize * 4));
        let (Some(count), Some(off)) = (count, off) else {
            return [0, 0];
        };
        if area as u16 >= count {
            return [0, 0];
        }
        let set = self.progress.arif_set[world as usize].max(0) as usize;
        let at = off as usize + area as usize * 0x40 + 0x10 + set.min(7) * 4;
        match (word(at), word(at + 2)) {
            (Some(f), Some(b)) => [f, b],
            _ => [0, 0],
        }
    }

    fn music_change(&mut self, world: u8, area: u8, ard: Option<[u16; 2]>) -> MusicChange {
        let pair = self.bgm_pair(world, area, ard);
        let mut change = MusicChange::default();
        for k in 0..2 {
            let before = self.last_pair.map(|p| p[k]);
            if before == Some(pair[k]) {
                continue;
            }
            change.slots[k] = Some(if pair[k] == 0 {
                None
            } else {
                self.library
                    .read(&format!("bgm/music{:03}.win32.scd", pair[k]))
                    .ok()
            });
        }
        self.last_pair = Some(pair);
        change
    }

    /// The place name of an area (place.bin of its world).
    pub fn place_name(&self, world: u8, area: u8) -> Option<&Vec<u8>> {
        self.place_names.get(&(world, area))
    }

    /// The installed areas of a world, in area order.
    pub fn rooms(&self, world: u8) -> &[u8] {
        self.rooms.get(world as usize).map_or(&[], Vec::as_slice)
    }

    /// FUN_1403f0560: destinations currently story-blocked ('dsa').
    pub fn blocked_destinations(&self) -> Vec<(u8, u8)> {
        self.progress.blocked_destinations()
    }

    /// FUN_1403f0470 → FUN_1403ad9a0: the disable-reason message of a
    /// blocked destination; a word with no message falls back to sys 0xadc.
    pub fn blocked_text(&mut self, world: u8, area: u8) -> Option<Vec<u8>> {
        let word = self.progress.blocked(world, area)?;
        if let Some(t) = self.reasons.get(&word) {
            return Some(t.clone());
        }
        let sys = self.library.find("msg/us/sys.bar")?;
        let texts = hud::load_messages_optional(
            &self.source,
            sys.index,
            sys.ordinal,
            sys.name_hash,
            &[word as u32, 0xadc],
        )
        .ok()?;
        let text = texts.into_iter().flatten().next()?;
        self.reasons.insert(word, text.clone());
        Some(text)
    }

    /// The (index, ordinal, hash) of an installed file for the HUD loaders.
    pub fn locate(&self, name: &str) -> Option<(String, usize, [u8; 16])> {
        self.library
            .find(name)
            .map(|f| (f.index.to_owned(), f.ordinal, f.name_hash))
    }

    #[allow(clippy::type_complexity)]
    fn townspeople(
        &mut self,
        code: &str,
        area: u8,
        state: &Area,
        spawns: &[Spawn],
        types: &HashMap<u32, u8>,
    ) -> (
        Vec<FieldNpc>,
        Vec<Deferred>,
        HashMap<u16, Vec<Op>>,
        HashMap<u32, Vec<u8>>,
    ) {
        let mut random = Lcg(0x4b1d_0000 ^ (area as u32) << 8 ^ 0x2545);
        let mut npcs = Vec::new();
        for (i, s) in spawns.iter().enumerate() {
            let Some(&oi) = self.object_index.get(&s.entity.object) else {
                continue;
            };
            if self.objects[oi].object_type != 15 {
                continue;
            }
            let kind = match self.kind_index.get(&s.entity.object) {
                Some(&k) => k,
                None => match NpcKind::load(&self.source, &mut self.library, &self.objects[oi]) {
                    Ok(kind) => {
                        self.kinds.push(kind);
                        let k = self.kind_base + self.kinds.len() - 1;
                        self.kind_index.insert(s.entity.object, k);
                        k
                    }
                    Err(e) => {
                        eprintln!("NPC {} not loaded: {e}", s.entity.object);
                        continue;
                    }
                },
            };
            npcs.push(build_npc(state, kind, s, i, &mut random));
        }
        let mut deferred = Vec::new();
        for s in state.zone_spawns(|id| types.get(&id).copied()) {
            let Some(&oi) = self.object_index.get(&s.entity.object) else {
                continue;
            };
            if self.objects[oi].object_type != 15 {
                continue;
            }
            let kind = match self.kind_index.get(&s.entity.object) {
                Some(&k) => k,
                None => match NpcKind::load(&self.source, &mut self.library, &self.objects[oi]) {
                    Ok(kind) => {
                        self.kinds.push(kind);
                        let k = self.kind_base + self.kinds.len() - 1;
                        self.kind_index.insert(s.entity.object, k);
                        k
                    }
                    Err(e) => {
                        eprintln!("NPC {} not loaded: {e}", s.entity.object);
                        continue;
                    }
                },
            };
            deferred.push(Deferred {
                spawn: s,
                kind,
                npc: None,
                away: 0,
            });
        }
        // Talk scripts: libretto-<world>.bar type-2 entry (kh2_first #474 for
        // tt): {u32 3, i32 count, count × {u16 talk, u16 kind, u32 offset}}.
        let mut scripts = HashMap::new();
        if let Ok(bytes) = self.library.read(&format!("libretto-{code}.bar"))
            && let Ok(table) = bar::read_bar(&bytes)
            && let Some(e) = table.entries.iter().find(|e| e.entry_type == 2)
            && let Some(entry) = bytes.get(e.offset as usize..e.offset as usize + e.size as usize)
            && entry.len() >= 8
        {
            let count = i32::from_le_bytes(entry[4..8].try_into().unwrap()).max(0) as usize;
            for r in 0..count {
                let Some(rec) = entry.get(8 + r * 8..16 + r * 8) else {
                    break;
                };
                let id = u16::from_le_bytes([rec[0], rec[1]]);
                let off = u32::from_le_bytes(rec[4..8].try_into().unwrap()) as usize;
                let wanted = npcs.iter().any(|n| n.talk == id)
                    || deferred.iter().any(|d| d.spawn.entity.talk == id);
                if wanted && let Some(script) = entry.get(off..) {
                    scripts.insert(id, dialogue::read_script(script));
                }
            }
        }
        let mut ids: Vec<u32> = scripts
            .values()
            .flatten()
            .filter_map(|op| match op {
                Op::Message(id) => Some(*id),
                _ => None,
            })
            .collect();
        ids.sort_unstable();
        ids.dedup();
        let mut lines = HashMap::new();
        if let (false, Some((index, ordinal, hash))) = (
            ids.is_empty(),
            self.locate(&format!("msg/{}/{code}.bar", self.language)),
        ) && let Ok(texts) =
            hud::load_messages_optional(&self.source, &index, ordinal, hash, &ids)
        {
            for (id, text) in ids.iter().zip(texts) {
                if let Some(t) = text {
                    lines.insert(*id, t);
                }
            }
        }
        (npcs, deferred, scripts, lines)
    }
}

impl LoadedArea {
    pub fn take_collision(&mut self) -> collision::World {
        self.collision.take().expect("area collision taken once")
    }
}

/// A townsperson from a spawn entity: its route, behaviour zones and
/// action records come from its spawn point (FUN_1404168c0).
pub fn build_npc(
    state: &Area,
    kind: usize,
    spawn: &Spawn,
    entity_index: usize,
    random: &mut Lcg,
) -> FieldNpc {
    let object = &state.objects[spawn.point];
    let mut npc = FieldNpc::spawn(
        kind,
        &spawn.entity,
        entity_index,
        object.point.walk_paths.first(),
        random,
    );
    let zones = object
        .zones
        .iter()
        .filter(|z| {
            let raw = &object.point.activators[z.activator].raw;
            u16::from_le_bytes([raw[0x2c], raw[0x2d]]) == 0
        })
        .map(|z| crate::npcs::NpcZone {
            id: z.option,
            zone: z.clone(),
        })
        .collect();
    let actions = object
        .point
        .return_params
        .iter()
        .map(crate::npcs::Action::read)
        .collect();
    npc.set_behaviour(zones, actions);
    npc
}

struct Flags<'a>(&'a Progress);
impl script::Game for Flags<'_> {
    fn flag_set(&self, flag: u32) -> bool {
        self.0.test(flag)
    }
}

/// The map BAR's first type-6 COCT as the body collision world
/// (FUN_1401613c0 +0x68 → FUN_1401722f0 with the area's group mask).
/// The map's effect container: the first type-18 entry of the map BAR.
fn map_pax(map: &[u8]) -> Option<data::object_effect::Pax> {
    let table = data::bar::read_bar(map).ok()?;
    let e = table
        .entries
        .iter()
        .find(|e| e.entry_type == 18 && e.size > 0)?;
    let bytes = map.get(e.offset as usize..e.offset as usize + e.size as usize)?;
    data::object_effect::read_pax(bytes).ok()
}

fn map_collision(map: &[u8], mask: u64) -> Result<collision::World> {
    let table = bar::read_bar(map)?;
    let e = table
        .entries
        .iter()
        .find(|e| e.entry_type == 6 && e.size > 0)
        .ok_or("map has no collision")?;
    let coct = room_collision::read(&map[e.offset as usize..e.offset as usize + e.size as usize])?;
    Ok(collision::World::new(&coct, mask)?)
}

/// The controller's view of the map collision: one body-versus-map move
/// per tick through the exe's solver (FUN_14016fec0 as FUN_1403b9090 calls
/// it). The scene is Y up; the exe is Y down: (x, y, z) ↔ (x, −y, −z).
pub struct MapWorld {
    world: collision::World,
    /// Floor normal from the previous tick (obj+0x768 is −normal), (0, −1,
    /// 0) engine-side without a floor.
    floor_normal: std::sync::Mutex<[f32; 3]>,
    /// Material of the floor polygon last hit (poly byte 0, 0 without one):
    /// the footstep sound's material (obj+0x780).
    floor_ground: std::sync::atomic::AtomicU8,
}

impl MapWorld {
    /// The visible collision polygons as scene-coordinate triangles.
    pub fn triangles(&self) -> Vec<[Vec3; 3]> {
        let scene = |v: [f32; 3]| Vec3::from_array(engine(v));
        self.world
            .polygons()
            .into_iter()
            .filter(|p| p.visible)
            .flat_map(|p| {
                let c: Vec<Vec3> = p.corners.iter().map(|&v| scene(v)).collect();
                (1..c.len().saturating_sub(1))
                    .map(|i| [c[0], c[i], c[i + 1]])
                    .collect::<Vec<_>>()
            })
            .collect()
    }

    pub fn new(world: collision::World) -> Self {
        Self {
            world,
            floor_normal: std::sync::Mutex::new([0., -1., 0.]),
            floor_ground: std::sync::atomic::AtomicU8::new(0),
        }
    }

    /// An attack volume's segment against the map (FUN_140171090, class 4)
    /// in scene coordinates: the hit point is scene, the polygon normal
    /// stays in the engine's space (it only turns an effect).
    pub fn attack_hit(&self, from: [f32; 3], to: [f32; 3]) -> Option<collision::AttackHit> {
        let hit = self.world.attack_ray(engine(from), engine(to))?;
        Some(collision::AttackHit {
            point: engine(hit.point),
            ..hit
        })
    }

    /// The floor material under Sora as of the last move.
    pub fn floor_ground(&self) -> u8 {
        self.floor_ground.load(std::sync::atomic::Ordering::Relaxed)
    }
}

fn engine(v: [f32; 3]) -> [f32; 3] {
    [v[0], -v[1], -v[2]]
}

impl MapWorld {
    /// Switches a background group's collision meshes (FUN_140162a00 /
    /// FUN_1401629b0 → FUN_1401715a0 / FUN_140174440).
    pub fn set_group(&mut self, group: u8, on: bool) {
        self.world.set_group(group, on);
    }
}

impl player::World for MapWorld {
    fn floor(&self, x: f32, z: f32, top: f32) -> Option<f32> {
        // The exe's floor ray (FUN_140173b30) straight down from `top`.
        let p0 = [x, -top, -z];
        let p1 = [x, -top + 100_000., -z];
        self.world.ray(p0, p1, SORA_BODY.class).map(|(p, _)| -p[1])
    }

    fn resolve_walls(&self, position: [f32; 2], _radius: f32, _low: f32, _high: f32) -> [f32; 2] {
        // Only the single collide call moves Sora; other callers get the
        // wanted point back.
        position
    }

    fn collide(
        &self,
        start: player::Vec3,
        displacement: player::Vec3,
        airborne: bool,
        _body: &player::Body,
    ) -> player::Collided {
        let mut d = engine(displacement);
        if !airborne {
            // FUN_1403b9090: the move is turned into the floor plane,
            // keeping its length (n = obj+0x768 = −floor normal).
            let n = *self.floor_normal.lock().expect("floor normal");
            d = collision::project_on_floor(d, [-n[0], -n[1], -n[2]]);
        }
        let s = collision::solve(
            &self.world,
            &SORA_BODY,
            &Query {
                start: engine(start),
                displacement: d,
                airborne,
                extra_snap: 0.,
            },
        );
        let floor = collision::floor_info(&self.world, &s);
        *self.floor_normal.lock().expect("floor normal") = floor.normal;
        self.floor_ground
            .store(floor.ground, std::sync::atomic::Ordering::Relaxed);
        // Airborne: landing needs a floor while not rising (disp.y ≥ 0 in
        // engine space) [FUN_1403b9090].
        let on_floor = if airborne {
            d[1] >= 0. && !s.no_floor
        } else {
            !s.no_floor
        };
        player::Collided {
            position: engine(s.position),
            on_floor,
        }
    }
}

/// The zone task's view of the player (FUN_1403a4f90): scene → engine.
pub fn probe(position: Vec3, yaw: f32, moving: Option<[f32; 2]>, running: bool) -> PlayerProbe {
    // Scene yaw φ faces (sin φ, cos φ); engine yaw Y = π − φ.
    let engine_yaw = ::area::wrap(std::f32::consts::PI - yaw);
    let (move_direction, move_speed) = match moving {
        Some([x, z]) => ([x, 0., -z], if running { 8 } else { 2 }),
        None => ([0.; 3], 0),
    };
    PlayerProbe {
        position: [position.x, -position.y, -position.z],
        yaw: engine_yaw,
        move_direction,
        move_speed,
        can_move: true,
    }
}

/// What the area asks the field to do this tick.
pub fn requests(
    area: &mut Area,
    probe: &PlayerProbe,
    blocked: impl Fn(u8, u8) -> bool,
) -> Vec<Request> {
    area.tick(probe, false, blocked)
}

/// Scene direction (x, z) from `from` toward the nearest unfired exit
/// trigger (type-1 point, opt-0 zone) whose destination is not `avoid`.
pub fn exit_target(area: &Area, avoid: Option<u8>, from: [f32; 2]) -> Option<[f32; 2]> {
    let mut best: Option<(f32, [f32; 2])> = None;
    for o in &area.objects {
        if o.point.kind != 1 || o.flags & 1 != 0 || Some(o.point.exit_area()) == avoid {
            continue;
        }
        for z in o.zones.iter().filter(|z| z.option == 0) {
            let d = [z.center[0] - from[0], -z.center[2] - from[1]];
            let l = d[0].hypot(d[1]);
            if l > 0. && best.is_none_or(|(b, _)| l < b) {
                best = Some((l, [d[0] / l, d[1] / l]));
            }
        }
    }
    best.map(|(_, d)| d)
}
