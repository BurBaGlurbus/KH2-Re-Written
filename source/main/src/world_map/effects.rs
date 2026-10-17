//! The world map's effects for `kh2fm wm`: the map's own PAX elements, run
//! per draw group (FUN_1402c7010 when a group switches on, FUN_1402c65a0
//! when it switches off), and the world map objects' PAX elements
//! (FUN_1403b6630 / FUN_1403b6670 → FUN_1402c6b30 on the object's
//! container). The effect VM is `effect`; the drawing follows the test
//! room's object effects (`test_room/drive_fx.rs`): per-DPD textures by
//! TEX0 (the element's own buffers shadow `effect/texcommon.dpd`), OMD
//! model draws, and blend and layer batches.
use ::effect::{Instance, ModelDraw, Quad, Rng, math::Mat};
use bevy::{
    asset::RenderAssetUsages,
    image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor},
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};
use data::object_effect::{self, Dpd, OmdHeader, OmdMesh, Pax};
use std::{collections::HashMap, sync::Arc};

/// The particle heap's size (FUN_1402bdb50's allocation, 0x80000 bytes at
/// start-up; the area start may pass another, which is not traced).
const PARTICLE_HEAP: usize = 0x80000;

/// Where an element starts.
pub enum Start {
    /// The map's element at this table index, placed by its own
    /// translation, rotation and scale (engine world space); it belongs to
    /// its draw group.
    Map { index: usize },
    /// An object's element by id, placed by the object's matrix and then
    /// its own transform. A bound element (flag bit 0) started by a
    /// motion trigger keeps to object `follow` [I: to the object, not its
    /// bone].
    Object {
        object: Mat,
        id: u16,
        follow: Option<usize>,
    },
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
struct TexKey {
    dpd: usize,
    tbp: u16,
    cbp: u16,
    tw: u8,
    th: u8,
}

impl TexKey {
    fn new(dpd: &Arc<Dpd>, tex0: u64) -> Self {
        TexKey {
            dpd: Arc::as_ptr(dpd) as usize,
            tbp: (tex0 & 0x3fff) as u16,
            cbp: ((tex0 >> 37) & 0x3fff) as u16,
            tw: ((tex0 >> 26) & 0xf) as u8,
            th: ((tex0 >> 30) & 0xf) as u8,
        }
    }
    fn size(&self) -> [u32; 2] {
        [1 << self.tw.min(10), 1 << self.th.min(10)]
    }
}

type ParsedModel = (OmdHeader, Vec<OmdMesh>);

#[derive(Default)]
struct Batch {
    positions: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
    colors: Vec<[f32; 4]>,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct BatchKey {
    texture: Option<TexKey>,
    additive: bool,
    repeat: bool,
    layer: i8,
}

struct Running {
    handle: u64,
    /// The PAX and element, for the log.
    label: (String, u16),
    instance: Instance,
    dpd: Arc<Dpd>,
    /// The draw group of a map element.
    group: Option<u8>,
    /// The object a bound element keeps to, and the element's transform.
    follow: Option<(usize, Mat)>,
    /// The object whose motion holds the element (bound, flag 0x20:
    /// FUN_1403c8000); its next motion releases it.
    motion: Option<usize>,
    /// The element's sound (a map element's loops until its group goes)
    /// and the ticks until it plays.
    sound: Option<u32>,
    sound_wait: f32,
    sound_started: bool,
}

/// Odd layers behind everything, even layers in front, 0 by depth
/// (FUN_1402c2720 / FUN_1402c27a0).
fn layer_rank(layer: u8) -> i8 {
    match layer {
        0 => 0,
        v if v % 2 == 1 => -((v as i8 + 1) / 2),
        v => (v / 2) as i8,
    }
}

/// The camera's view matrix in engine world space (Y down), in the effect
/// VM's convention: view x right, y down, z forward (as the test room's).
pub fn view(eye: [f32; 3], target: [f32; 3]) -> Mat {
    let e = Vec3::from_array(eye);
    let f = (Vec3::from_array(target) - e).normalize_or(Vec3::Z);
    let down = (Vec3::Y - f * Vec3::Y.dot(f)).normalize_or(Vec3::Y);
    let right = down.cross(f);
    [
        [right.x, down.x, f.x, 0.],
        [right.y, down.y, f.y, 0.],
        [right.z, down.z, f.z, 0.],
        [-right.dot(e), -down.dot(e), -f.dot(e), 1.],
    ]
}

fn scene(p: [f32; 3]) -> [f32; 3] {
    [p[0], -p[1], -p[2]]
}

/// What one tick asks of the sound side and the log.
#[derive(Default)]
pub struct Step {
    pub sounds: Vec<u32>,
    pub stops: Vec<u32>,
    pub log: Vec<serde_json::Value>,
}

#[derive(Resource, Default)]
pub struct Effects {
    common: Option<Arc<Dpd>>,
    running: Vec<Running>,
    rng: Rng,
    models: HashMap<(usize, i16), Option<ParsedModel>>,
    batches: HashMap<BatchKey, Batch>,
    dirty: bool,
    images: HashMap<TexKey, Handle<Image>>,
    materials: HashMap<BatchKey, Handle<StandardMaterial>>,
    spawned: Vec<Entity>,
    dpds: Vec<Arc<Dpd>>,
    reported: Vec<u32>,
    /// The draw groups whose map elements run.
    pub map_mask: u64,
}

impl Effects {
    pub fn new(common: Option<Dpd>) -> Self {
        Effects {
            common: common.map(Arc::new),
            ..Default::default()
        }
    }

