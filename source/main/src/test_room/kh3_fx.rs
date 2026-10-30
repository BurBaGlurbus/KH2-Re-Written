//! KH3's effects and sounds in the duel [T282]: the clips' own notifies (attach
//! effect, weapon swing, voice, sound), the Cascade evaluator (`effect::cascade`,
//! ported from StrawHatPro's kh-vfx, MIT) and SEAD banks from the player's paks.
//! Everything here is presentation: each PC replays it from the Sora's `Visual`,
//! the same clip and clock on both, so the wire carries only ids and hit events.
//!
//! Materials are approximated: a texture, its opacity and a tint, from the
//! parameters that survive cooking (the reference's generic path), blended as
//! the rest of the room's effects are, on stored sRGB values.

#![allow(clippy::type_complexity, clippy::chunks_exact_to_as_chunks)]

use super::*;
use bevy::image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use data::kh3::anim::BonePose;
use data::kh3::material::{self, Blend as MatBlend, MaterialParams};
use data::kh3::props::Props;
use data::kh3::sead::{self, Banks, Keys, Kind as SoundKind};
use data::kh3::sora::{self, SoraData};
use data::kh3::static_mesh::StaticMesh;
use data::kh3::texture::Texture;
use data::kh3::{Paks, effect_json};
use effect::cascade::{Blend, Camera, DrawList, EffectDef, EffectId, SpawnOptions, World, Xform};
use std::collections::HashSet;
use std::sync::atomic::{AtomicUsize, Ordering};
use visual::effect_draw::{Blend as DrawBlend, EffectMaterial};

const KEY_BASE: u32 = 0x4b33_0000;
const SWING_BANK: &str = "/Game/Sound/SE/weapon/02001";
/// `se10010`, the rock surface's steps: slot 1 a walk's, slot 2 a run's.
const FOOT_WALK: &str = "/Game/Sound/SE/footstep/10000_sora/10010/se10010_001";
const FOOT_RUN: &str = "/Game/Sound/SE/footstep/10000_sora/10010/se10010_002";
/// `BoneSet_Sora1`'s toes.
const TOES: [&str; 2] = ["L_ashi2", "R_ashi2"];
/// A foot this close to the sole of the pose may be coming down (cm).
const STEP_HEIGHT: f32 = 25.;
/// The triangles of mesh particles built on the CPU each frame; past it every
/// few are left out (a shot's bolts ask for a million).
const MESH_BUDGET: usize = 150_000;
/// The least time between two of a Sora's steps (a foot that settles twice).
const STEP_GAP: f32 = 0.18;
const TRAIL: &str = "/Game/Effects/wep/w_so010/par/VW_SO010_LOC0";

/// A material as the renderer draws it.
#[derive(Clone, Debug)]
pub(crate) struct FxMaterial {
    pub(crate) texture: Option<String>,
    pub(crate) opacity: Option<String>,
    pub(crate) mono: bool,
    pub(crate) blend: MatBlend,
    pub(crate) tint: [f32; 3],
}

/// One row of an attach-effect notify.
#[derive(Clone, Debug)]
pub(crate) struct EffectRow {
    pub(crate) effect: String,
    pub(crate) socket: String,
    pub(crate) on_weapon: bool,
    pub(crate) attach: bool,
    pub(crate) location: [f32; 3],
    pub(crate) rotation: [f32; 3],
    pub(crate) scale: [f32; 3],
    pub(crate) fade_in: f32,
    pub(crate) delay: f32,
    pub(crate) color: [f32; 4],
    /// Stops when the animation changes (the usual `WhenChangeAnimationType`).
    pub(crate) ends_with_clip: bool,
    pub(crate) state: bool,
}

#[derive(Clone, Debug)]
pub(crate) enum Cue {
    Effect(EffectRow),
    Trail,
    Sound {
        path: String,
        volume: f32,
        pitch: f32,
    },
    Voice(Vec<(String, u32)>),
    /// A sound held for the window, its end sound after it.
    Held {
        path: String,
        end: Option<String>,
    },
}

#[derive(Clone, Debug)]
pub(crate) struct Trigger {
    pub(crate) start: f32,
    pub(crate) end: f32,
    pub(crate) cue: Cue,
}

/// What the load thread hands over.
pub(crate) struct FxData {
    pub(crate) defs: Vec<EffectDef>,
    pub(crate) materials: Vec<(String, FxMaterial)>,
    pub(crate) meshes: Vec<(String, StaticMesh)>,
    pub(crate) triggers: HashMap<String, Vec<Trigger>>,
    pub(crate) textures: HashMap<String, Texture>,
    pub(crate) sounds: SoundBook,
    /// Effect name to the sounds its SeAttach data plays.
    pub(crate) effect_sounds: HashMap<String, Vec<String>>,
}

/// Decoded sounds by their asset.
#[derive(Default)]
pub(crate) struct SoundBook {
    pub(crate) sounds: HashMap<String, Arc<Playable>>,
}

/// A resolved sound: its notes, with clips decoded.
pub(crate) struct Playable {
    pub(crate) kind: SoundKind,
    pub(crate) volume: f32,
    pub(crate) asset_volume: f32,
    pub(crate) spatial: bool,
    pub(crate) window: f32,
    pub(crate) sequences: Vec<(u8, u8, Vec<PlayNote>)>,
    pub(crate) cursor: AtomicUsize,
}

pub(crate) struct PlayNote {
    pub(crate) delay: f32,
    pub(crate) clip: audio::Clip,
    pub(crate) volume: f32,
    pub(crate) volume_random: Option<(f32, f32, u8)>,
    pub(crate) pitch_random: Option<(f32, f32, u8)>,
}

fn vector(p: &Props, k: &str, or: [f32; 3]) -> [f32; 3] {
    p.floats(k)
        .filter(|v| v.len() == 3)
        .map_or(or, |v| [v[0], v[1], v[2]])
}

fn english(path: &str) -> String {
    path.replace("/Localization/ja/", "/Localization/en/")
}

/// The rows of a notify's `MetaAttachDataList` and weapon lists.
fn effect_rows(n: &sora::Notify, out: &mut Vec<Trigger>) {
    if let Some(list) = n.props.array("AttachDataList") {
        for row in list {
            let Some(row) = row.as_props() else { continue };
            if row.bool("bUseCommonAttachData") == Some(true)
                && row
                    .str("AttachType")
                    .is_some_and(|t| t.ends_with("ATTACH_TO_WEAPON"))
            {
                out.push(Trigger {
                    start: n.start,
                    end: n.end,
                    cue: Cue::Trail,
                });
            }
        }
    }
    for list in n.props.array("MetaAttachDataList").unwrap_or(&[]) {
        let Some(list) = list.as_props() else {
            continue;
        };
        let on_weapon = list.str("AttachType").is_some_and(|t| t.contains("WEAPON"));
        let list_socket = list.str("SocketName").unwrap_or("");
        for row in list.array("MetaAttachData").unwrap_or(&[]) {
            let Some(row) = row.as_props() else { continue };
            let socket = row
                .str("SocketName")
                .filter(|s| *s != "None")
                .unwrap_or(list_socket);
            for e in row.array("EffectDataList").unwrap_or(&[]) {
                let Some(e) = e.as_props() else { continue };
                // The plain Keyblade's group is 0; -1 plays for any.
                let group = e.int("GroupID").unwrap_or(0);
                if group != 0 && group != -1 {
                    continue;
                }
                let Some(path) = e
                    .object("AttachEffect")
                    .and_then(|o| n.imports.get(&o).cloned())
                else {
                    continue;
                };
                let name = effect_json::base_name(&path).to_ascii_lowercase();
                if name.starts_with("vw_so") && !name.starts_with("vw_so010") {
                    continue;
                }
                let (c, a) = (
                    row.floats("ParticleColor")
                        .filter(|v| v.len() >= 3)
                        .map_or([1.; 3], |v| [v[0], v[1], v[2]]),
                    row.f32("ParticleAlpha").unwrap_or(1.),
                );
                out.push(Trigger {
                    start: n.start,
                    end: n.end,
                    cue: Cue::Effect(EffectRow {
                        effect: path,
                        socket: socket.to_owned(),
                        on_weapon,
                        attach: row.bool("ToAttach").unwrap_or(true),
                        location: vector(row, "LocationOffset", [0.; 3]),
                        rotation: vector(row, "RotationOffset", [0.; 3]),
                        scale: vector(row, "ScaleOffset", [1.; 3]),
                        fade_in: row.f32("FadeIn").unwrap_or(0.),
                        delay: row.f32("StartDelayTime").unwrap_or(0.),
                        color: [c[0], c[1], c[2], a],
                        ends_with_clip: !row
                            .str("WhenChangeAnimationType")
                            .is_some_and(|t| t.ends_with("NONE")),
                        state: n.state,
                    }),
                });
            }
        }
    }
}

