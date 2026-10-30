//! KH3 Sora as the duel's KH3 mode needs him, read from the player's paks:
//! the mesh, skeleton sockets, material looks, Kingdom Key and the anim set
//! with each clip's notifies. Paths and rules from StrawHatPro's documents
//! (MIT; see CREDITS.md).
use super::anim::{AnimSequence, BonePose, nlerp};
use super::material::{self, MaterialParams, import_path};
use super::package::Package;
use super::props::{Props, Value, export_props};
use super::skeletal::{Bone, SkeletalMesh};
use super::texture::Texture;
use super::{Paks, Result, bad};
use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

pub const MESH: &str = "/Game/Character/pc/p_ex001/mdl/p_ex001_0";
pub const SKELETON: &str = "/Game/Character/pc/p_ex001/mdl/p_ex001_0_Skeleton";
pub const ANIM_SET: &str = "/Game/Character/pc/p_ex001/anm/p_ex001_AnimSet";
pub const KEYBLADE: &str = "/Game/Character/wep/w_so010/mdl/w_so010_0";
const MOUTH: &str = "/Game/Shader/Material_set/SQEX_MouthTexSet/Mouth_M_0d00";

/// A socket on a skeleton.
#[derive(Clone, Debug, PartialEq)]
pub struct Socket {
    pub name: String,
    pub bone: String,
    pub location: [f32; 3],
    /// Pitch, yaw, roll in degrees.
    pub rotation: [f32; 3],
    pub scale: [f32; 3],
}

/// One notify of a clip. `class` is the export's class; `props` its data.
#[derive(Clone, Debug, PartialEq)]
pub struct Notify {
    /// `NotifyName`.
    pub name: String,
    pub class: String,
    pub state: bool,
    pub start: f32,
    pub end: f32,
    pub props: Props,
    /// Package paths of the object references the notify's own properties
    /// hold, by the object's index (they mean nothing outside the package).
    pub imports: HashMap<i32, String>,
}

#[derive(Clone, Debug)]
pub struct Clip {
    pub name: String,
    pub seq: AnimSequence,
    pub root_motion: bool,
    /// `m_InterpolationTime`, 0 when not serialized.
    pub blend: f32,
    pub notifies: Vec<Notify>,
}

/// What a row of the anim set plays.
#[derive(Clone, Debug, PartialEq)]
pub enum RowData {
    Clip(usize),
    /// Samples by their X, with the blend parameter's range.
    Blend1D {
        samples: Vec<(f32, usize)>,
        blend: f32,
    },
    /// Samples by (X, Y).
    Blend2D {
        samples: Vec<([f32; 2], usize)>,
        blend: f32,
    },
}

/// What a blend space's asset says besides its samples.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Space {
    /// (min, max, grid cells) of each axis.
    pub x: (f32, f32, usize),
    pub y: (f32, f32, usize),
    /// `InterpolationParam`'s times.
    pub smoothing: f32,
    pub y_smoothing: f32,
    /// `NotifyTriggerMode` SQEX_AllAnimation.
    pub all_animation: bool,
    /// Each grid point's (index into `SampleData`, weight).
    pub grid: Vec<Vec<(usize, f32)>>,
}

#[derive(Clone, Debug)]
pub struct Row {
    pub name: String,
    pub data: RowData,
    pub space: Option<Space>,
}

#[derive(Clone, Debug, Default)]
pub struct AnimSet {
    pub rows: Vec<Row>,
    pub clips: Vec<Clip>,
    by_name: HashMap<String, u16>,
}

impl AnimSet {
    /// The row's index, the same on every PC with this install.
    pub fn row(&self, name: &str) -> Option<u16> {
        self.by_name.get(name).copied()
    }

    /// The clip a row plays at `x` (the heaviest sample of a blend space).
    pub fn main_clip(&self, row: u16, x: f32) -> Option<&Clip> {
        match &self.rows.get(usize::from(row))?.data {
            RowData::Clip(i) => self.clips.get(*i),
            RowData::Blend1D { samples, .. } => {
                let best = samples.iter().min_by(|a, b| {
                    (a.0 - x)
                        .abs()
                        .partial_cmp(&(b.0 - x).abs())
                        .unwrap_or(std::cmp::Ordering::Equal)
                })?;
                self.clips.get(best.1)
            }
            RowData::Blend2D { samples, .. } => self.clips.get(samples.first()?.1),
        }
    }

    /// `m_InterpolationTime` of a row.
    pub fn row_blend(&self, row: u16) -> f32 {
        match self.rows.get(usize::from(row)).map(|r| &r.data) {
            Some(RowData::Clip(i)) => self.clips.get(*i).map_or(0., |c| c.blend),
            Some(RowData::Blend1D { blend, .. } | RowData::Blend2D { blend, .. }) => *blend,
            None => 0.,
        }
    }

