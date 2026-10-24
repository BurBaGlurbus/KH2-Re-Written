//! Reads everything one event needs from the bound installation, by the
//! names the event script itself gives.
use data::{
    event_script::{self, Command, Load},
    model_inspection::{self, InspectionModel, ModelItem, ModelOptions},
    model_inspection_textures::{TextureMode, UvProfile},
    model_pose::PoseProfile,
    motion::{self, InterpolatedMotion},
    motion_trigger, object_effect, objentry,
    path_lookup::Library,
    probe_source::ProbeSource,
};
use std::collections::{BTreeMap, HashMap};
use visual::hud::{self, SystemFont};
use visual::room::{self, LoadedRoom};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

/// Which event to play: `ard/<lang>/<world><area>.ard`, entry `name`.
pub struct EventId {
    pub world: String,
    pub area: u8,
    pub name: String,
}

pub struct Actor {
    pub id: i16,
    /// The script's actor name (the motion directory, e.g. "w_ex010").
    pub name: String,
    /// The object entry's model (e.g. W_EX010_10).
    pub model_name: String,
    pub model: InspectionModel,
}

/// One motion frame trigger (the type-16 entry of an ANB): fires when the
/// motion time passes `frame` (FUN_14040e960 → FUN_14040dd70).
#[derive(Clone, Debug)]
pub struct FrameTrigger {
    pub frame: f32,
    pub id: u8,
    pub parameters: Vec<i16>,
}

/// What the effect presentation reads: the ReadEffect PAX files and the
/// motion triggers of the event's animations.
pub struct EffectFiles {
    /// ReadEffect PAX files (`event/effect/<name>.pax`) by their Id.
    pub paxes: Vec<(i16, object_effect::Pax)>,
    /// Frame triggers by (actor id, script animation path).
    pub triggers: HashMap<(i16, String), Vec<FrameTrigger>>,
}

pub struct Loaded {
    pub commands: Vec<Command>,
    pub room: LoadedRoom,
    pub actors: Vec<Actor>,
    /// Motions by (actor id, script path), e.g. (0, "anm_tt/ETT61301E").
    pub motions: HashMap<(i16, String), InterpolatedMotion>,
    /// Voice SCDs by name (voice/<lang>/event/<name>.win32.scd).
    pub voices: HashMap<String, Vec<u8>>,
    /// Music SCDs by BGM number (bgm/music<NNN>.win32.scd).
    pub music: HashMap<i16, Vec<u8>>,
    /// Subtitle strings by message id (msg/<lang>/<world>.bar).
    pub messages: HashMap<u16, Vec<u8>>,
    /// Sound banks named by SeData (se/se<NNN>.win32.scd).
    pub banks: Vec<Vec<u8>>,
    /// Pause menu rows: "Continue" and "Skip Scene" (sys.bar 0x8ab4, 0x8ab3).
    pub pause_labels: [Vec<u8>; 2],
    pub font: SystemFont,
    /// ReadEffect PAX files and motion triggers for the effect player.
    pub paxes: EffectFiles,
    /// The resident common effect textures (`effect/texcommon.dpd`).
    pub texcommon: Option<object_effect::Dpd>,
    pub summary: serde_json::Value,
}

const LANGUAGE: &str = "us";

fn locator(library: &Library, name: &str) -> Result<(&'static str, usize, [u8; 16])> {
    let found = library
        .find(name)
        .ok_or_else(|| format!("{name} is not in the installed packages"))?;
    Ok((found.index, found.ordinal, found.name_hash))
}

/// `anm_<dir>/<file>` for actor `actor` gives `anm/<lang>/<dir>/<actor>/<file>.anb`.
pub fn motion_path(script: &str, actor: &str, prefix: &str) -> Option<String> {
    let rest = script.strip_prefix("anm_")?;
    let (dir, file) = rest.split_once('/')?;
    Some(format!("anm/{prefix}/{dir}/{actor}/{file}.anb"))
}

