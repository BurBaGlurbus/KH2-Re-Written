//! Source-bound 3D research. Explicit texture/UV choices; no installed rendering claim.
use bevy::{
    app::AppExit,
    asset::RenderAssetUsages,
    camera::ScalingMode,
    core_pipeline::tonemapping::{DebandDither, Tonemapping},
    ecs::system::SystemParam,
    image::ImageSampler,
    mesh::VertexAttributeValues,
    prelude::*,
    render::{
        render_resource::{Extent3d, PrimitiveTopology, TextureDimension, TextureFormat},
        view::{
            NoIndirectDrawing,
            screenshot::{Screenshot, ScreenshotCaptured},
        },
    },
    window::PrimaryWindow,
};
use data::{
    model_inspection::{InspectionModel, read_selected_models},
    model_inspection_textures::TextureMode,
    probe_source::ProbeSource,
};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    fs::{File, OpenOptions},
    io::Write,
    path::PathBuf,
};

#[derive(Component)]
struct ModelPart;
#[derive(SystemParam)]
struct InspectorAssets<'w> {
    meshes: ResMut<'w, Assets<Mesh>>,
    materials: ResMut<'w, Assets<StandardMaterial>>,
    images: ResMut<'w, Assets<Image>>,
}
#[derive(Resource)]
struct Inspector {
    models: Vec<InspectionModel>,
    item: usize,
    pose: usize,
    uv: usize,
    texture_mode: usize,
    part: Option<usize>,
    direction: Vec3,
    up: Vec3,
    zoom: f32,
    dirty: bool,
    mesh_handles: Vec<Handle<Mesh>>,
    material_handles: Vec<Handle<StandardMaterial>>,
    image_handles: Vec<Handle<Image>>,
    log: File,
    frames: u64,
    dwell: u64,
    settled_at: f64,
    exit_after: Option<u64>,
    tour: bool,
    direct_draw: bool,
    step: usize,
    paths: Vec<PathBuf>,
    pending: bool,
    captured: usize,
}
impl Inspector {
    fn event(&mut self, value: serde_json::Value) {
        serde_json::to_writer(&mut self.log, &value).expect("private model log write");
        writeln!(self.log).expect("private model log newline");
        self.log.flush().expect("private model log flush");
    }
    fn tour_state(&mut self) {
        let mut remaining = self.step;
        for (item, model) in self.models.iter().enumerate() {
            let count =
                model.poses.len() * model.uv_profile_count() * model.texture_modes().len() * 4;
            if remaining < count {
                self.item = item;
                let choice = remaining / 4;
                self.pose = choice / (model.uv_profile_count() * model.texture_modes().len());
                self.uv = (choice / model.texture_modes().len()) % model.uv_profile_count();
                self.texture_mode = choice % model.texture_modes().len();
                self.part = (remaining % 4 == 3).then(|| {
                    model
                        .parts
                        .iter()
                        .enumerate()
                        .filter(|(_, p)| !p.indices.is_empty())
                        .max_by_key(|(_, p)| p.cross_part_corners)
                        .unwrap()
                        .0
                });
                (self.direction, self.up) = axes(remaining % 4);
                self.zoom = 1.0;
                self.dirty = true;
                self.dwell = 0;
                return;
            }
            remaining -= count;
        }
        panic!("bounded model tour state");
    }
}
fn axes(view: usize) -> (Vec3, Vec3) {
    match view {
        1 => (Vec3::NEG_Y, Vec3::Z),
        2 => (Vec3::X, Vec3::Y),
        _ => (Vec3::Z, Vec3::Y),
    }
}
fn color(part: usize) -> [u8; 3] {
    [53, 97, 29].map(|n| (64 + part * n % 128) as u8)
}
/// Presentation-only common fit: both profiles share one center/radius, so
/// selecting a profile never silently changes the inspection normalization.
fn frame(model: &InspectionModel) -> ([f64; 3], f64) {
    let mut low = [f64::INFINITY; 3];
    let mut high = [f64::NEG_INFINITY; 3];
    for position in model.poses.iter().flat_map(|pose| &pose.position_bits) {
        for axis in 0..3 {
            let value = f32::from_bits(position[axis]) as f64;
            low[axis] = low[axis].min(value);
            high[axis] = high[axis].max(value);
        }
    }
    let center = std::array::from_fn(|axis| (low[axis] + high[axis]) * 0.5);
    let radius = low
        .iter()
        .zip(high)
        .map(|(&l, h)| (h - l) * 0.5)
        .fold(0.0f64, f64::max)
        .max(f64::MIN_POSITIVE);
    (center, radius)
}
fn run(args: Vec<PathBuf>) -> Result<AppExit, Box<dyn std::error::Error>> {
    let mut args = args.into_iter();
    let root = args.next().ok_or("selected install root required")?;
    let manifest = args.next().ok_or("source manifest required")?;
    let selection = args.next().ok_or("explicit model selection required")?;
    let output = args.next().ok_or("new private JSONL log required")?;
    let mut tour = false;
    let mut direct_draw = false;
    let mut capture_dir = None;
    let mut exit_after = None;
    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some("--tour") => tour = true,
            Some("--direct-draw") => direct_draw = true,
            Some("--capture-dir") => {
                capture_dir = Some(args.next().ok_or("capture directory required")?)
            }
            Some("--frames") => {
                let value = args
                    .next()
                    .ok_or("frame count required")?
                    .to_string_lossy()
                    .parse::<u64>()?;
                if !(180..=100000).contains(&value) {
                    return Err("frame count outside 180..100000".into());
                }
                exit_after = Some(value);
            }
            _ => return Err("unknown model inspector option".into()),
        }
    }
    if tour && capture_dir.is_none() {
        return Err("tour requires an existing private capture directory".into());
    }
    let source = ProbeSource::bind(&root, &manifest)?;
    let output = source.output_path(&output)?;
    if let Some(directory) = &capture_dir {
        source.output_path(&directory.join("capture-admission.png"))?;
    }
    eprintln!("Fresh full selected-source scan; model research remains quarantined.");
    let models = read_selected_models(&source, &selection)?;
    let count = if tour {
        models
            .iter()
            .map(|model| {
                model.poses.len() * model.uv_profile_count() * model.texture_modes().len() * 4
            })
            .sum()
    } else {
        usize::from(capture_dir.is_some())
    };
    let mut paths = Vec::new();
    if let Some(directory) = capture_dir {
        for step in 0..count {
            paths.push(source.output_path(&directory.join(format!("model-{step:03}.png")))?);
        }
    }
    let mut state = Inspector {
        models,
        item: 0,
        pose: 0,
        uv: 0,
        texture_mode: 0,
        part: None,
        direction: Vec3::Z,
        up: Vec3::Y,
        zoom: 1.0,
        dirty: true,
        mesh_handles: Vec::new(),
        material_handles: Vec::new(),
        image_handles: Vec::new(),
        log: OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(output)?,
        frames: 0,
        dwell: 0,
        settled_at: 0.0,
        exit_after,
        tour,
        direct_draw,
        step: 0,
        paths,
        pending: false,
        captured: 0,
    };
    let textured = state.models[0].textures.is_some();
    state.event(serde_json::json!({"event":"admitted","qualification":"quarantined_model_research_only",
        "source_content_identity":source.content_identity(),"fresh_full_scan":true,"models":state.models.len(),
        "tour":tour,"expected_captures":count,"gameplay_enabled":false,
        "drawing_profile":if direct_draw {"bevy_no_indirect_drawing_v1"} else {"bevy_default_drawing_v1"},
        "presentation":if textured {"common fit; orthographic; explicit texture/UV choices; base mip zero; nearest clamp; SRGB texture; fixed opaque/unlit/double-sided; missing or ambiguous HD is magenta; no installed material/animation/filtering claim"} else {"common fit; orthographic; unlit part colors; double-sided opaque triangles; no source material, normal or renderer claim"}}));
    if textured {
        let details:Vec<_>=state.models.iter().enumerate().map(|(item,m)|serde_json::json!({"item":item,"index":m.index,"ordinal":m.ordinal,
            "bar_ordinal":m.bar_ordinal,"textures":m.textures.as_ref().map(|t|t.as_ref()),"uv_routes":m.texture_routes})).collect();
        state.event(serde_json::json!({"event":"texture_admission","models":details,"gameplay_enabled":false,"runtime_verified":false}));
    }
    if tour {
        state.tour_state();
    }
    let mut app = App::new();
    app.insert_resource(state)
        .insert_resource(ClearColor(Color::BLACK))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "QUARANTINED KH2FM model research".into(),
                resolution:
                    bevy::window::WindowResolution::new(900, 900).with_scale_factor_override(1.0),
                ..default()
            }),
            ..default()
        }))
        .add_systems(Startup, setup)
        .add_systems(Update, (controls, refresh, finish).chain());
    Ok(app.run())
}
fn setup(mut commands: Commands, state: Res<Inspector>) {
    let camera = commands
        .spawn((
            Camera3d::default(),
            Projection::Orthographic(OrthographicProjection {
                near: 0.1,
                far: 10.0,
                scaling_mode: ScalingMode::FixedVertical {
                    viewport_height: 2.6,
                },
                ..OrthographicProjection::default_3d()
            }),
            Transform::from_xyz(0.0, 0.0, 4.0).looking_at(Vec3::ZERO, Vec3::Y),
            Msaa::Off,
            Tonemapping::None,
            DebandDither::Disabled,
        ))
        .id();
    if state.direct_draw {
        commands.entity(camera).insert(NoIndirectDrawing);
    }
}
fn controls(
    keys: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<Inspector>,
    mut exit: MessageWriter<AppExit>,
) {
    if keys.just_pressed(KeyCode::Escape) {
        let complete = !state.tour || state.captured == state.paths.len();
        state.event(serde_json::json!({"event":"escape","tour_complete":complete}));
        exit.write(if complete {
            AppExit::Success
        } else {
            AppExit::Error(std::num::NonZeroU8::new(1).unwrap())
        });
    }
    if state.tour || state.pending {
        return;
    }
    if keys.just_pressed(KeyCode::KeyT) {
        state.texture_mode =
            (state.texture_mode + 1) % state.models[state.item].texture_modes().len();
        state.dirty = true;
    }
    if keys.just_pressed(KeyCode::KeyU) {
        state.uv = (state.uv + 1) % state.models[state.item].uv_profile_count();
        state.dirty = true;
    }
    for (key, delta) in [(KeyCode::ArrowRight, 1isize), (KeyCode::ArrowLeft, -1)] {
        if keys.just_pressed(key) {
            state.item =
                (state.item as isize + delta).rem_euclid(state.models.len() as isize) as usize;
            state.pose = 0;
            state.part = None;
            state.dirty = true;
        }
    }
    if keys.just_pressed(KeyCode::KeyP) {
        state.pose = (state.pose + 1) % state.models[state.item].poses.len();
        state.dirty = true;
    }
    if keys.just_pressed(KeyCode::Space) {
        state.part = match state.part {
            None => Some(0),
            Some(p) if p + 1 < state.models[state.item].parts.len() => Some(p + 1),
            _ => None,
        };
        state.dirty = true;
    }
    for (key, view) in [
        (KeyCode::Digit1, 0),
        (KeyCode::Digit2, 1),
        (KeyCode::Digit3, 2),
    ] {
        if keys.just_pressed(key) {
            (state.direction, state.up) = axes(view);
            state.dirty = true;
        }
    }
    for (key, axis, angle) in [
        (KeyCode::KeyJ, Vec3::Y, -0.15),
        (KeyCode::KeyL, Vec3::Y, 0.15),
        (KeyCode::KeyI, Vec3::X, -0.15),
        (KeyCode::KeyK, Vec3::X, 0.15),
    ] {
        if keys.just_pressed(key) {
            let rotation = Quat::from_axis_angle(axis, angle);
            state.direction = rotation * state.direction;
            state.up = rotation * state.up;
            state.dirty = true;
        }
    }
    for (key, factor) in [(KeyCode::Equal, 0.8), (KeyCode::Minus, 1.25)] {
        if keys.just_pressed(key) {
            state.zoom = (state.zoom * factor).clamp(0.125, 8.0);
            state.dirty = true;
        }
    }
    if keys.just_pressed(KeyCode::KeyF) {
        state.zoom = 1.0;
        state.dirty = true;
    }
}
fn refresh(
    mut commands: Commands,
    mut state: ResMut<Inspector>,
    assets: InspectorAssets,
    parts: Query<Entity, With<ModelPart>>,
    mut cameras: Query<(&mut Transform, &mut Projection, Has<NoIndirectDrawing>), With<Camera3d>>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
    time: Res<Time<Real>>,
) {
    if !state.dirty {
        return;
    }
    let InspectorAssets {
        mut meshes,
        mut materials,
        mut images,
    } = assets;
    for entity in &parts {
        commands.entity(entity).despawn();
    }
    for handle in state.mesh_handles.drain(..) {
        meshes.remove(handle.id());
    }
    for handle in state.material_handles.drain(..) {
        materials.remove(handle.id());
    }
    for handle in state.image_handles.drain(..) {
        images.remove(handle.id());
    }
    let model = &state.models[state.item];
    let pose = &model.poses[state.pose];
    let (center, radius) = frame(model);
    let normalized: Vec<[f32; 3]> = pose
        .position_bits
        .iter()
        .map(|position| {
            std::array::from_fn(|axis| {
                ((f32::from_bits(position[axis]) as f64 - center[axis]) / radius) as f32
            })
        })
        .collect();
    let mut rendered = Vec::new();
    let mut mesh_handles = Vec::new();
    let mut material_handles = Vec::new();
    let mut image_handles: HashMap<usize, Handle<Image>> = HashMap::new();
    let mode = model.texture_modes()[state.texture_mode];
    for (ordinal, part) in model.parts.iter().enumerate() {
        if part.indices.is_empty() || state.part.is_some_and(|p| p != ordinal) {
            continue;
        }
        let positions: Vec<_> = part
            .indices
            .iter()
            .map(|&i| normalized[i as usize])
            .collect();
        let mut mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions);
        let Some(VertexAttributeValues::Float32x3(actual_positions)) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("mesh position attribute");
        };
        let words: Vec<[u32; 3]> = actual_positions
            .iter()
            .map(|p| p.map(f32::to_bits))
            .collect();
        let mut rgb = color(ordinal);
        let mut texture_handle = None;
        let mut observation =
            serde_json::json!({"part":ordinal,"source":part,"rgb":rgb,"mesh_position_bits":words});
        if let Some(set) = &model.textures {
            let route = &model.texture_routes[state.uv].parts[ordinal];
            let binding = set
                .bind(route.environment_ordinal, mode)
                .expect("admitted texture binding");
            if mode != TextureMode::PartColors {
                mesh.insert_attribute(
                    Mesh::ATTRIBUTE_UV_0,
                    route
                        .expanded_uv_bits
                        .iter()
                        .map(|uv| uv.map(f32::from_bits))
                        .collect::<Vec<_>>(),
                );
                let Some(VertexAttributeValues::Float32x2(uvs)) =
                    mesh.attribute(Mesh::ATTRIBUTE_UV_0)
                else {
                    panic!("mesh UV attribute")
                };
                observation["mesh_uv_bits"] = serde_json::to_value(
                    uvs.iter()
                        .map(|uv| uv.map(f32::to_bits))
                        .collect::<Vec<_>>(),
                )
                .unwrap();
                if let Some(image_index) = binding.image_index {
                    let image = &set.images[image_index];
                    let level = &image.levels[0];
                    let handle = image_handles
                        .entry(image_index)
                        .or_insert_with(|| {
                            let mut image = Image::new(
                                Extent3d {
                                    width: level.width,
                                    height: level.height,
                                    depth_or_array_layers: 1,
                                },
                                TextureDimension::D2,
                                level.pixels_rgba.clone(),
                                TextureFormat::Rgba8UnormSrgb,
                                RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
                            );
                            image.sampler = ImageSampler::nearest();
                            images.add(image)
                        })
                        .clone();
                    observation["image"] = serde_json::to_value(image).unwrap();
                    observation["image_upload_input_sha256"] = format!(
                        "{:x}",
                        Sha256::digest(images.get(&handle).unwrap().data.as_ref().unwrap())
                    )
                    .into();
                    observation["uploaded_mip_count"] = images
                        .get(&handle)
                        .unwrap()
                        .texture_descriptor
                        .mip_level_count
                        .into();
                    texture_handle = Some(handle);
                    rgb = [255; 3];
                } else {
                    rgb = [255, 0, 255];
                }
            }
            observation["texture_binding"] = serde_json::to_value(binding).unwrap();
            observation["rgb"] = serde_json::to_value(rgb).unwrap();
        }
        rendered.push(observation);
        let mesh = meshes.add(mesh);
        let material = materials.add(StandardMaterial {
            base_color: Color::srgb_u8(rgb[0], rgb[1], rgb[2]),
            base_color_texture: texture_handle,
            alpha_mode: AlphaMode::Opaque,
            unlit: true,
            cull_mode: None,
            ..default()
        });
        commands.spawn((
            Mesh3d(mesh.clone()),
            MeshMaterial3d(material.clone()),
            ModelPart,
        ));
        mesh_handles.push(mesh);
        material_handles.push(material);
    }
    let mut metadata = serde_json::json!({"event":"view","step":state.step,"item":state.item,"index":model.index,
        "ordinal":model.ordinal,"bar_ordinal":model.bar_ordinal,"pose_profile":pose.profile,
        "arithmetic_profile":pose.arithmetic_profile,"runtime_verified":false,"excluded_linked_groups":model.excluded_linked_groups,
        "source_position_bits":pose.position_bits,"center":center,"radius":radius,"part":state.part,
        "direction":state.direction.to_array(),"up":state.up.to_array(),"zoom":state.zoom,"rendered":rendered});
    if model.textures.is_some() {
        metadata["texture_mode"] = serde_json::to_value(mode).unwrap();
        metadata["uv_profile"] =
            serde_json::to_value(model.texture_routes[state.uv].profile).unwrap();
    }
    let (mut transform, mut projection, no_indirect_drawing) =
        cameras.single_mut().expect("research model camera");
    assert_eq!(
        no_indirect_drawing, state.direct_draw,
        "inspection camera drawing mode"
    );
    *transform =
        Transform::from_translation(state.direction * 4.0).looking_at(Vec3::ZERO, state.up);
    if let Projection::Orthographic(p) = &mut *projection {
        p.scale = state.zoom;
    }
    windows.single_mut().expect("research model window").title = if model.textures.is_some() {
        format!(
            "QUARANTINED model {}/{} | {:?} | T texture {:?}; U UV {:?} | part {:?} | P pose; Space part; 1/2/3 axes; I/J/K/L orbit; +/-; F fit; Esc",
            state.item + 1,
            state.models.len(),
            pose.profile,
            mode,
            model.texture_routes[state.uv].profile,
            state.part
        )
    } else {
        format!(
            "QUARANTINED model {}/{} | {:?} | part {:?} | arrows; P pose; Space part; 1/2/3 axes; I/J/K/L orbit; +/-; F fit; Esc",
            state.item + 1,
            state.models.len(),
            pose.profile,
            state.part
        )
    };
    state.mesh_handles = mesh_handles;
    state.material_handles = material_handles;
    state.image_handles = image_handles.into_values().collect();
    let camera_metadata = serde_json::json!({"event":"camera_drawing","step":state.step,
        "no_indirect_drawing":no_indirect_drawing,"installed_drawing_mode_verified":false});
    state.event(camera_metadata);
    state.event(metadata);
    state.dirty = false;
    state.dwell = 0;
    state.settled_at = time.elapsed_secs_f64();
}
fn finish(
    mut commands: Commands,
    mut state: ResMut<Inspector>,
    mut exit: MessageWriter<AppExit>,
    time: Res<Time<Real>>,
) {
    state.frames += 1;
    state.dwell += 1;
    if !state.pending
        && !state.dirty
        && state.captured < state.paths.len()
        && state.dwell >= if state.step == 0 { 180 } else { 30 }
        && time.elapsed_secs_f64() - state.settled_at >= if state.step == 0 { 5.0 } else { 1.0 }
    {
        let path = state.paths[state.step].clone();
        state.pending = true;
        commands.spawn(Screenshot::primary_window()).observe(move |event: On<ScreenshotCaptured>, mut state: ResMut<Inspector>| {
            let dynamic = event.image.clone().try_into_dynamic().expect("model GPU screenshot conversion");
            let rgba = dynamic.to_rgba8();
            let mut file = OpenOptions::new().write(true).create_new(true).open(&path).expect("new private model capture");
            dynamic.write_to(&mut file, image::ImageFormat::Png).expect("private model capture encode");
            file.sync_all().expect("private model capture flush");
            let frames = state.frames; let step = state.step;
            state.event(serde_json::json!({"event":"captured","step":step,"frames":frames,
                "file":path.file_name().and_then(|name|name.to_str()),"width":rgba.width(),"height":rgba.height(),
                "pixels_sha256":format!("{:x}",Sha256::digest(rgba.as_raw()))}));
            state.pending = false; state.captured += 1;
            if state.tour && state.captured < state.paths.len() { state.step += 1; state.tour_state(); }
        });
    }
    if (state.tour && !state.pending && state.captured == state.paths.len())
        || state.exit_after.is_some_and(|limit| state.frames >= limit)
    {
        let complete = !state.pending && state.captured == state.paths.len();
        let frames = state.frames;
        let captures = state.captured;
        state.event(serde_json::json!({"event":"exit","frames":frames,"captures":captures,"complete":complete}));
        exit.write(if complete {
            AppExit::Success
        } else {
            AppExit::Error(std::num::NonZeroU8::new(1).unwrap())
        });
    }
}
pub fn main(args: Vec<PathBuf>) -> std::process::ExitCode {
    match run(args) {
        Ok(AppExit::Success) => std::process::ExitCode::SUCCESS,
        Ok(_) => std::process::ExitCode::FAILURE,
        Err(error) => {
            eprintln!("Model research rejected: {error}");
            std::process::ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use data::model_pose::{ModelPose, PoseProfile};
    #[test]
    fn common_frame_preserves_profile_displacements_and_handles_extreme_finite_coordinates() {
        let poses = [PoseProfile::Legacy, PoseProfile::Modern]
            .into_iter()
            .enumerate()
            .map(|(i, profile)| ModelPose {
                profile,
                arithmetic_profile: "authored",
                matrix_bits: Vec::new(),
                position_bits: vec![
                    [if i == 0 { -f32::MAX } else { f32::MAX }; 3].map(f32::to_bits),
                ],
                runtime_verified: false,
            })
            .collect();
        let model = InspectionModel {
            skinning: data::model_pose::Skinning::default(),
            index: String::new(),
            ordinal: 0,
            bar_ordinal: 0,
            excluded_linked_groups: 0,
            parts: Vec::new(),
            poses,
            textures: None,
            texture_routes: Vec::new(),
        };
        let (center, radius) = frame(&model);
        assert_eq!(center, [0.0; 3]);
        assert_eq!(radius, f32::MAX as f64);
        assert_eq!((f32::MAX as f64 / radius) as f32, 1.0);
        assert_eq!((-f32::MAX as f64 / radius) as f32, -1.0);
    }
}
