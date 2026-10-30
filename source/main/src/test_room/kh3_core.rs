//! Gives KH3's Sora (`kh3_sora`) his numbers and animation tables from the player's paks: the sheets through the
//! `data` crate's recipes, the anim set as its clock and notifies need it. Once a process, on the load thread.

use data::kh3::anim::BonePose;
use data::kh3::sheets::{self, Val};
use data::kh3::sora::{self, Clip, Notify, RowData, SoraData};
use data::kh3::{Paks, Result, props::Props};
use kh3_sora::anim::{
    ANIM_ROW_NAMES, AnimData, AnimTables, BlendSpace2d, NotifyClass, NotifyData, NotifyDef,
    RootKey, SequenceData,
};
use kh3_sora::sheets::{PV, Store};
use std::collections::HashMap;

fn leak(s: &str) -> &'static str {
    Box::leak(s.to_owned().into_boxed_str())
}

fn slice<T>(v: Vec<T>) -> &'static [T] {
    Box::leak(v.into_boxed_slice())
}

/// One value as the core holds it.
fn value(v: &Val) -> PV {
    match v {
        Val::N(n) => PV::N(*n),
        Val::B(b) => PV::B(*b),
        Val::S(s) => PV::S(leak(s)),
        Val::A(a) => PV::A(slice(a.clone())),
        Val::L(l) => PV::L(slice(l.iter().map(|s| leak(s)).collect())),
        Val::Nil => PV::Nil,
    }
}

/// `ESomeEnum::VALUE` to `VALUE`.
fn label(v: &str) -> &'static str {
    leak(v.rsplit("::").next().unwrap_or(v))
}

fn opt_label(p: &Props, name: &str) -> Option<&'static str> {
    p.str(name).filter(|v| *v != "None").map(label)
}

/// The properties the core's rules read; sounds and effects are the presenter's (`kh3_fx` replays them from the
/// clips, on each PC).
fn decode(n: &Notify) -> NotifyData {
    let p = &n.props;
    match n.class.as_str() {
        "TresAnimNotifyState_Attack" => NotifyData::Attack {
            group: leak(p.str("CollisionGrpName").unwrap_or("None")),
            row: leak(p.str("m_AttackDataIDName").unwrap_or("None")),
            hit_se: opt_label(p, "m_WeaponHitSEAliasID"),
            interval: p.f32("AttackInterval"),
            hit_se_asset: None,
            override_hit_effect: p.bool("m_bOverrideHitEffect").unwrap_or(false),
            hit_effect_group: p
                .str("m_HitEffectGrpName")
                .filter(|v| *v != "None")
                .map(leak),
        },
        "TresAnimNotifyState_IK" => NotifyData::Ik {
            start_interpolation_time: p.f32("m_StartInterpolationTime").unwrap_or(0.),
            end_interpolation_time: p.f32("m_EndInterpolationTime").unwrap_or(0.),
            target_alpha: p.f32("m_TargetAlpha").unwrap_or(1.),
        },
        "TresAnimNotify_Timing" | "TresAnimNotifyState_BpEvent" => NotifyData::Param {
            param: p.int("m_Param").map(|v| v as i32),
        },
        "TresAnimNotify_Shot" => NotifyData::Shot {
            location_index: p.int("LocationIndex").map(|v| v as i32),
            attach_fire_effect: p.bool("m_bAttachFireEffect").unwrap_or(false),
        },
        "TresAnimNotify_LandAnimPose" => NotifyData::LandAnimPose {
            pose: opt_label(p, "m_LandAnimPose"),
        },
        "TresAnimNotify_LastAnimPose" => NotifyData::LastAnimPose {
            pose: opt_label(p, "m_LastAnimPose"),
        },
        _ => NotifyData::None,
    }
}

fn notify_def(n: &Notify) -> NotifyDef {
    NotifyDef {
        class: NotifyClass::from_game_name(&n.class),
        state: n.state,
        time: n.start,
        end: n.end,
        data: decode(n),
        props: &[],
    }
}

