use bevy::{app::AppExit, prelude::*};
use std::{
    fs::{self, File},
    io::Write,
    path::PathBuf,
};

#[derive(Component)]
struct TestObject;

#[derive(Resource)]
struct SmokeRun {
    log: File,
    frames: u64,
    exit_after: Option<u64>,
    gamepad_control: bool,
}

fn main() {
    let mut args = std::env::args().skip(1);
    let mut exit_after = None;
    let mut gamepad_control = false;
    let mut log_path = validation::host::local_data_root()
        .expect("no local data folder is known")
        .join("KH2FinalRewrite/logs/smoke.log");
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--gamepad" => gamepad_control = true,
            "--frames" => {
                exit_after = Some(
                    args.next()
                        .expect("--frames needs a number")
                        .parse()
                        .expect("invalid frame count"),
                )
            }
            "--log" => log_path = PathBuf::from(args.next().expect("--log needs a path")),
            _ => panic!("unknown argument: {arg}"),
        }
    }
    if let Some(parent) = log_path.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent).expect("create smoke log directory");
    }
    let mut log = File::create(log_path).expect("create smoke log");
    writeln!(log, "P03 development smoke; no game data; bevy=0.19.1").unwrap();
    App::new()
        .insert_resource(SmokeRun {
            log,
            frames: 0,
            exit_after,
            gamepad_control,
        })
        .insert_resource(ClearColor(Color::srgb(0.025, 0.04, 0.075)))
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "KH2FM development smoke — WASD/arrows; Esc exits".into(),
                        resolution: (960, 540).into(),
                        ..default()
                    }),
                    ..default()
                })
                .set(AssetPlugin {
                    file_path: format!("{}/assets", env!("CARGO_MANIFEST_DIR")),
                    ..default()
                }),
        )
        .add_systems(Startup, setup)
        .add_systems(Update, (move_object, finish_smoke))
        .run();
}

fn setup(mut commands: Commands, assets: Res<AssetServer>, mut run: ResMut<SmokeRun>) {
    commands.spawn(Camera2d);
    commands.spawn((
        Sprite {
            image: assets.load("development-checker.bmp"),
            custom_size: Some(Vec2::splat(80.0)),
            ..default()
        },
        Transform::default(),
        TestObject,
    ));
    writeln!(run.log, "window_started; development_asset_requested").unwrap();
    run.log.flush().unwrap();
}

fn move_object(
    keys: Res<ButtonInput<KeyCode>>,
    pads: Query<&Gamepad>,
    time: Res<Time>,
    run: Res<SmokeRun>,
    mut objects: Query<&mut Transform, With<TestObject>>,
) {
    let mut direction = Vec2::ZERO;
    for (key, delta) in [
        (KeyCode::KeyW, Vec2::Y),
        (KeyCode::ArrowUp, Vec2::Y),
        (KeyCode::KeyS, Vec2::NEG_Y),
        (KeyCode::ArrowDown, Vec2::NEG_Y),
        (KeyCode::KeyA, Vec2::NEG_X),
        (KeyCode::ArrowLeft, Vec2::NEG_X),
        (KeyCode::KeyD, Vec2::X),
        (KeyCode::ArrowRight, Vec2::X),
    ] {
        if keys.pressed(key) {
            direction += delta;
        }
    }
    if run.gamepad_control {
        for pad in &pads {
            let stick = pad.left_stick();
            if stick.length_squared() > 0.15 * 0.15 {
                direction += stick;
            }
        }
    }
    for mut transform in &mut objects {
        transform.translation +=
            (direction.clamp_length_max(1.0) * 240.0 * time.delta_secs()).extend(0.0);
    }
}

fn finish_smoke(
    mut run: ResMut<SmokeRun>,
    keys: Res<ButtonInput<KeyCode>>,
    pads: Query<&Gamepad>,
    sprites: Query<&Sprite, With<TestObject>>,
    assets: Res<Assets<Image>>,
    mut exit: MessageWriter<AppExit>,
) {
    run.frames += 1;
    if run.frames == 120 {
        let loaded = sprites.iter().all(|sprite| assets.contains(&sprite.image));
        writeln!(
            run.log,
            "frame=120; sample_loaded={loaded}; gamepads={}",
            pads.iter().count()
        )
        .unwrap();
        for pad in &pads {
            writeln!(run.log, "gamepad_left_stick={:?}", pad.left_stick()).unwrap();
        }
        run.log.flush().unwrap();
    }
    if keys.just_pressed(KeyCode::Escape) || run.exit_after.is_some_and(|limit| run.frames >= limit)
    {
        let frames = run.frames;
        let loaded = sprites.iter().all(|sprite| assets.contains(&sprite.image));
        writeln!(
            run.log,
            "clean_exit; frames={frames}; sample_loaded={loaded}"
        )
        .unwrap();
        run.log.flush().unwrap();
        exit.write(if loaded {
            AppExit::Success
        } else {
            AppExit::Error(std::num::NonZeroU8::new(1).unwrap())
        });
    }
}