    /// FUN_1402c6b30 / FUN_1402c7010's per-element start. The element's
    /// sound (> 0) plays at its start, after its start wait.
    pub fn start(&mut self, handle: u64, pax: &Arc<Pax>, start: Start, out: &mut Step) {
        let element = match &start {
            Start::Map { index } => pax.elements.get(*index),
            Start::Object { id, .. } => pax.by_id(*id),
        };
        let Some(element) = element else {
            out.log.push(serde_json::json!({"event":"effect_missing","pax":pax.name,
                "start":match start { Start::Map { index } => format!("map {index}"), Start::Object { id, .. } => format!("element {id}") }}));
            return;
        };
        let Some(dpx) = pax.effect(element.effect_number) else {
            return;
        };
        let local = ::effect::instance_matrix(element.translation, element.rotation, element.scale);
        let world = match &start {
            Start::Map { .. } => local,
            Start::Object { object, .. } => ::effect::math::mul(object, &local),
        };
        let follow = match &start {
            Start::Object {
                follow: Some(o), ..
            } if element.flags & 1 != 0 => Some((*o, local)),
            _ => None,
        };
        let motion = follow.filter(|_| element.flags & 0x20 != 0).map(|(o, _)| o);
        let dpd = pax.dpds[dpx.dpd].clone();
        let wait = element.start_wait;
        let sound = (element.sound > 0).then_some(element.sound as u32);
        if let Some(id) = sound
            && wait <= 0.
        {
            out.sounds.push(id);
        }
        let group = matches!(start, Start::Map { .. }).then_some(element.group);
        out.log.push(
            serde_json::json!({"event":"effect","pax":pax.name,"element":element.id,
            "group":group,"dpd":dpx.dpd,"pdata":dpx.pdata,"sound":sound,"wait":wait,
            "at":[world[3][0],world[3][1],world[3][2]]}),
        );
        if let Some(instance) = Instance::new(dpd.clone(), dpx.pdata, world, wait) {
            if !self.dpds.iter().any(|x| Arc::ptr_eq(x, &dpd)) {
                self.dpds.push(dpd.clone());
            }
            self.running.push(Running {
                handle,
                label: (pax.name.clone(), element.id),
                instance,
                dpd,
                group,
                follow,
                motion,
                sound,
                sound_wait: wait,
                sound_started: wait <= 0.,
            });
        }
    }

    /// FUN_1402be8a0: the element is freed at once.
    pub fn stop(&mut self, handle: u64, out: &mut Step) {
        for r in self.running.iter().filter(|r| r.handle == handle) {
            if r.group.is_some()
                && let Some(id) = r.sound.filter(|_| r.sound_started)
            {
                out.stops.push(id);
            }
        }
        self.running.retain(|r| r.handle != handle);
    }