fn trigger_of(n: &sora::Notify, out: &mut Vec<Trigger>) {
    let object = |p: &Props, k: &str| p.object(k).and_then(|o| n.imports.get(&o).cloned());
    match n.class.as_str() {
        "TresAnimNotify_AttachEffect" | "TresAnimNotifyState_AttachEffect" => effect_rows(n, out),
        "TresAnimNotify_PlayWeaponSwingSE" | "TresAnimNotifyState_PlayWeaponSwingSE" => {
            let Some(id) = n.props.str("m_SoundAliasID") else {
                return;
            };
            let Some(index) = id
                .rsplit("SWING")
                .next()
                .and_then(|s| s.parse::<u32>().ok())
                .filter(|i| *i <= 10)
            else {
                return;
            };
            out.push(Trigger {
                start: n.start,
                end: n.end,
                cue: Cue::Sound {
                    path: format!("{SWING_BANK}/se02001_{}", 100 + index),
                    volume: n.props.f32("m_VolumeMultiplier").unwrap_or(1.),
                    pitch: n.props.f32("m_PitchMultiplier").unwrap_or(1.),
                },
            });
        }
        "TresAnimNotify_PlayVoice" | "TresAnimNotifyState_PlayVoice" => {
            let mut list = Vec::new();
            if let Some(v) = object(&n.props, "m_VoiceAsset") {
                list.push((english(&v), 1));
            }
            for r in n.props.array("m_RandomVoiceAssets").unwrap_or(&[]) {
                let Some(r) = r.as_props() else { continue };
                if let Some(v) = object(r, "Voice") {
                    let w = r.int("RandomWeights").unwrap_or(1).max(1) as u32;
                    list.push((english(&v), w));
                }
            }
            if !list.is_empty() {
                out.push(Trigger {
                    start: n.start,
                    end: n.end,
                    cue: Cue::Voice(list),
                });
            }
        }
        "SQEXSEADAnimNotify_PlaySound" => {
            if let Some(path) = object(&n.props, "Sound") {
                out.push(Trigger {
                    start: n.start,
                    end: n.end,
                    cue: Cue::Sound {
                        path,
                        volume: n.props.f32("VolumeMultiplier").unwrap_or(1.),
                        pitch: n.props.f32("PitchMultiplier").unwrap_or(1.),
                    },
                });
            }
        }
        "SQEXSEADAnimNotifyState_AttachSound" => {
            if let Some(path) = object(&n.props, "Sound") {
                out.push(Trigger {
                    start: n.start,
                    end: n.end,
                    cue: Cue::Held {
                        path,
                        end: object(&n.props, "LoopEndSound"),
                    },
                });
            }
        }
        _ => {}
    }
}

/// The name priority of the reference's generic material path.
fn pick_textures(m: &MaterialParams) -> (Option<String>, Option<String>) {
    const EMISSIVE: [&str; 10] = [
        "BaseColorTex",
        "Tex Emi",
        "Tex Main",
        "Tex Core",
        "Emi Tex",
        "texture",
        "base_tex",
        "tex_A",
        "Tex Glow",
        "1 Grayscale Texture A",
    ];
    const OPACITY: [&str; 5] = ["Tex Opa", "1 Tex Opa", "Tex Opq", "Tex Mask", "Tex_mask1"];
    let emissive = EMISSIVE
        .iter()
        .find_map(|k| m.textures.get(*k).cloned())
        .or_else(|| {
            m.textures.iter().find_map(|(k, v)| {
                let l = k.to_ascii_lowercase();
                let bad = [
                    "nml",
                    "normal",
                    "wpo",
                    "dist",
                    "break",
                    "constraint",
                    "opa",
                    "mask",
                ];
                (!bad.iter().any(|b| l.contains(b))).then(|| v.clone())
            })
        });
    let opacity = OPACITY.iter().find_map(|k| m.textures.get(*k).cloned());
    (emissive, opacity)
}

fn fx_material(m: &MaterialParams) -> FxMaterial {
    let (texture, opacity) = pick_textures(m);
    let scalar = |k: &str| m.scalars.get(k).copied();
    let vector = |k: &str| m.vectors.get(k).copied();
    let mult = scalar("Bright Multi Emi").filter(|m| *m > 0.).unwrap_or(1.);
    let tint = vector("2c White Color")
        .or_else(|| vector("Emi Color"))
        .map_or([1.; 3], |c| [c[0], c[1], c[2]]);
    FxMaterial {
        texture,
        opacity,
        mono: false,
        blend: m.blend,
        tint: [tint[0] * mult, tint[1] * mult, tint[2] * mult],
    }
}

