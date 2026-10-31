//! KH3's animation on the duel's screen [T282]: the clips are read from the player's own KH3 paks (`data::kh3::sora`),
//! played each frame from the layer's `Visual`, and put on KH2's Sora and Keyblade (`kh3_retarget`). The effects find
//! KH3's bones on KH2's joints.

#![allow(clippy::type_complexity)]

use super::*;
use data::kh3::anim::BonePose;
use data::kh3::sora::{self, Animator, SoraData};
use data::motion_eval::World;

/// The loaded KH3 assets of a session.
pub struct Loaded {
    pub sora: SoraData,
    pub book: Arc<player::kh3::attack::Book>,
    pub fx: Arc<kh3_fx::FxData>,
}

type Job = std::thread::JoinHandle<Result<Loaded, String>>;

#[derive(Resource, Default)]
pub struct Kh3Model {
    job: Option<Job>,
    pub loaded: Option<Arc<Loaded>>,
    pub error: Option<String>,
    retarget: Option<kh3_retarget::Retarget>,
    rigs: [Rig; 2],
}

#[derive(Default)]
struct Rig {
    animator: Animator,
    component: Vec<BonePose>,
    posed: kh3_retarget::Posed,
    blade: BladeShow,
}

/// `TresWeaponBase`'s show and hide (appear 0x141784D50, disappear 0x141784FF0): the mesh changes after a delay. KH2's
/// Keyblade has no `OpacityFade`, so the fade's time passes with the mesh still drawn or still hidden.
#[derive(Default)]
struct BladeShow {
    out: bool,
    delay: f32,
}

impl BladeShow {
    fn drawn(&mut self, out: bool, times: &sora::BladeTimes, dt: f32) -> bool {
        if out != self.out {
            self.out = out;
            self.delay = if out {
                times.appear_delay
            } else {
                times.disappear_delay + times.disappear_dither
            };
        }
        self.delay = (self.delay - dt).max(0.);
        (self.delay > 0.) != out
    }
}

/// KH3's centimetres to the rewrite's units (Sora's size against the room).
fn scale() -> f32 {
    player::kh3::tuning::length_of(1.)
}