    /// FUN_1402c65a0: the group's map elements leave, their sounds stop.
    pub fn stop_group(&mut self, group: u8, out: &mut Step) {
        for r in self.running.iter().filter(|r| r.group == Some(group)) {
            if let Some(id) = r.sound.filter(|_| r.sound_started) {
                out.stops.push(id);
            }
        }
        self.running.retain(|r| r.group != Some(group));
    }

    /// FUN_1403c7300 at a motion change: the elements the object's motion
    /// holds are released (flag 0x2000000) and end as their programs do.
    pub fn release_motion(&mut self, object: usize) {
        for r in self.running.iter_mut().filter(|r| r.motion == Some(object)) {
            r.instance.release();
            r.motion = None;
        }
    }

    /// Bound elements follow their objects (matrix: an object's matrix).
    pub fn follow(&mut self, matrix: impl Fn(usize) -> Option<Mat>) {
        for r in self.running.iter_mut() {
            if let Some((o, local)) = &r.follow
                && let Some(m) = matrix(*o)
            {
                r.instance.world = ::effect::math::mul(&m, local);
            }
        }
    }

    /// Each running element and its live parts, for the log.
    pub fn parts(&self) -> Vec<(String, u16, usize)> {
        self.running
            .iter()
            .map(|r| (r.label.0.clone(), r.label.1, r.instance.live_parts()))
            .collect()
    }

    /// The vertices this tick drew.
    pub fn vertices(&self) -> usize {
        self.batches.values().map(|b| b.positions.len()).sum()
    }

    pub fn running(&self) -> usize {
        self.running.len()
    }

    /// One tick: every instance steps against the camera and what it drew
    /// is batched in scene space.
    pub fn step(&mut self, view: &Mat, out: &mut Step) {
        for r in self.running.iter_mut() {
            if !r.sound_started {
                r.sound_wait -= 1.;
                if r.sound_wait <= 0. {
                    r.sound_started = true;
                    if let Some(id) = r.sound {
                        out.sounds.push(id);
                    }
                }
            }
        }
        // The particle heap every running effect shares: each instance may
        // grow into what is free this tick (parts only; the emitters' own
        // pools are not counted) [I].
        let used: Vec<usize> = self
            .running
            .iter()
            .map(|r| r.instance.heap_used())
            .collect();
        let total: usize = used.iter().sum();
        for (r, u) in self.running.iter_mut().zip(used) {
            r.instance.heap_limit = Some(u + PARTICLE_HEAP.saturating_sub(total));
        }
        let view_inverse = Mat4::from_cols_array_2d(view).inverse();
        self.batches.clear();
        self.dirty = true;
        let mut quads: Vec<(Arc<Dpd>, i8, Vec<Quad>)> = Vec::new();
        let mut draws: Vec<(Arc<Dpd>, i8, Vec<ModelDraw>)> = Vec::new();
        let mut triangles: Vec<(i8, Vec<::effect::Triangle>)> = Vec::new();
        for r in self.running.iter_mut() {
            let q = r.instance.frame(view, &mut self.rng);
            for &k in &r.instance.unsupported {
                if !self.reported.contains(&k) {
                    self.reported.push(k);
                    out.log
                        .push(serde_json::json!({"event":"effect_unsupported","program":k}));
                }
            }
            // A program this translation lacks: stepped, not shown.
            let complete = r.instance.unsupported.is_empty();
            let d = std::mem::take(&mut r.instance.model_draws);
            let t = std::mem::take(&mut r.instance.triangles);
            let layer = layer_rank(r.instance.layer());
            if complete && !q.is_empty() {
                quads.push((r.dpd.clone(), layer, q));
            }
            if complete && !d.is_empty() {
                draws.push((r.dpd.clone(), layer, d));
            }
            if complete && !t.is_empty() {
                triangles.push((layer, t));
            }
        }
        self.running.retain(|r| !r.instance.finished());
        let to_scene = |p: [f32; 3]| {
            scene(
                view_inverse
                    .transform_point3(Vec3::from_array(p))
                    .to_array(),
            )
        };
        let grade = |c: [f32; 4]| {
            [
                c[0].clamp(0., 1.99).powf(2.2),
                c[1].clamp(0., 1.99).powf(2.2),
                c[2].clamp(0., 1.99).powf(2.2),
                c[3].clamp(0., 1.),
            ]
        };
        for (dpd, layer, list) in quads {
            for q in list {
                let key = TexKey::new(&dpd, q.tex0);
                let size = key.size();
                let batch = self
                    .batches
                    .entry(BatchKey {
                        texture: Some(key),
                        additive: q.blend == 0x48,
                        repeat: true,
                        layer,
                    })
                    .or_default();
                for i in [0, 1, 2, 0, 2, 3] {
                    batch.positions.push(to_scene(q.corners[i]));
                    batch
                        .uvs
                        .push([q.uv[i][0] / size[0] as f32, q.uv[i][1] / size[1] as f32]);
                    batch
                        .colors
                        .push(grade(q.corner_colours.map_or(q.colour, |c| c[i])));
                }
            }
        }
        // Untextured gouraud triangles (GS colour bytes, 0x80 = 1.0).
        for (layer, list) in triangles {
            for t in list {
                let batch = self
                    .batches
                    .entry(BatchKey {
                        texture: None,
                        additive: t.blend == 0x48,
                        repeat: false,
                        layer,
                    })
                    .or_default();
                for i in 0..3 {
                    batch.positions.push(to_scene(t.corners[i]));
                    batch.uvs.push([0.; 2]);
                    batch
                        .colors
                        .push(grade(t.colours[i].map(|v| v as f32 / 128.)));
                }
            }
        }
        for (dpd, layer, list) in draws {
            for d in list {
                self.model(&dpd, &d, layer, &to_scene);
            }
        }
    }