/// Reads the sounds, effects, materials and meshes the clips name.
pub(crate) fn load(
    paks: &Paks,
    data: &SoraData,
    book: &player::kh3::attack::Book,
    install: &data::kh3::Install,
) -> Result<FxData, String> {
    let mut triggers: HashMap<String, Vec<Trigger>> = HashMap::new();
    for clip in &data.anims.clips {
        let mut list = Vec::new();
        for n in &clip.notifies {
            trigger_of(n, &mut list);
        }
        if !list.is_empty() {
            triggers.insert(clip.name.clone(), list);
        }
    }
    let mut effects: Vec<String> = vec![TRAIL.to_owned()];
    let mut sounds: Vec<String> = (1..=10)
        .map(|i| format!("{SWING_BANK}/se02001_{}", 100 + i))
        .chain((1..=7).map(|i| format!("{SWING_BANK}/se02001_00{i}")))
        .chain([FOOT_WALK.to_owned(), FOOT_RUN.to_owned()])
        .collect();
    for list in triggers.values() {
        for t in list {
            match &t.cue {
                Cue::Effect(r) => effects.push(r.effect.clone()),
                Cue::Sound { path, .. } => sounds.push(path.clone()),
                Cue::Voice(v) => sounds.extend(v.iter().map(|x| x.0.clone())),
                Cue::Held { path, end, .. } => {
                    sounds.push(path.clone());
                    sounds.extend(end.clone());
                }
                Cue::Trail => {}
            }
        }
    }
    // The spells' effects and sounds.
    fn stage_assets(
        s: &player::kh3::spell::Stage,
        effects: &mut Vec<String>,
        sounds: &mut Vec<String>,
    ) {
        effects.extend(s.body_effect.clone());
        effects.extend(s.hit_effect.clone());
        effects.extend(s.map_hit_effect.clone());
        sounds.extend(s.hit_sound.clone());
        sounds.extend(s.map_hit_sound.clone());
        for (e, c) in &s.fire_effects {
            effects.extend(e.clone());
            sounds.extend(c.clone());
        }
        if let Some(n) = &s.next {
            stage_assets(n, effects, sounds);
        }
    }
    for spell in book.spells.values() {
        effects.extend(spell.cast_effect.clone());
        stage_assets(&spell.stage, &mut effects, &mut sounds);
    }
    for stage in book.shots.iter().flatten() {
        stage_assets(stage, &mut effects, &mut sounds);
    }
    effects.extend(book.hit_effect.clone());
    // What the core names (combat, status, the shotlock's own).
    effects.extend(
        book.by_name
            .values()
            .filter(|p| p.to_ascii_lowercase().starts_with("/game/effects/"))
            .cloned(),
    );
    effects.sort_unstable();
    effects.dedup();
    // Effects, with what they spawn.
    let mut defs = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let mut materials: Vec<String> = Vec::new();
    let mut meshes: Vec<String> = Vec::new();
    let mut todo = effects;
    let mut skipped = 0;
    while let Some(path) = todo.pop() {
        if !seen.insert(path.to_ascii_lowercase()) {
            continue;
        }
        let Ok(built) = effect_json::build(paks, &path) else {
            skipped += 1;
            continue;
        };
        match EffectDef::from_json(&built.json) {
            Ok(def) => defs.push(def),
            Err(_) => skipped += 1,
        }
        materials.extend(built.materials);
        meshes.extend(built.meshes);
        todo.extend(built.nested);
    }
    materials.sort_unstable();
    materials.dedup();
    meshes.sort_unstable();
    meshes.dedup();
    let mut mesh_data = Vec::new();
    for path in &meshes {
        if let Ok(mesh) = StaticMesh::load(paks, path) {
            mesh_data.push((effect_json::base_name(path), mesh));
        }
    }
    let mut fx_materials = Vec::new();
    let mut texture_paths: Vec<String> = Vec::new();
    for path in &materials {
        if let Ok(m) = material::read(paks, path) {
            let fx = fx_material(&m);
            texture_paths.extend(fx.texture.clone());
            texture_paths.extend(fx.opacity.clone());
            fx_materials.push((effect_json::base_name(path), fx));
        }
    }
    // Meshes' own slot materials.
    for (_, mesh) in &mesh_data {
        for path in &mesh.materials {
            if path.is_empty() || materials.contains(path) {
                continue;
            }
            if let Ok(m) = material::read(paks, path) {
                let fx = fx_material(&m);
                texture_paths.extend(fx.texture.clone());
                texture_paths.extend(fx.opacity.clone());
                fx_materials.push((effect_json::base_name(path), fx));
            }
        }
    }
    texture_paths.sort_unstable();
    texture_paths.dedup();
    let mut textures = HashMap::new();
    for path in &texture_paths {
        let Ok(Some(p)) = paks.game_package(path) else {
            continue;
        };
        if let Some(i) = p.export_of_class("Texture2D")
            && let Ok(t) = Texture::read(&p, i)
        {
            textures.insert(path.clone(), t);
        }
    }
    for (_, m) in &mut fx_materials {
        m.mono = m
            .texture
            .as_ref()
            .and_then(|t| textures.get(t))
            .is_some_and(|t| matches!(t.format.as_str(), "PF_BC4" | "PF_G8"));
    }
    // Sounds through the effects' SeAttach data.
    let mut effect_sounds: HashMap<String, Vec<String>> = HashMap::new();
    for name in defs.iter().map(|d| d.name.clone()) {
        for folder in ["pc", "magic", "system", "common", "weapon"] {
            let path = format!("/Game/Sound/SE/_Data/SeAttach/{folder}/{name}");
            let Ok(Some(p)) = paks.game_package(&path) else {
                continue;
            };
            let Some(i) = p.export_of_class("SQEX_EffectSeAttachDataAsset") else {
                continue;
            };
            let Ok((props, _)) = data::kh3::props::export_props(&p, i) else {
                continue;
            };
            let mut list = Vec::new();
            for row in props.array("SEDatas").unwrap_or(&[]) {
                if let Some(s) = row
                    .as_props()
                    .and_then(|r| r.object("Sound"))
                    .and_then(|o| p.import_package(o))
                {
                    list.push(s.to_owned());
                }
            }
            if !list.is_empty() {
                sounds.extend(list.iter().cloned());
                effect_sounds.insert(name.clone(), list);
            }
            break;
        }
    }
    sounds.sort_unstable();
    sounds.dedup();
    let book = load_sounds(paks, install, &sounds)?;
    info!(
        "KH3 effects: {} systems ({skipped} skipped), {} materials, {} meshes, {} textures, {} of {} sounds",
        defs.len(),
        fx_materials.len(),
        mesh_data.len(),
        textures.len(),
        book.sounds.len(),
        sounds.len(),
    );
    Ok(FxData {
        defs,
        materials: fx_materials,
        meshes: mesh_data,
        triggers,
        textures,
        sounds: book,
        effect_sounds,
    })
}

fn load_sounds(
    paks: &Paks,
    install: &data::kh3::Install,
    sounds: &[String],
) -> Result<SoundBook, String> {
    let image = std::fs::read(install.exe()).map_err(|e| e.to_string())?;
    let tables = data::kh3::hca::Tables::find(&image).map_err(|e| e.to_string())?;
    let banks = Banks::default();
    // The XOR table, from the first obfuscated bank.
    let bank = banks
        .resolve(paks, &format!("{SWING_BANK}/se02001_101"))
        .ok_or("the Keyblade's sound bank is missing")?;
    let key = sead::find_key(&image, &tables, &bank.bank).map_err(|e| e.to_string())?;
    drop(image);
    let keys = Keys {
        tables,
        sead_key: key,
    };
    let mut clips: HashMap<(u16, u16), Option<audio::Clip>> = HashMap::new();
    let mut book = SoundBook::default();
    for path in sounds {
        let Some(r) = banks.resolve(paks, path) else {
            continue;
        };
        let Some(Some(sound)) = r.bank.sounds.get(r.slot) else {
            continue;
        };
        let mut sequences = Vec::new();
        for seq in &sound.sequences {
            let mut notes = Vec::new();
            for n in &seq.notes {
                let clip = clips
                    .entry((r.bank.number, n.material))
                    .or_insert_with(|| {
                        let m = r.bank.materials.get(usize::from(n.material))?;
                        let d = sead::decode_material(m, &keys).ok()?;
                        Some(audio::Clip {
                            channels: u16::from(d.channels),
                            rate: d.rate,
                            samples: d.samples.into(),
                            loop_frames: d.looped,
                        })
                    })
                    .clone();
                if let Some(clip) = clip {
                    notes.push(PlayNote {
                        delay: n.delay_ms as f32 / 1000.,
                        clip,
                        volume: n.volume,
                        volume_random: n.volume_random,
                        pitch_random: n.pitch_random,
                    });
                }
            }
            sequences.push((seq.weight, seq.group, notes));
        }
        // Macros 1, 3, 5, 7: the same sound started within the window is cancelled.
        let window = match sound.start_macro {
            1 => 0.067,
            3 => 0.133,
            5 => 0.266,
            7 => 0.533,
            10 => 2.,
            _ => 0.,
        };
        book.sounds.insert(
            path.clone(),
            Arc::new(Playable {
                kind: sound.kind,
                volume: sound.volume,
                asset_volume: r.volume,
                spatial: r.spatial,
                window,
                sequences,
                cursor: AtomicUsize::new(0),
            }),
        );
    }
    Ok(book)
}

/// A live effect: which Sora and socket it follows.
struct Live {
    id: EffectId,
    seat: u8,
    row: Option<EffectRow>,
    /// The clip's play it belongs to, for its end.
    serial: u32,
    trail: bool,
    state_end: Option<f32>,
}

/// One Sora's replay state.
#[derive(Default)]
struct Replay {
    serial: u32,
    clip: String,
    time: f32,
    /// Held sounds' mixer keys and when they end.
    held: Vec<(u32, f32)>,
}

/// A sound scheduled in the near future.
struct Pending {
    at: f32,
    clip: audio::Clip,
    gain: f32,
    pitch: f32,
    key: u32,
}

#[derive(Resource, Default)]
pub(crate) struct Kh3Fx {
    world: Option<World>,
    data: Option<Arc<FxData>>,
    materials: HashMap<u32, (Option<Handle<Image>>, MatBlend, [f32; 3], bool)>,
    meshes: HashMap<u32, StaticMesh>,
    /// A mesh's own first material, for an emitter that names none.
    mesh_materials: HashMap<u32, u32>,
    live: Vec<Live>,
    /// The spell actors' body effects: seat, the actor's serial, the effect.
    bodies: Vec<(u8, u32, EffectId)>,
    replays: [Replay; 2],
    accumulator: f32,
    batches: HashMap<(u32, u8), Batch>,
    effect_materials: HashMap<(u32, u8), Handle<EffectMaterial>>,
    pending: Vec<Pending>,
    last_start: HashMap<String, f32>,
    clock: f32,
    next_key: u32,
    /// The sound being started may loop until its key stops.
    holding: bool,
    rng: u32,
    draw: DrawList,
    /// Each Sora's toes: last height, and whether they were coming down.
    feet: [[(f32, bool); 2]; 4],
    last_step: [f32; 4],
}

