//! What the simulation reads from KH3's data [T282]: each attack clip's windows,
//! root track and blade points, the attack data table, the hit shapes and level
//! 99's stats, read from the player's paks into `player::kh3::attack::Book`.
//! (The player crate has no dependency on the readers.)

use data::kh3::anim::BonePose;
use data::kh3::package::Package;
use data::kh3::props::{self, Props};
use data::kh3::sora::{self, Clip, SoraData};
use data::kh3::{Paks, Result};
use player::kh3::attack::{AttackRow, Book, ClipInfo, HitWindow, Shape};
use player::kh3::spell::{ActorShape, Homing, Respawn, Spell, Stage};
use player::kh3::visual;
use std::collections::HashMap;
use std::sync::Arc;

const ATTACK_DATA: &str = "/Game/DataTable/Player/Attack/p_ex001_AttackData";
const LEVEL_UP: &str = "/Game/DataTable/Player/LevelUp/p_ex001_LevelUpData";
const BASE_PARAM: &str = "/Game/DataTable/Player/Base/p_ex001_BaseParamData";
const KEYBLADE_BASE: &str = "/Game/Blueprints/Player/p_ex001/w_so_KeybladeBase";
const EQUIP_DATA: &str = "/Game/Load/Tres/TresEquipItemData";

/// Rows the layer plays besides the attack kinds' (spells, shotlock).
const OTHER_ROWS: [&str; 24] = [
    "MagicFire",
    "MagicFireMF",
    "MagicFireMR",
    "MagicFireML",
    "MagicAirFire",
    "MagicAirFireMF",
    "MagicAirFireMR",
    "MagicAirFireML",
    "MagicShoot",
    "MagicAirShoot",
    "MagicCast",
    "MagicCastM",
    "MagicAirCast",
    "MagicAirCastM",
    "MagicChant",
    "MagicAirChant",
    "M_1473",
    "M_1474",
    "M_1481",
    "M_1482",
    "M_1483",
    "ShootFlowEnd",
    "ShootFlowStart",
    "ChaseAttack",
];

/// Mesh axes to the actor's: forward = mesh Y, right = -mesh X, up = mesh Z.
fn actor(p: [f32; 3]) -> [f32; 3] {
    [p[1], -p[0], p[2]]
}

/// Blade hit points through the pose: hand, its socket, the blade's bone, its
/// socket.
struct Blade {
    hand: usize,
    grip: BonePose,
    bone: BonePose,
    atk: [BonePose; 2],
}

fn socket_pose(s: &sora::Socket) -> BonePose {
    BonePose {
        rotation: sora::quat_of_rotator(s.rotation),
        translation: s.location,
        scale: s.scale,
    }
}

fn blade(data: &SoraData) -> Option<Blade> {
    let grip = data.sockets.iter().find(|s| s.name == "R_wep_skt")?;
    let hand = data.bone(&grip.bone)?;
    let sock = |name: &str| data.keyblade_sockets.iter().find(|s| s.name == name);
    let (a, b) = (sock("ATK_00")?, sock("ATK_01")?);
    let wep = data.keyblade.bones.iter().position(|x| x.name == a.bone)?;
    let local: Vec<BonePose> = data
        .keyblade
        .bones
        .iter()
        .map(|x| BonePose {
            rotation: x.rotation,
            translation: x.translation,
            scale: x.scale,
        })
        .collect();
    let mut comp = Vec::new();
    sora::to_component(&data.keyblade.bones, &local, &mut comp);
    Some(Blade {
        hand,
        grip: socket_pose(grip),
        bone: *comp.get(wep)?,
        atk: [socket_pose(a), socket_pose(b)],
    })
}

fn string(p: &Props, name: &str) -> String {
    p.str(name).unwrap_or("").to_owned()
}

