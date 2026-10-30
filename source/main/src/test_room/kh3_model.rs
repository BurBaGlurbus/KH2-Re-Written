//! KH3 Sora on the duel's screen [T282]: the mesh, Kingdom Key and clips read
//! from the player's own KH3 paks (`data::kh3::sora`), one rig per seat, posed
//! each frame from the layer's `Visual`. KH2's rig stays underneath, hidden: the
//! KH2 effects that are still anchored to its bones keep their places.
//!
//! The look is good enough, not KH3's shaders: base colours and flat colours
//! from the material parameters, lit by one directional light.

#![allow(clippy::type_complexity)]

use super::*;
use bevy::image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
use bevy::mesh::skinning::{SkinnedMesh, SkinnedMeshInverseBindposes};
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use data::kh3::anim::BonePose;
use data::kh3::skeletal::{Lod, SkeletalMesh};
use data::kh3::sora::{self, Animator, SlotLook, SoraData};
use data::kh3::texture::Texture;

/// The loaded KH3 assets of a session.
pub(crate) struct Loaded {
    pub(crate) sora: SoraData,
    pub(crate) book: Arc<player::kh3::attack::Book>,
    pub(crate) fx: Arc<kh3_fx::FxData>,
}

type Job = std::thread::JoinHandle<Result<Loaded, String>>;

#[derive(Resource, Default)]
pub(crate) struct Kh3Model {
    job: Option<Job>,
    pub(crate) loaded: Option<Arc<Loaded>>,
    pub(crate) error: Option<String>,
    shared: Option<Shared>,
    rigs: [Option<Rig>; 2],
}

/// The handles both rigs use.
struct Shared {
    body: Vec<Part>,
    blade: Vec<Part>,
    body_joints: Vec<usize>,
    blade_joints: Vec<usize>,
    body_binds: Handle<SkinnedMeshInverseBindposes>,
    blade_binds: Handle<SkinnedMeshInverseBindposes>,
}

struct Part {
    mesh: Handle<Mesh>,
    material: Handle<FogMaterial>,
    /// The game's own shading, when its shader was translated.
    shaded: Option<kh3_material::Kh3Material>,
}

struct Rig {
    root: Entity,
    joints: Vec<(usize, Entity)>,
    blade: Entity,
    blade_joints: Vec<(usize, Entity)>,
    animator: Animator,
    component: Vec<BonePose>,
    secondary: Option<kh3_cloth::Secondary>,
    /// This Sora's materials, which follow his actor.
    shaded: Vec<Handle<kh3_material::Kh3Material>>,
}

#[derive(Component)]
pub(crate) struct Kh3Root;
#[derive(Component)]
pub(crate) struct Kh3Light;

/// The actor's place in KH3's world for the cloth.
fn kh3_cloth_at(position: [f32; 3]) -> kh3_rig::glam::Vec3 {
    kh3_rig::glam::Vec3::from_array(kh3_fx::to_kh3(Vec3::from_array(position)).to_array())
}

/// KH3's centimetres to the rewrite's units (Sora's size against the room).
fn scale() -> f32 {
    player::kh3::tuning::length_of(1.)
}

/// A pose in UE's axes (Z up, left-handed) to ours (Y up, right-handed):
/// the mesh's +Y (his front) goes to +Z.
fn converted(p: &BonePose, k: f32) -> Transform {
    Transform {
        translation: Vec3::new(p.translation[0], p.translation[2], p.translation[1]) * k,
        rotation: Quat::from_xyzw(
            -p.rotation[0],
            -p.rotation[2],
            -p.rotation[1],
            p.rotation[3],
        ),
        scale: Vec3::new(p.scale[0], p.scale[2], p.scale[1]),
    }
}

