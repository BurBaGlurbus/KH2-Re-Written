//! `SeqBlur` (0x17): the exe's full-screen feedback pass (FUN_14015f250,
//! private screen-spec 3). Once per event frame, while on, the saved
//! previous frame is drawn over the current one at alpha a/128, zoomed
//! (514/512, 418/416) about the screen centre:
//! `out = prev * (a / 128) + cur * (1 - a / 128)`; the result is saved as
//! the next previous frame. While off the saved frame still follows the
//! current one every update.
//!
//! Two render targets alternate: the camera writing the image that is not
//! the history reads the history. `history` only advances on event-frame
//! steps, so the 60 Hz presentation does not feed back twice per update.
use bevy::{
    asset::RenderAssetUsages,
    camera::{CompositingSpace, ScalingMode, visibility::RenderLayers},
    core_pipeline::tonemapping::{DebandDither, Tonemapping},
    mesh::PrimitiveTopology,
    prelude::*,
    render::render_resource::{Extent3d, TextureFormat},
};
use data::event_script::Command;
use visual::overlay::{LayoutMaterial, Ui2dFrame};

/// First render layer used by the two blur cameras.
const LAYER_BASE: usize = 8;
/// (x, y) size of the zoomed quad over the screen: (514/512, 418/416).
pub const ZOOM: [f32; 2] = [514. / 512., 418. / 416.];
/// The Drive change's quad (FUN_14040d000 sets cx = cy = 2 before
/// FUN_1403f7880: (0x100 + 2)·2 / 512, (0xd0 + 2)·2 / 416).
pub const DRIVE_ZOOM: [f32; 2] = [516. / 512., 420. / 416.];

#[derive(Resource)]
pub struct Blur {
    /// Sheet slots (in `Ui2dFrame`) of the two output images.
    sheets: [usize; 2],
    cameras: [Entity; 2],
    /// The "previous frame" quad of camera i (it samples image 1 - i).
    prev_meshes: [Handle<Mesh>; 2],
    history: usize,
    last_step: i32,
    /// `(frame, alpha byte)` switches in script order; alpha 0 = off.
    switches: Vec<(i32, u8)>,
    shown: usize,
}

fn quad(half: [f32; 2], alpha: f32) -> Mesh {
    let [hx, hy] = half;
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(
        Mesh::ATTRIBUTE_POSITION,
        vec![[-hx, hy, 0.], [hx, hy, 0.], [-hx, -hy, 0.], [hx, -hy, 0.]],
    )
    .with_inserted_attribute(
        Mesh::ATTRIBUTE_UV_0,
        vec![[0., 0.], [1., 0.], [0., 1.], [1., 1.]],
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, vec![[1., 1., 1., alpha]; 4])
    .with_inserted_indices(bevy::mesh::Indices::U32(vec![0, 1, 2, 1, 3, 2]))
}

impl Blur {
    /// `SeqBlur` switches: `sw != 0` = off, `sw == 0` = on with `alpha`.
    pub fn switches(commands: &[Command]) -> Vec<(i32, u8)> {
        let mut list: Vec<_> = commands
            .iter()
            .filter_map(|c| match c {
                Command::Blur {
                    frame,
                    switch,
                    alpha,
                    ..
                } => Some((
                    *frame as i32,
                    if *switch != 0 { 0 } else { (*alpha).max(1) },
                )),
                _ => None,
            })
            .collect();
        list.sort_by_key(|s| s.0);
        list
    }

    /// The previous-frame weight (0..=1) at event frame `frame`.
    pub fn weight(&self, frame: f32) -> f32 {
        self.switches
            .iter()
            .rev()
            .find(|(at, _)| *at as f32 <= frame)
            .map_or(0., |(_, a)| *a as f32 / 128.)
    }