/// A clip's windows and tracks.
fn clip_info(data: &SoraData, clip: &Clip, blade: Option<&Blade>) -> ClipInfo {
    let mut info = ClipInfo {
        length: clip.seq.length,
        root_motion: clip.root_motion,
        ..ClipInfo::default()
    };
    for n in &clip.notifies {
        match n.class.as_str() {
            "TresAnimNotifyState_Attack" => info.windows.push(HitWindow {
                from: n.start,
                to: n.end,
                group: string(&n.props, "CollisionGrpName"),
                row: string(&n.props, "m_AttackDataIDName"),
                interval: n.props.f32("AttackInterval").unwrap_or(0.),
                hit_se: n
                    .props
                    .str("m_WeaponHitSEAliasID")
                    .and_then(|h| h.rsplit("HIT").next())
                    .and_then(|h| h.parse().ok())
                    .unwrap_or(1),
            }),
            "TresAnimNotifyState_Traction" => info.traction.push((n.start, n.end)),
            "TresAnimNotify_EnableInput" => {
                info.enable_input.get_or_insert(n.start);
            }
            "TresAnimNotify_EnableChange" => {
                info.enable_change.get_or_insert(n.start);
            }
            "TresAnimNotify_FaceAt" => info.face_at.push(n.start),
            "TresAnimNotify_Shot" => info.shots.push(n.start),
            _ => {}
        }
    }
    let frames = clip.seq.num_frames.max(1) as usize;
    let at = |f: usize| (f as f32 / 30.).min(clip.seq.length);
    if clip.root_motion {
        info.root = (0..frames)
            .map(|f| {
                let (p, yaw) = sora::root_at(clip, at(f));
                let a = actor(p);
                [a[0], a[1], a[2], yaw]
            })
            .collect();
    }
    if let (Some(b), false) = (blade, info.windows.is_empty()) {
        let mut pose = Vec::new();
        let mut comp = Vec::new();
        for f in 0..frames {
            sora::sample_clip(clip, at(f), &data.reference, &mut pose);
            sora::to_component(&data.mesh.bones, &pose, &mut comp);
            let Some(hand) = comp.get(b.hand) else {
                continue;
            };
            let root = sora::compose(&sora::compose(hand, &b.grip), &b.bone);
            let point = |s: &BonePose| actor(sora::compose(&root, s).translation);
            info.atk.push([point(&b.atk[0]), point(&b.atk[1])]);
        }
    }
    info
}

fn bare_enum(v: &str) -> String {
    let tail = v.rsplit("::").next().unwrap_or(v);
    tail.strip_prefix("TRES_DMG_KIND_")
        .or_else(|| tail.strip_prefix("TRES_DMG_ATTR_"))
        .unwrap_or(tail)
        .to_owned()
}

fn attack_rows(paks: &Paks, book: &mut Book) -> Result<()> {
    let Some(p) = paks.game_package(ATTACK_DATA)? else {
        return Ok(());
    };
    let Some(i) = p.export_of_class("DataTable") else {
        return Ok(());
    };
    for (name, row) in props::data_table_rows(&p, i)? {
        book.rows.insert(
            name.to_ascii_lowercase(),
            AttackRow {
                power: row.f32("m_Power").unwrap_or(0.),
                damage_kind: bare_enum(row.str("m_DamageKind").unwrap_or("")),
                attribute: bare_enum(row.str("m_DamageAttribute").unwrap_or("")),
                power_scale: row.f32("m_DamagePowerScale").unwrap_or(1.),
                reflect_level: row.int("m_ReflectLevel").unwrap_or(0) as i32,
                form_point: row.int("m_FormPoint").unwrap_or(0) as f32,
                magic: row.bool("m_bIsMagicAttack").unwrap_or(false),
                ignore_guard: row.bool("m_bIsIvalidGuard").unwrap_or(false),
                counter: row.bool("m_bIsCounterAttack").unwrap_or(false),
                damage_min: row.f32("m_DamageMin").unwrap_or(0.),
            },
        );
    }
    Ok(())
}

