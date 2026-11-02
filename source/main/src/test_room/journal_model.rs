//! The Character Files' model pages, the host side (FUN_14032ee20,
//! FUN_1402feb20): the page's object and its `_MEMO` motion are read off the
//! main thread, posed each frame and rendered alone into their own target,
//! which the journal pastes.
use super::*;
use data::{model_inspection::InspectionModel, motion::SelectedMotion, path_lookup::Library};
use std::{sync::Arc, thread::JoinHandle};
use visual::pause_menu::journal::{Journal, MODEL_REACH, MODEL_TARGET};

/// Free of the camp's (3..6), the overlay's (7), the blur's (8..) and the
/// skateboard's (31).
const LAYER: usize = 30;

/// FUN_1402ff990's table: object → TEXA number (the Organization's numbers on
/// M_EX350's rows).
const NUMBERS: [(u16, u16); 13] = [
    (0x926, 0x41),
    (0x9fe, 0x42),
    (0x928, 0x43),
    (0x929, 0x44),
    (0x92a, 0x45),
    (0x92b, 0x46),
    (0x92c, 0x47),
    (0x92d, 0x48),
    (0x92e, 0x49),
    (0x92f, 0x4a),
    (0x9ff, 0x4b),
    (0x931, 0x4c),
    (0x932, 0x4d),
];

pub struct Loaded {
    model: InspectionModel,
    motion: SelectedMotion,
    /// The first key, and the clock's total and return, in ticks.
    start: f32,
    total: f32,
    back: f32,
}

/// The `_MEMO` name of a set (FUN_1403e0bd0): the entry's set, or the model's
/// name when it has none.
pub fn memo_set(set: &str, model: &str) -> String {
    let stem = set.strip_suffix(".mset").unwrap_or(set);
    let stem = if stem.is_empty() { model } else { stem };
    format!("obj/{stem}_MEMO.mset")
}

/// One update of the clock (FUN_1403001b0).
pub fn clock(t: f32, total: f32, back: f32) -> f32 {
    let mut t = t + 1.;
    if total < t {
        t -= total;
    }
    if t < back {
        t += back;
    }
    t
}

/// A model whose file holds levels (T340's containers, as `M_EX500_GM`): its
/// first, with the originals' images.
fn first_level(
    library: &mut Library,
    path: &str,
) -> Result<InspectionModel, Box<dyn std::error::Error>> {
    let archive = library.read(path)?;
    let table = data::bar::read_bar(&archive)?;
    let slice = |kind: u16| {
        let e = table
            .entries
            .iter()
            .find(|e| e.entry_type == kind && e.size > 0)?;
        let at = e.offset as usize;
        Some((archive.get(at..at + e.size as usize)?, at))
    };
    let (payload, _) = slice(4).ok_or_else(|| format!("{path} has no model"))?;
    let payload = visual::scenery::level_payload(payload, 0);
    let bank = slice(7).map(|(b, at)| {
        let (b, skip) = visual::scenery::level_bank(b, 0);
        (b, at + skip, 0)
    });
    Ok(model_inspection::model_from_payload(payload, bank, &[])?)
}

pub fn load(
    source: &ProbeSource,
    objects: &[objentry::ObjectEntry],
    object: u16,
    motion: u16,
) -> Result<Loaded, Box<dyn std::error::Error>> {
    let mut library = Library::open(source)?;
    let entry = objects
        .iter()
        .find(|o| o.object_id == u32::from(object))
        .ok_or_else(|| format!("objentry {object:#x} absent"))?;
    let name = String::from_utf8(entry.model_name.clone())?;
    let set = String::from_utf8(entry.animation_name.clone())?;
    // FUN_1403e0b50: this boss's model has a regional copy.
    let path = if name.contains("B_CA040") {
        format!("obj/us/{name}.mdlx")
    } else {
        format!("obj/{name}.mdlx")
    };
    let found = library
        .find(&path)
        .ok_or_else(|| format!("{path} is not installed"))?;
    let model = match model_inspection::read_model_at(
        source,
        found.index,
        found.ordinal,
        found.name_hash,
    ) {
        Ok(mut models) if !models.is_empty() => models.remove(0),
        _ => first_level(&mut library, &path)?,
    };
    let set = memo_set(&set, &name);
    let found = library
        .find(&set)
        .ok_or_else(|| format!("{set} is not installed"))?;
    let selected =
        motion::read_memo_motion(source, found.index, found.ordinal, found.name_hash, motion)?
            .ok_or_else(|| format!("{set} has no motion {motion:#x}"))?;
    let (start, end, fps, back) = selected.motion.frame_window()?;
    if !(1. ..=60.).contains(&fps) || !(0. ..end).contains(&start) {
        return Err(format!("{set} motion {motion:#x} frame window {start}..{end}").into());
    }
    let ticks = |frames: f32| (frames * 60. / fps).round();
    Ok(Loaded {
        model,
        motion: selected,
        start: ticks(start),
        total: ticks(end - start),
        back: ticks((back - start).max(0.)),
    })
}