/// The root bone per frame, since frame 0, in the actor's axes (forward = mesh Y, right = -mesh X).
fn root_track(clip: &Clip) -> &'static [RootKey] {
    if !clip.root_motion {
        return &[];
    }
    let Some(track) = clip.seq.tracks.iter().find(|t| t.bone == 0) else {
        return &[];
    };
    let n = clip.seq.num_frames.max(1) as usize;
    let at = |i: usize| {
        if n > 1 {
            i as f32 * clip.seq.length / (n - 1) as f32
        } else {
            0.
        }
    };
    let first = clip.seq.sample_track(track, 0.);
    let q0 = first.rotation;
    let conj = [-q0[0], -q0[1], -q0[2], q0[3]];
    let mut last_yaw = 0f32;
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let pose = clip.seq.sample_track(track, at(i));
        let d = [
            pose.translation[0] - first.translation[0],
            pose.translation[1] - first.translation[1],
            pose.translation[2] - first.translation[2],
        ];
        let (a, b) = (pose.rotation, conj);
        let dz = a[3] * b[2] + a[0] * b[1] - a[1] * b[0] + a[2] * b[3];
        let dw = a[3] * b[3] - a[0] * b[0] - a[1] * b[1] - a[2] * b[2];
        let mut yaw = 2. * dz.atan2(dw).to_degrees();
        while yaw - last_yaw > 180. {
            yaw -= 360.;
        }
        while yaw - last_yaw < -180. {
            yaw += 360.;
        }
        last_yaw = yaw;
        out.push(RootKey {
            time: if i + 1 == n { clip.seq.length } else { at(i) },
            x: d[1],
            y: -d[0],
            z: d[2],
            yaw,
        });
    }
    slice(out)
}

/// The Keyblade's two hit sockets per frame, for clips with a hit window.
fn sockets(
    data: &SoraData,
    clip: &Clip,
    blade: Option<&super::kh3_book::Blade>,
) -> &'static [[f32; 6]] {
    let Some(b) = blade else {
        return &[];
    };
    if !clip
        .notifies
        .iter()
        .any(|n| n.class == "TresAnimNotifyState_Attack")
    {
        return &[];
    }
    let n = clip.seq.num_frames.max(1) as usize;
    let (mut pose, mut comp) = (Vec::new(), Vec::new());
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let t = if n > 1 {
            i as f32 * clip.seq.length / (n - 1) as f32
        } else {
            0.
        };
        sora::sample_clip(clip, t, &data.reference, &mut pose);
        sora::to_component(&data.mesh.bones, &pose, &mut comp);
        let Some(points) = b.points(&comp) else {
            continue;
        };
        out.push([
            points[0][0],
            points[0][1],
            points[0][2],
            points[1][0],
            points[1][1],
            points[1][2],
        ]);
    }
    slice(out)
}

fn sequence(data: &SoraData, clip: &Clip, blade: Option<&super::kh3_book::Blade>) -> SequenceData {
    let find = |name: &str| clip.notifies.iter().find(|n| n.name == name);
    let time = |name: &str| find(name).map_or(-1., |n| n.start);
    SequenceData {
        name: leak(&clip.name),
        length: clip.seq.length,
        frames: clip.seq.num_frames,
        root_motion_enabled: clip.root_motion,
        interpolation_time: clip.blend,
        jump_start: time("TresJumpStart"),
        enable_change: time("TresEnableChange"),
        reverse_leg: find("TresReverseLeg").map_or((-1., -1.), |n| (n.start, n.end)),
        root: root_track(clip),
        notifies: slice(clip.notifies.iter().map(notify_def).collect()),
        sockets: sockets(data, clip, blade),
    }
}

/// The play modes of the rows the movement states play themselves: (name, mode bits, blend-in; -1 the asset's).
const STAGE1: [(&str, u8, f32); 18] = [
    ("IdleN", 2, -1.),
    ("IdleBreakN", 2, -1.),
    ("WalkLoopN", 1, -1.),
    ("WalkEndR", 2, -1.),
    ("WalkEndL", 2, -1.),
    ("RunStartN", 2, -1.),
    ("RunLoopN", 1, -1.),
    ("RunEndR", 2, -1.),
    ("RunEndL", 2, -1.),
    ("RunSwayR", 2, 0.1),
    ("RunSwayL", 2, 0.1),
    ("JumpNormalStart", 0, -1.),
    ("JumpNormalRiseLoop", 1, -1.),
    ("JumpNormalDownLoop", 1, -1.),
    ("Land", 2, -1.),
    ("LightLand", 2, -1.),
    ("LightLandR", 2, -1.),
    ("LightLandL", 2, -1.),
];