fn vector(p: &Props, name: &str) -> [f32; 3] {
    p.floats(name)
        .filter(|v| v.len() == 3)
        .map_or([0.; 3], |v| [v[0], v[1], v[2]])
}

fn shapes(paks: &Paks, book: &mut Book) -> Result<()> {
    let Some(p) = paks.game_package(KEYBLADE_BASE)? else {
        return Ok(());
    };
    let Some(i) = p.export_named("TresAtkColl0") else {
        return Ok(());
    };
    let (coll, _) = props::export_props(&p, i)?;
    for unit in coll.array("CollisionShapesSrc").unwrap_or(&[]) {
        let Some(u) = unit.as_props() else { continue };
        let kind = u.str("m_ShapeType").unwrap_or("");
        let size = vector(u, "m_Size");
        let socket = u.str("m_SocketName1").unwrap_or("");
        let capsule2 = kind.ends_with("CAPSULE2");
        let capsule1 = kind.ends_with("CAPSULE1");
        book.shapes.insert(
            string(u, "m_GrpName"),
            Shape {
                capsule: capsule2,
                upright: capsule1,
                radius: size[0],
                half_length: size[2],
                grow: vector(u, "m_IncSize")[0],
                grow_max: vector(u, "m_IncMaxSize")[0],
                frozen: u.bool("m_bDisableLocation1Attach").unwrap_or(false),
                from_owner: !socket.starts_with("ATK_"),
                offset: vector(u, "m_RelativeLocation1"),
            },
        );
    }
    Ok(())
}

fn table_row(paks: &Paks, path: &str, row: &str) -> Result<Option<Props>> {
    let Some(p) = paks.game_package(path)? else {
        return Ok(None);
    };
    let Some(i) = p.export_of_class("DataTable") else {
        return Ok(None);
    };
    Ok(props::data_table_rows(&p, i)?
        .into_iter()
        .find(|r| r.0 == row)
        .map(|r| r.1))
}

/// Level 99's strength, magic, defense with the Kingdom Key, and HP.
fn stats(paks: &Paks, book: &mut Book) -> Result<()> {
    let mut s = [62., 64., 59., 120.];
    if let Some(row) = table_row(paks, LEVEL_UP, "99")? {
        for (k, name) in ["m_AttackPower", "m_MagicPower", "m_DefensePower"]
            .iter()
            .enumerate()
        {
            if let Some(v) = row.int(name) {
                s[k] = v as f32;
            }
        }
    }
    if let Some(row) = table_row(paks, BASE_PARAM, "Default")? {
        for name in ["m_MaxHitPoint", "m_MaxHP", "m_HP", "m_HitPoint"] {
            if let Some(v) = row.int(name).or_else(|| row.f32(name).map(|f| f as i64)) {
                s[3] = v as f32;
                break;
            }
        }
    }
    if let Some(p) = paks.game_package(EQUIP_DATA)?
        && let Some(i) = p.export_of_class("DataTable")
    {
        for (_, row) in props::data_table_rows(&p, i)? {
            if row
                .str("m_KeyName")
                .is_some_and(|k| k.ends_with("WEP_KEYBLADE_SO_00"))
            {
                s[0] += row.int("m_AttackPlus").unwrap_or(0) as f32;
                s[1] += row.int("m_MagicPlus").unwrap_or(0) as f32;
                break;
            }
        }
    }
    book.stats = s;
    Ok(())
}

/// A blueprint chain, child first: each level's package and class object.
fn chain(paks: &Paks, path: &str) -> Vec<Package> {
    let mut out = Vec::new();
    let mut at = path.to_owned();
    for _ in 0..8 {
        let package = at.split('.').next().unwrap_or(&at).to_owned();
        let Ok(Some(p)) = paks.game_package(&package) else {
            break;
        };
        let class = p
            .exports
            .iter()
            .position(|e| e.name.ends_with("_C") && !e.name.starts_with("Default__"));
        let parent = class
            .map(|c| p.exports[c].sup)
            .filter(|s| *s < 0)
            .and_then(|s| p.import_package(s).map(str::to_owned))
            .filter(|n| n.starts_with("/Game/"));
        out.push(p);
        match parent {
            Some(next) => at = next,
            None => break,
        }
    }
    out
}