fn image_of(t: &Texture, srgb: bool) -> Option<Image> {
    let format = match (t.format.as_str(), srgb) {
        ("PF_DXT1", true) => TextureFormat::Bc1RgbaUnormSrgb,
        ("PF_DXT1", false) => TextureFormat::Bc1RgbaUnorm,
        ("PF_DXT5", true) => TextureFormat::Bc3RgbaUnormSrgb,
        ("PF_DXT5", false) => TextureFormat::Bc3RgbaUnorm,
        ("PF_BC7", true) => TextureFormat::Bc7RgbaUnormSrgb,
        ("PF_BC7", false) => TextureFormat::Bc7RgbaUnorm,
        ("PF_BC5", _) => TextureFormat::Bc5RgUnorm,
        ("PF_BC4", _) => TextureFormat::Bc4RUnorm,
        ("PF_B8G8R8A8", true) => TextureFormat::Rgba8UnormSrgb,
        ("PF_B8G8R8A8", false) => TextureFormat::Rgba8Unorm,
        _ => return None,
    };
    let first = t.mips.first()?;
    // Compressed mips below 4 x 4 keep a whole block, as a texture's chain
    // does; the GPU takes the chain down to where the sizes stay whole.
    let usable: Vec<_> = t
        .mips
        .iter()
        .take_while(|m| m.width >= 4 && m.height >= 4)
        .collect();
    let mut image = Image::default();
    image.texture_descriptor.size = Extent3d {
        width: first.width,
        height: first.height,
        depth_or_array_layers: 1,
    };
    image.texture_descriptor.dimension = TextureDimension::D2;
    image.texture_descriptor.format = format;
    image.texture_descriptor.mip_level_count = usable.len().max(1) as u32;
    let mut bytes: Vec<u8> = usable
        .iter()
        .flat_map(|m| m.bytes.iter().copied())
        .collect();
    if t.format == "PF_B8G8R8A8" {
        for texel in bytes.chunks_exact_mut(4) {
            texel.swap(0, 2);
        }
    }
    image.data = Some(bytes);
    let address = |name: &str| match t.props.str(name) {
        Some(mode) if mode.ends_with("TA_Clamp") => ImageAddressMode::ClampToEdge,
        Some(mode) if mode.ends_with("TA_Mirror") => ImageAddressMode::MirrorRepeat,
        _ => ImageAddressMode::Repeat,
    };
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: address("AddressX"),
        address_mode_v: address("AddressY"),
        mag_filter: ImageFilterMode::Linear,
        min_filter: ImageFilterMode::Linear,
        mipmap_filter: ImageFilterMode::Linear,
        anisotropy_clamp: 4,
        ..default()
    });
    Some(image)
}

fn lod_meshes(lod: &Lod, joint_of: &[u16], k: f32) -> Vec<(u16, Mesh)> {
    let mut out = Vec::with_capacity(lod.sections.len());
    for s in &lod.sections {
        let from = s.base_vertex as usize;
        let to = (from + s.vertices.max(0) as usize).min(lod.positions.len());
        if from >= to {
            continue;
        }
        let first = s.base_index as usize;
        let last = (first + s.triangles as usize * 3).min(lod.indices.len());
        let indices: Vec<u32> = lod.indices[first..last]
            .iter()
            .map(|i| i.saturating_sub(s.base_vertex))
            .collect();
        let positions: Vec<[f32; 3]> = lod.positions[from..to]
            .iter()
            .map(|p| [p[0] * k, p[2] * k, p[1] * k])
            .collect();
        let normals: Vec<[f32; 3]> = lod.normals[from..to]
            .iter()
            .map(|n| [n[0], n[2], n[1]])
            .collect();
        let joints: Vec<[u16; 4]> = lod.joints[from..to]
            .iter()
            .map(|j| j.map(|b| joint_of.get(usize::from(b)).copied().unwrap_or(0)))
            .collect();
        let mut mesh = Mesh::new(
            bevy::mesh::PrimitiveTopology::TriangleList,
            bevy::asset::RenderAssetUsages::RENDER_WORLD,
        );
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, lod.uv0[from..to].to_vec());
        if lod.uv1.len() >= to {
            mesh.insert_attribute(Mesh::ATTRIBUTE_UV_1, lod.uv1[from..to].to_vec());
        }
        if lod.tangents.len() >= to {
            let tangents: Vec<[f32; 4]> = lod.tangents[from..to]
                .iter()
                .zip(&lod.normals[from..to])
                .map(|(t, n)| [t[0], t[2], t[1], n[3]])
                .collect();
            mesh.insert_attribute(Mesh::ATTRIBUTE_TANGENT, tangents);
        }
        mesh.insert_attribute(
            Mesh::ATTRIBUTE_JOINT_INDEX,
            bevy::mesh::VertexAttributeValues::Uint16x4(joints),
        );
        mesh.insert_attribute(Mesh::ATTRIBUTE_JOINT_WEIGHT, lod.weights[from..to].to_vec());
        mesh.insert_indices(bevy::mesh::Indices::U32(indices));
        out.push((s.material, mesh));
    }
    out
}