struct Batch {
    entity: Entity,
    mesh: Handle<Mesh>,
}

#[derive(Component)]
pub(crate) struct Kh3FxPart;

/// A mono or masked texture as RGBA, whose alpha the shader can use.
fn bc4_block(b: &[u8], out: &mut [u8; 16]) {
    let (a0, a1) = (u32::from(b[0]), u32::from(b[1]));
    let mut table = [0u32; 8];
    table[0] = a0;
    table[1] = a1;
    if a0 > a1 {
        for i in 1..7 {
            table[i + 1] = ((7 - i as u32) * a0 + i as u32 * a1) / 7;
        }
    } else {
        for i in 1..5 {
            table[i + 1] = ((5 - i as u32) * a0 + i as u32 * a1) / 5;
        }
        table[6] = 0;
        table[7] = 255;
    }
    let bits = u64::from_le_bytes([b[2], b[3], b[4], b[5], b[6], b[7], 0, 0]);
    for (i, slot) in out.iter_mut().enumerate() {
        *slot = table[((bits >> (3 * i)) & 7) as usize] as u8;
    }
}

/// Mip level with the longest side at most `limit`, decoded to one channel
/// when the format allows it (BC4, G8, the alpha of DXT5).
fn single_channel(t: &Texture, limit: u32) -> Option<(u32, u32, Vec<u8>)> {
    let mip = t
        .mips
        .iter()
        .find(|m| m.width.max(m.height) <= limit)
        .or(t.mips.last())?;
    let (w, h) = (mip.width as usize, mip.height as usize);
    match t.format.as_str() {
        "PF_G8" => Some((mip.width, mip.height, mip.bytes.clone())),
        "PF_BC4" | "PF_DXT5" => {
            let stride = if t.format == "PF_BC4" { 8 } else { 16 };
            let mut out = vec![0u8; w * h];
            let (bw, bh) = (w.div_ceil(4), h.div_ceil(4));
            for by in 0..bh {
                for bx in 0..bw {
                    let block = mip.bytes.get((by * bw + bx) * stride..)?;
                    let mut px = [0u8; 16];
                    bc4_block(block, &mut px);
                    for (i, v) in px.iter().enumerate() {
                        let (x, y) = (bx * 4 + i % 4, by * 4 + i / 4);
                        if x < w && y < h {
                            out[y * w + x] = *v;
                        }
                    }
                }
            }
            Some((mip.width, mip.height, out))
        }
        _ => None,
    }
}

/// A colour texture's mip at most `limit` wide as RGBA8 (BC1, BC3, BGRA; BC7
/// has no decoder here: `None`).
fn rgba_of(t: &Texture, limit: u32) -> Option<(u32, u32, Vec<u8>)> {
    let mip = t
        .mips
        .iter()
        .find(|m| m.width.max(m.height) <= limit)
        .or(t.mips.last())?;
    let (w, h) = (mip.width as usize, mip.height as usize);
    let mut out = vec![255u8; w * h * 4];
    match t.format.as_str() {
        "PF_B8G8R8A8" => {
            for (o, c) in out.chunks_exact_mut(4).zip(mip.bytes.chunks_exact(4)) {
                o.copy_from_slice(&[c[2], c[1], c[0], c[3]]);
            }
        }
        "PF_DXT1" | "PF_DXT5" => {
            let stride = if t.format == "PF_DXT1" { 8 } else { 16 };
            let at = if t.format == "PF_DXT1" { 0 } else { 8 };
            let (bw, bh) = (w.div_ceil(4), h.div_ceil(4));
            for by in 0..bh {
                for bx in 0..bw {
                    let block = mip.bytes.get((by * bw + bx) * stride..)?;
                    let c0 = u16::from_le_bytes([block[at], block[at + 1]]);
                    let c1 = u16::from_le_bytes([block[at + 2], block[at + 3]]);
                    let rgb = |c: u16| {
                        [
                            u32::from(c >> 11) * 255 / 31,
                            u32::from((c >> 5) & 63) * 255 / 63,
                            u32::from(c & 31) * 255 / 31,
                        ]
                    };
                    let (a, b) = (rgb(c0), rgb(c1));
                    let mix = |x: u32, y: u32, n: u32, d: u32| (x * (d - n) + y * n) / d;
                    let table: [[u32; 4]; 4] = if c0 > c1 || at == 8 {
                        [
                            [a[0], a[1], a[2], 255],
                            [b[0], b[1], b[2], 255],
                            [
                                mix(a[0], b[0], 1, 3),
                                mix(a[1], b[1], 1, 3),
                                mix(a[2], b[2], 1, 3),
                                255,
                            ],
                            [
                                mix(a[0], b[0], 2, 3),
                                mix(a[1], b[1], 2, 3),
                                mix(a[2], b[2], 2, 3),
                                255,
                            ],
                        ]
                    } else {
                        [
                            [a[0], a[1], a[2], 255],
                            [b[0], b[1], b[2], 255],
                            [
                                mix(a[0], b[0], 1, 2),
                                mix(a[1], b[1], 1, 2),
                                mix(a[2], b[2], 1, 2),
                                255,
                            ],
                            [0, 0, 0, 0],
                        ]
                    };
                    let bits = u32::from_le_bytes([
                        block[at + 4],
                        block[at + 5],
                        block[at + 6],
                        block[at + 7],
                    ]);
                    let mut alpha = [255u8; 16];
                    if at == 8 {
                        bc4_block(block, &mut alpha);
                    }
                    for i in 0..16 {
                        let (x, y) = (bx * 4 + i % 4, by * 4 + i / 4);
                        if x < w && y < h {
                            let c = table[((bits >> (2 * i)) & 3) as usize];
                            let o = (y * w + x) * 4;
                            out[o] = c[0] as u8;
                            out[o + 1] = c[1] as u8;
                            out[o + 2] = c[2] as u8;
                            out[o + 3] = if at == 8 { alpha[i] } else { c[3] as u8 };
                        }
                    }
                }
            }
        }
        _ => return None,
    }
    Some((mip.width, mip.height, out))
}

/// One channel of a mask texture, the alpha when it has one that varies.
fn mask_of(t: &Texture, limit: u32) -> Option<(u32, u32, Vec<u8>)> {
    if matches!(t.format.as_str(), "PF_BC4" | "PF_G8") {
        return single_channel(t, limit);
    }
    let (w, h, px) = rgba_of(t, limit)?;
    let use_alpha = px.chunks_exact(4).any(|c| c[3] < 250);
    let ch = if use_alpha { 3 } else { 0 };
    Some((w, h, px.chunks_exact(4).map(|c| c[ch]).collect()))
}

/// A colour under a mask: the reference's emissive with its separate opacity.
fn masked_image(color: Option<&Texture>, mask: &Texture) -> Option<Image> {
    let (w, h, a) = mask_of(mask, 512)?;
    let tiny = color.is_none_or(|t| t.width <= 8 && t.height <= 8);
    let colour = if tiny {
        None
    } else {
        color.and_then(|t| {
            if matches!(t.format.as_str(), "PF_BC4" | "PF_G8") {
                single_channel(t, 512).map(|(cw, ch, v)| {
                    let px: Vec<u8> = v.iter().flat_map(|x| [*x, *x, *x, 255]).collect();
                    (cw, ch, px)
                })
            } else {
                rgba_of(t, 512)
            }
        })
    };
    let mut px = Vec::with_capacity(a.len() * 4);
    for y in 0..h as usize {
        for x in 0..w as usize {
            let alpha = a[y * w as usize + x];
            let c = colour.as_ref().map_or([255, 255, 255], |(cw, ch, p)| {
                let (cx, cy) = (x * *cw as usize / w as usize, y * *ch as usize / h as usize);
                let o = (cy * *cw as usize + cx) * 4;
                [p[o], p[o + 1], p[o + 2]]
            });
            px.extend_from_slice(&[c[0], c[1], c[2], alpha]);
        }
    }
    let mut image = Image::new(
        Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        px,
        TextureFormat::Rgba8Unorm,
        bevy::asset::RenderAssetUsages::RENDER_WORLD,
    );
    image.sampler = sampler();
    Some(image)
}