/// A property through the chain: the child's value, or a parent's.
fn through<T>(
    levels: &[Package],
    export: &str,
    get: impl Fn(&Package, &Props) -> Option<T>,
) -> Option<T> {
    for p in levels {
        let index = if export == "Default" {
            p.exports
                .iter()
                .position(|e| e.name.starts_with("Default__"))
        } else {
            p.export_named(export)
        };
        let Some(i) = index else { continue };
        let Ok((props, _)) = props::export_props(p, i) else {
            continue;
        };
        if let Some(v) = get(p, &props) {
            return Some(v);
        }
    }
    None
}

fn object_path(p: &Package, o: i32) -> Option<String> {
    (o < 0).then(|| p.import_package(o).map(str::to_owned))?
}

fn stage_of(
    paks: &Paks,
    path: &str,
    depth: u8,
    cache: &mut HashMap<String, Arc<Stage>>,
) -> Option<Arc<Stage>> {
    let key = path.split('.').next().unwrap_or(path).to_ascii_lowercase();
    if let Some(s) = cache.get(&key) {
        return Some(Arc::clone(s));
    }
    let levels = chain(paks, path);
    if levels.is_empty() || depth > 4 {
        return None;
    }
    let f = |e: &str, k: &str| through(&levels, e, |_, q| q.f32(k));
    let name = path
        .rsplit('/')
        .next()
        .unwrap_or(path)
        .split('.')
        .next()
        .unwrap_or(path)
        .to_owned();
    let mut stage = Stage {
        life: f("Default", "m_MainLife").unwrap_or(5.),
        speed: f("ProjectileComp", "InitialSpeed").unwrap_or(2000.),
        max_speed: f("ProjectileComp", "MaxSpeed").unwrap_or(2000.),
        accel: f("ProjectileComp", "m_Acceleration").unwrap_or(0.),
        move_delay: f("ProjectileComp", "m_fMoveStartDelay").unwrap_or(0.),
        map_stop: f("ProjectileComp", "m_CheckMapHitNormal").unwrap_or(0.),
        cure: f("Default", "m_AppendCure"),
        survives_hit: through(&levels, "Default", |_, q| q.bool("m_bNoDestroyOnHit"))
            .unwrap_or(false),
        barrier: name.contains("Water") && name.ends_with("Nml")
            || name.contains("Watara") && name.ends_with("Nml")
            || name.contains("Wataga") && name.ends_with("Nml")
            || name.contains("Watagan") && name.ends_with("Nml"),
        name,
        ..Stage::default()
    };
    if through(&levels, "ProjectileComp", |_, q| q.bool("m_bEnableHoming")).unwrap_or(false) {
        stage.homing = Some(Homing {
            delay: f("ProjectileComp", "m_HomingStartDelay").unwrap_or(0.),
            turn: f("ProjectileComp", "m_HomingTurnMax").unwrap_or(0.),
            accel: f("ProjectileComp", "m_HomingTurnAccel").unwrap_or(0.),
            cap: f("ProjectileComp", "m_HomingMaxTurnMax").unwrap_or(0.),
            min_dist: f("ProjectileComp", "m_HomingMinDist").unwrap_or(0.),
        });
    }
    // Shapes: the child's list, if it has one.
    stage.shapes = through(&levels, "TresAtkColl0", |_, q| {
        let list = q.array("CollisionShapesSrc")?;
        Some(
            list.iter()
                .filter_map(|u| u.as_props())
                .map(|u| {
                    let (size, inc, top) = (
                        vector(u, "m_Size"),
                        vector(u, "m_IncSize"),
                        vector(u, "m_IncMaxSize"),
                    );
                    let scale = u
                        .floats("m_Scale")
                        .filter(|v| v.len() == 3)
                        .map_or([1.; 3], |v| [v[0], v[1], v[2]]);
                    let inc_scale = vector(u, "m_IncScale");
                    let top_scale = vector(u, "m_IncMaxScale");
                    let convex = u.str("m_ShapeType").is_some_and(|t| t.ends_with("CONVEX"));
                    let (radius, grow, grow_max, half, half_grow, half_max) = if convex {
                        (
                            100. * scale[0],
                            100. * inc_scale[0],
                            100. * top_scale[0],
                            50. * scale[2],
                            50. * inc_scale[2],
                            50. * top_scale[2],
                        )
                    } else {
                        (size[0], inc[0], top[0], size[2], inc[2], top[2])
                    };
                    ActorShape {
                        row: string(u, "m_DefaultAttackDataIDName"),
                        radius,
                        grow,
                        grow_max,
                        half,
                        half_grow,
                        half_max,
                        centre_z: vector(u, "m_RelativeLocation1")[2],
                        burst: None,
                    }
                })
                .collect::<Vec<_>>(),
        )
        .filter(|l: &Vec<ActorShape>| !l.is_empty())
    })
    .unwrap_or_default();
    stage.row_finish = through(&levels, "Default", |_, q| {
        q.str("m_OverrideFinishAttackDataID")
            .filter(|s| *s != "None")
            .map(str::to_owned)
    });
    stage.row_grand = through(&levels, "Default", |_, q| {
        q.str("m_OverrideSituationAttackDataID")
            .filter(|s| *s != "None")
            .map(str::to_owned)
    });
    stage.body_effect = through(&levels, "ParticleComp", |p, q| {
        q.object("Template").and_then(|o| object_path(p, o))
    });
    stage.fire_effects = through(&levels, "Default", |p, q| {
        let list = q.array("MyFireEffects")?;
        Some(
            list.iter()
                .filter_map(|u| u.as_props())
                .map(|u| {
                    (
                        u.object("MyParticleSystem").and_then(|o| object_path(p, o)),
                        u.object("MySoundCue").and_then(|o| object_path(p, o)),
                    )
                })
                .collect::<Vec<_>>(),
        )
    })
    .unwrap_or_default();
    let hit = |key: &'static str| {
        through(&levels, "TresAtkColl0", move |p, q| {
            let list = q.array("m_HitEffects")?;
            let first = list.first()?.as_props()?;
            first.object(key).and_then(|o| object_path(p, o))
        })
    };
    stage.hit_effect = hit("m_PawnHitEffect");
    stage.map_hit_effect = hit("m_MapHitEffect");
    stage.hit_sound = hit("m_PawnHitSEAsset");
    stage.map_hit_sound = hit("m_MapHitSEAsset");
    if through(&levels, "Default", |_, q| {
        q.bool("m_bReSpawnProjectileOnTimeOut")
    })
    .unwrap_or(false)
    {
        stage.respawn = match through(&levels, "Default", |_, q| {
            q.str("m_RespawnCheckType").map(str::to_owned)
        })
        .unwrap_or_default()
        .rsplit("TYPE_")
        .next()
        .unwrap_or("")
        {
            "HIT_PAWN" => Respawn::HitPawn,
            "HIT_ANY" => Respawn::HitAny,
            "HIT_MAP" => Respawn::HitMap,
            "NO_HIT" => Respawn::NoHit,
            _ => Respawn::HitAlways,
        };
        let next = through(&levels, "Default", |p, q| {
            q.object("m_ReSpawnProjectileClass")
                .and_then(|o| object_path(p, o))
        });
        stage.next = next.and_then(|n| stage_of(paks, &n, depth + 1, cache));
    }
    // The Firaga and Blizzaga families' point-blank burst: 250 cm shrinking over .07 s.
    if stage.move_delay > 0.
        && let Some(s) = stage.shapes.first_mut()
    {
        s.burst = Some((250., stage.move_delay));
    }
    let arc = Arc::new(stage);
    cache.insert(key, Arc::clone(&arc));
    Some(arc)
}