/// The joints the first LOD uses: skeleton bones in order, and each bone's
/// place among them (a mesh takes at most 256 joints; Sora has 274 bones).
fn used_joints(mesh: &SkeletalMesh) -> (Vec<usize>, Vec<u16>) {
    let mut used = vec![false; mesh.bones.len()];
    for (j, w) in mesh.lod.joints.iter().zip(&mesh.lod.weights) {
        for k in 0..4 {
            if w[k] > 0.
                && let Some(u) = used.get_mut(usize::from(j[k]))
            {
                *u = true;
            }
        }
    }
    let bones: Vec<usize> = (0..used.len()).filter(|&i| used[i]).collect();
    let mut place = vec![0u16; used.len()];
    for (at, &b) in bones.iter().enumerate() {
        place[b] = at as u16;
    }
    (bones, place)
}

fn binds(mesh: &SkeletalMesh, bones: &[usize], k: f32) -> Vec<Mat4> {
    let local: Vec<BonePose> = mesh
        .bones
        .iter()
        .map(|b| BonePose {
            rotation: b.rotation,
            translation: b.translation,
            scale: b.scale,
        })
        .collect();
    let mut component = Vec::new();
    sora::to_component(&mesh.bones, &local, &mut component);
    bones
        .iter()
        .map(|&b| converted(&component[b], k).to_matrix().inverse())
        .collect()
}

fn material_of(
    look: &SlotLook,
    data: &SoraData,
    images: &mut Assets<Image>,
    cache: &mut HashMap<String, Handle<Image>>,
    materials: &mut Assets<FogMaterial>,
) -> Handle<FogMaterial> {
    let texture = look.color.as_deref().and_then(|path| {
        if let Some(h) = cache.get(path) {
            return Some(h.clone());
        }
        let image = image_of(data.textures.get(path)?, true)?;
        let handle = images.add(image);
        cache.insert(path.to_owned(), handle.clone());
        Some(handle)
    });
    let colour = look
        .flat
        .map_or(Color::WHITE, |c| Color::linear_rgba(c[0], c[1], c[2], c[3]));
    // A base texture's alpha feeds other shader inputs, not opacity (only
    // the eyelashes' mask is one).
    let alpha_mode = if look.translucent {
        AlphaMode::Blend
    } else {
        AlphaMode::Opaque
    };
    materials.add_fogged(StandardMaterial {
        base_color: colour,
        base_color_texture: texture,
        perceptual_roughness: 0.9,
        metallic: 0.,
        reflectance: 0.2,
        double_sided: look.two_sided,
        cull_mode: if look.two_sided {
            None
        } else {
            Some(bevy::render::render_resource::Face::Back)
        },
        alpha_mode,
        ..default()
    })
}