    /// Spawns the two feedback cameras over `scene`; `size` = target size.
    #[allow(clippy::too_many_arguments)]
    pub fn spawn(
        commands: &mut Commands,
        meshes: &mut Assets<Mesh>,
        materials: &mut Assets<LayoutMaterial>,
        images: &mut Assets<Image>,
        frame: &mut Ui2dFrame,
        scene: &Handle<Image>,
        size: [u32; 2],
        switches: Vec<(i32, u8)>,
        zoom: [f32; 2],
    ) -> Self {
        let outputs: [Handle<Image>; 2] = std::array::from_fn(|_| {
            images.add(Image::new_target_texture(
                size[0],
                size[1],
                TextureFormat::Rgba8UnormSrgb,
                None,
            ))
        });
        let sheets = [0, 1].map(|i| frame.add_image(outputs[i].clone(), size, true));
        let material = |texture: &Handle<Image>, materials: &mut Assets<LayoutMaterial>| {
            materials.add(LayoutMaterial {
                texture: texture.clone(),
                blend: 0,
                srgb_source: true,
            })
        };
        let mut cameras = Vec::new();
        let mut prev_meshes = Vec::new();
        for i in 0..2 {
            let layer = RenderLayers::layer(LAYER_BASE + i);
            commands.spawn((
                Mesh2d(meshes.add(quad([0.5, 0.5], 1.))),
                MeshMaterial2d(material(scene, materials)),
                Transform::from_xyz(0., 0., 0.),
                layer.clone(),
            ));
            let prev = meshes.add(quad([0.5 * zoom[0], 0.5 * zoom[1]], 0.));
            commands.spawn((
                Mesh2d(prev.clone()),
                MeshMaterial2d(material(&outputs[1 - i], materials)),
                Transform::from_xyz(0., 0., 0.1),
                layer.clone(),
            ));
            prev_meshes.push(prev);
            cameras.push(
                commands
                    .spawn((
                        Camera2d,
                        Camera {
                            order: -1,
                            is_active: false,
                            clear_color: ClearColorConfig::Custom(Color::BLACK),
                            ..default()
                        },
                        bevy::camera::RenderTarget::Image(outputs[i].clone().into()),
                        Msaa::Off,
                        Tonemapping::None,
                        DebandDither::Disabled,
                        CompositingSpace::Srgb,
                        layer,
                        Projection::Orthographic(OrthographicProjection {
                            scaling_mode: ScalingMode::Fixed {
                                width: 1.,
                                height: 1.,
                            },
                            ..OrthographicProjection::default_2d()
                        }),
                    ))
                    .id(),
            );
        }
        Blur {
            sheets,
            cameras: [cameras[0], cameras[1]],
            prev_meshes: [prev_meshes[0].clone(), prev_meshes[1].clone()],
            history: 0,
            last_step: -1,
            switches,
            shown: 0,
        }
    }

    /// Sheet slot holding this frame's picture (scene plus feedback).
    pub fn sheet(&self) -> usize {
        self.sheets[self.shown]
    }

    /// Follows the scene image to a new size (the window's).
    pub fn resize(&self, size: [u32; 2], images: &mut Assets<Image>, frame: &mut Ui2dFrame) {
        for &sheet in &self.sheets {
            if let Some(mut image) = images.get_mut(&frame.sheets[sheet].image) {
                image.resize(Extent3d {
                    width: size[0],
                    height: size[1],
                    depth_or_array_layers: 1,
                });
            }
            frame.sheets[sheet].size = size;
        }
    }

    /// Resizes the previous picture's quads (`ZOOM` for `SeqBlur`, 1 for
    /// the Drive afterimage).
    pub fn set_zoom(&self, zoom: [f32; 2], meshes: &mut Assets<Mesh>) {
        for handle in &self.prev_meshes {
            if let Some(mut mesh) = meshes.get_mut(handle) {
                let [hx, hy] = [0.5 * zoom[0], 0.5 * zoom[1]];
                mesh.insert_attribute(
                    Mesh::ATTRIBUTE_POSITION,
                    vec![[-hx, hy, 0.], [hx, hy, 0.], [-hx, -hy, 0.], [hx, -hy, 0.]],
                );
            }
        }
    }

    /// One feedback step outside an event: the last picture becomes the
    /// history (only when `advance`: an event's `SeqBlur` takes a new one
    /// once per event frame) and the other camera draws the scene with the
    /// history over it at `weight` (0 = the plain scene). `None` stops both
    /// cameras.
    pub fn feed(
        &mut self,
        weight: Option<f32>,
        advance: bool,
        cameras: &mut Query<&mut Camera>,
        meshes: &mut Assets<Mesh>,
    ) {
        if advance {
            self.history = self.shown;
        }
        let out = 1 - self.history;
        for (i, entity) in self.cameras.into_iter().enumerate() {
            if let Ok(mut camera) = cameras.get_mut(entity) {
                camera.is_active = weight.is_some() && i == out;
            }
        }
        if let Some(mut mesh) = meshes.get_mut(&self.prev_meshes[out]) {
            mesh.insert_attribute(
                Mesh::ATTRIBUTE_COLOR,
                vec![[1., 1., 1., weight.unwrap_or(0.)]; 4],
            );
        }
        self.shown = out;
    }
}

/// Picks this frame's output image, sets the feedback weight and advances
/// the history on event-frame steps. Runs before `compose`.
pub(super) fn drive(
    event: Res<super::Event>,
    mut blur: ResMut<Blur>,
    mut cameras: Query<&mut Camera>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let frame = event.frame;
    // A new event frame: the last picture shown becomes the history.
    let step = frame.floor() as i32;
    if step != blur.last_step {
        blur.last_step = step;
        blur.history = blur.shown;
    }
    let out = 1 - blur.history;
    for (i, entity) in blur.cameras.into_iter().enumerate() {
        if let Ok(mut camera) = cameras.get_mut(entity) {
            camera.is_active = i == out;
        }
    }
    let weight = blur.weight(frame);
    if let Some(mut mesh) = meshes.get_mut(&blur.prev_meshes[out]) {
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, vec![[1., 1., 1., weight]; 4]);
    }
    blur.shown = out;
}