fn sampler() -> ImageSampler {
    ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        mag_filter: ImageFilterMode::Linear,
        min_filter: ImageFilterMode::Linear,
        mipmap_filter: ImageFilterMode::Linear,
        ..default()
    })
}

/// A texture as stored: UNORM, because effects blend on stored values.
fn stored_image(t: &Texture) -> Option<Image> {
    let format = match t.format.as_str() {
        "PF_DXT1" => TextureFormat::Bc1RgbaUnorm,
        "PF_DXT5" => TextureFormat::Bc3RgbaUnorm,
        "PF_BC7" => TextureFormat::Bc7RgbaUnorm,
        "PF_B8G8R8A8" => TextureFormat::Bgra8Unorm,
        _ => return None,
    };
    let usable: Vec<_> = t
        .mips
        .iter()
        .take_while(|m| m.width >= 4 && m.height >= 4)
        .collect();
    let first = usable.first()?;
    let mut image = Image::default();
    image.texture_descriptor.size = Extent3d {
        width: first.width,
        height: first.height,
        depth_or_array_layers: 1,
    };
    image.texture_descriptor.dimension = TextureDimension::D2;
    image.texture_descriptor.format = format;
    image.texture_descriptor.mip_level_count = usable.len() as u32;
    image.data = Some(
        usable
            .iter()
            .flat_map(|m| m.bytes.iter().copied())
            .collect(),
    );
    image.sampler = sampler();
    Some(image)
}

/// Mono values to a white texture with that alpha (their "x^2 x tint").
fn mono_image(w: u32, h: u32, v: &[u8], opaque: bool) -> Image {
    let mut px = Vec::with_capacity(v.len() * 4);
    for x in v {
        let c = if opaque { *x } else { 255 };
        px.extend_from_slice(&[c, c, c, if opaque { 255 } else { *x }]);
    }
    let mut image = Image::new(
        Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        px,
        TextureFormat::Rgba8Unorm,
        bevy::asset::RenderAssetUsages::RENDER_WORLD,
    );
    image.sampler = sampler();
    image
}

impl Kh3Fx {
    /// Takes over what the load thread read.
    fn start(&mut self, data: Arc<FxData>, images: &mut Assets<Image>) {
        let mut world = World::new();
        for def in &data.defs {
            world.register(def.clone());
        }
        let mut textures: HashMap<String, Option<Handle<Image>>> = HashMap::new();
        for (name, m) in &data.materials {
            let Some(path) = m.texture.as_ref().or(m.opacity.as_ref()) else {
                continue;
            };
            let additive = m.blend == MatBlend::Additive;
            let handle = textures
                .entry(format!("{path}|{:?}|{additive}", m.opacity))
                .or_insert_with(|| {
                    if let Some(mask) = m.opacity.as_ref().and_then(|o| data.textures.get(o)) {
                        let color = m.texture.as_ref().and_then(|c| data.textures.get(c));
                        if let Some(image) = masked_image(color, mask) {
                            return Some(images.add(image));
                        }
                    }
                    let t = data.textures.get(path)?;
                    let image = match single_channel(t, 512) {
                        Some((w, h, v)) if matches!(t.format.as_str(), "PF_BC4" | "PF_G8") => {
                            mono_image(w, h, &v, additive)
                        }
                        _ => stored_image(t)?,
                    };
                    Some(images.add(image))
                })
                .clone();
            let id = world.intern_material(name);
            self.materials.insert(id, (handle, m.blend, m.tint, m.mono));
        }
        for (name, mesh) in &data.meshes {
            let id = world.intern_mesh(name);
            self.meshes.insert(id, mesh.clone());
            if let Some(first) = mesh.materials.first().filter(|m| !m.is_empty()) {
                let material = world.intern_material(&effect_json::base_name(first));
                self.mesh_materials.insert(id, material);
            }
        }
        self.world = Some(world);
        self.data = Some(data);
        self.rng = 0x1234_5678;
        self.next_key = 1;
    }

    fn random(&mut self) -> f32 {
        let mut x = self.rng.max(1);
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.rng = x;
        (x >> 8) as f32 / (1u32 << 24) as f32
    }
}

/// `(x, y, z)` in the room to KH3's space (cm, Z up, left-handed).
fn to_kh3(p: Vec3) -> glam::Vec3 {
    let k = 1. / player::kh3::tuning::length_of(1.);
    glam::Vec3::new(p.x * k, p.z * k, p.y * k)
}

fn from_kh3(p: glam::Vec3) -> [f32; 3] {
    let k = player::kh3::tuning::length_of(1.);
    [p.x * k, p.z * k, p.y * k]
}

fn axis(v: Vec3) -> glam::Vec3 {
    glam::Vec3::new(v.x, v.z, v.y)
}

/// The Sora's pose for sockets: his place and turn in KH3's space, and his
/// bones in the mesh's.
pub(crate) struct Body<'a> {
    pub(crate) seat: u8,
    pub(crate) at: glam::Vec3,
    pub(crate) yaw: f32,
    pub(crate) component: &'a [BonePose],
}

fn quat_mat(q: [f32; 4]) -> glam::Mat3 {
    glam::Mat3::from_quat(glam::Quat::from_xyzw(q[0], q[1], q[2], q[3]))
}

/// A socket's transform in KH3's space.
fn socket_xform(data: &SoraData, body: &Body, name: &str, on_weapon: bool) -> Option<Xform> {
    let bone_pose = |bone: &str| data.bone(bone).and_then(|b| body.component.get(b)).copied();
    let local = |s: &sora::Socket| BonePose {
        rotation: sora::quat_of_rotator(s.rotation),
        translation: s.location,
        scale: s.scale,
    };
    let pose = if on_weapon {
        let grip = data.sockets.iter().find(|s| s.name == "R_wep_skt")?;
        let hand = bone_pose(&grip.bone)?;
        let root = sora::compose(&hand, &local(grip));
        let sock = data.keyblade_sockets.iter().find(|s| s.name == name);
        match sock {
            Some(s) => {
                let bones = &data.keyblade.bones;
                let at = bones.iter().position(|b| b.name == s.bone)?;
                let mut chain = BonePose {
                    rotation: bones[at].rotation,
                    translation: bones[at].translation,
                    scale: bones[at].scale,
                };
                let mut up = bones[at].parent;
                while let Ok(i) = usize::try_from(up) {
                    let b = bones.get(i)?;
                    let p = BonePose {
                        rotation: b.rotation,
                        translation: b.translation,
                        scale: b.scale,
                    };
                    chain = sora::compose(&p, &chain);
                    up = b.parent;
                }
                sora::compose(&sora::compose(&root, &chain), &local(s))
            }
            None => root,
        }
    } else if let Some(s) = data.sockets.iter().find(|s| s.name == name) {
        sora::compose(&bone_pose(&s.bone)?, &local(s))
    } else {
        bone_pose(name).unwrap_or(BonePose {
            rotation: [0., 0., 0., 1.],
            translation: [0.; 3],
            scale: [1.; 3],
        })
    };
    // His turn: the room's yaw about Y is a turn about KH3's Z the other way
    // (the swap of Y and Z flips the hand).
    let actor = glam::Mat3::from_rotation_z(-body.yaw);
    let t = glam::Vec3::from_array(pose.translation);
    Some(Xform {
        position: body.at + actor * t,
        rotation: actor * quat_mat(pose.rotation),
        scale: glam::Vec3::from_array(pose.scale),
    })
}