/// The cast effect of an element (`ead_wep_cal0` by group).
fn cast_effect(attr: i64) -> Option<String> {
    let (folder, name) = match attr {
        1 => ("fire", "vm_fir_cal0"),
        2 => ("bliz", "vm_blz_cal0"),
        3 => ("thd", "vm_thd_cal0"),
        4 => ("water", "vm_wat_cal0"),
        5 => ("air", "vm_air_cal0"),
        7 => ("cure", "vm_cur_cal0"),
        _ => return None,
    };
    Some(format!("/Game/Effects/magic/{folder}/par/{name}"))
}

/// `p_ex001_MagicSet` and `TresCmdData`: each spell's blueprint and numbers.
fn spells(paks: &Paks, book: &mut Book) -> Result<()> {
    const MAGIC_SET: &str = "/Game/Blueprints/Player/p_ex001/p_ex001_MagicSet";
    const CMD_DATA: &str = "/Game/Load/Common/TresCmdData";
    let Some(set) = paks.game_package(MAGIC_SET)? else {
        return Ok(());
    };
    let Some(i) = set.exports.iter().position(|e| e.is_asset) else {
        return Ok(());
    };
    let (props, _) = props::export_props(&set, i)?;
    let commands = paks.game_package(CMD_DATA)?;
    let rows: Vec<(String, Props)> = match &commands {
        Some(p) => match p.export_of_class("DataTable") {
            Some(t) => props::data_table_rows(p, t)?,
            None => Vec::new(),
        },
        None => Vec::new(),
    };
    let mut cache = HashMap::new();
    for unit in props.array("m_Units").unwrap_or(&[]) {
        let Some(u) = unit.as_props() else { continue };
        let Some(cmd) = u.int("m_Command").or_else(|| {
            u.str("m_Command")
                .and_then(|s| s.rsplit("::").next())
                .and_then(command_id)
        }) else {
            continue;
        };
        let cmd = cmd as u16;
        let Some(path) = u.object("m_AssetData00").and_then(|o| object_path(&set, o)) else {
            continue;
        };
        let Some(stage) = stage_of(paks, &path, 0, &mut cache) else {
            continue;
        };
        let row = rows.get(usize::from(cmd)).map(|r| &r.1);
        let attr = row.and_then(|r| r.int("Attr")).unwrap_or(0);
        book.spells.insert(
            cmd,
            Spell {
                cmd,
                mp: row.and_then(|r| r.int("Mp")).unwrap_or(0) as i32,
                attr: attr as u8,
                cast_type: u.bool("m_bIsCastType").unwrap_or(false),
                direct: u.bool("m_bIsDirectPos").unwrap_or(false),
                grand: (110..=125).contains(&cmd),
                stage,
                cast_effect: cast_effect(attr),
            },
        );
    }
    Ok(())
}