fn shared(
    data: &SoraData,
    meshes: &mut Assets<Mesh>,
    images: &mut Assets<Image>,
    materials: &mut Assets<FogMaterial>,
    bind_assets: &mut Assets<SkinnedMeshInverseBindposes>,
) -> Shared {
    let k = scale();
    let mut cache = HashMap::new();
    let mut shaded_images: HashMap<String, Handle<Image>> = HashMap::new();
    let (body_joints, body_place) = used_joints(&data.mesh);
    let (blade_joints, blade_place) = used_joints(&data.keyblade);
    let sampler = |address: ImageAddressMode, images: &mut Assets<Image>| {
        let mut image = Image::new_fill(
            Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            &[255; 4],
            TextureFormat::Rgba8Unorm,
            bevy::asset::RenderAssetUsages::RENDER_WORLD,
        );
        image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
            address_mode_u: address,
            address_mode_v: address,
            mag_filter: ImageFilterMode::Linear,
            min_filter: ImageFilterMode::Linear,
            mipmap_filter: ImageFilterMode::Linear,
            anisotropy_clamp: 4,
            ..default()
        });
        images.add(image)
    };
    let samplers = (
        sampler(ImageAddressMode::Repeat, images),
        sampler(ImageAddressMode::ClampToEdge, images),
    );
    let mut parts = |mesh: &SkeletalMesh,
                     looks: &[SlotLook],
                     slots: &[data::kh3::charmat::Slot],
                     place: &[u16]|
     -> Vec<Part> {
        let mats: Vec<Option<Handle<FogMaterial>>> = looks
            .iter()
            .map(|l| (!l.skip).then(|| material_of(l, data, images, &mut cache, materials)))
            .collect();
        let shaded: Vec<Option<kh3_material::Kh3Material>> = slots
            .iter()
            .map(|s| {
                let mut image = |path: &str| -> Option<Handle<Image>> {
                    if let Some(h) = shaded_images.get(path) {
                        return Some(h.clone());
                    }
                    let t = data.textures.get(path)?;
                    let handle = images.add(image_of(t, t.props.bool("SRGB").unwrap_or(true))?);
                    shaded_images.insert(path.to_owned(), handle.clone());
                    Some(handle)
                };
                kh3_material::material_of(&data.materials, s, &mut image, &samplers)
            })
            .collect();
        lod_meshes(&mesh.lod, place, k)
            .into_iter()
            .filter_map(|(slot, m)| {
                let slot = usize::from(slot);
                let material = mats.get(slot).cloned().flatten();
                let shaded = shaded.get(slot).cloned().flatten();
                if material.is_none() && shaded.is_none() {
                    return None;
                }
                Some(Part {
                    material: material.unwrap_or_default(),
                    shaded,
                    mesh: meshes.add(m),
                })
            })
            .collect()
    };
    let body = parts(&data.mesh, &data.looks, &data.materials.body, &body_place);
    let blade = parts(
        &data.keyblade,
        &data.keyblade_looks,
        &data.materials.blade,
        &blade_place,
    );
    let body_binds = bind_assets.add(SkinnedMeshInverseBindposes::from(binds(
        &data.mesh,
        &body_joints,
        k,
    )));
    let blade_binds = bind_assets.add(SkinnedMeshInverseBindposes::from(binds(
        &data.keyblade,
        &blade_joints,
        k,
    )));
    Shared {
        body,
        blade,
        body_joints,
        blade_joints,
        body_binds,
        blade_binds,
    }
}

fn spawn_rig(
    commands: &mut Commands,
    s: &Shared,
    world: Option<Entity>,
    shaded_assets: &mut Assets<kh3_material::Kh3Material>,
) -> Rig {
    let mut shaded_handles = Vec::new();
    let root = commands
        .spawn((Kh3Root, Transform::default(), Visibility::Hidden))
        .id();
    if let Some(world) = world {
        commands.entity(root).insert(ChildOf(world));
    }
    let joint_entities = |commands: &mut Commands, parent: Entity, bones: &[usize]| {
        bones
            .iter()
            .map(|&b| {
                let e = commands
                    .spawn((Transform::default(), Visibility::Inherited, ChildOf(parent)))
                    .id();
                (b, e)
            })
            .collect::<Vec<_>>()
    };
    let joints = joint_entities(commands, root, &s.body_joints);
    let blade = commands
        .spawn((Transform::default(), Visibility::Inherited, ChildOf(root)))
        .id();
    let blade_joints = joint_entities(commands, blade, &s.blade_joints);
    let mut parts = |commands: &mut Commands,
                     parent: Entity,
                     list: &[Part],
                     binds: &Handle<SkinnedMeshInverseBindposes>,
                     j: &[(usize, Entity)]| {
        for part in list {
            let mut e = commands.spawn_empty();
            if let Some(m) = &part.shaded {
                let handle = shaded_assets.add(m.clone());
                shaded_handles.push(handle.clone());
                e.insert(MeshMaterial3d(handle));
            } else {
                e.insert(MeshMaterial3d(part.material.clone()));
            }
            e.insert((
                Mesh3d(part.mesh.clone()),
                SkinnedMesh {
                    inverse_bindposes: binds.clone(),
                    joints: j.iter().map(|x| x.1).collect(),
                },
                bevy::camera::visibility::NoFrustumCulling,
                Transform::default(),
                Visibility::Inherited,
                ChildOf(parent),
            ));
        }
    };
    parts(commands, root, &s.body, &s.body_binds, &joints);
    parts(commands, blade, &s.blade, &s.blade_binds, &blade_joints);
    Rig {
        root,
        joints,
        blade,
        blade_joints,
        animator: Animator::default(),
        component: Vec::new(),
        secondary: None,
        shaded: shaded_handles,
    }
}