fn row_xform(base: &Xform, row: &EffectRow) -> Xform {
    let offset = glam::Vec3::from_array(row.location);
    let r = effect::cascade::rotator(row.rotation[0], row.rotation[1], row.rotation[2]);
    Xform {
        position: base.position + base.rotation * offset,
        rotation: base.rotation * r,
        scale: base.scale * glam::Vec3::from_array(row.scale),
    }
}

/// Starts a sound's notes.
fn play_sound(
    fx: &mut Kh3Fx,
    path: &str,
    volume: f32,
    pitch: f32,
    at: Option<glam::Vec3>,
    camera: glam::Vec3,
    key: Option<u32>,
) {
    let Some(data) = fx.data.clone() else { return };
    let Some(sound) = data.sounds.sounds.get(path) else {
        return;
    };
    let now = fx.clock;
    if sound.window > 0.
        && fx
            .last_start
            .get(path)
            .is_some_and(|t| now - *t < sound.window)
    {
        return;
    }
    fx.last_start.insert(path.to_owned(), now);
    let mut distance_gain = 1.;
    if sound.spatial
        && let Some(p) = at
    {
        let d = (p - camera).length();
        distance_gain = if d <= 400. {
            1.
        } else {
            ((4000. - d) / 3600.).max(0.)
        };
    }
    // The sequences to play: all for a Sound, one weighted group else.
    let picked: Vec<usize> = match sound.kind {
        SoundKind::Sound | SoundKind::Empty | SoundKind::Switch => {
            (0..sound.sequences.len()).collect()
        }
        SoundKind::Order => {
            let n = sound.sequences.len().max(1);
            let i = sound.cursor.fetch_add(1, Ordering::Relaxed) % n;
            vec![i]
        }
        SoundKind::Random | SoundKind::Cycle => {
            let r = fx.random() * 101.;
            let mut sum = 0.;
            let mut pick = 0;
            for (i, s) in sound.sequences.iter().enumerate() {
                sum += f32::from(s.0);
                if r <= sum {
                    pick = i;
                    break;
                }
            }
            vec![pick]
        }
    };
    let base = sound.volume * sound.asset_volume * volume * distance_gain;
    for i in picked {
        let Some((_, _, notes)) = sound.sequences.get(i) else {
            continue;
        };
        for n in notes {
            let (mut gain, mut rate) = (base * n.volume, pitch);
            if let Some((max, min, mode)) = n.volume_random {
                let v = min + fx.random() * (max - min);
                gain = if mode == 2 { gain * v } else { gain + v };
            }
            if let Some((max, min, mode)) = n.pitch_random {
                let v = min + fx.random() * (max - min);
                rate = if mode == 2 { rate * v } else { rate + v };
            }
            let key = key.unwrap_or_else(|| {
                fx.next_key = fx.next_key.wrapping_add(1) & 0xff;
                KEY_BASE | 0x8000 | fx.next_key
            });
            let mut clip = n.clip.clone();
            if !fx.holding {
                clip.loop_frames = None;
            }
            fx.pending.push(Pending {
                at: now + n.delay,
                clip,
                gain: gain.clamp(0., 1.),
                pitch: rate.clamp(1. / 512., 2.),
                key,
            });
        }
    }
}

/// Stops a mixer key on the next frame's sounds.
fn stop_key(fx: &mut Kh3Fx, key: u32) {
    fx.pending.push(Pending {
        at: fx.clock,
        clip: audio::Clip {
            channels: 1,
            rate: 48000,
            samples: Arc::from(Vec::new()),
            loop_frames: None,
        },
        gain: 0.,
        pitch: 1.,
        key,
    });
}

/// Fires a play's notifies between two times.
#[allow(clippy::too_many_arguments)]
fn replay(
    fx: &mut Kh3Fx,
    data: &SoraData,
    body: &Body,
    index: u16,
    serial: u32,
    time: f32,
    x: f32,
    camera: glam::Vec3,
) {
    let Some(clip) = data.anims.main_clip(index, x) else {
        return;
    };
    let seat = usize::from(body.seat);
    let fresh = fx.replays[seat].serial != serial || fx.replays[seat].clip != clip.name;
    if fresh {
        // The clip changed: effects that end with it loop out, held sounds fade.
        for l in &mut fx.live {
            if l.seat == body.seat && l.serial == fx.replays[seat].serial {
                let ends = l.trail || l.row.as_ref().is_some_and(|r| r.ends_with_clip || r.state);
                if ends && let Some(w) = fx.world.as_mut() {
                    w.stop(l.id, false);
                    l.row = None;
                }
            }
        }
        let r = &mut fx.replays[seat];
        r.serial = serial;
        r.clip = clip.name.clone();
        r.time = -1e-4;
        let held = std::mem::take(&mut r.held);
        for (key, _) in held {
            stop_key(fx, key);
        }
    }
    let from = fx.replays[seat].time;
    if time < from {
        // A loop wrapped.
        fx.replays[seat].time = -1e-4;
    }
    let from = fx.replays[seat].time;
    fx.replays[seat].time = time;
    let ended: Vec<u32> = fx.replays[seat]
        .held
        .iter()
        .filter(|h| h.1 <= time)
        .map(|h| h.0)
        .collect();
    fx.replays[seat].held.retain(|h| h.1 > time);
    for key in ended {
        stop_key(fx, key);
    }
    let Some(triggers) = fx
        .data
        .as_ref()
        .and_then(|d| d.triggers.get(&clip.name))
        .cloned()
    else {
        return;
    };
    for t in &triggers {
        // State ends.
        if t.end > from && t.end <= time {
            for l in &mut fx.live {
                if l.seat == body.seat
                    && l.serial == serial
                    && l.state_end == Some(t.end)
                    && let Some(w) = fx.world.as_mut()
                {
                    w.stop(l.id, false);
                    l.state_end = None;
                    l.trail = false;
                }
            }
        }
        if t.start <= from || t.start > time {
            continue;
        }
        match &t.cue {
            Cue::Effect(row_data) => {
                let Some(base) = socket_xform(data, body, &row_data.socket, row_data.on_weapon)
                else {
                    continue;
                };
                let name = effect_json::base_name(&row_data.effect).to_ascii_lowercase();
                let options = SpawnOptions {
                    seed: None,
                    color: row_data.color,
                    start_delay: row_data.delay,
                    fade_in: row_data.fade_in,
                };
                let Some(w) = fx.world.as_mut() else { continue };
                let Some(id) = w.spawn(&name, row_xform(&base, row_data), options) else {
                    continue;
                };
                fx.live.push(Live {
                    id,
                    seat: body.seat,
                    row: row_data.attach.then(|| row_data.clone()),
                    serial,
                    trail: false,
                    state_end: row_data.state.then_some(t.end),
                });
                let sounds = fx
                    .data
                    .as_ref()
                    .and_then(|d| d.effect_sounds.get(&name))
                    .cloned();
                for s in sounds.unwrap_or_default() {
                    play_sound(fx, &s, 1., 1., Some(base.position), camera, None);
                }
            }
            Cue::Trail => {
                let name = effect_json::base_name(TRAIL).to_ascii_lowercase();
                let Some(top) = socket_xform(data, body, "vfx_top", true) else {
                    continue;
                };
                let Some(w) = fx.world.as_mut() else { continue };
                if let Some(id) = w.spawn(&name, top, SpawnOptions::default()) {
                    fx.live.push(Live {
                        id,
                        seat: body.seat,
                        row: None,
                        serial,
                        trail: true,
                        state_end: Some(t.end),
                    });
                }
            }
            Cue::Sound {
                path,
                volume,
                pitch,
            } => {
                let at = socket_xform(data, body, "kubi", false).map(|x| x.position);
                play_sound(fx, path, *volume, *pitch, at, camera, None);
            }
            Cue::Voice(list) => {
                let total: u32 = list.iter().map(|v| v.1).sum();
                let mut r = (fx.random() * total as f32) as u32;
                let mut pick = list.first().map(|v| v.0.clone());
                for (p, w) in list {
                    if r < *w {
                        pick = Some(p.clone());
                        break;
                    }
                    r -= *w;
                }
                // One line at a time per Sora.
                let key = KEY_BASE | 0x100 | u32::from(body.seat);
                if let Some(p) = pick {
                    let at = socket_xform(data, body, "atama", false).map(|x| x.position);
                    stop_key(fx, key);
                    play_sound(fx, &p, 1., 1., at, camera, Some(key));
                }
            }
            Cue::Held { path, .. } => {
                let at = socket_xform(data, body, "kubi", false).map(|x| x.position);
                fx.next_key = fx.next_key.wrapping_add(1) & 0xff;
                let key = KEY_BASE | 0x4000 | fx.next_key;
                fx.holding = true;
                play_sound(fx, path, 1., 1., at, camera, Some(key));
                fx.holding = false;
                fx.replays[seat].held.push((key, t.end));
            }
        }
    }
}

