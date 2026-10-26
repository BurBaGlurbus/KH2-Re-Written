//! The LAN duel's Mirage Arena: Birth by Sleep's arena stage
//! (`arc/map/vs03.arc` in the user's install, the BBS package bound and
//! checked by `data::bbs_source`), decoded on a thread (`data::bbs_map`) as
//! soon as a duel session exists, and shown in place of the test room's
//! arena while a duel's rules choose it: its meshes under the scene root,
//! its collision for the Soras and the camera, its centre for the starts.
//! The test room's arena comes back when the duel ends or the host picks it.
//!
//! The PC remaster's images stand in for the PSP textures where one matches
//! (`bbs_map::match_hd`), uploaded with their mip chains; the sky's layers
//! scroll as their texture table says, by the tick.
//!
//! BBS maps are in metres (an adult NPC model is 1.7 tall, the three
//! nephews 0.84); the room's units are KH2's, where Sora stands about 160
//! tall, so the map is scaled by 100 [duel choice, T248].

use super::*;
use data::bbs_map::{self, MapModel};
use std::path::PathBuf;
use std::thread::JoinHandle;

/// The map units to the room's.
pub(crate) const SCALE: f32 = 100.;
/// The sky's: its domes (2-5 m) drawn around the camera well past the
/// arena (about 4300 across) and inside the camera's far plane (200000).
const SKY_SCALE: f32 = 15_000.;
/// The arena's archive.
const MAP: &str = "arc/map/vs03.arc";
/// The largest archive read.
const MAP_LIMIT: u32 = 16 << 20;

/// Where this instance stands with the arena.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) enum Avail {
    /// Not asked for yet, or still loading.
    #[default]
    Unknown,
    Ready,
    /// The install cannot show it (the reason, for the lobby).
    Missing(String),
}

/// What the loading thread hands back.
struct Loaded {
    model: MapModel,
    /// Collision in scene coordinates: (corners, floor, wall).
    collision: Vec<([[f32; 3]; 3], bool, bool)>,
    /// Camera collision in game world coordinates.
    camera: Vec<[[f32; 3]; 3]>,
    /// Each texture's HD replacement, if one matched.
    hd: Vec<Option<bbs_map::MipChain>>,
}

/// A mesh of the arena (shown while the duel is in it).
#[derive(Component)]
pub(crate) struct MirageMesh;

/// A sky mesh of the arena: it stays centred on the camera.
#[derive(Component)]
pub(crate) struct MirageSky;

/// A material whose texture scrolls (U, V a tick).
#[derive(Component)]
pub(crate) struct MirageScroll {
    material: Handle<FogMaterial>,
    speed: [f32; 2],
}

#[derive(Resource, Default)]
pub(crate) struct MirageArena {
    /// The install root and the BBS manifest's path (beside the KH2 one).
    pub paths: Option<(PathBuf, PathBuf)>,
    job: Option<JoinHandle<Result<Loaded, String>>>,
    loaded: Option<Loaded>,
    shown: bool,
    /// The room's own collision and camera collision while the arena's are
    /// in use.
    saved: Option<(RoomWorld, CameraCollision)>,
    /// Room entities hidden while the arena shows, and how they were.
    hidden: Vec<(Entity, Visibility)>,
}

impl MirageArena {
    /// The arena read from the install at `root`, its BBS manifest at
    /// `manifest`.
    pub(crate) fn new(root: PathBuf, manifest: PathBuf) -> Self {
        Self {
            paths: Some((root, manifest)),
            ..default()
        }
    }
}

/// One merged mesh of the map (a texture and blend's sections).
#[derive(Default)]
struct Batch {
    positions: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
    colors: Vec<[f32; 4]>,
}

/// The room's own arena parts (hidden while the Mirage Arena shows).
type RoomParts = (Or<(With<RoomPart>, With<ArenaFloor>)>, Without<MirageMesh>);

/// BBS map space to the room's scene (+Y up both; no mirror).
fn scene(p: [f32; 3]) -> [f32; 3] {
    p.map(|c| c * SCALE)
}