/// Starts reading KH3 once a KH3 duel may happen.
fn start_load(model: &mut Kh3Model, game: &Game) {
    if model.job.is_some() || model.loaded.is_some() || model.error.is_some() || !game.duel {
        return;
    }
    let Some(install) = game.kh3_install.clone() else {
        return;
    };
    let hint = crate::launcher::kh3_folder().1;
    model.job = Some(
        std::thread::Builder::new()
            .name("kh3 load".into())
            .spawn(move || {
                let paks =
                    data::kh3::Paks::open(&install, hint, true).map_err(|e| e.to_string())?;
                crate::launcher::remember_kh3_key_offset(paks.key_offset);
                let sora = SoraData::load(&paks).map_err(|e| e.to_string())?;
                let mut book = kh3_book::read(&paks, &sora).map_err(|e| e.to_string())?;
                kh3_core::install(&paks, &sora).map_err(|e| e.to_string())?;
                kh3_core::name_assets(&paks, &sora, &mut book);
                let fx = kh3_fx::load(&paks, &sora, &book, &install)?;
                Ok(Loaded {
                    sora,
                    book: Arc::new(book),
                    fx: Arc::new(fx),
                })
            })
            .expect("kh3 load thread"),
    );
}

/// The Sora on a seat: the room's, or a rival's.
fn fighter_of(game: &Game, seat: u8) -> Option<&Fighter> {
    if seat == game.local_slot {
        Some(&game.sora)
    } else {
        game.rivals
            .iter()
            .find(|r| r.slot == seat)
            .map(|r| &r.fighter)
    }
}