    /// An OMD model draw (pppDrawMdl3), batched as the test room does.
    fn model(
        &mut self,
        dpd: &Arc<Dpd>,
        d: &ModelDraw,
        layer: i8,
        to_scene: &impl Fn([f32; 3]) -> [f32; 3],
    ) {
        let cached = self
            .models
            .entry((Arc::as_ptr(dpd) as usize, d.model))
            .or_insert_with(|| {
                let m = dpd.models.get(&d.model)?;
                let header = object_effect::omd_header(&dpd.bytes, m.hdr).ok()?;
                let meshes = object_effect::omd_meshes(&dpd.bytes, m.hdr).ok()?;
                Some((header, meshes))
            });
        let Some((header, meshes)) = cached.as_ref() else {
            return;
        };
        let vertex = |i: u16| -> Option<[f32; 3]> {
            let v = match &d.vertices {
                Some(o) => o.get(i as usize).copied(),
                None => dpd.vertex(d.model, i),
            }?;
            let p = ::effect::math::apply(&d.a, [v[0] as f32, v[1] as f32, v[2] as f32, 1.]);
            Some(to_scene([p[0], p[1], p[2]]))
        };
        let additive = header.alpha == 0x48;
        let repeat = header.flags & 4 != 0;
        let part = d.rgba.map(|c| c as f32 / 128.);
        let mut out: Vec<(BatchKey, Batch)> = Vec::new();
        for mesh in meshes {
            let texture = mesh.textured.then(|| TexKey::new(dpd, mesh.tex0));
            let shift = texture.map_or(4, |t| t.tw as u32 + 4).min(31);
            let mut batch = Batch::default();
            for t in &mesh.triangles {
                let Some(p) = t.index.map(vertex).into_iter().collect::<Option<Vec<_>>>() else {
                    continue;
                };
                for (k, point) in p.iter().enumerate() {
                    batch.positions.push(*point);
                    let (u, v) = (t.uv[k][0] as i32, t.uv[k][1] as i32);
                    let su = d.scroll[0] >> shift;
                    let sv = d.scroll[1] >> (texture.map_or(4, |t| t.th as u32 + 4).min(31));
                    batch.uvs.push([
                        u.wrapping_add(su) as f32 / 4096.,
                        v.wrapping_add(sv) as f32 / 4096.,
                    ]);
                    let c = t.colour[k];
                    let o: [f32; 4] = std::array::from_fn(|i| {
                        ((c[i] as f32 / 255.) * part[i]).min(1.) * (255. / 128.)
                    });
                    batch.colors.push([
                        o[0].powf(2.2),
                        o[1].powf(2.2),
                        o[2].powf(2.2),
                        o[3].min(1.),
                    ]);
                }
            }
            out.push((
                BatchKey {
                    texture,
                    additive,
                    repeat,
                    layer,
                },
                batch,
            ));
        }
        for (key, b) in out {
            let e = self.batches.entry(key).or_default();
            e.positions.extend(b.positions);
            e.uvs.extend(b.uvs);
            e.colors.extend(b.colors);
        }
    }