/// `ETresCommandKind` names of the magic commands.
fn command_id(name: &str) -> Option<i64> {
    const NAMES: [(&str, i64); 34] = [
        ("FIRE", 29),
        ("FIRA", 30),
        ("FIRAGA", 31),
        ("BLIZZARD", 33),
        ("BLIZZARA", 34),
        ("BLIZZAGA", 35),
        ("THUNDER", 37),
        ("THUNDARA", 38),
        ("THUNDAGA", 39),
        ("WATER", 41),
        ("WATARA", 42),
        ("WATAGA", 43),
        ("AERO", 45),
        ("AERORA", 46),
        ("AEROGA", 47),
        ("CURE", 49),
        ("CURA", 50),
        ("CURAGA", 51),
        ("S_FIRA", 110),
        ("S_FIRAGA", 111),
        ("S_FIRAGAN", 112),
        ("S_BLIZZARA", 113),
        ("S_BLIZZAGA", 114),
        ("S_BLIZZAGAN", 115),
        ("S_THUNDARA", 116),
        ("S_THUNDAGA", 117),
        ("S_THUNDAGAN", 118),
        ("S_WATARA", 119),
        ("S_WATAGA", 120),
        ("S_WATAGAN", 121),
        ("S_AERORA", 122),
        ("S_AEROGA", 123),
        ("S_AEROGAN", 124),
        ("S_CURAGAN", 125),
    ];
    NAMES.iter().find(|n| n.0 == name).map(|n| n.1)
}