    /// The row's length in seconds at `x`.
    pub fn row_length(&self, row: u16, x: f32) -> f32 {
        self.main_clip(row, x).map_or(0., |c| c.seq.length)
    }
}

/// A clip's pose over the reference pose. Root-motion clips lock the root
/// bone to the reference, as `RootMotionRootLock = RefPose` does.
pub fn sample_clip(clip: &Clip, time: f32, reference: &[BonePose], out: &mut Vec<BonePose>) {
    out.clear();
    out.extend_from_slice(reference);
    for track in &clip.seq.tracks {
        if let Some(slot) = out.get_mut(track.bone as usize) {
            *slot = clip.seq.sample_track(track, time);
        }
    }
    if clip.root_motion
        && let (Some(slot), Some(r)) = (out.first_mut(), reference.first())
    {
        *slot = *r;
    }
}

/// The local-space blend of two poses, `b` weighted by `w`: rotation
/// sign-corrected nlerp, translation linear, scale in log space (SQEX).
pub fn blend_pose(a: &BonePose, b: &BonePose, w: f32) -> BonePose {
    let lerp = |x: f32, y: f32| x + (y - x) * w;
    let log = |x: f32, y: f32| {
        if x > 0. && y > 0. {
            (x.ln() * (1. - w) + y.ln() * w).exp()
        } else {
            lerp(x, y)
        }
    };
    BonePose {
        rotation: nlerp(a.rotation, b.rotation, w),
        translation: [
            lerp(a.translation[0], b.translation[0]),
            lerp(a.translation[1], b.translation[1]),
            lerp(a.translation[2], b.translation[2]),
        ],
        scale: [
            log(a.scale[0], b.scale[0]),
            log(a.scale[1], b.scale[1]),
            log(a.scale[2], b.scale[2]),
        ],
    }
}