fn load(root: PathBuf, manifest: PathBuf) -> Result<Loaded, String> {
    let mut source = data::bbs_source::BbsSource::open(&root, &manifest)
        .map_err(|e| format!("Birth by Sleep is not readable in this install: {e}"))?;
    let arc = source
        .read(MAP, MAP_LIMIT)
        .map_err(|e| format!("{MAP}: {e}"))?;
    let files = bbs_map::arc_files(&arc).map_err(|e| e.to_string())?;
    let file = |ext: &str| {
        files
            .iter()
            .find(|f| f.name.ends_with(ext))
            .map(|f| f.bytes)
            .ok_or_else(|| format!("{MAP} has no {ext}"))
    };
    let model = bbs_map::pmp(file(".pmp")?).map_err(|e| e.to_string())?;
    // The remaster's images (missing or undecodable ones leave the PSP
    // textures).
    let chains: Vec<bbs_map::MipChain> = source
        .read_hd(MAP)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(_, bytes)| {
            let d = data::hd_image::decode_bc3_mip_chain(&bytes, bytes.len()).ok()?;
            Some(
                d.layout
                    .levels
                    .iter()
                    .zip(d.levels_rgba)
                    .map(|(l, rgba)| (l.width, l.height, rgba))
                    .collect(),
            )
        })
        .collect();
    let matched = bbs_map::match_hd(&model.textures, &chains);
    let mut chains: Vec<Option<bbs_map::MipChain>> = chains.into_iter().map(Some).collect();
    let hd = matched
        .iter()
        .map(|m| m.and_then(|k| chains.get_mut(k).and_then(Option::take)))
        .collect();
    let faces = bbs_map::bcd(file(".bcd")?).map_err(|e| e.to_string())?;
    let mut collision = Vec::new();
    let mut camera = Vec::new();
    for f in &faces {
        let floor = f.normal[1] >= FLOOR_MIN_NORMAL_Y;
        for t in f.triangles() {
            let s = t.map(scene);
            collision.push((s, floor, !floor));
            camera.push(s.map(to_world));
        }
    }
    Ok(Loaded {
        model,
        collision,
        camera,
        hd,
    })
}

/// An HD image with its mip chain (repeating, trilinear), on the GPU only.
fn mip_image(chain: &bbs_map::MipChain, images: &mut Assets<Image>) -> Handle<Image> {
    use bevy::image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
    use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
    let (w, h, first) = &chain[0];
    // Made at its top level, then given every level (mip after mip, as the
    // GPU upload reads them).
    let mut image = Image::new(
        Extent3d {
            width: *w,
            height: *h,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        first.clone(),
        TextureFormat::Rgba8UnormSrgb,
        bevy::asset::RenderAssetUsages::RENDER_WORLD,
    );
    image.data = Some(
        chain
            .iter()
            .flat_map(|(_, _, p)| p.iter().copied())
            .collect(),
    );
    image.texture_descriptor.mip_level_count = chain.len() as u32;
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        mag_filter: ImageFilterMode::Linear,
        min_filter: ImageFilterMode::Linear,
        mipmap_filter: ImageFilterMode::Linear,
        ..default()
    });
    images.add(image)
}

/// Starts the load once a duel session exists, and reports when it ends.
pub(crate) fn tick(
    mut arena: ResMut<MirageArena>,
    net: Option<Res<duel_net::DuelNet>>,
    mut game: ResMut<Game>,
) {
    if net.is_none() || arena.loaded.is_some() || game.mirage != Avail::Unknown {
        return;
    }
    if arena.job.is_none() {
        let Some((root, manifest)) = arena.paths.clone() else {
            game.mirage = Avail::Missing("no install paths".into());
            return;
        };
        arena.job = Some(std::thread::spawn(move || load(root, manifest)));
        return;
    }
    if !arena.job.as_ref().is_some_and(|j| j.is_finished()) {
        return;
    }
    let result = arena
        .job
        .take()
        .map(|j| {
            j.join()
                .map_err(|_| "the arena's loading thread stopped".to_string())
        })
        .unwrap_or_else(|| Err("no load".into()));
    let tick = game.tick;
    match result.and_then(|r| r) {
        Ok(l) => {
            let triangles: usize = l.model.meshes.iter().map(|m| m.positions.len() / 3).sum();
            game.event(serde_json::json!({"event":"mirage_loaded","tick":tick,
                "meshes":l.model.meshes.len(),"triangles":triangles,
                "textures":l.model.textures.len(),"collision":l.collision.len()}));
            arena.loaded = Some(l);
            game.mirage = Avail::Ready;
        }
        Err(e) => {
            game.event(serde_json::json!({"event":"mirage_missing","tick":tick,"why":e}));
            game.mirage = Avail::Missing(e);
        }
    }
}