/// One frame of the KH3 model.
#[allow(clippy::too_many_arguments)]
pub(crate) fn update(
    mut commands: Commands,
    mut model: ResMut<Kh3Model>,
    mut game: ResMut<Game>,
    time: Res<Time>,
    mut transforms: Query<&mut Transform>,
    mut visibilities: Query<&mut Visibility>,
    worlds: Query<Entity, With<visual::smoothing::SmoothRoot>>,
    hidden: Query<Entity, Or<(With<SoraRoot>, With<WeaponRoot>, With<rival::RivalRoot>)>>,
    assets: (
        ResMut<Assets<Mesh>>,
        ResMut<Assets<Image>>,
        ResMut<Assets<FogMaterial>>,
        ResMut<Assets<SkinnedMeshInverseBindposes>>,
    ),
    shading: (
        ResMut<Assets<kh3_material::Kh3Material>>,
        ResMut<Assets<bevy::shader::Shader>>,
    ),
    mut ambient: ResMut<GlobalAmbientLight>,
    mut fx: ResMut<kh3_fx::Kh3Fx>,
    mut effect_materials: ResMut<Assets<visual::effect_draw::EffectMaterial>>,
    mut audio_res: Option<ResMut<Audio>>,
    cameras: Query<
        &GlobalTransform,
        (With<Camera3d>, Without<SnapshotCamera>, Without<MenuCamera>),
    >,
) {
    let (mut meshes, mut images, mut materials, mut bind_assets) = assets;
    let (mut shaded_assets, mut shaders) = shading;
    let model = &mut *model;
    start_load(model, &game);
    // Reading KH3 takes some seconds: the duel waits for it.
    game.kh3_loading = model.job.is_some() && duel_kh3::on(&game);
    if model.job.as_ref().is_some_and(|j| j.is_finished())
        && let Some(job) = model.job.take()
    {
        match job.join() {
            Ok(Ok(loaded)) => {
                info!(
                    "KH3 assets: {} rows, {} clips",
                    loaded.sora.anims.rows.len(),
                    loaded.sora.anims.clips.len()
                );
                game.kh3_book = Some(Arc::clone(&loaded.book));
                kh3_fx::begin(&mut fx, Arc::clone(&loaded.fx), &mut images);
                game.event(serde_json::json!({"event":"kh3_assets",
                    "clips":loaded.book.clips.len(),"rows":loaded.book.rows.len(),
                    "shapes":loaded.book.shapes.len(),"stats":loaded.book.stats}));
                model.loaded = Some(Arc::new(loaded));
            }
            Ok(Err(why)) => {
                eprintln!("KH3 assets not loaded: {why}");
                let tick = game.tick;
                game.event(serde_json::json!({"event":"kh3_load_failed","tick":tick,"why":why}));
                model.error = Some(why);
            }
            Err(_) => model.error = Some("the KH3 load stopped".into()),
        }
    }
    // The layers' requests (a joiner gets them through the wire).
    let mut requests = Vec::new();
    requests.append(&mut game.kh3_fx_remote);
    let game = &*game;
    let on = duel_kh3::on(game);
    let Some(loaded) = model.loaded.clone().filter(|_| on) else {
        for rig in model.rigs.iter().flatten() {
            if let Ok(mut v) = visibilities.get_mut(rig.root) {
                *v = Visibility::Hidden;
            }
        }
        return;
    };
    let data = &loaded.sora;
    if model.shared.is_none() {
        // KH3's material shaders, translated at load.
        let _ = shaders.insert(
            &kh3_material::SHADER,
            bevy::shader::Shader::from_wgsl(
                kh3_material::shader_text(&data.materials),
                "kh3_material.wgsl",
            ),
        );
        model.shared = Some(shared(
            data,
            &mut meshes,
            &mut images,
            &mut materials,
            &mut bind_assets,
        ));
        // The room's materials are unlit, so this only touches KH3's Sora.
        commands.spawn((
            Kh3Light,
            DirectionalLight {
                illuminance: 3000.,
                ..default()
            },
            Transform::from_xyz(0.4, 1., 0.5).looking_at(Vec3::ZERO, Vec3::Y),
        ));
        ambient.brightness = 300.;
    }
    // KH2's rigs stay under the floor of what is drawn.
    for e in &hidden {
        if let Ok(mut v) = visibilities.get_mut(e) {
            *v = Visibility::Hidden;
        }
    }
    let Some(s) = model.shared.as_ref() else {
        return;
    };
    let k = scale();
    let dt = time.delta_secs();
    let world = worlds.iter().next();
    let camera = cameras
        .iter()
        .next()
        .map(|c| (c.translation(), *c.forward(), *c.right(), *c.up()));
    let camera_at = camera.map_or(Vec3::ZERO, |c| c.0);
    kh3_fx::requests(&mut fx, requests, camera_at);
    for seat in 0..2u8 {
        let Some(fighter) = fighter_of(game, seat) else {
            continue;
        };
        let rig = model.rigs[usize::from(seat)]
            .get_or_insert_with(|| spawn_rig(&mut commands, s, world, &mut shaded_assets));
        if let Ok(mut v) = visibilities.get_mut(rig.root) {
            *v = Visibility::Inherited;
        }
        let p = &fighter.player;
        if let Ok(mut t) = transforms.get_mut(rig.root) {
            t.translation = Vec3::from_array(p.position);
            t.rotation = Quat::from_rotation_y(p.yaw)
                * Quat::from_rotation_x(fighter.kh3.lean[0])
                * Quat::from_rotation_z(fighter.kh3.lean[1]);
        }
        kh3_fx::actors_step(
            &mut fx,
            seat,
            &fighter.kh3.actors,
            &fighter.kh3.bolts,
            &loaded.book,
        );
        let local = fighter.kh3.visual;
        let remote = fighter.kh3.remote_visual;
        let v = &match remote {
            Some(r) => player::kh3::visual::Visual {
                row: "",
                serial: u32::from(r.serial),
                time: r.time,
                at: r.at,
                blend: r.blend,
                looped: r.looped,
            },
            None => local,
        };
        let row = match remote {
            Some(r) => Some(r.row),
            None => data.anims.row(v.row),
        };
        let Some(row) = row else {
            continue;
        };
        // A looping row wraps at its own length.
        let length = data.anims.row_length(row, v.at[0]).max(1e-3);
        let time_in = if v.looped {
            v.time % length
        } else {
            v.time.min(length)
        };
        rig.animator.update(
            &data.anims,
            &data.reference,
            row,
            v.serial,
            time_in,
            v.at,
            v.blend,
            dt,
        );
        sora::to_component(&data.mesh.bones, &rig.animator.saved, &mut rig.component);
        let actor = kh3_cloth_at(p.position);
        for h in &rig.shaded {
            if let Some(mut m) = shaded_assets.get_mut(h) {
                m.place(actor, p.yaw, 1. / scale(), time.elapsed_secs());
            }
        }
        rig.secondary
            .get_or_insert_with(|| kh3_cloth::Secondary::new(data))
            .step(
                data.anims.main_clip(row, v.at[0]),
                time_in,
                dt,
                &rig.animator.saved,
                &mut rig.component,
                kh3_cloth_at(p.position),
                p.yaw,
                !p.in_air(),
            );
        fx.surface = kh3_autose::surface_of_arena(game.duel_arena);
        kh3_fx::body_step(
            &mut fx,
            data,
            seat,
            p.position,
            p.yaw,
            &rig.component,
            v,
            row,
            time_in,
            camera_at,
            !p.in_air(),
        );
        for &(bone, e) in &rig.joints {
            if let (Ok(mut t), Some(c)) = (transforms.get_mut(e), rig.component.get(bone)) {
                *t = converted(c, k);
            }
        }
        // The Kingdom Key: its root on the hand's socket (SnapToTarget).
        let mut blade_at = None;
        if let Some(sock) = data.sockets.iter().find(|x| x.name == "R_wep_skt")
            && let Some(hand) = data.bone(&sock.bone).and_then(|b| rig.component.get(b))
        {
            let local = BonePose {
                rotation: sora::quat_of_rotator(sock.rotation),
                translation: sock.location,
                scale: sock.scale,
            };
            let at = sora::compose(hand, &local);
            blade_at = Some(at);
            if let Ok(mut t) = transforms.get_mut(rig.blade) {
                *t = converted(&at, k);
            }
            if let Ok(mut vis) = visibilities.get_mut(rig.blade) {
                *vis = if p.armed {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                };
            }
        }
        // Blade bones under their root: the reference pose.
        let mut blade_component = Vec::new();
        let blade_local: Vec<BonePose> = data
            .keyblade
            .bones
            .iter()
            .map(|b| BonePose {
                rotation: b.rotation,
                translation: b.translation,
                scale: b.scale,
            })
            .collect();
        sora::to_component(&data.keyblade.bones, &blade_local, &mut blade_component);
        if let Some(at) = blade_at
            && let Some(secondary) = &mut rig.secondary
        {
            secondary.keychain(dt, &at, p.armed, &mut blade_component);
        }
        for &(bone, e) in &rig.blade_joints {
            if let (Ok(mut t), Some(c)) = (transforms.get_mut(e), blade_component.get(bone)) {
                *t = converted(c, k);
            }
        }
    }
    let mut guard = audio_res
        .as_mut()
        .and_then(|a| a.output.as_ref())
        .and_then(|o| o.mixer.lock().ok());
    kh3_fx::frame(
        &mut fx,
        dt,
        camera,
        &mut commands,
        &mut meshes,
        &mut effect_materials,
        &mut visibilities,
        guard.as_deref_mut(),
    );
}