/// A row's pose at `time`; a blend space weighs its samples by `at`.
pub fn sample_row(
    set: &AnimSet,
    row: u16,
    time: f32,
    at: [f32; 2],
    reference: &[BonePose],
    out: &mut Vec<BonePose>,
    scratch: &mut Vec<BonePose>,
) {
    let Some(data) = set.rows.get(usize::from(row)).map(|r| &r.data) else {
        out.clear();
        out.extend_from_slice(reference);
        return;
    };
    match data {
        RowData::Clip(i) => {
            if let Some(c) = set.clips.get(*i) {
                sample_clip(c, time, reference, out);
            }
        }
        RowData::Blend1D { samples, .. } => {
            let mut low = None;
            let mut high = None;
            for &(x, c) in samples {
                if x <= at[0] && low.is_none_or(|(l, _)| x > l) {
                    low = Some((x, c));
                }
                if x >= at[0] && high.is_none_or(|(h, _)| x < h) {
                    high = Some((x, c));
                }
            }
            let (a, b) = match (low, high) {
                (Some(a), Some(b)) => (a, b),
                (Some(a), None) | (None, Some(a)) => (a, a),
                (None, None) => return,
            };
            blend_samples(
                set,
                a.1,
                b.1,
                weight_between(a.0, b.0, at[0]),
                time,
                reference,
                out,
                scratch,
            );
        }
        RowData::Blend2D { samples, .. } => {
            // The two samples nearest (X, Y), weighted by distance.
            let distance = |s: &[f32; 2]| ((s[0] - at[0]).powi(2) + (s[1] - at[1]).powi(2)).sqrt();
            let mut sorted: Vec<_> = samples.iter().collect();
            sorted.sort_by(|a, b| {
                distance(&a.0)
                    .partial_cmp(&distance(&b.0))
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            let Some(first) = sorted.first() else { return };
            let second = sorted.get(1).unwrap_or(first);
            let (d0, d1) = (distance(&first.0), distance(&second.0));
            let w = if d0 + d1 > 1e-4 { d0 / (d0 + d1) } else { 0. };
            blend_samples(set, first.1, second.1, w, time, reference, out, scratch);
        }
    }
}

fn weight_between(a: f32, b: f32, x: f32) -> f32 {
    if (b - a).abs() < 1e-5 {
        0.
    } else {
        ((x - a) / (b - a)).clamp(0., 1.)
    }
}

#[allow(clippy::too_many_arguments)]
fn blend_samples(
    set: &AnimSet,
    a: usize,
    b: usize,
    w: f32,
    time: f32,
    reference: &[BonePose],
    out: &mut Vec<BonePose>,
    scratch: &mut Vec<BonePose>,
) {
    let Some(ca) = set.clips.get(a) else { return };
    sample_clip(ca, time, reference, out);
    if a == b || w <= 0. {
        return;
    }
    let Some(cb) = set.clips.get(b) else { return };
    // Samples of a space share their phase, not always their length.
    let t = if ca.seq.length > 0. {
        time / ca.seq.length * cb.seq.length
    } else {
        time
    };
    sample_clip(cb, t, reference, scratch);
    for (p, q) in out.iter_mut().zip(scratch.iter()) {
        *p = blend_pose(p, q, w);
    }
}

/// The root bone's place and yaw at `time`, in the mesh's axes, for root
/// motion: the (x, y) of its translation, its z, and the yaw from its
/// rotation (around Z).
pub fn root_at(clip: &Clip, time: f32) -> ([f32; 3], f32) {
    let Some(track) = clip.seq.tracks.iter().find(|t| t.bone == 0) else {
        return ([0.; 3], 0.);
    };
    let pose = clip.seq.sample_track(track, time);
    let q = pose.rotation;
    let yaw = (2. * (q[3] * q[2] + q[0] * q[1])).atan2(1. - 2. * (q[1] * q[1] + q[2] * q[2]));
    (pose.translation, yaw)
}

/// A root delta between two times, in the actor's axes: (forward, right, up,
/// yaw), by the mesh's yaw-270 placement (forward = mesh Y, right = -mesh X).
pub fn root_delta(clip: &Clip, from: f32, to: f32) -> [f32; 4] {
    let (a, ay) = root_at(clip, from);
    let (b, by) = root_at(clip, to);
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let (s, c) = (-ay).sin_cos();
    let (x, y) = (dx * c - dy * s, dx * s + dy * c);
    [y, -x, b[2] - a[2], by - ay]
}

/// What a material slot draws: a good-enough look from the parameters.
#[derive(Clone, Debug, Default)]
pub struct SlotLook {
    pub name: String,
    pub color: Option<String>,
    pub normal: Option<String>,
    /// A flat colour in place of (or tinting) the texture, linear.
    pub flat: Option<[f32; 4]>,
    /// The texture only gives alpha.
    pub alpha_only: bool,
    pub skip: bool,
    pub translucent: bool,
    pub two_sided: bool,
}

fn mean(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    [
        (a[0] + b[0]) / 2.,
        (a[1] + b[1]) / 2.,
        (a[2] + b[2]) / 2.,
        1.,
    ]
}

/// The look of a material by what its root is.
pub fn slot_look(slot: &str, m: &MaterialParams) -> SlotLook {
    let mut look = SlotLook {
        name: slot.to_owned(),
        two_sided: m.two_sided,
        normal: ["NormalTex", "Normalmap", "EyeNormalTexture"]
            .iter()
            .find_map(|k| m.textures.get(*k).cloned()),
        ..SlotLook::default()
    };
    let vector = |k: &str| m.vectors.get(k).copied();
    match m.root.as_str() {
        "SQEX_KHHairBaseMat" => {
            look.flat = Some(match (vector("HairColor"), vector("HairPointEndColor")) {
                (Some(a), Some(b)) => mean(a, b),
                _ => [0.1, 0.05, 0.02, 1.],
            });
        }
        "SQEX_Mouth" => look.color = Some(MOUTH.to_owned()),
        "SQEX_WhiteEyeMaterial" => look.color = m.textures.get("EyeTexture").cloned(),
        "SQEX_EyeLens" => look.skip = true,
        "SQEX_IrisTexEyeMaterial" => {
            look.flat = Some(match (vector("Color"), vector("Color_Dark")) {
                (Some(a), Some(b)) => mean(a, b),
                _ => [0.1, 0.5, 0.9, 1.],
            });
        }
        "SQEX_BaseEyelashes" => {
            look.color = m.textures.get("EyelashesMask").cloned();
            look.flat = vector("EyelashesColor");
            look.alpha_only = true;
            look.translucent = true;
        }
        _ => {
            look.color = ["FormChangeTex0", "BaseColorTex", "BaseTexture"]
                .iter()
                .find_map(|k| m.textures.get(*k).cloned());
        }
    }
    look
}

/// The `/Game/...` path of a mesh's material slot.
fn slot_material(p: &Package, object: i32) -> Option<String> {
    import_path(p, object)
}

/// Everything the duel's model needs, in memory.
#[derive(Debug, Default)]
pub struct SoraData {
    pub mesh: SkeletalMesh,
    pub looks: Vec<SlotLook>,
    pub sockets: Vec<Socket>,
    pub keyblade: SkeletalMesh,
    pub keyblade_looks: Vec<SlotLook>,
    pub keyblade_sockets: Vec<Socket>,
    pub textures: HashMap<String, Texture>,
    pub anims: AnimSet,
    pub reference: Vec<BonePose>,
    pub rig: super::rig::RigData,
}

fn read_sockets(p: &Package) -> Vec<Socket> {
    let mut out = Vec::new();
    for i in 0..p.exports.len() {
        if p.class_of(i) != "SkeletalMeshSocket" {
            continue;
        }
        let Ok((props, _)) = export_props(p, i) else {
            continue;
        };
        let vector = |k: &str, or: [f32; 3]| {
            props
                .floats(k)
                .filter(|v| v.len() == 3)
                .map_or(or, |v| [v[0], v[1], v[2]])
        };
        out.push(Socket {
            name: props.str("SocketName").unwrap_or("").to_owned(),
            bone: props.str("BoneName").unwrap_or("").to_owned(),
            location: vector("RelativeLocation", [0.; 3]),
            rotation: vector("RelativeRotation", [0.; 3]),
            scale: vector("RelativeScale", [1.; 3]),
        });
    }
    out
}

fn read_mesh(paks: &Paks, path: &str) -> Result<(Package, SkeletalMesh)> {
    let p = paks
        .game_package(path)?
        .ok_or_else(|| bad(format!("{path} is missing")))?;
    let i = p
        .export_of_class("SkeletalMesh")
        .ok_or_else(|| bad(format!("{path} has no mesh")))?;
    let mesh = SkeletalMesh::read(&p, i)?;
    Ok((p, mesh))
}

fn read_notifies(p: &Package, props: &Props) -> Vec<Notify> {
    let mut out = Vec::new();
    for item in props.array("Notifies").unwrap_or(&[]) {
        let Some(item) = item.as_props() else {
            continue;
        };
        let at = item
            .f32("LinkValue")
            .or_else(|| item.f32("DisplayTime"))
            .unwrap_or(0.);
        let start = at + item.f32("TriggerTimeOffset").unwrap_or(0.);
        let state_object = item.object("NotifyStateClass").filter(|o| *o != 0);
        let object = state_object.or_else(|| item.object("Notify").filter(|o| *o != 0));
        let Some(object) = object.filter(|o| *o > 0) else {
            continue;
        };
        let export = object as usize - 1;
        let Ok((data, _)) = export_props(p, export) else {
            continue;
        };
        let state = state_object.is_some();
        let end = if state {
            at + item.f32("Duration").unwrap_or(0.) + item.f32("EndTriggerTimeOffset").unwrap_or(0.)
        } else {
            start
        };
        let mut imports = HashMap::new();
        collect_imports(p, &data, &mut imports);
        out.push(Notify {
            name: item.str("NotifyName").unwrap_or("").to_owned(),
            class: p.class_of(export).to_owned(),
            state,
            start,
            end,
            props: data,
            imports,
        });
    }
    out.sort_by(|a, b| {
        a.start
            .partial_cmp(&b.start)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    out
}

fn collect_imports(p: &Package, props: &Props, out: &mut HashMap<i32, String>) {
    fn value(p: &Package, v: &Value, out: &mut HashMap<i32, String>) {
        match v {
            Value::Object(o) if *o < 0 => {
                if let Some(path) = import_path(p, *o) {
                    out.insert(*o, path);
                }
            }
            Value::Struct(_, inner) => collect_imports(p, inner, out),
            Value::Array(items) => {
                for i in items {
                    value(p, i, out);
                }
            }
            _ => {}
        }
    }
    for prop in &props.0 {
        value(p, &prop.value, out);
    }
}

/// A clip's package to the clip.
pub fn read_clip(paks: &Paks, path: &str) -> Result<Clip> {
    let p = paks
        .game_package(path)?
        .ok_or_else(|| bad(format!("{path} is missing")))?;
    let i = p
        .export_of_class("AnimSequence")
        .ok_or_else(|| bad(format!("{path} is not a sequence")))?;
    let seq = AnimSequence::read(&p, i)?;
    let notifies = read_notifies(&p, &seq.props);
    Ok(Clip {
        name: p.exports[i].name.clone(),
        root_motion: seq.props.bool("bEnableRootMotion").unwrap_or(false),
        blend: seq.props.f32("m_InterpolationTime").unwrap_or(0.),
        seq,
        notifies,
    })
}

enum PendingRow {
    Clip(String),
    Space1(Vec<(f32, String)>, f32, Space),
    Space2(Vec<([f32; 2], String)>, f32, Space),
}

/// A blend space's axes, input filter and grid.
fn space_of(props: &Props) -> Space {
    let axis = |index: i32| {
        let b = props
            .all("BlendParameters")
            .find(|q| q.index == index)
            .and_then(|q| q.value.as_props());
        (
            b.and_then(|b| b.f32("Min")).unwrap_or(0.),
            b.and_then(|b| b.f32("Max")).unwrap_or(100.),
            b.and_then(|b| b.int("GridNum")).unwrap_or(4) as usize,
        )
    };
    let time = |index: i32| {
        props
            .all("InterpolationParam")
            .find(|q| q.index == index)
            .and_then(|q| q.value.as_props())
            .and_then(|b| b.f32("InterpolationTime"))
            .unwrap_or(0.)
    };
    let mut grid = Vec::new();
    for g in props.array("GridSamples").unwrap_or(&[]) {
        let mut cell = Vec::new();
        if let Some(g) = g.as_props() {
            for k in 0..3 {
                let index = g.all("Indices").find(|q| q.index == k);
                let weight = g.all("Weights").find(|q| q.index == k);
                if let Some(i) = index.and_then(|q| q.value.as_int())
                    && i >= 0
                {
                    let w = weight.and_then(|q| q.value.as_f32()).unwrap_or(0.);
                    cell.push((i as usize, w));
                }
            }
        }
        grid.push(cell);
    }
    Space {
        x: axis(0),
        y: axis(1),
        smoothing: time(0),
        y_smoothing: time(1),
        all_animation: props
            .str("NotifyTriggerMode")
            .is_some_and(|m| m.contains("SQEX_AllAnimation")),
        grid,
    }
}

fn read_space(paks: &Paks, path: &str, two: bool) -> Result<PendingRow> {
    let p = paks
        .game_package(path)?
        .ok_or_else(|| bad(format!("{path} is missing")))?;
    let i = p
        .export_of_classes(&[
            "BlendSpace1D",
            "BlendSpace",
            "AimOffsetBlendSpace1D",
            "AimOffsetBlendSpace",
        ])
        .ok_or_else(|| bad(format!("{path} is not a blend space")))?;
    let (props, _) = export_props(&p, i)?;
    let blend = props.f32("m_InterpolationTime").unwrap_or(0.);
    let mut one = Vec::new();
    let mut both = Vec::new();
    for s in props.array("SampleData").unwrap_or(&[]) {
        let Some(s) = s.as_props() else { continue };
        let Some(clip) = s.object("Animation").and_then(|o| import_path(&p, o)) else {
            continue;
        };
        let v = s.floats("SampleValue").unwrap_or(&[0., 0., 0.]);
        let (x, y) = (
            v.first().copied().unwrap_or(0.),
            v.get(1).copied().unwrap_or(0.),
        );
        one.push((x, clip.clone()));
        both.push(([x, y], clip));
    }
    let space = space_of(&props);
    Ok(if two {
        PendingRow::Space2(both, blend, space)
    } else {
        PendingRow::Space1(one, blend, space)
    })
}

/// Runs `work` over `items` on a few threads, keeping their order.
fn parallel<T: Sync, R: Send>(items: &[T], work: impl Fn(&T) -> R + Sync) -> Vec<R> {
    let threads = std::thread::available_parallelism()
        .map_or(4, |n| n.get())
        .min(8);
    let next = AtomicUsize::new(0);
    let done: Mutex<Vec<(usize, R)>> = Mutex::new(Vec::with_capacity(items.len()));
    std::thread::scope(|s| {
        for _ in 0..threads {
            s.spawn(|| {
                loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    let Some(item) = items.get(i) else { break };
                    let r = work(item);
                    if let Ok(mut d) = done.lock() {
                        d.push((i, r));
                    }
                }
            });
        }
    });
    let mut done = done.into_inner().unwrap_or_default();
    done.sort_by_key(|(i, _)| *i);
    done.into_iter().map(|(_, r)| r).collect()
}

/// The anim set, its row names to clips, every clip decoded.
pub fn read_anim_set(paks: &Paks, path: &str) -> Result<AnimSet> {
    let p = paks
        .game_package(path)?
        .ok_or_else(|| bad(format!("{path} is missing")))?;
    let i = p
        .export_of_class("TresAnimSet")
        .ok_or_else(|| bad(format!("{path} is not an anim set")))?;
    let (props, _) = export_props(&p, i)?;
    let mut wanted: Vec<(String, i32)> = Vec::new();
    for row in props.array("Anims").unwrap_or(&[]) {
        let Some(row) = row.as_props() else { continue };
        if let (Some(name), Some(object)) = (row.str("AnimName"), row.object("AnimData")) {
            wanted.push((name.to_owned(), object));
        }
    }
    let pending: Vec<Option<(String, PendingRow)>> = parallel(&wanted, |(name, object)| {
        let import = usize::try_from(-i64::from(*object) - 1).ok();
        let class = import
            .and_then(|k| p.imports.get(k))
            .map_or("", |im| im.class.as_str());
        let path = import_path(&p, *object)?;
        let row = match class {
            "AnimSequence" => PendingRow::Clip(path),
            "BlendSpace1D" | "AimOffsetBlendSpace1D" => read_space(paks, &path, false).ok()?,
            "BlendSpace" | "AimOffsetBlendSpace" => read_space(paks, &path, true).ok()?,
            _ => return None,
        };
        Some((name.clone(), row))
    });
    let mut paths: Vec<String> = Vec::new();
    for (_, row) in pending.iter().flatten() {
        match row {
            PendingRow::Clip(c) => paths.push(c.clone()),
            PendingRow::Space1(s, _, _) => paths.extend(s.iter().map(|x| x.1.clone())),
            PendingRow::Space2(s, _, _) => paths.extend(s.iter().map(|x| x.1.clone())),
        }
    }
    paths.sort_unstable();
    paths.dedup();
    let loaded: Vec<Result<Clip>> = parallel(&paths, |c| read_clip(paks, c));
    let mut index = HashMap::new();
    let mut clips = Vec::with_capacity(paths.len());
    for (path, clip) in paths.iter().zip(loaded) {
        // A clip that does not read leaves its rows out, not the whole set.
        if let Ok(c) = clip {
            index.insert(path.clone(), clips.len());
            clips.push(c);
        }
    }
    let mut set = AnimSet {
        clips,
        ..AnimSet::default()
    };
    for (name, row) in pending.into_iter().flatten() {
        let mut space = None;
        let data = match row {
            PendingRow::Clip(c) => index.get(&c).map(|i| RowData::Clip(*i)),
            PendingRow::Space1(s, blend, info) => {
                space = Some(info);
                let samples: Vec<_> = s
                    .iter()
                    .filter_map(|(x, c)| index.get(c).map(|i| (*x, *i)))
                    .collect();
                (!samples.is_empty()).then_some(RowData::Blend1D { samples, blend })
            }
            PendingRow::Space2(s, blend, info) => {
                space = Some(info);
                let samples: Vec<_> = s
                    .iter()
                    .filter_map(|(x, c)| index.get(c).map(|i| (*x, *i)))
                    .collect();
                (!samples.is_empty()).then_some(RowData::Blend2D { samples, blend })
            }
        };
        if let Some(data) = data {
            set.by_name.insert(name.clone(), set.rows.len() as u16);
            set.rows.push(Row { name, data, space });
        }
    }
    Ok(set)
}

fn looks_of(paks: &Paks, p: &Package, mesh: &SkeletalMesh) -> Vec<SlotLook> {
    let paths: Vec<(String, Option<String>)> = mesh
        .materials
        .iter()
        .map(|s| (s.slot.clone(), slot_material(p, s.object)))
        .collect();
    parallel(&paths, |(slot, path)| {
        path.as_deref()
            .and_then(|path| material::read(paks, path).ok())
            .map_or_else(
                || SlotLook {
                    name: slot.clone(),
                    ..SlotLook::default()
                },
                |m| slot_look(slot, &m),
            )
    })
}

fn reference_of(bones: &[Bone]) -> Vec<BonePose> {
    bones
        .iter()
        .map(|b| BonePose {
            rotation: b.rotation,
            translation: b.translation,
            scale: b.scale,
        })
        .collect()
}

impl SoraData {
    pub fn load(paks: &Paks) -> Result<Self> {
        let (mesh_package, mesh) = read_mesh(paks, MESH)?;
        let looks = looks_of(paks, &mesh_package, &mesh);
        let skeleton = paks
            .game_package(SKELETON)?
            .ok_or_else(|| bad("Sora's skeleton is missing"))?;
        let sockets = read_sockets(&skeleton);
        let (blade_package, keyblade) = read_mesh(paks, KEYBLADE)?;
        let keyblade_looks = looks_of(paks, &blade_package, &keyblade);
        let mut keyblade_sockets = read_sockets(&blade_package);
        let blade_skeleton = format!("{KEYBLADE}_Skeleton");
        if let Some(p) = paks.game_package(&blade_skeleton)? {
            keyblade_sockets.extend(read_sockets(&p));
        }
        let anims = read_anim_set(paks, ANIM_SET)?;
        let mut wanted: Vec<String> = looks
            .iter()
            .chain(&keyblade_looks)
            .filter(|l| !l.skip)
            .flat_map(|l| l.color.iter().chain(&l.normal).cloned())
            .collect();
        wanted.sort_unstable();
        wanted.dedup();
        let textures = parallel(&wanted, |path| {
            let p = paks.game_package(path).ok().flatten()?;
            let i = p.export_of_class("Texture2D")?;
            Texture::read(&p, i).ok().map(|t| (path.clone(), t))
        });
        let reference = reference_of(&mesh.bones);
        let rig = super::rig::read(paks, &mesh.bones, &keyblade.bones)?;
        Ok(Self {
            mesh,
            looks,
            sockets,
            keyblade,
            keyblade_looks,
            keyblade_sockets,
            textures: textures.into_iter().flatten().collect(),
            anims,
            reference,
            rig,
        })
    }

    /// A mesh bone's index by name.
    pub fn bone(&self, name: &str) -> Option<usize> {
        self.mesh.bones.iter().position(|b| b.name == name)
    }
}

/// UE's composition of two transforms: no shear, the scales multiply.
pub fn compose(parent: &BonePose, child: &BonePose) -> BonePose {
    let scaled = [
        child.translation[0] * parent.scale[0],
        child.translation[1] * parent.scale[1],
        child.translation[2] * parent.scale[2],
    ];
    let r = rotate(parent.rotation, scaled);
    BonePose {
        rotation: quat_mul(parent.rotation, child.rotation),
        translation: [
            parent.translation[0] + r[0],
            parent.translation[1] + r[1],
            parent.translation[2] + r[2],
        ],
        scale: [
            parent.scale[0] * child.scale[0],
            parent.scale[1] * child.scale[1],
            parent.scale[2] * child.scale[2],
        ],
    }
}

pub fn quat_mul(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    [
        a[3] * b[0] + a[0] * b[3] + a[1] * b[2] - a[2] * b[1],
        a[3] * b[1] - a[0] * b[2] + a[1] * b[3] + a[2] * b[0],
        a[3] * b[2] + a[0] * b[1] - a[1] * b[0] + a[2] * b[3],
        a[3] * b[3] - a[0] * b[0] - a[1] * b[1] - a[2] * b[2],
    ]
}

pub fn rotate(q: [f32; 4], v: [f32; 3]) -> [f32; 3] {
    let u = [q[0], q[1], q[2]];
    let cross = |a: [f32; 3], b: [f32; 3]| {
        [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ]
    };
    let t = cross(u, v);
    let t = [t[0] * 2., t[1] * 2., t[2] * 2.];
    let c = cross(u, t);
    [
        v[0] + q[3] * t[0] + c[0],
        v[1] + q[3] * t[1] + c[1],
        v[2] + q[3] * t[2] + c[2],
    ]
}

/// A rotator (pitch, yaw, roll, degrees) as UE turns it into a quaternion.
pub fn quat_of_rotator(r: [f32; 3]) -> [f32; 4] {
    let half = std::f32::consts::PI / 360.;
    let (sp, cp) = (r[0] * half).sin_cos();
    let (sy, cy) = (r[1] * half).sin_cos();
    let (sr, cr) = (r[2] * half).sin_cos();
    [
        cr * sp * sy - sr * cp * cy,
        -cr * sp * cy - sr * cp * sy,
        cr * cp * sy - sr * sp * cy,
        cr * cp * cy + sr * sp * sy,
    ]
}

/// Bones' component-space transforms from local poses and parents.
pub fn to_component(bones: &[Bone], local: &[BonePose], out: &mut Vec<BonePose>) {
    out.clear();
    for (i, pose) in local.iter().enumerate() {
        let parent = bones.get(i).map_or(-1, |b| b.parent);
        let c = usize::try_from(parent)
            .ok()
            .and_then(|p| out.get(p))
            .map_or(*pose, |p| compose(p, pose));
        out.push(c);
    }
}

/// The saved-pose blend of KH3's montage slot, for presentation: a new play
/// starts at weight 0 over what was last shown and reaches 1 linearly over
/// `blend_in` (`docs/sora-animation-spec` in the reference).
#[derive(Clone, Debug, Default)]
pub struct Animator {
    pub saved: Vec<BonePose>,
    pub row: Option<u16>,
    pub serial: u32,
    pub elapsed: f32,
    pub blend_in: f32,
    scratch_a: Vec<BonePose>,
    scratch_b: Vec<BonePose>,
}

impl Animator {
    /// One render step: the row at `time`, blended over the saved pose. The
    /// result is `self.saved`.
    #[allow(clippy::too_many_arguments)]
    pub fn update(
        &mut self,
        set: &AnimSet,
        reference: &[BonePose],
        row: u16,
        serial: u32,
        time: f32,
        at: [f32; 2],
        blend_in: f32,
        dt: f32,
    ) {
        if self.saved.len() != reference.len() {
            self.saved = reference.to_vec();
        }
        if self.row != Some(row) || self.serial != serial {
            self.row = Some(row);
            self.serial = serial;
            self.elapsed = 0.;
            self.blend_in = if blend_in < 0. {
                set.row_blend(row)
            } else {
                blend_in
            };
        }
        self.elapsed += dt;
        let weight = if self.blend_in <= 0. {
            1.
        } else {
            (self.elapsed / self.blend_in).min(1.)
        };
        let (mut a, mut b) = (
            std::mem::take(&mut self.scratch_a),
            std::mem::take(&mut self.scratch_b),
        );
        sample_row(set, row, time, at, reference, &mut a, &mut b);
        if weight >= 1. - 1e-5 {
            std::mem::swap(&mut self.saved, &mut a);
        } else {
            for (s, m) in self.saved.iter_mut().zip(a.iter()) {
                *s = blend_pose(s, m, weight);
            }
        }
        self.scratch_a = a;
        self.scratch_b = b;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pose(t: [f32; 3]) -> BonePose {
        BonePose {
            rotation: [0., 0., 0., 1.],
            translation: t,
            scale: [1.; 3],
        }
    }

    #[test]
    fn composition_applies_parent_rotation_and_scale() {
        // 90 degrees about Z.
        let s = std::f32::consts::FRAC_1_SQRT_2;
        let parent = BonePose {
            rotation: [0., 0., s, s],
            translation: [1., 0., 0.],
            scale: [2.; 3],
        };
        let c = compose(&parent, &pose([1., 0., 0.]));
        assert!((c.translation[0] - 1.).abs() < 1e-5);
        assert!((c.translation[1] - 2.).abs() < 1e-5);
        assert_eq!(c.scale, [2.; 3]);
    }

    #[test]
    fn component_space_follows_parents() {
        let bone = |name: &str, parent| Bone {
            name: name.into(),
            parent,
            rotation: [0., 0., 0., 1.],
            translation: [0.; 3],
            scale: [1.; 3],
        };
        let bones = [bone("a", -1), bone("b", 0)];
        let mut out = Vec::new();
        to_component(&bones, &[pose([1., 0., 0.]), pose([0., 2., 0.])], &mut out);
        assert_eq!(out[1].translation, [1., 2., 0.]);
    }

    #[test]
    fn rotators_are_ue_quaternions() {
        let q = quat_of_rotator([0., 90., 0.]);
        let v = rotate(q, [1., 0., 0.]);
        assert!((v[1] - 1.).abs() < 1e-5, "{v:?}");
    }

    #[test]
    fn looks_follow_the_root_material() {
        let mut m = MaterialParams {
            root: "SQEX_KHHairBaseMat".into(),
            ..MaterialParams::default()
        };
        m.vectors.insert("HairColor".into(), [0., 0., 0., 1.]);
        m.vectors
            .insert("HairPointEndColor".into(), [1., 1., 1., 1.]);
        assert_eq!(slot_look("h", &m).flat, Some([0.5, 0.5, 0.5, 1.]));
        m.root = "SQEX_EyeLens".into();
        assert!(slot_look("e", &m).skip);
        m.root = "p_ex001_Basemat".into();
        m.textures.insert("FormChangeTex0".into(), "/Game/T".into());
        assert_eq!(slot_look("b", &m).color.as_deref(), Some("/Game/T"));
    }

    fn set_with_two_clips() -> AnimSet {
        let clip = |name: &str| Clip {
            name: name.into(),
            seq: AnimSequence {
                length: 1.,
                num_frames: 31,
                ..AnimSequence::default()
            },
            root_motion: false,
            blend: 0.2,
            notifies: Vec::new(),
        };
        let mut set = AnimSet {
            clips: vec![clip("a"), clip("b")],
            ..AnimSet::default()
        };
        set.rows.push(Row {
            name: "A".into(),
            data: RowData::Clip(0),
            space: None,
        });
        set.by_name.insert("A".into(), 0);
        set
    }

    #[test]
    fn the_animator_blends_in_over_the_saved_pose() {
        let set = set_with_two_clips();
        let reference = [pose([1., 0., 0.])];
        let mut a = Animator {
            saved: vec![pose([0., 0., 0.])],
            ..Animator::default()
        };
        a.update(&set, &reference, 0, 1, 0., [0., 0.], -1., 0.1);
        assert!((a.saved[0].translation[0] - 0.5).abs() < 1e-5);
        a.update(&set, &reference, 0, 1, 0., [0., 0.], -1., 0.1);
        assert!((a.saved[0].translation[0] - 1.).abs() < 1e-5);
    }

    #[test]
    fn row_names_are_found() {
        let set = set_with_two_clips();
        assert_eq!(set.row("A"), Some(0));
        assert_eq!(set.row("Z"), None);
        assert_eq!(set.row_blend(0), 0.2);
    }
}
