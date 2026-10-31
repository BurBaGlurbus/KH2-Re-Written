//! The Skate 3 score HUD while riding, in place of the KH2 HUD: skate-sim's
//! trick-display movie drawn as textured triangles on its own 2D camera in
//! the movie's 1280x720 space.
use bevy::{
    asset::{AssetPath, RenderAssetUsages, embedded_asset, embedded_path},
    camera::{CameraOutputMode, CompositingSpace, ScalingMode, visibility::RenderLayers},
    core_pipeline::tonemapping::{DebandDither, Tonemapping},
    mesh::PrimitiveTopology,
    prelude::*,
    render::render_resource::{
        AsBindGroup, BlendState, Extent3d, ShaderType, TextureDimension, TextureFormat,
    },
    shader::ShaderRef,
    sprite_render::{AlphaMode2d, Material2d, Material2dPlugin},
};
use std::collections::HashMap;

/// The HUD's own render layer and camera order (above the scene and the
/// KH2 2D layers).
const LAYER: usize = 31;
const ORDER: isize = 50;

#[derive(Clone, Copy, ShaderType)]
struct ColourTransform {
    multiply: Vec4,
    add: Vec4,
}

#[derive(Asset, TypePath, AsBindGroup, Clone)]
struct HudMaterial {
    #[uniform(0)]
    colour: ColourTransform,
    #[texture(1)]
    #[sampler(2)]
    texture: Handle<Image>,
}

impl Material2d for HudMaterial {
    fn fragment_shader() -> ShaderRef {
        ShaderRef::Path(
            AssetPath::from_path_buf(embedded_path!("skate_hud.wgsl")).with_source("embedded"),
        )
    }

    fn alpha_mode(&self) -> AlphaMode2d {
        AlphaMode2d::Blend
    }
}

#[derive(Resource, Default)]
struct HudAssets {
    /// Texture key -> image ("" is plain white for untextured units).
    textures: HashMap<String, Handle<Image>>,
    slots: Vec<(Entity, Handle<Mesh>, Handle<HudMaterial>)>,
}

pub struct SkateHudPlugin;

impl Plugin for SkateHudPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "skate_hud.wgsl");
        app.add_plugins(Material2dPlugin::<HudMaterial>::default())
            .init_resource::<HudAssets>()
            .add_systems(Startup, camera)
            .add_systems(Update, render);
    }
}

/// The layer's camera. It draws only while the HUD has something on it: its
/// transparent layer is otherwise a pass and a full-screen blend for nothing.
#[derive(Component)]
struct HudCamera;

fn camera(mut commands: Commands) {
    commands.spawn((
        HudCamera,
        Camera2d,
        Projection::Orthographic(OrthographicProjection {
            scaling_mode: ScalingMode::Fixed {
                width: 1280.,
                height: 720.,
            },
            ..OrthographicProjection::default_2d()
        }),
        // Drawn on a transparent layer, then laid over the frame as
        // premultiplied colour (skate-game composites the HUD the same way).
        Camera {
            order: ORDER,
            is_active: false,
            clear_color: ClearColorConfig::Custom(Color::NONE),
            output_mode: CameraOutputMode::Write {
                blend_state: Some(BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                clear_color: ClearColorConfig::None,
            },
            ..default()
        },
        RenderLayers::layer(LAYER),
        Msaa::Off,
        Tonemapping::None,
        DebandDither::Disabled,
        CompositingSpace::Srgb,
    ));
}

fn image(rgba: Vec<u8>, size: [u32; 2]) -> Image {
    Image::new(
        Extent3d {
            width: size[0],
            height: size[1],
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        rgba,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    )
}

fn render(
    mut commands: Commands,
    mut skate: ResMut<crate::skate::Skate>,
    mut assets: ResMut<HudAssets>,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<HudMaterial>>,
    mut cameras: Query<&mut Camera, With<HudCamera>>,
) {
    let draws = match skate.score_hud() {
        Some((hud, root)) => {
            if assets.textures.is_empty() {
                assets
                    .textures
                    .insert(String::new(), images.add(image(vec![255; 4], [1, 1])));
                for (path, size) in hud.texture_files() {
                    let file = root.join(&path);
                    match std::fs::read(&file) {
                        Ok(bytes) if bytes.len() == size[0] as usize * size[1] as usize * 4 => {
                            let handle = images.add(image(bytes, size));
                            assets.textures.insert(path, handle);
                        }
                        Ok(_) => eprintln!("Skate 3 HUD texture {path} has the wrong size"),
                        Err(e) => eprintln!("Skate 3 HUD texture {path}: {e}"),
                    }
                }
            }
            hud.draws().unwrap_or_else(|e| {
                eprintln!("Skate 3 HUD stopped: {e}");
                Vec::new()
            })
        }
        None => Vec::new(),
    };
    let mut used = 0;
    for draw in &draws {
        let Some(texture) = assets.textures.get(&draw.texture).cloned() else {
            continue;
        };
        let mut mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
        );
        mesh.insert_attribute(
            Mesh::ATTRIBUTE_POSITION,
            draw.vertices
                .iter()
                .map(|v| [v.position[0] - 640., 360. - v.position[1], 0.])
                .collect::<Vec<_>>(),
        );
        mesh.insert_attribute(
            Mesh::ATTRIBUTE_UV_0,
            draw.vertices.iter().map(|v| v.uv).collect::<Vec<_>>(),
        );
        mesh.insert_attribute(
            Mesh::ATTRIBUTE_NORMAL,
            vec![[0., 0., 1.]; draw.vertices.len()],
        );
        let material = HudMaterial {
            colour: ColourTransform {
                multiply: Vec4::from_array(draw.multiply),
                add: Vec4::from_array(draw.add),
            },
            texture,
        };
        if used == assets.slots.len() {
            let mesh = meshes.add(mesh);
            let material = materials.add(material);
            let entity = commands
                .spawn((
                    Mesh2d(mesh.clone()),
                    MeshMaterial2d(material.clone()),
                    Transform::from_xyz(0., 0., used as f32 * 0.01),
                    RenderLayers::layer(LAYER),
                ))
                .id();
            assets.slots.push((entity, mesh, material));
        } else {
            let (entity, mesh_handle, material_handle) = assets.slots[used].clone();
            if let Some(mut old) = meshes.get_mut(&mesh_handle) {
                *old = mesh;
            }
            if let Some(mut old) = materials.get_mut(&material_handle) {
                *old = material;
            }
            commands.entity(entity).insert(Visibility::Visible);
        }
        used += 1;
    }
    for (entity, ..) in &assets.slots[used..] {
        commands.entity(*entity).insert(Visibility::Hidden);
    }
    for mut camera in &mut cameras {
        camera.is_active = used > 0;
    }
}