/// Starts reading KH3 once a KH3 duel may happen.
fn start_load(model: &mut Kh3Model, game: &Game) {
    if model.job.is_some() || model.loaded.is_some() || model.error.is_some() || !game.versus {
        return;
    }
    let Some(install) = game.ext.online().kh3_install.clone() else {
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

/// KH3's animation is what KH2's Sora shows: KH2's own presenters leave him alone.
pub fn shows_kh3(game: &Game) -> bool {
    duel_kh3::on(game) && game.ext.online().kh3_book.is_some()
}

/// The room's Sora and the rivals, for the frame's writes.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Models<'w, 's> {
    transforms: Query<'w, 's, &'static mut Transform>,
    visibilities: Query<'w, 's, &'static mut Visibility>,
    meshes: ResMut<'w, Assets<Mesh>>,
    room_root: Query<'w, 's, Entity, With<SoraRoot>>,
    room_hand: Query<'w, 's, Entity, With<WeaponRoot>>,
    room_parts: Query<'w, 's, (&'static SoraPart, &'static Mesh3d)>,
    room_blades: Query<'w, 's, (&'static WeaponPart, &'static Mesh3d)>,
    rival_roots: Query<'w, 's, (Entity, &'static rival_play::RivalRoot)>,
    rival_hands: Query<'w, 's, (Entity, &'static rival_play::RivalHand)>,
    rival_parts: Query<'w, 's, (&'static rival_play::RivalPart, &'static Mesh3d)>,
    rival_blades: Query<'w, 's, (&'static rival_play::RivalWeaponPart, &'static Mesh3d)>,
}

impl Models<'_, '_> {
    /// Sets one seat's drawing: where it stands, its skinned points, its Keyblade.
    #[allow(clippy::too_many_arguments)]
    fn draw(
        &mut self,
        game: &Game,
        seat: u8,
        place: Transform,
        points: &[[f32; 3]],
        weapon: Transform,
        drawn: bool,
        blade: Option<&[Vec3]>,
    ) {
        let local = seat == game.local_slot;
        let root = if local {
            self.room_root.iter().next()
        } else {
            self.rival_roots
                .iter()
                .find(|(_, r)| r.0 == seat)
                .map(|x| x.0)
        };
        if let Some(mut t) = root.and_then(|e| self.transforms.get_mut(e).ok()) {
            *t = place;
        }
        let rival = game.rivals.iter().find(|r| r.slot == seat);
        if local {
            for (part, handle) in &self.room_parts {
                let at = game.part_indices[part.0]
                    .iter()
                    .filter_map(|&i| points.get(i as usize).copied());
                visual::room::set_positions(&mut self.meshes, &handle.0, at);
            }
            if let Some(blade) = blade {
                for (part, handle) in &self.room_blades {
                    let at = game.weapon_indices[part.0]
                        .iter()
                        .filter_map(|&i| blade.get(i as usize).map(|v| v.to_array()));
                    visual::room::set_positions(&mut self.meshes, &handle.0, at);
                }
            }
            if let Some(hand) = self.room_hand.iter().next() {
                if let Ok(mut t) = self.transforms.get_mut(hand) {
                    *t = weapon;
                }
                if let Ok(mut v) = self.visibilities.get_mut(hand) {
                    *v = if drawn {
                        Visibility::Inherited
                    } else {
                        Visibility::Hidden
                    };
                }
            }
            return;
        }
        let Some(rival) = rival else {
            return;
        };
        for (part, handle) in &self.rival_parts {
            if part.0 != seat {
                continue;
            }
            if let Some(indices) = rival.shown.part_indices.get(part.1) {
                let at = indices
                    .iter()
                    .filter_map(|&i| points.get(i as usize).copied());
                visual::room::set_positions(&mut self.meshes, &handle.0, at);
            }
        }
        if let Some(blade) = blade {
            for (part, handle) in &self.rival_blades {
                if part.0 != seat || part.1 != 0 {
                    continue;
                }
                if let Some(indices) = rival.shown.weapon_indices[0].get(part.2) {
                    let at = indices
                        .iter()
                        .filter_map(|&i| blade.get(i as usize).map(|v| v.to_array()));
                    visual::room::set_positions(&mut self.meshes, &handle.0, at);
                }
            }
        }
        for (hand, r) in &self.rival_hands {
            if r.0 != seat {
                continue;
            }
            if let Ok(mut t) = self.transforms.get_mut(hand) {
                *t = weapon;
            }
            if let Ok(mut v) = self.visibilities.get_mut(hand) {
                *v = if drawn && r.1 == 0 {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                };
            }
        }
    }
}

/// One frame of KH3's animation on KH2's Sora.
#[allow(clippy::too_many_arguments)]
pub fn update(
    mut commands: Commands,
    mut model: ResMut<Kh3Model>,
    mut game: ResMut<Game>,
    time: Res<Time>,
    mut models: Models,
    mut images: ResMut<Assets<Image>>,
    mut fx: ResMut<kh3_fx::Kh3Fx>,
    mut effect_materials: ResMut<Assets<visual::effect_draw::EffectMaterial>>,
    mut audio_res: Option<ResMut<Audio>>,
    cameras: Query<
        &GlobalTransform,
        (With<Camera3d>, Without<SnapshotCamera>, Without<MenuCamera>),
    >,
) {
    let model = &mut *model;
    start_load(model, &game);
    // Reading KH3 takes some seconds: the duel waits for it.
    game.ext.online_mut().kh3_loading = model.job.is_some() && duel_kh3::on(&game);
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
                game.ext.online_mut().kh3_book = Some(Arc::clone(&loaded.book));
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
    requests.append(&mut game.ext.online_mut().kh3_fx_remote);
    let game = &*game;
    let Some(loaded) = model.loaded.clone().filter(|_| duel_kh3::on(game)) else {
        return;
    };
    let data = &loaded.sora;
    // KH2's Sora and his Keyblade at rest and in his idle (hair, hood, key chain).
    let idle = game
        .poses
        .get(&(Action::Idle, true))
        .and_then(|c| game.clip_poses.get(*c));
    let Some((idle, skin)) = idle.and_then(|c| Some((c, c.skin.clone()?))) else {
        return;
    };
    if model.retarget.is_none() {
        let bind = skin
            .skinning
            .bind_pose(data::model_pose::PoseProfile::Legacy)
            .ok()
            .map(|pose| {
                pose.matrix_bits
                    .iter()
                    .map(|m| Mat4::from_cols_array(&m.map(f32::from_bits)).to_cols_array_2d())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        model.retarget = kh3_retarget::Retarget::new(
            &bind,
            &skin.parents,
            &data.mesh.bones,
            &data.sockets,
            scale(),
        );
        if model.retarget.is_none() {
            eprintln!("Sora's skeleton is not the one the KH3 animation expects");
            model.error = Some("Sora's skeleton is not the one the KH3 animation expects".into());
        }
    }
    let Some(retarget) = model.retarget.as_ref() else {
        return;
    };
    let dt = time.delta_secs();
    let camera = cameras
        .iter()
        .next()
        .map(|c| (c.translation(), *c.forward(), *c.right(), *c.up()));
    let camera_at = camera.map_or(Vec3::ZERO, |c| c.0);
    kh3_fx::requests(&mut fx, requests, camera_at);
    // KH2's idle runs at 60 frames a second.
    let idle_frame = |n: usize| (game.tick as usize).checked_rem(n).unwrap_or(0);
    let idle_joints: &[World] = idle
        .joints
        .get(idle_frame(idle.joints.len()))
        .map_or(&[], Vec::as_slice);
    let blade = idle
        .weapon_frames
        .get(idle_frame(idle.weapon_frames.len()))
        .map(Vec::as_slice);
    for seat in 0..2u8 {
        let Some(fighter) = fighter_of(game, seat) else {
            continue;
        };
        let rig = &mut model.rigs[usize::from(seat)];
        let p = &fighter.player;
        kh3_fx::actors_step(
            &mut fx,
            seat,
            &fighter.ext.fighter().kh3.actors,
            &fighter.ext.fighter().kh3.bolts,
            &loaded.book,
        );
        let local = fighter.ext.fighter().kh3.visual;
        let remote = fighter.ext.fighter().kh3.remote_visual;
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
        retarget.pose(&rig.component, idle_joints, &mut rig.posed);
        fx.surface = kh3_autose::surface_of_arena(game.ext.online().duel_arena);
        kh3_fx::body_step(
            &mut fx,
            data,
            seat,
            p.position,
            p.yaw,
            &rig.posed.bones,
            v,
            row,
            time_in,
            camera_at,
            !p.in_air(),
        );
        let Ok(pose) = skin.skinning.pose_from_worlds(&rig.posed.joints) else {
            continue;
        };
        let points: Vec<[f32; 3]> = pose
            .position_bits
            .iter()
            .map(|b| b.map(f32::from_bits))
            .collect();
        let place = Transform {
            translation: Vec3::from_array(p.position),
            rotation: duel_kh3::turn(p.yaw, &fighter.ext.fighter().kh3),
            scale: Vec3::ONE,
        };
        let drawn = rig.blade.drawn(p.armed, &data.blade_times, dt);
        models.draw(game, seat, place, &points, rig.posed.weapon, drawn, blade);
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
        &mut models.meshes,
        &mut effect_materials,
        &mut models.visibilities,
        guard.as_deref_mut(),
    );
}