/// Everything a `FxRequest` may name, sorted.
fn fx_names(book: &Book) -> Vec<String> {
    fn stage(s: &Stage, out: &mut Vec<String>) {
        for o in [
            &s.hit_effect,
            &s.map_hit_effect,
            &s.hit_sound,
            &s.map_hit_sound,
        ] {
            out.extend(o.clone());
        }
        for (e, c) in &s.fire_effects {
            out.extend(e.clone());
            out.extend(c.clone());
        }
        if let Some(n) = &s.next {
            stage(n, out);
        }
    }
    let mut out: Vec<String> = (1..=7)
        .map(|i| format!("/Game/Sound/SE/weapon/02001/se02001_00{i}"))
        .collect();
    out.extend(book.hit_effect.clone());
    for s in book.spells.values() {
        out.extend(s.cast_effect.clone());
        stage(&s.stage, &mut out);
    }
    for s in book.shots.iter().flatten() {
        stage(s, &mut out);
    }
    out.sort_unstable();
    out.dedup();
    out
}

/// The Kingdom Key's pawn hit effect.
fn hit_effect(paks: &Paks) -> Option<String> {
    ["/Game/Blueprints/Player/p_ex001/w_so010_Wep", KEYBLADE_BASE]
        .iter()
        .find_map(|path| {
            let p = paks.game_package(path).ok().flatten()?;
            let i = p.export_named("TresAtkColl0")?;
            let (coll, _) = props::export_props(&p, i).ok()?;
            coll.array("m_HitEffects")?.iter().find_map(|row| {
                let o = row.as_props()?.object("m_PawnHitEffect")?;
                object_path(&p, o)
            })
        })
}

/// Reads the book.
pub(crate) fn read(paks: &Paks, data: &SoraData) -> Result<Book> {
    let mut book = Book::default();
    let b = blade(data);
    let wanted = (0u8..=0x7f)
        .filter_map(visual::attack_row)
        .chain(OTHER_ROWS.iter().copied());
    for row in wanted {
        if book.clips.contains_key(row) {
            continue;
        }
        let Some(index) = data.anims.row(row) else {
            continue;
        };
        if let Some(clip) = data.anims.main_clip(index, 0.) {
            book.clips
                .insert(row.to_owned(), clip_info(data, clip, b.as_ref()));
        }
    }
    attack_rows(paks, &mut book)?;
    shapes(paks, &mut book)?;
    stats(paks, &mut book)?;
    spells(paks, &mut book)?;
    let mut cache = HashMap::new();
    book.shots = [
        stage_of(
            paks,
            "/Game/Blueprints/Projectile/ShootFlow/TresProjKC_Shot",
            0,
            &mut cache,
        ),
        stage_of(
            paks,
            "/Game/Blueprints/Projectile/ShootFlow/TresProjKC_Ragnarok",
            0,
            &mut cache,
        ),
    ];
    book.hit_effect = hit_effect(paks);
    for (i, row) in data.anims.rows.iter().enumerate() {
        book.row_ids.insert(row.name.clone(), i as u16);
    }
    book.names = fx_names(&book);
    Ok(book)
}