    fn image(&mut self, key: TexKey, repeat: bool, images: &mut Assets<Image>) -> Handle<Image> {
        if let Some(h) = self.images.get(&key) {
            return h.clone();
        }
        let [w, h] = key.size();
        let mut pixels = vec![0u8; (w * h * 4) as usize];
        let own = self
            .dpds
            .iter()
            .find(|d| Arc::as_ptr(d) as usize == key.dpd)
            .cloned();
        let mut sources: Vec<&object_effect::DpdTexture> = Vec::new();
        if let Some(d) = &own {
            sources.extend(d.textures.iter().filter(|t| t.dbp == key.tbp));
        }
        if sources.is_empty()
            && let Some(c) = &self.common
        {
            sources.extend(c.textures.iter().filter(|t| t.dbp == key.tbp));
        }
        let palette = own
            .iter()
            .map(|d| d.as_ref())
            .chain(self.common.as_deref())
            .flat_map(|d| d.textures.iter())
            .find(|t| t.cbp == key.cbp)
            .map(|t| t.palette.clone());
        for t in sources {
            let rgba =
                object_effect::decode_psmt8(&t.indices, palette.as_deref().unwrap_or(&t.palette));
            for row in 0..t.height {
                let y = t.y as u32 + row;
                if y >= h {
                    break;
                }
                for col in 0..t.width {
                    let x = t.x as u32 + col;
                    if x >= w {
                        break;
                    }
                    let s = ((row * t.width + col) * 4) as usize;
                    let d = ((y * w + x) * 4) as usize;
                    pixels[d..d + 4].copy_from_slice(&rgba[s..s + 4]);
                }
            }
        }
        let mut image = Image::new(
            Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            pixels,
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
        );
        let mode = if repeat {
            ImageAddressMode::Repeat
        } else {
            ImageAddressMode::ClampToEdge
        };
        image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
            address_mode_u: mode,
            address_mode_v: mode,
            mag_filter: ImageFilterMode::Linear,
            min_filter: ImageFilterMode::Linear,
            ..default()
        });
        let handle = images.add(image);
        self.images.insert(key, handle.clone());
        handle
    }
}

/// Rebuilds the effect meshes from this tick's batches under `parent`.
pub fn present(
    commands: &mut Commands,
    fx: &mut Effects,
    parent: Option<Entity>,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    images: &mut Assets<Image>,
) {
    if !fx.dirty {
        return;
    }
    fx.dirty = false;
    for e in std::mem::take(&mut fx.spawned) {
        commands.entity(e).despawn();
    }
    let batches = std::mem::take(&mut fx.batches);
    for (key, batch) in &batches {
        if batch.positions.is_empty() {
            continue;
        }
        let material = match fx.materials.get(key) {
            Some(m) => m.clone(),
            None => {
                let texture = key.texture.map(|t| fx.image(t, key.repeat, images));
                let m = materials.add(StandardMaterial {
                    base_color: Color::WHITE,
                    base_color_texture: texture,
                    unlit: true,
                    cull_mode: None,
                    depth_bias: key.layer as f32 * 500.,
                    alpha_mode: if key.additive {
                        AlphaMode::Add
                    } else {
                        AlphaMode::Blend
                    },
                    ..default()
                });
                fx.materials.insert(*key, m.clone());
                m
            }
        };
        let mesh = visual::room::mesh(
            batch.positions.clone(),
            batch.uvs.clone(),
            batch.colors.clone(),
        );
        let mut e = commands.spawn((
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(material),
            Transform::default(),
            Visibility::default(),
        ));
        if let Some(p) = parent {
            e.insert(ChildOf(p));
        }
        fx.spawned.push(e.id());
    }
}