fn anim_tables(data: &SoraData) -> Result<AnimTables> {
    let set = &data.anims;
    let blade = super::kh3_book::blade(data);
    let sequences: Vec<SequenceData> = set
        .clips
        .iter()
        .map(|c| sequence(data, c, blade.as_ref()))
        .collect();
    let fallback = set.row("IdleN").map_or(0, usize::from);
    let mut anims = Vec::with_capacity(ANIM_ROW_NAMES.len());
    for name in ANIM_ROW_NAMES {
        let row = set
            .row(name)
            .and_then(|i| set.rows.get(usize::from(i)))
            .or_else(|| set.rows.get(fallback))
            .ok_or_else(|| data::kh3::bad_text("the anim set has no rows"))?;
        let (samples, space2d, lengths): (
            Vec<(f32, usize)>,
            Option<&'static BlendSpace2d>,
            Vec<f32>,
        ) = match &row.data {
            RowData::Clip(i) => (vec![(0., *i)], None, vec![sequences[*i].length]),
            RowData::Blend1D { samples, .. } => {
                let mut s = samples.clone();
                s.sort_by(|a, b| a.0.total_cmp(&b.0));
                let lengths = s.iter().map(|x| sequences[x.1].length).collect();
                (s, None, lengths)
            }
            RowData::Blend2D { samples, .. } => {
                let info = row.space.clone().unwrap_or_default();
                let grid: Vec<&'static [(usize, f32)]> =
                    info.grid.iter().map(|g| slice(g.clone())).collect();
                let space: &'static BlendSpace2d = Box::leak(Box::new(BlendSpace2d {
                    x_min: info.x.0,
                    x_max: info.x.1,
                    x_grid: info.x.2,
                    y_min: info.y.0,
                    y_max: info.y.1,
                    y_grid: info.y.2,
                    y_smoothing: info.y_smoothing,
                    samples: slice(samples.iter().map(|s| (s.0[0], s.0[1], s.1)).collect()),
                    grid: slice(grid),
                }));
                let lengths = samples.iter().map(|x| sequences[x.1].length).collect();
                (
                    samples.iter().map(|s| (s.0[0], s.1)).collect(),
                    Some(space),
                    lengths,
                )
            }
        };
        let top = lengths.iter().copied().fold(0f32, f32::max);
        let low = lengths.iter().copied().fold(f32::MAX, f32::min);
        let differ = top - low > 1e-4;
        let enabled = samples.iter().any(|s| sequences[s.1].root_motion_enabled);
        let asset = set.row_blend(set.row(row.name.as_str()).unwrap_or(0));
        let stage1 = STAGE1.iter().find(|r| r.0 == *name);
        let space = row.space.clone().unwrap_or_default();
        anims.push(AnimData {
            name,
            stage1_mode: stage1.map(|r| r.1),
            looping: stage1.is_some_and(|r| r.1 & 1 != 0),
            root_motion_bit: stage1.is_some_and(|r| r.1 & 2 != 0),
            root_motion: stage1.is_some_and(|r| r.1 & 2 != 0) && enabled,
            root_motion_enabled: enabled,
            blend_in: stage1.map_or(asset, |r| if r.2 < 0. { asset } else { r.2 }),
            length: if differ { top } else { lengths[0] },
            sample_lengths_differ: differ,
            blend_space: !matches!(row.data, RowData::Clip(_)),
            axis_min: space.x.0,
            axis_max: space.x.1,
            axis_smoothing: space.smoothing,
            all_animation: space.all_animation,
            samples: slice(samples),
            space2d,
        });
    }
    // The socket of the wielder's root (`vfx_cure`) through the reference pose, and 200 forward of it in its frame.
    let (mut cure, mut cure_far) = ([0., 0., 105.], [200., 0., 105.]);
    if let Some(s) = data.sockets.iter().find(|s| s.name == "vfx_cure")
        && let Some(bone) = data.bone(&s.bone)
    {
        let mut comp = Vec::new();
        sora::to_component(&data.mesh.bones, &data.reference, &mut comp);
        if let Some(parent) = comp.get(bone) {
            let socket = BonePose {
                rotation: sora::quat_of_rotator(s.rotation),
                translation: s.location,
                scale: s.scale,
            };
            let at = sora::compose(parent, &socket);
            let far = sora::compose(
                &at,
                &BonePose {
                    rotation: [0., 0., 0., 1.],
                    translation: [200., 0., 0.],
                    scale: [1.; 3],
                },
            );
            let actor = |p: [f32; 3]| [p[1], -p[0], p[2]];
            cure = actor(at.translation);
            cure_far = actor(far.translation);
        }
    }
    Ok(AnimTables {
        anims,
        sequences,
        notify_assets: Vec::new(),
        socket_vfx_cure: cure,
        socket_vfx_cure_plus_200: cure_far,
    })
}