/// Shows the arena while the duel's rules choose it (and it loaded), and
/// the room's own arena otherwise.
#[allow(clippy::too_many_arguments)]
pub(crate) fn show(
    mut commands: Commands,
    mut arena: ResMut<MirageArena>,
    mut game: ResMut<Game>,
    mut rig: ResMut<CameraRig>,
    root: Option<Res<SceneRoot>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<FogMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut room_parts: Query<(Entity, &mut Visibility), RoomParts>,
    mirage_meshes: Query<Entity, With<MirageMesh>>,
) {
    let want = game.duel
        && usize::from(game.duel_rules.arena) == duel_arena::MIRAGE
        && arena.loaded.is_some();
    if want == arena.shown {
        return;
    }
    let tick = game.tick;
    if want {
        let Some(root) = root else { return };
        let Some(l) = arena.loaded.as_ref() else {
            return;
        };
        // The room's arena hides; its collision steps aside.
        let mut hidden = Vec::new();
        for (e, mut v) in &mut room_parts {
            hidden.push((e, *v));
            *v = Visibility::Hidden;
        }
        let textures: Vec<Handle<Image>> = l
            .model
            .textures
            .iter()
            .zip(&l.hd)
            .map(|(t, hd)| match hd {
                Some(chain) => mip_image(chain, &mut images),
                None => visual::room::region_image(
                    t.width,
                    t.height,
                    t.rgba.clone(),
                    [true, true],
                    &mut images,
                ),
            })
            .collect();
        let hd_count = l.hd.iter().filter(|h| h.is_some()).count();
        let scroll = |t: Option<usize>| {
            t.and_then(|i| l.model.texture_scroll.get(i).copied())
                .filter(|s| s[0] != 0. || s[1] != 0.)
        };
        // The map's meshes merge by texture and blend: one draw (and one
        // material) each instead of one per object section. The sky's few
        // layers stay apart, in their order.
        let mut batches: std::collections::BTreeMap<(bool, Option<usize>), Batch> =
            std::collections::BTreeMap::new();
        for m in &l.model.meshes {
            let texture = m.texture.and_then(|i| textures.get(i).cloned());
            if m.sky {
                // The sky blends behind the map from wherever the camera is.
                let material = materials.add_plain(StandardMaterial {
                    base_color: Color::WHITE,
                    base_color_texture: texture,
                    unlit: true,
                    cull_mode: None,
                    alpha_mode: AlphaMode::Blend,
                    ..default()
                });
                let mut e = commands.spawn((
                    MirageMesh,
                    MirageSky,
                    Mesh3d(meshes.add(mesh(m.positions.clone(), m.uvs.clone(), m.colors.clone()))),
                    MeshMaterial3d(material.clone()),
                    Transform::from_scale(Vec3::splat(SKY_SCALE)),
                ));
                if let Some(speed) = scroll(m.texture) {
                    e.insert(MirageScroll { material, speed });
                }
                continue;
            }
            let b = batches.entry((m.translucent, m.texture)).or_default();
            b.positions.extend(m.positions.iter().map(|&p| scene(p)));
            b.uvs.extend_from_slice(&m.uvs);
            b.colors.extend_from_slice(&m.colors);
        }
        let mut draws = 0;
        for ((translucent, texture), b) in batches {
            let material = materials.add_plain(StandardMaterial {
                base_color: Color::WHITE,
                base_color_texture: texture.and_then(|i| textures.get(i).cloned()),
                unlit: true,
                cull_mode: None,
                // Alpha-tested where opaque; the PMO's translucent list
                // blends.
                alpha_mode: if translucent {
                    AlphaMode::Blend
                } else {
                    AlphaMode::Mask(0.5)
                },
                ..default()
            });
            let mut e = commands.spawn((
                ChildOf(root.0),
                MirageMesh,
                Mesh3d(meshes.add(mesh(b.positions, b.uvs, b.colors))),
                MeshMaterial3d(material.clone()),
                Transform::default(),
            ));
            if let Some(speed) = scroll(texture) {
                e.insert(MirageScroll { material, speed });
            }
            draws += 1;
        }
        game.event(
            serde_json::json!({"event":"mirage_draws","tick":tick,"draws":draws,
            "sections":l.model.meshes.len(),"hd_textures":hd_count}),
        );
        let world = RoomWorld::Triangles(TriangleWorld::new(
            l.collision.iter().copied(),
            FLOOR_MIN_NORMAL_Y,
        ));
        let camera = CameraCollision::from_triangles(l.camera.clone());
        let old_world = std::mem::replace(&mut game.world, world);
        let old_camera = std::mem::replace(&mut rig.collision, camera);
        arena.saved = Some((old_world, old_camera));
        arena.hidden = hidden;
        arena.shown = true;
        game.duel_arena = duel_arena::MIRAGE;
    } else {
        for e in &mirage_meshes {
            commands.entity(e).despawn();
        }
        for (e, v) in std::mem::take(&mut arena.hidden) {
            if let Ok((_, mut now)) = room_parts.get_mut(e) {
                *now = v;
            }
        }
        if let Some((world, camera)) = arena.saved.take() {
            game.world = world;
            rig.collision = camera;
        }
        arena.shown = false;
        game.duel_arena = duel_arena::TEST_ROOM;
    }
    game.event(serde_json::json!({"event":"mirage_shown","tick":tick,"shown":arena.shown}));
}

/// Keeps the arena's sky centred on the camera.
pub(crate) fn follow_sky(
    camera: Query<&GlobalTransform, CameraOnly>,
    mut sky: Query<&mut Transform, With<MirageSky>>,
) {
    let Some(at) = camera.iter().next().map(GlobalTransform::translation) else {
        return;
    };
    for mut t in &mut sky {
        t.translation = at;
    }
}

/// Scrolls the arena's moving textures by the tick (the same at any frame
/// rate).
pub(crate) fn scroll(
    game: Res<Game>,
    scrolls: Query<&MirageScroll>,
    mut materials: ResMut<Assets<FogMaterial>>,
) {
    let t = game.tick as f64;
    for s in &scrolls {
        let off = Vec2::new(
            (f64::from(s.speed[0]) * t).rem_euclid(1.) as f32,
            (f64::from(s.speed[1]) * t).rem_euclid(1.) as f32,
        );
        if let Some(mut m) = materials.get_mut(&s.material) {
            m.base.uv_transform = bevy::math::Affine2::from_translation(off);
        }
    }
}