/// `KH2RW_JOURNAL_MODEL_CENSUS=1`: every `char` row's model and motion read
/// once with the page's own loader, one line a row, off the main thread.
fn census(source: Arc<ProbeSource>, j: &JournalSource) {
    let (chars, objects) = (j.chars.clone(), j.objects.clone());
    std::thread::spawn(move || {
        let mut bad = 0;
        for (row, c) in chars.iter().enumerate() {
            let result = load(&source, &objects, c.object, c.motion);
            let parts = result.as_ref().map_or(0, |l| l.model.parts.len());
            let name = objects
                .iter()
                .find(|o| o.object_id == u32::from(c.object))
                .map(|o| String::from_utf8_lossy(&o.model_name).into_owned());
            let error = result.err().map(|e| e.to_string());
            bad += usize::from(error.is_some());
            eprintln!(
                "journal_model_census row {row} world {:#x} object {:#x} motion {:#x} model {name:?} parts {parts} flags {:#x} big {:?} {} error {error:?}",
                c.world, c.object, c.motion, c.flags, c.big, c.big_scale
            );
        }
        eprintln!("journal_model_census done rows {} bad {bad}", chars.len());
    });
}

#[derive(Component)]
pub struct JournalModelPart(usize);
#[derive(Component)]
pub struct JournalModelCamera;

struct Shown {
    key: (u16, u16),
    loaded: Loaded,
    root: Entity,
    indices: Vec<Vec<u32>>,
    /// The last clock reading and the page's ticks then.
    time: f32,
    ticks: u32,
    hidden: Option<u64>,
    /// The number's material and page, until its parts wear it.
    number: Option<(Handle<FogMaterial>, usize)>,
}

type ModelLoad = ((u16, u16), JoinHandle<Result<Loaded, String>>);

/// The page's model. Its target is made when a page first asks, and kept.
#[derive(Resource, Default)]
pub struct JournalModelSlot {
    loading: Option<ModelLoad>,
    shown: Option<Shown>,
    camera: Option<Entity>,
    census: bool,
    /// Tried once: the page stays without a model.
    rejected: Vec<(u16, u16)>,
}

/// The parts this page moves.
pub struct Scene<'a, 'w, 's> {
    pub commands: &'a mut Commands<'w, 's>,
    pub meshes: &'a mut Assets<Mesh>,
    pub materials: &'a mut Assets<FogMaterial>,
    pub images: &'a mut Assets<Image>,
    pub frame: &'a mut Ui2dFrame,
}

impl JournalModelSlot {
    /// Starts the page's load and takes a finished one.
    fn take_load(
        &mut self,
        open: &mut Journal,
        source: &Arc<ProbeSource>,
        objects: &Arc<Vec<objentry::ObjectEntry>>,
        scene: &mut Scene<'_, '_, '_>,
    ) {
        if let Some(key) = open.cache.wanted_model.take() {
            if self.shown.as_ref().is_some_and(|s| s.key == key) {
                open.cache.model_ready = Some(key);
            } else if !self.rejected.contains(&key)
                && self.loading.as_ref().is_none_or(|l| l.0 != key)
            {
                let (source, objects) = (source.clone(), objects.clone());
                let thread = std::thread::spawn(move || {
                    load(&source, &objects, key.0, key.1).map_err(|e| e.to_string())
                });
                self.loading = Some((key, thread));
            }
        }
        if self.loading.as_ref().is_none_or(|l| !l.1.is_finished()) {
            return;
        }
        let Some((key, thread)) = self.loading.take() else {
            return;
        };
        match thread.join() {
            Ok(Ok(loaded)) => {
                self.show(key, loaded, scene);
                open.cache.model_ready = Some(key);
            }
            Ok(Err(e)) => {
                eprintln!("Journal model {:#x} rejected: {e}", key.0);
                self.rejected.push(key);
            }
            Err(_) => {
                eprintln!("Journal model {:#x}: the load panicked", key.0);
                self.rejected.push(key);
            }
        }
    }