/// The effects the core names by their short names (combat, status, items, the shotlock and the airstep).
const CORE_EFFECTS: [&str; 37] = [
    "VS_non_HIT0",
    "VW_SO010_APP0",
    "VW_SO010_WHT0",
    "ead_shf_tame0",
    "vm_air_zan0",
    "vm_cur_hed0",
    "vm_cur_hed1",
    "vm_cur_hed2",
    "vm_fir_zan1",
    "vp_fca_eng00",
    "vp_ffa_arv0",
    "vs_abn_brn0",
    "vs_abn_frz0",
    "vs_abn_frz3",
    "vs_abn_frz_in",
    "vs_abn_frz_out0",
    "vs_abn_slw00",
    "vs_abn_snz0",
    "vs_abn_stn1",
    "vs_abn_thd0",
    "vs_blz_hit0",
    "vs_com_haj00",
    "vs_com_kyp01",
    "vs_com_lv2",
    "vs_com_mtk0",
    "vs_com_yor00",
    "vs_dak_hit1",
    "vs_fir_hit0",
    "vs_itm_pot0",
    "vs_thd_hit0",
    "vs_wat_hit0",
    "vs_win_hit0",
    "vw_so010_hit0",
    "vw_so010_rgn_hit00",
    "vw_so010_rgn_wht00",
    "vw_so010_str0",
    "vp_snc_hit0",
];

/// Finds the assets of the names the core gives (the mounted paths' last part), adds them to what the presenters
/// load and to the wire's list. The weapon's sound aliases (`HIT1`..) are its bank's.
pub(crate) fn name_assets(paks: &Paks, book: &mut player::kh3::attack::Book) {
    let wanted: std::collections::HashSet<String> = CORE_EFFECTS
        .iter()
        .map(|n| n.to_ascii_lowercase())
        .chain(book.names.iter().filter_map(|n| {
            n.rsplit('/')
                .next()
                .map(|b| b.split('.').next().unwrap_or(b).to_ascii_lowercase())
        }))
        .collect();
    for path in paks.mounted.names() {
        let Some(stem) = path.strip_suffix(".uasset") else {
            continue;
        };
        let Some(base) = stem.rsplit('/').next() else {
            continue;
        };
        if !wanted.contains(base) {
            continue;
        }
        let Some(rest) = stem.strip_prefix("kingdom hearts iii/content/") else {
            continue;
        };
        if rest.starts_with("effects/") || rest.starts_with("sound/") {
            book.by_name
                .entry(base.to_owned())
                .or_insert_with(|| format!("/Game/{rest}"));
        }
    }
    for n in &book.names {
        if let Some(b) = n.rsplit('/').next() {
            let b = b.split('.').next().unwrap_or(b).to_ascii_lowercase();
            book.by_name.insert(b, n.clone());
        }
    }
    for i in 1..=7 {
        book.by_name.insert(
            format!("hit{i}"),
            format!("/Game/Sound/SE/weapon/02001/se02001_00{i}"),
        );
    }
    let mut names: Vec<String> = book.names.clone();
    names.extend(book.by_name.values().cloned());
    names.sort_unstable();
    names.dedup();
    book.names = names;
}

/// Reads and installs everything. A row this install cannot give is an error: the mode needs them all.
pub(crate) fn install(paks: &Paks, data: &SoraData) -> Result<()> {
    if kh3_sora::sheets::installed() {
        return Ok(());
    }
    if !sheets::has_constants() {
        return Err(data::kh3::bad_text(
            "this build was made without KH3's constants (they are not in the repository)"
                .to_owned(),
        ));
    }
    let read = sheets::read(paks);
    if let Some((sheet, id, why)) = read
        .missing
        .iter()
        .find(|m| !(m.0 == "sora-stats-rules" && m.1 == "prize.values"))
    {
        return Err(data::kh3::bad_text(format!(
            "this KINGDOM HEARTS III's data differs ({sheet} {id}: {why})"
        )));
    }
    let mut store = Store::default();
    for (name, rows) in &read.params {
        let rows: Vec<(&'static str, PV)> = rows.iter().map(|(i, v)| (leak(i), value(v))).collect();
        store.sheets.insert((*name).to_owned(), rows);
    }
    for (name, rows) in &read.tables {
        let rows: Vec<HashMap<&'static str, PV>> = rows
            .iter()
            .map(|cells| cells.iter().map(|(c, v)| (leak(c), value(v))).collect())
            .collect();
        store.tables.insert((*name).to_owned(), rows);
    }
    kh3_sora::sheets::install(store);
    kh3_sora::anim::install(anim_tables(data)?);
    Ok(())
}
