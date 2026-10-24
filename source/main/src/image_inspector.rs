//! Quarantined source-bound texture research; no game loop or rule model.
use bevy::{
    app::AppExit,
    asset::RenderAssetUsages,
    image::ImageSampler,
    prelude::*,
    render::{
        render_resource::{Extent3d, TextureDimension, TextureFormat},
        view::screenshot::{Screenshot, ScreenshotCaptured},
    },
    window::PrimaryWindow,
};
use data::{
    inspection::{InspectionImage, Projection, read_selected_images},
    probe_source::ProbeSource,
};
use sha2::{Digest, Sha256};
use std::{
    fs::{File, OpenOptions},
    io::Write,
    path::PathBuf,
};

#[derive(Component)]
struct TextureView;
#[derive(Resource)]
struct Inspector {
    items: Vec<InspectionImage>,
    selected: usize,
    mode: Projection,
    zoom: f32,
    fit: bool,
    dirty: bool,
    handle: Option<Handle<Image>>,
    log: File,
    frames: u64,
    dwell: u64,
    exit_after: Option<u64>,
    tour: bool,
    step: usize,
    capture_paths: Vec<PathBuf>,
    pending: bool,
    captured: usize,
}
impl Inspector {
    fn event(&mut self, value: serde_json::Value) {
        serde_json::to_writer(&mut self.log, &value).expect("private inspector log write");
        writeln!(self.log).expect("private inspector log newline");
        self.log.flush().expect("private inspector log flush");
    }
}
fn run(args: Vec<PathBuf>) -> Result<AppExit, Box<dyn std::error::Error>> {
    let mut args = args.into_iter();
    let root = args.next().ok_or("install root required")?;
    let manifest = args.next().ok_or("source manifest required")?;
    let selection = args.next().ok_or("explicit image selection required")?;
    let output = args.next().ok_or("new private JSONL log required")?;
    let mut tour = false;
    let mut capture_dir = None;
    let mut exit_after = None;
    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some("--tour") => tour = true,
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
            _ => return Err("unknown inspector option".into()),
        }
    }
    if tour && capture_dir.is_none() {
        return Err("tour requires existing private capture directory".into());
    }
    let source = ProbeSource::bind(&root, &manifest)?;
    let output = source.output_path(&output)?;
    if let Some(directory) = &capture_dir {
        source.output_path(&directory.join("capture-admission.png"))?;
    }
    eprintln!("Full selected-source rehash; candidate remains quarantined.");
    let items = read_selected_images(&source, &selection)?;
    let count = if tour {
        items.len() * Projection::ALL.len()
    } else {
        usize::from(capture_dir.is_some())
    };
    let mut capture_paths = Vec::new();
    if let Some(directory) = capture_dir {
        for step in 0..count {
            capture_paths.push(source.output_path(&directory.join(format!("view-{step:03}.png")))?);
        }
    }
    let log = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)?;
    let mut inspector = Inspector {
        items,
        selected: 0,
        mode: Projection::Composite,
        zoom: 1.0,
        fit: false,
        dirty: true,
        handle: None,
        log,
        frames: 0,
        dwell: 0,
        exit_after,
        tour,
        step: 0,
        capture_paths,
        pending: false,
        captured: 0,
    };
    inspector.event(serde_json::json!({"event":"admitted","qualification":"quarantined_image_research_only",
        "source_content_identity":source.content_identity(),"fresh_full_scan":true,"images":inspector.items.len(),
        "tour":tour,"expected_captures":count,"gameplay_enabled":false,
        "display_projection":"opaque stored-byte sRGB display; no installed shader claim"}));
    let mut app = App::new();
    app.insert_resource(inspector)
        .insert_resource(ClearColor(Color::srgb(0.055, 0.065, 0.08)))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "KH2FM quarantined texture research".into(),
                resolution:
                    bevy::window::WindowResolution::new(1100, 900).with_scale_factor_override(1.0),
                ..default()
            }),
            ..default()
        }))
        .add_systems(Startup, setup)
        .add_systems(Update, (controls, refresh, finish).chain());
    Ok(app.run())
}
fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);
    commands.spawn((Sprite::default(), Transform::default(), TextureView));
}
fn controls(
    keys: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<Inspector>,
    mut exit: MessageWriter<AppExit>,
) {
    if keys.just_pressed(KeyCode::Escape) {
        let complete = !state.tour || state.captured == state.capture_paths.len();
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
    for (key, delta) in [(KeyCode::ArrowRight, 1isize), (KeyCode::ArrowLeft, -1)] {
        if keys.just_pressed(key) {
            state.selected =
                (state.selected as isize + delta).rem_euclid(state.items.len() as isize) as usize;
            state.dirty = true;
        }
    }
    for (key, mode) in [
        (KeyCode::KeyC, Projection::Composite),
        (KeyCode::KeyV, Projection::StoredRgb),
        (KeyCode::KeyR, Projection::Red),
        (KeyCode::KeyG, Projection::Green),
        (KeyCode::KeyB, Projection::Blue),
        (KeyCode::KeyA, Projection::Alpha),
    ] {
        if keys.just_pressed(key) {
            state.mode = mode;
            state.dirty = true;
        }
    }
    if keys.just_pressed(KeyCode::KeyF) {
        state.fit = true;
        state.dirty = true;
    }
    if keys.just_pressed(KeyCode::Digit1) {
        state.fit = false;
        state.zoom = 1.0;
        state.dirty = true;
    }
    for (key, factor) in [(KeyCode::Equal, 2.0), (KeyCode::Minus, 0.5)] {
        if keys.just_pressed(key) {
            state.fit = false;
            state.zoom = (state.zoom * factor).clamp(0.25, 8.0);
            state.dirty = true;
        }
    }
}
fn refresh(
    mut state: ResMut<Inspector>,
    mut images: ResMut<Assets<Image>>,
    mut sprites: Query<(&mut Sprite, &mut Transform), With<TextureView>>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
) {
    if !state.dirty {
        return;
    }
    let item = &state.items[state.selected];
    let pixels = state
        .mode
        .pixels(&item.decoded)
        .expect("bounded image projection");
    let projection_hash = format!("{:x}", Sha256::digest(&pixels));
    let (width, height) = (item.decoded.width, item.decoded.height);
    let format = item.decoded.format;
    let metadata = serde_json::json!({"event":"view","item":state.selected,"index":item.index,"ordinal":item.ordinal,
        "replacement_ordinal":item.replacement_ordinal,"width":width,"height":height,"mode":state.mode,
        "pixel_decode_profile":item.decoded.pixel_decode_profile,
        "stored_pixels_sha256":format!("{:x}",Sha256::digest(&item.decoded.pixels_rgba)),
        "projection_sha256":projection_hash,"zoom":state.zoom,"fit":state.fit});
    let mut image = Image::new(
        Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        pixels,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    image.sampler = ImageSampler::nearest();
    let handle = images.add(image);
    if let Some(old) = state.handle.replace(handle.clone()) {
        images.remove(old.id());
    }
    let mut window = windows.single_mut().expect("inspector window");
    let zoom = if state.fit {
        ((window.width() - 32.0) / width as f32).min((window.height() - 32.0) / height as f32)
    } else {
        state.zoom
    };
    for (mut sprite, mut transform) in &mut sprites {
        sprite.image = handle.clone();
        sprite.custom_size = Some(Vec2::new(width as f32, height as f32));
        transform.scale = Vec3::new(zoom, zoom, 1.0);
    }
    window.title = format!(
        "QUARANTINED KH2FM image {}/{} | {:?} {}x{} | {:?} | arrows images; C/V/R/G/B/A; F fit; 1 native; +/- zoom; Esc",
        state.selected + 1,
        state.items.len(),
        format,
        width,
        height,
        state.mode
    );
    state.event(metadata);
    state.dirty = false;
    state.dwell = 0;
}
fn finish(mut commands: Commands, mut state: ResMut<Inspector>, mut exit: MessageWriter<AppExit>) {
    state.frames += 1;
    state.dwell += 1;
    if !state.pending
        && state.captured < state.capture_paths.len()
        && state.dwell >= if state.step == 0 { 120 } else { 12 }
    {
        let path = state.capture_paths[state.step].clone();
        state.pending = true;
        commands.spawn(Screenshot::primary_window()).observe(
            move |event: On<ScreenshotCaptured>, mut state: ResMut<Inspector>| {
                let dynamic = event
                    .image
                    .clone()
                    .try_into_dynamic()
                    .expect("GPU screenshot conversion");
                let rgba = dynamic.to_rgba8();
                let mut file = OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&path)
                    .expect("new private screenshot");
                dynamic
                    .write_to(&mut file, image::ImageFormat::Png)
                    .expect("private screenshot encode");
                file.sync_all().expect("private screenshot flush");
                let frames = state.frames;
                let item = state.selected;
                let mode = state.mode;
                let step = state.step;
                state.event(
                    serde_json::json!({"event":"captured","step":step,"item":item,"mode":mode,
                "frames":frames,"width":rgba.width(),"height":rgba.height(),"file":path.file_name().and_then(|name|name.to_str()),
                "pixels_sha256":format!("{:x}",Sha256::digest(rgba.as_raw()))}),
                );
                state.pending = false;
                state.captured += 1;
                if state.tour && state.captured < state.capture_paths.len() {
                    state.step += 1;
                    state.selected = state.step / Projection::ALL.len();
                    state.mode = Projection::ALL[state.step % Projection::ALL.len()];
                    state.dirty = true;
                }
            },
        );
    }
    let tour_done = state.tour && !state.pending && state.captured == state.capture_paths.len();
    if tour_done || state.exit_after.is_some_and(|limit| state.frames >= limit) {
        let complete = !state.pending && state.captured == state.capture_paths.len();
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
            eprintln!("Image research rejected: {error}");
            std::process::ExitCode::FAILURE
        }
    }
}