    fn show(&mut self, key: (u16, u16), loaded: Loaded, scene: &mut Scene<'_, '_, '_>) {
        if let Some(old) = self.shown.take() {
            scene.commands.entity(old.root).despawn();
        }
        let layer = RenderLayers::layer(LAYER);
        let root = scene
            .commands
            .spawn((
                Transform::default(),
                Visibility::Hidden,
                layer.clone(),
                JournalModelRoot,
            ))
            .id();
        let pose = positions(&loaded, 0.).unwrap_or_default();
        for (ordinal, entity) in spawn_model(
            scene.commands,
            &loaded.model,
            &pose,
            root,
            scene.meshes,
            scene.materials,
            scene.images,
        ) {
            if let Some(entity) = entity {
                scene
                    .commands
                    .entity(entity)
                    .insert((JournalModelPart(ordinal), layer.clone()));
            }
        }
        let number = number(&loaded, key.0, scene);
        let indices = loaded
            .model
            .parts
            .iter()
            .map(|p| p.indices.clone())
            .collect();
        self.shown = Some(Shown {
            key,
            loaded,
            root,
            indices,
            time: 0.,
            ticks: 0,
            hidden: None,
            number,
        });
    }

    /// The target and its camera: as the camp's, twice as wide and tall, so
    /// the page's shift never cuts the model (`MODEL_REACH`).
    fn camera(&mut self, sheets: &mut Option<usize>, scene: &mut Scene<'_, '_, '_>) {
        if self.camera.is_some() {
            return;
        }
        let target = scene.images.add(visual::room::render_target(
            MODEL_TARGET[0],
            MODEL_TARGET[1],
            TextureFormat::Rgba8UnormSrgb,
        ));
        let camera = scene
            .commands
            .spawn((
                Camera3d::default(),
                bevy::light::cluster::ClusterConfig::None,
                Camera {
                    order: -3,
                    is_active: false,
                    clear_color: ClearColorConfig::Custom(Color::NONE),
                    ..default()
                },
                bevy::camera::RenderTarget::Image(target.clone().into()),
                Projection::Perspective(PerspectiveProjection {
                    fov: 2. * (240. * MODEL_REACH / 879.1928).atan(),
                    near: 1.,
                    far: 100_000.,
                    ..default()
                }),
                Transform::from_xyz(0., 50., 800.).looking_at(Vec3::new(0., 0., -55.), Vec3::Y),
                Msaa::Off,
                Tonemapping::None,
                DebandDither::Disabled,
                NoIndirectDrawing,
                RenderLayers::layer(LAYER),
                JournalModelCamera,
            ))
            .id();
        self.camera = Some(camera);
        *sheets = Some(scene.frame.add_image(target, MODEL_TARGET, true));
    }
}

#[derive(Component)]
pub struct JournalModelRoot;

/// The vertices at clock time `t`.
fn positions(l: &Loaded, t: f32) -> Option<Vec<Vec3>> {
    let (pose, _) = l
        .model
        .skinning
        .sample_evaluated(&l.motion.motion, l.start + t)
        .ok()?;
    Some(
        pose.position_bits
            .iter()
            .map(|p| Vec3::from_array(p.map(f32::from_bits)))
            .collect(),
    )
}

/// FUN_1402ff990: the object's TEXA number, on the parts of its page.
fn number(
    l: &Loaded,
    object: u16,
    scene: &mut Scene<'_, '_, '_>,
) -> Option<(Handle<FogMaterial>, usize)> {
    let &(_, number) = NUMBERS.iter().find(|n| n.0 == object)?;
    let set = l.model.textures.as_ref()?;
    // A number outside the file's range is ignored (FUN_1401c06b0).
    let play = set
        .play
        .iter()
        .find(|p| p.patch.animation_for(number).is_some())?;
    let (width, height, rgba) = outfit::texa_page(set, play, number)?;
    let texture = visual::room::static_rgba_image(width, height, rgba, scene.images);
    let material = scene.materials.add_fogged(StandardMaterial {
        base_color: Color::WHITE,
        base_color_texture: Some(texture),
        unlit: true,
        cull_mode: None,
        alpha_mode: AlphaMode::Mask(0.5),
        ..default()
    });
    Some((material, play.patch.environment))
}

pub type Cameras<'w, 's> = Query<'w, 's, &'static mut Camera, With<JournalModelCamera>>;
pub type Roots<'w, 's> = Query<
    'w,
    's,
    (&'static mut Transform, &'static mut Visibility),
    (With<JournalModelRoot>, Without<JournalModelPart>),
>;
pub type Parts<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static JournalModelPart,
        &'static Mesh3d,
        &'static mut Visibility,
    ),
    Without<JournalModelRoot>,
>;