/// A spell's or a shot's effect, by its package, for the room's magic.
pub(crate) fn spawn_at(
    fx: &mut Kh3Fx,
    effect: &str,
    at: [f32; 3],
    yaw: f32,
    camera: Vec3,
) -> Option<EffectId> {
    let name = effect_json::base_name(effect).to_ascii_lowercase();
    let place = to_kh3(Vec3::from_array(at));
    let world = fx.world.as_mut()?;
    let xform = Xform {
        position: place,
        rotation: glam::Mat3::from_rotation_z(-yaw)
            * glam::Mat3::from_rotation_z(std::f32::consts::FRAC_PI_2),
        scale: glam::Vec3::ONE,
    };
    let id = world.spawn(&name, xform, SpawnOptions::default());
    let sounds = fx
        .data
        .as_ref()
        .and_then(|d| d.effect_sounds.get(&name))
        .cloned();
    for s in sounds.unwrap_or_default() {
        play_sound(fx, &s, 1., 1., Some(place), to_kh3(camera), None);
    }
    id
}

pub(crate) fn play_at(fx: &mut Kh3Fx, sound: &str, at: [f32; 3], camera: Vec3) {
    play_sound(
        fx,
        sound,
        1.,
        1.,
        Some(to_kh3(Vec3::from_array(at))),
        to_kh3(camera),
        None,
    );
}

/// The requests a Sora's layer made this tick.
pub(crate) fn requests(fx: &mut Kh3Fx, list: Vec<player::kh3::spell::FxRequest>, camera: Vec3) {
    for r in list {
        if let Some(e) = &r.effect {
            spawn_at(fx, e, r.at, r.yaw, camera);
        }
        if let Some(s) = &r.sound {
            play_at(fx, s, r.at, camera);
        }
    }
}

/// The spell actors' body effects: one per live actor, following it.
pub(crate) fn actors_step(
    fx: &mut Kh3Fx,
    seat: u8,
    actors: &[player::kh3::spell::Actor],
    bolts: &[player::kh3::Bolt],
    book: &player::kh3::attack::Book,
) {
    let Some(world) = fx.world.as_mut() else {
        return;
    };
    // A shot is an actor with the shot's stage.
    let shots: Vec<player::kh3::spell::Actor> = bolts
        .iter()
        .filter_map(|b| {
            let stage = book.shots[usize::from(b.ragnarok)].clone()?;
            Some(player::kh3::spell::Actor {
                serial: b.serial | 0x8000,
                cmd: 0,
                speed: 0.,
                stage,
                pos: b.pos,
                dir: b.dir,
                age: 0.,
                turn: 0.,
                homing_on: false,
                finish: false,
                grand: false,
                hit: false,
                previous: b.pos,
                dead: false,
            })
        })
        .collect();
    let all: Vec<&player::kh3::spell::Actor> = actors.iter().chain(&shots).collect();
    let live = &mut fx.bodies;
    live.retain(|(s, serial, id)| {
        if *s != seat {
            return true;
        }
        let here = all.iter().any(|a| a.serial == *serial);
        if !here {
            world.stop(*id, false);
        }
        here
    });
    for a in all {
        let place = to_kh3(Vec3::from_array(a.pos));
        let forward = glam::Vec3::new(a.dir[0], a.dir[2], a.dir[1]).normalize_or(glam::Vec3::X);
        let yaw = forward.y.atan2(forward.x);
        let pitch = forward.z.clamp(-1., 1.).asin();
        let rotation = effect::cascade::rotator(pitch.to_degrees(), yaw.to_degrees(), 0.);
        let xform = Xform {
            position: place,
            rotation,
            scale: glam::Vec3::ONE,
        };
        if let Some(entry) = live.iter().find(|e| e.0 == seat && e.1 == a.serial) {
            world.set_transform(entry.2, xform);
        } else if let Some(body) = &a.stage.body_effect {
            let name = effect_json::base_name(body).to_ascii_lowercase();
            if let Some(id) = world.spawn(&name, xform, SpawnOptions::default()) {
                live.push((seat, a.serial, id));
            }
        }
    }
}

/// One Sora's step of the effects, from the model's update.
#[allow(clippy::too_many_arguments)]
pub(crate) fn body_step(
    fx: &mut Kh3Fx,
    data: &SoraData,
    seat: u8,
    position: [f32; 3],
    yaw: f32,
    component: &[BonePose],
    visual: &player::kh3::visual::Visual,
    row: u16,
    time_in: f32,
    camera: Vec3,
) {
    if fx.world.is_none() {
        return;
    }
    let body = Body {
        seat,
        at: to_kh3(Vec3::from_array(position)),
        yaw,
        component,
    };
    replay(
        fx,
        data,
        &body,
        row,
        visual.serial,
        time_in,
        visual.at[0],
        to_kh3(camera),
    );
    footsteps(fx, data, &body, visual.row, camera);
    // Attached effects follow their sockets; trails take the blade's edge.
    let mut live = std::mem::take(&mut fx.live);
    for l in &live {
        if l.seat != seat {
            continue;
        }
        let Some(w) = fx.world.as_mut() else { break };
        if l.trail {
            if let (Some(top), Some(bottom)) = (
                socket_xform(data, &body, "vfx_top", true),
                socket_xform(data, &body, "vfx_bottom", true),
            ) {
                w.set_transform(l.id, top);
                w.set_trail(l.id, top.position, bottom.position);
            }
        } else if let Some(row) = &l.row
            && let Some(base) = socket_xform(data, &body, &row.socket, row.on_weapon)
        {
            w.set_transform(l.id, row_xform(&base, row));
        }
    }
    if let Some(w) = fx.world.as_ref() {
        live.retain(|l| w.is_alive(l.id));
    }
    fx.live = live;
}

/// `SQEXSEADAutoSeComponent`'s footsteps: a step as a toe stops coming down.
/// [approximate: the component's own detection was not read; the bones and
/// the bank are its.]
fn footsteps(fx: &mut Kh3Fx, data: &SoraData, body: &Body, row: &str, camera: Vec3) {
    let sound = match row {
        "WalkLoopB" => Some(FOOT_WALK),
        "RunLoopB" => Some(FOOT_RUN),
        _ => None,
    };
    let Some(seat) = fx.feet.get(usize::from(body.seat)).copied() else {
        return;
    };
    let mut seat = seat;
    for (foot, name) in seat.iter_mut().zip(TOES) {
        let Some(height) = data
            .bone(name)
            .and_then(|i| body.component.get(i))
            .map(|b| b.translation[2])
        else {
            continue;
        };
        let falling = height < foot.0 - 0.01;
        if foot.1
            && !falling
            && height < STEP_HEIGHT
            && fx.clock - fx.last_step[usize::from(body.seat)] >= STEP_GAP
            && let Some(sound) = sound
        {
            fx.last_step[usize::from(body.seat)] = fx.clock;
            play_sound(fx, sound, 1., 1., Some(body.at), to_kh3(camera), None);
        }
        *foot = (height, falling);
    }
    fx.feet[usize::from(body.seat)] = seat;
}