pub fn load(source: &ProbeSource, id: &EventId) -> Result<Loaded> {
    source.verify_session()?;
    let mut library = Library::open(source)?;
    let ard = library.read(&format!("ard/{LANGUAGE}/{}{:02}.ard", id.world, id.area))?;
    let commands = event_script::read(event_script::find_in_ard(&ard, &id.name)?)?;
    let map = library.read(&format!("map/{}{:02}.map", id.world, id.area))?;
    let room = room::load_room_archive(&map, None)?;

    let mut actor_names: HashMap<i16, (u16, String)> = HashMap::new();
    let mut motion_requests = Vec::new();
    let mut voice_names = Vec::new();
    let mut music_ids = Vec::new();
    let mut message_ids = Vec::new();
    let mut bank_numbers = Vec::new();
    let mut effect_files = Vec::new();
    for command in &commands {
        match command {
            Command::SoundData { bank, .. } => bank_numbers.push(*bank),
            Command::Loads { loads, .. } => {
                for l in loads {
                    match l {
                        Load::Actor {
                            object,
                            actor,
                            name,
                        } => {
                            actor_names.insert(*actor, (*object, name.clone()));
                        }
                        Load::Motion { actor, name, .. } => {
                            motion_requests.push((*actor, name.clone()))
                        }
                        Load::Effect { id, name } => effect_files.push((*id, name.clone())),
                        _ => {}
                    }
                }
            }
            Command::Actor {
                object,
                actor,
                name,
            } => {
                actor_names
                    .entry(*actor)
                    .or_insert_with(|| (*object, name.clone()));
            }
            Command::Animation { actor, name, .. } => motion_requests.push((*actor, name.clone())),
            Command::Voices(v) => voice_names.extend(v.iter().map(|v| v.name.clone())),
            Command::ReadBgm { bgm, .. } | Command::SetBgm { bgm, .. } => music_ids.push(*bgm),
            Command::Subtitle { message, .. } => message_ids.push(*message),
            _ => {}
        }
    }

    let options = ModelOptions {
        pose_profiles: vec![PoseProfile::Legacy],
        textures: true,
        texture_modes: vec![TextureMode::OriginalIndexed],
        uv_profiles: vec![UvProfile::Kkdf2IntegerTruncated],
    };
    // The model is the object entry's (e.g. 0x68 is W_EX010_10, Oathkeeper);
    // the script's actor name selects the motion directory.
    let objects = objentry::read(&library.read("00objentry.bin")?)?;
    let mut ids: Vec<_> = actor_names.keys().copied().collect();
    ids.sort_unstable();
    let mut actors = Vec::new();
    for id in ids {
        let (object, name) = actor_names[&id].clone();
        let entry = objects
            .iter()
            .find(|o| o.object_id == object as u32)
            .ok_or_else(|| format!("object entry {object:#x} absent"))?;
        let model_name = String::from_utf8_lossy(&entry.model_name).into_owned();
        let (index, ordinal, name_hash) = locator(&library, &format!("obj/{model_name}.mdlx"))?;
        let mut models = model_inspection::read_models(
            source,
            &[ModelItem {
                index: index.into(),
                ordinal,
                name_hash,
            }],
            &options,
        )?;
        actors.push(Actor {
            id,
            name,
            model_name,
            model: models.remove(0),
        });
    }

    let mut motions = HashMap::new();
    let mut triggers: HashMap<(i16, String), Vec<FrameTrigger>> = HashMap::new();
    for (actor, script) in motion_requests {
        if motions.contains_key(&(actor, script.clone())) {
            continue;
        }
        let actor_name = &actor_names
            .get(&actor)
            .ok_or("motion for an undeclared actor")?
            .1;
        let path = motion_path(&script, actor_name, LANGUAGE)
            .ok_or_else(|| format!("motion name {script} has no anm_ prefix"))?;
        let anb = library.read(&path)?;
        // The ANB's trigger entry (BAR type 16): its frame records.
        match motion_trigger::inspect_anb(&anb) {
            Ok(entries) => {
                let list: Vec<FrameTrigger> = entries
                    .iter()
                    .flat_map(|e| e.records.frames.iter())
                    .map(|f| FrameTrigger {
                        frame: f.frame as f32,
                        id: f.trigger,
                        parameters: f.parameters.clone(),
                    })
                    .collect();
                if !list.is_empty() {
                    triggers.insert((actor, script.clone()), list);
                }
            }
            Err(e) => eprintln!("{path}: motion triggers unreadable: {e}"),
        }
        motions.insert((actor, script), motion::read_anb(&anb, 0)?.motion);
    }

    let mut voices = HashMap::new();
    for name in voice_names {
        let bytes = library.read(&format!("voice/{LANGUAGE}/event/{name}.win32.scd"))?;
        voices.insert(name, bytes);
    }
    let mut music = HashMap::new();
    for id in music_ids {
        if let std::collections::hash_map::Entry::Vacant(e) = music.entry(id) {
            e.insert(library.read(&format!("bgm/music{id:03}.win32.scd"))?);
        }
    }

    let (mi, mo, mh) = locator(&library, &format!("msg/{LANGUAGE}/{}.bar", id.world))?;
    message_ids.sort_unstable();
    message_ids.dedup();
    let texts = hud::load_messages(
        source,
        mi,
        mo,
        mh,
        &message_ids.iter().map(|&m| m as u32).collect::<Vec<_>>(),
    )?;
    let messages = message_ids.iter().copied().zip(texts).collect();
    let mut banks = Vec::new();
    for bank in bank_numbers {
        banks.push(library.read(&format!("se/se{bank:03}.win32.scd"))?);
    }
    let (si, so, sh) = locator(&library, &format!("msg/{LANGUAGE}/sys.bar"))?;
    let mut labels = hud::load_messages(source, si, so, sh, &[0x8ab4, 0x8ab3])?;
    let font = hud::load_font(
        source,
        locator(&library, &format!("msg/{LANGUAGE}/fontimage.bar"))?,
        locator(&library, &format!("msg/{LANGUAGE}/fontinfo.bar"))?,
        b"evt\0",
    )?;

    let mut pax_files = Vec::new();
    for (id, name) in effect_files {
        let pax = object_effect::read_pax_file(&mut library, &format!("event/effect/{name}.pax"))?;
        pax_files.push((id, pax));
    }
    let paxes = EffectFiles {
        paxes: pax_files,
        triggers,
    };
    let texcommon = match object_effect::read_dpd_file(&mut library, "effect/texcommon.dpd") {
        Ok(dpd) => Some(dpd),
        Err(e) => {
            eprintln!("effect/texcommon.dpd unavailable: {e}");
            None
        }
    };

    let summary = serde_json::json!({
        "event": "event_loaded",
        "commands": commands.len(),
        "actors": actors.iter().map(|a| (a.id, a.name.clone(), a.model_name.clone(),
            a.model.parts.len(), a.model.skinning.bones.len())).collect::<Vec<_>>(),
        "motions": motions.iter().map(|((a, n), m)| (format!("{a}:{n}"),
            format!("{:?}", m.frame_window().ok()))).collect::<BTreeMap<_, _>>(),
        "voices": voices.len(),
        "music": music.keys().collect::<Vec<_>>(),
        "messages": message_ids,
        "room_models": room.rooms.len(),
        "effect_paxes": paxes.paxes.iter().map(|(i, p)| (*i, p.name.clone(), p.elements.len())).collect::<Vec<_>>(),
        "texcommon_textures": texcommon.as_ref().map(|d| d.textures.len()),
        "motion_triggers": paxes.triggers.iter().map(|((a, n), t)| (format!("{a}:{n}"), t.iter().map(|t| (t.frame, t.id, t.parameters.clone())).collect::<Vec<_>>())).collect::<BTreeMap<_, _>>(),
    });
    Ok(Loaded {
        commands,
        room,
        actors,
        motions,
        voices,
        music,
        messages,
        banks,
        pause_labels: [labels.remove(0), labels.remove(0)],
        font,
        paxes,
        texcommon,
        summary,
    })
}