/// One frame: the load, the camera, the pose.
pub fn update(
    slot: &mut JournalModelSlot,
    camp: &mut CampRes,
    scene: &mut Scene<'_, '_, '_>,
    cameras: &mut Cameras,
    roots: &mut Roots,
    parts: &mut Parts,
) {
    if !slot.census
        && std::env::var_os("KH2RW_JOURNAL_MODEL_CENSUS").is_some()
        && let (Some(source), Some(j)) = (&camp.picture_source, &camp.journal_source)
    {
        slot.census = true;
        census(source.clone(), j);
    }
    let view = match (
        camp.journal.as_mut(),
        camp.picture_source.as_ref(),
        camp.journal_source.as_ref(),
    ) {
        (Some(open), Some(source), Some(j)) => {
            if open.cache.wanted_model.is_some() || slot.loading.is_some() {
                slot.camera(&mut camp.journal_sheets.model, scene);
                slot.take_load(open, source, &j.objects, scene);
            }
            open.model_view()
        }
        _ => None,
    };
    let want = view.filter(|v| {
        slot.shown
            .as_ref()
            .is_some_and(|s| s.key == (v.object, v.motion))
    });
    for mut camera in cameras.iter_mut() {
        camera.is_active = want.is_some();
    }
    let visible = if want.is_some() {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };
    for (_, mut v) in roots.iter_mut() {
        if *v != visible {
            *v = visible;
        }
    }
    let (Some(view), Some(shown)) = (want, slot.shown.as_mut()) else {
        return;
    };
    // The page counts its updates; a pose is only worked out when shown.
    for _ in shown.ticks..view.ticks {
        shown.time = clock(shown.time, shown.loaded.total, shown.loaded.back);
    }
    if view.ticks < shown.ticks {
        shown.time = 0.;
    }
    shown.ticks = view.ticks;
    if let Ok((mut t, _)) = roots.get_mut(shown.root) {
        *t = Transform {
            translation: Vec3::new(0., 0., -view.z),
            rotation: Quat::from_rotation_y(-view.turn.to_radians()),
            scale: Vec3::splat(view.scale),
        };
    }
    let number = shown.number.take();
    let positions = positions(&shown.loaded, shown.time);
    for (entity, part, mesh, mut v) in parts.iter_mut() {
        if let Some((material, page)) = &number {
            let on_page = shown
                .loaded
                .model
                .texture_routes
                .first()
                .and_then(|r| r.parts.get(part.0))
                .is_some_and(|r| r.environment_ordinal == *page);
            if on_page {
                scene
                    .commands
                    .entity(entity)
                    .insert(MeshMaterial3d(material.clone()));
            }
        }
        if shown.hidden != Some(view.hidden_parts) {
            // [I: the mask counts the model's mesh parts in order.]
            let hide = part.0 < 64 && view.hidden_parts >> part.0 & 1 != 0;
            let want = if hide {
                Visibility::Hidden
            } else {
                Visibility::Inherited
            };
            if *v != want {
                *v = want;
            }
        }
        if let (Some(p), Some(indices)) = (&positions, shown.indices.get(part.0)) {
            visual::room::set_positions(
                scene.meshes,
                &mesh.0,
                indices.iter().map(|&i| p[i as usize].to_array()),
            );
        }
    }
    shown.hidden = Some(view.hidden_parts);
}

/// The room's system.
#[allow(clippy::too_many_arguments)]
pub fn present(
    mut commands: Commands,
    mut slot: ResMut<JournalModelSlot>,
    camp: Option<ResMut<CampRes>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<FogMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut frame: ResMut<Ui2dFrame>,
    mut cameras: Cameras,
    mut roots: Roots,
    mut parts: Parts,
) {
    let Some(mut camp) = camp else { return };
    update(
        &mut slot,
        &mut camp,
        &mut Scene {
            commands: &mut commands,
            meshes: &mut meshes,
            materials: &mut materials,
            images: &mut images,
            frame: &mut frame,
        },
        &mut cameras,
        &mut roots,
        &mut parts,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memo_names_follow_the_set_or_the_model() {
        assert_eq!(memo_set("M_EX020.mset", "M_EX020"), "obj/M_EX020_MEMO.mset");
        assert_eq!(memo_set("", "M_EX030"), "obj/M_EX030_MEMO.mset");
    }

    #[test]
    fn the_clock_wraps_past_the_total_and_stays_above_the_return() {
        assert_eq!(clock(0., 10., 0.), 1.);
        assert_eq!(clock(10., 10., 0.), 1.);
        assert_eq!(clock(5., 20., 10.), 16.);
    }
}