/// Steps the evaluator at 60 Hz, starts sounds due and draws.
#[allow(clippy::too_many_arguments)]
pub(crate) fn frame(
    fx: &mut Kh3Fx,
    dt: f32,
    camera: Option<(Vec3, Vec3, Vec3, Vec3)>,
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    effect_materials: &mut Assets<EffectMaterial>,
    visibilities: &mut Query<&mut Visibility>,
    mixer: Option<&mut audio::Mixer>,
) {
    let Some(mut world) = fx.world.take() else {
        return;
    };
    fx.clock += dt;
    fx.accumulator += dt;
    let mut steps = 0;
    while fx.accumulator >= 1. / 60. && steps < 4 {
        world.step(1. / 60.);
        fx.accumulator -= 1. / 60.;
        steps += 1;
    }
    if steps == 4 {
        fx.accumulator = 0.;
    }
    if let Some(mixer) = mixer {
        let clock = fx.clock;
        let mut i = 0;
        while i < fx.pending.len() {
            if fx.pending[i].at <= clock {
                let p = fx.pending.swap_remove(i);
                if p.clip.samples.is_empty() {
                    mixer.stop(p.key);
                } else {
                    mixer.play(p.clip, p.key, p.gain, p.pitch);
                }
            } else {
                i += 1;
            }
        }
    }
    if let Some((position, forward, right, up)) = camera {
        let camera = Camera {
            position: to_kh3(position),
            forward: axis(forward),
            right: axis(right),
            up: axis(up),
        };
        let mut list = std::mem::take(&mut fx.draw);
        list.clear();
        world.draw(fx.accumulator * 60., &camera, &mut list);
        draw_list(fx, &list, commands, meshes, effect_materials, visibilities);
        fx.draw = list;
    }
    fx.world = Some(world);
}

#[derive(Default)]
struct Verts {
    positions: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
    colors: Vec<[f32; 4]>,
    indices: Vec<u32>,
}

impl Verts {
    fn push(&mut self, p: glam::Vec3, uv: [f32; 2], c: [f32; 4]) -> u32 {
        let i = self.positions.len() as u32;
        self.positions.push(from_kh3(p));
        self.uvs.push(uv);
        self.colors.push(c);
        i
    }
}

fn gamma(c: [f32; 4], tint: [f32; 3]) -> [f32; 4] {
    let g = |x: f32, t: f32| (x * t).max(0.).powf(1. / 2.2);
    [
        g(c[0], tint[0]),
        g(c[1], tint[1]),
        g(c[2], tint[2]),
        c[3].clamp(0., 1.),
    ]
}

/// The pass of a draw: 0 translucent, 1 additive.
fn pass(mode: Blend, material: MatBlend) -> u8 {
    match mode {
        Blend::Additive => 1,
        Blend::Translucent => 0,
        Blend::Material => u8::from(material == MatBlend::Additive),
    }
}

fn draw_list(
    fx: &mut Kh3Fx,
    list: &DrawList,
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    effect_materials: &mut Assets<EffectMaterial>,
    visibilities: &mut Query<&mut Visibility>,
) {
    let mut batches: HashMap<(u32, u8), Verts> = HashMap::new();
    let material = |fx: &Kh3Fx, id: u32| {
        fx.materials
            .get(&id)
            .map_or((MatBlend::Additive, [1.; 3]), |m| (m.1, m.2))
    };
    for q in &list.quads {
        if !fx.materials.contains_key(&q.material) {
            continue;
        }
        let (blend, tint) = material(fx, q.material);
        let v = batches
            .entry((q.material, pass(q.mode, blend)))
            .or_default();
        let c = gamma(q.color, tint);
        let uv = q.uv;
        let a = v.push(q.corners[0], [uv[0], uv[1]], c);
        let b = v.push(q.corners[1], [uv[2], uv[1]], c);
        let d = v.push(q.corners[2], [uv[2], uv[3]], c);
        let e = v.push(q.corners[3], [uv[0], uv[3]], c);
        v.indices.extend_from_slice(&[a, b, d, a, d, e]);
    }
    for r in &list.ribbons {
        if !fx.materials.contains_key(&r.material) {
            continue;
        }
        let (blend, tint) = material(fx, r.material);
        let v = batches
            .entry((r.material, pass(r.blend, blend)))
            .or_default();
        let mut last: Option<(u32, u32)> = None;
        for p in &r.points {
            let c = gamma(p.color, tint);
            let (u, top, bottom) = if r.uv_turned {
                (r.v_scale, [0., p.u * r.tiles], [r.v_scale, p.u * r.tiles])
            } else {
                (r.v_scale, [p.u * r.tiles, 0.], [p.u * r.tiles, r.v_scale])
            };
            let _ = u;
            let a = v.push(p.a, top, c);
            let b = v.push(p.b, bottom, c);
            if let Some((la, lb)) = last {
                v.indices.extend_from_slice(&[la, lb, a, a, lb, b]);
            }
            last = Some((a, b));
        }
    }
    let triangles: usize = list
        .meshes
        .iter()
        .filter_map(|m| fx.meshes.get(&m.mesh))
        .map(|m| m.indices.len() / 3)
        .sum();
    let stride = triangles / MESH_BUDGET + 1;
    for m in list.meshes.iter().step_by(stride) {
        let id = if fx.materials.contains_key(&m.material) {
            m.material
        } else {
            match fx
                .mesh_materials
                .get(&m.mesh)
                .filter(|i| fx.materials.contains_key(i))
            {
                Some(i) => *i,
                None => continue,
            }
        };
        let (blend, tint) = material(fx, id);
        let Some(mesh) = fx.meshes.get(&m.mesh) else {
            continue;
        };
        let v = batches.entry((id, pass(m.blend, blend))).or_default();
        let c = gamma(m.color, tint);
        let base = v.positions.len() as u32;
        let (flip_u, flip_v) = (m.uv_mode as i32 & 4 != 0, m.uv_mode as i32 & 8 != 0);
        for (i, p) in mesh.positions.iter().enumerate() {
            let world = m.transform.transform_point3(glam::Vec3::from_array(*p));
            let mut uv = mesh.uv.get(i).copied().unwrap_or([0.; 2]);
            if flip_u {
                uv[0] = 1. - uv[0];
            }
            if flip_v {
                uv[1] = 1. - uv[1];
            }
            let tone = mesh.colors.get(i).map_or(1., |x| f32::from(x[3]) / 255.);
            v.push(world, uv, [c[0], c[1], c[2], c[3] * tone]);
        }
        v.indices.extend(mesh.indices.iter().map(|i| i + base));
    }
    let mut shown = HashSet::new();
    for (key, verts) in batches {
        if verts.indices.is_empty() {
            continue;
        }
        shown.insert(key);
        let batch = fx.batches.entry(key).or_insert_with(|| {
            let mesh = meshes.add(Mesh::new(
                bevy::mesh::PrimitiveTopology::TriangleList,
                bevy::asset::RenderAssetUsages::RENDER_WORLD
                    | bevy::asset::RenderAssetUsages::MAIN_WORLD,
            ));
            let texture = fx.materials.get(&key.0).and_then(|m| m.0.clone());
            let blend = if key.1 == 1 {
                DrawBlend::Add
            } else {
                DrawBlend::Alpha
            };
            let material = effect_materials.add(EffectMaterial::new(blend, texture, 0., true));
            fx.effect_materials.insert(key, material.clone());
            let entity = commands
                .spawn((
                    Kh3FxPart,
                    Mesh3d(mesh.clone()),
                    MeshMaterial3d(material),
                    Transform::default(),
                    Visibility::Visible,
                    bevy::camera::visibility::NoFrustumCulling,
                ))
                .id();
            Batch { entity, mesh }
        });
        if let Some(mut mesh) = meshes.get_mut(&batch.mesh) {
            mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, verts.positions);
            mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, verts.uvs);
            mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, verts.colors);
            mesh.insert_indices(bevy::mesh::Indices::U32(verts.indices));
        }
        if let Ok(mut vis) = visibilities.get_mut(batch.entity) {
            *vis = Visibility::Visible;
        }
    }
    for (key, batch) in &fx.batches {
        if !shown.contains(key)
            && let Ok(mut vis) = visibilities.get_mut(batch.entity)
        {
            *vis = Visibility::Hidden;
        }
    }
}

/// Hooks up what the load thread read.
pub(crate) fn begin(fx: &mut Kh3Fx, data: Arc<FxData>, images: &mut Assets<Image>) {
    fx.start(data, images);
}
