//! The KH3 layer against synthetic rooms: a flat floor, a wall, a pole.

use super::*;
use crate::{Clip, TriangleWorld};

fn params() -> Params {
    Params {
        walk_speed: 2.,
        run_speed: 8.,
        jump_height: 185.,
        turn_speed: 0.2618,
        fall_max: 16.,
        land_inertia: 0.9,
        land_drag: 0.8,
        head_height: 100.,
        step_height: 20.,
        walk_threshold: 0.8,
        radius: 30.,
        height: 140.,
        disarm_ticks: 120,
    }
}

fn clips() -> Clips {
    let looping = Clip::looping(30, 0);
    Clips {
        idle: looping.clone(),
        walk: looping.clone(),
        run: looping.clone(),
        field_idle: looping.clone(),
        field_walk: looping.clone(),
        field_run: looping.clone(),
        jump: looping.clone(),
        fall: looping.clone(),
        land: Clip::looping(20, 20),
        records: Vec::new(),
        attacks: Vec::new(),
        spells: Vec::new(),
        hurt: Vec::new(),
        guard: None,
        guard_recoil: None,
        wall_recoil: None,
        item: None,
        drive: None,
        drive_in: None,
        growth: growth::MOTIONS
            .iter()
            .map(|&m| (m, Clip::looping(40, 40), Vec::new()))
            .collect(),
        scripted: Vec::new(),
    }
}

/// A floor 8000 across at y 0, and walls (each a quad, corners at y 0 and
/// `h`) from `a` to `b` in XZ.
fn room(walls: &[([f32; 2], [f32; 2], f32)]) -> TriangleWorld {
    let f = 4000.;
    let mut tris = vec![
        ([[-f, 0., -f], [f, 0., -f], [f, 0., f]], true, false),
        ([[-f, 0., -f], [f, 0., f], [-f, 0., f]], true, false),
    ];
    for &(a, b, h) in walls {
        tris.push((
            [[a[0], 0., a[1]], [b[0], 0., b[1]], [b[0], h, b[1]]],
            false,
            true,
        ));
        tris.push((
            [[a[0], 0., a[1]], [b[0], h, b[1]], [a[0], h, a[1]]],
            false,
            true,
        ));
    }
    TriangleWorld::new(tris, 0.7)
}

struct Rig {
    kh3: Kh3,
    player: Player,
    p: Params,
    clips: Clips,
    world: TriangleWorld,
}

impl Rig {
    fn new(world: TriangleWorld) -> Rig {
        let mut r = Rig {
            kh3: Kh3::default(),
            player: Player::new([0.; 3], 0.),
            p: params(),
            clips: clips(),
            world,
        };
        r.step(&Input::default(), &Pad::default(), None);
        r
    }

    fn step(&mut self, input: &Input, pad: &Pad, foe: Option<Foe>) -> Outcome {
        let scene = Scene { foe, poles: &[] };
        self.kh3.tick(
            &mut self.player,
            input,
            pad,
            &self.p,
            &self.clips,
            &self.world,
            &scene,
        )
    }

    fn idle(&mut self, n: usize, held: bool) {
        for _ in 0..n {
            self.step(
                &Input {
                    jump_up: !held,
                    ..Input::default()
                },
                &Pad::default(),
                None,
            );
        }
    }
}

fn press_jump() -> Input {
    Input {
        jump: true,
        jump_up: false,
        ..Input::default()
    }
}

#[test]
fn the_jump_leaves_on_its_fifth_tick_and_peaks_at_the_held_height() {
    let mut r = Rig::new(room(&[]));
    assert_eq!(r.kh3.mode, Mode::Ground);
    r.step(&press_jump(), &Pad::default(), None);
    let mut left = None;
    let mut peak: f32 = 0.;
    for tick in 2..200 {
        // The button held until the apex, then let go (no glide).
        let up = r.kh3.velocity[1] <= 0. && tick > 10;
        r.step(
            &Input {
                jump_up: up,
                ..Input::default()
            },
            &Pad::default(),
            None,
        );
        if left.is_none() && r.player.position[1] > 0. {
            left = Some(tick);
        }
        peak = peak.max(r.player.position[1]);
        if matches!(r.kh3.mode, Mode::Land { .. }) {
            break;
        }
    }
    // Leaves on the fifth tick after the press (the 4th of the take-off,
    // then the first air tick moves him).
    assert_eq!(left, Some(6));
    let held = tuning::JUMP * tuning::JUMP / (2. * tuning::GRAVITY_RISE);
    assert!((peak - held).abs() < 25., "peak {peak}, held {held}");
    assert!(matches!(r.kh3.mode, Mode::Land { .. }));
    assert!((r.player.position[1]).abs() < 1e-3);
}

#[test]
fn a_tapped_jump_is_lower() {
    let mut r = Rig::new(room(&[]));
    r.step(&press_jump(), &Pad::default(), None);
    let mut peak: f32 = 0.;
    for _ in 0..120 {
        r.step(
            &Input {
                jump_up: true,
                ..Input::default()
            },
            &Pad::default(),
            None,
        );
        peak = peak.max(r.player.position[1]);
    }
    let held = tuning::JUMP * tuning::JUMP / (2. * tuning::GRAVITY_RISE);
    assert!(peak < held * 0.75 && peak > held * 0.4, "{peak} vs {held}");
}

#[test]
fn doubleflight_once_per_airtime() {
    let mut r = Rig::new(room(&[]));
    r.step(&press_jump(), &Pad::default(), None);
    // Rise until the vertical speed is at most 500 cm/s.
    let mut n = 0;
    while r.kh3.velocity[1] > tuning::DOUBLEFLIGHT_BELOW || n < 6 {
        r.step(
            &Input {
                jump_up: false,
                ..Input::default()
            },
            &Pad::default(),
            None,
        );
        n += 1;
        assert!(n < 60);
    }
    let before = r.player.position[1];
    r.step(&press_jump(), &Pad::default(), None);
    assert!(matches!(
        r.kh3.mode,
        Mode::Takeoff {
            jump: Jump::Double,
            ..
        }
    ));
    r.idle(4, true);
    assert_eq!(
        r.kh3.velocity[1],
        tuning::DOUBLEFLIGHT - tuning::GRAVITY_RISE
    );
    let mut peak = 0f32;
    let let_go = Input {
        jump_up: true,
        ..Input::default()
    };
    for _ in 0..40 {
        r.step(&let_go, &Pad::default(), None);
        peak = peak.max(r.player.position[1]);
    }
    let rise = tuning::DOUBLEFLIGHT * tuning::DOUBLEFLIGHT / (2. * tuning::GRAVITY_RISE);
    assert!(peak - before > rise * 0.8, "{} vs {rise}", peak - before);
    // A third press does nothing.
    let vy = r.kh3.velocity[1];
    r.step(
        &Input {
            jump: true,
            jump_up: true,
            ..Input::default()
        },
        &Pad::default(),
        None,
    );
    assert!(matches!(r.kh3.mode, Mode::Air { .. }) && r.kh3.velocity[1] < vy);
}

#[test]
fn the_dodge_roll_covers_its_curve_with_invulnerability_first() {
    let mut r = Rig::new(room(&[]));
    let roll = Input {
        guard: true,
        direction: [1., 0.],
        ..Input::default()
    };
    r.step(&roll, &Pad::default(), None);
    assert!(matches!(r.kh3.mode, Mode::Roll { .. }));
    assert_eq!(r.player.action, Action::Growth(growth::motion::DODGE_ROLL));
    let mut invulnerable = 0;
    for _ in 0..tuning::ROLL_TICKS {
        if r.player.invulnerable > 0. {
            invulnerable += 1;
        }
        r.step(&Input::default(), &Pad::default(), None);
    }
    assert!(
        (r.player.position[0] - 600. * tuning::SCALE).abs() < 1.,
        "{:?}",
        r.player.position
    );
    assert!((tuning::ROLL_CANCEL - 1..=tuning::ROLL_CANCEL + 1).contains(&invulnerable));
    assert_eq!(r.kh3.mode, Mode::Ground);
}

#[test]
fn the_air_slide_is_level_invulnerable_and_cools_down() {
    let mut r = Rig::new(room(&[]));
    r.step(&press_jump(), &Pad::default(), None);
    r.idle(12, false);
    let y = r.player.position[1];
    let x = r.player.position[0];
    let slide = Input {
        guard: true,
        direction: [1., 0.],
        jump_up: true,
        ..Input::default()
    };
    r.step(&slide, &Pad::default(), None);
    assert!(matches!(r.kh3.mode, Mode::Slide { .. }));
    for _ in 0..tuning::SLIDE_TICKS {
        assert!(r.player.invulnerable > 0.);
        assert!((r.player.position[1] - y).abs() < 1e-3, "no gravity");
        r.step(
            &Input {
                jump_up: true,
                ..Input::default()
            },
            &Pad::default(),
            None,
        );
    }
    assert!((r.player.position[0] - x - tuning::SLIDE_DISTANCE).abs() < 1.);
    assert!(matches!(r.kh3.mode, Mode::Air { .. }));
    // Refused until the cooldown ends.
    r.step(&slide, &Pad::default(), None);
    assert!(matches!(r.kh3.mode, Mode::Air { .. }));
}

#[test]
fn walls_classify_by_the_stick() {
    let wall = Wall {
        normal: [0., -1.],
        surface: [0., 100.],
    };
    assert_eq!(Kh3::wall_kind(&wall, [0., 1.]), Some((true, 0.)));
    let a = 30f32.to_radians();
    assert!(matches!(
        Kh3::wall_kind(&wall, [a.sin(), a.cos()]),
        Some((true, _))
    ));
    let a = 42f32.to_radians();
    assert_eq!(Kh3::wall_kind(&wall, [a.sin(), a.cos()]), None, "dead zone");
    let a = 80f32.to_radians();
    assert!(matches!(
        Kh3::wall_kind(&wall, [a.sin(), a.cos()]),
        Some((false, _))
    ));
    assert_eq!(Kh3::wall_kind(&wall, [0., -1.]), None, "away from it");
}

#[test]
fn running_into_a_wall_runs_up_it_and_jumps_off() {
    // A wall across z = 200, 1500 tall.
    let mut r = Rig::new(room(&[([-1000., 200.], [1000., 200.], 1500.)]));
    let run = Input {
        direction: [0., 1.],
        jump_up: true,
        ..Input::default()
    };
    let mut ran = false;
    for _ in 0..60 {
        r.step(&run, &Pad::default(), None);
        if matches!(r.kh3.mode, Mode::WallRun { up: true, .. }) {
            ran = true;
            break;
        }
    }
    assert!(ran, "{:?} at {:?}", r.kh3.mode, r.player.position);
    let y = r.player.position[1];
    for _ in 0..20 {
        r.step(&run, &Pad::default(), None);
    }
    assert!((r.player.position[1] - y - 20. * tuning::WALL_RUN).abs() < 1.);
    assert!(r.kh3.lean[0] < -1.3, "pitched back: {:?}", r.kh3.lean);
    assert!(
        (r.player.position[2] - (200. - 1.)).abs() < 2.,
        "on the wall"
    );
    r.step(&press_jump(), &Pad::default(), None);
    assert!(matches!(
        r.kh3.mode,
        Mode::Air {
            jump: Jump::Super,
            ..
        }
    ));
    assert_eq!(r.kh3.velocity[1], tuning::SUPER_JUMP);
    assert!(r.player.position[2] < 200. - r.p.radius, "off the wall");
}

#[test]
fn the_shotlock_locks_burns_focus_and_fires_a_homing_volley() {
    let mut r = Rig::new(room(&[]));
    let foe = Foe {
        position: [0., 0., 1500.],
        centre: [0., 80., 1500.],
    };
    let hold = Pad {
        shotlock: true,
        ..Pad::default()
    };
    for _ in 0..tuning::AIM_HOLD {
        r.step(&Input::default(), &hold, Some(foe));
    }
    assert!(r.kh3.aim.is_some());
    for _ in 0..200 {
        r.step(&Input::default(), &hold, Some(foe));
    }
    let aim = r.kh3.aim.unwrap();
    assert_eq!(aim.locks, tuning::MAX_LOCKS);
    // 24 locks at 2.24 / 24 s each: about 45 focus (2.24 s at 20 a
    // second), less what the regeneration gave back.
    let spent = tuning::FOCUS_MAX - r.kh3.focus;
    assert!((38. ..48.).contains(&spent), "{spent}");
    let fire = Input {
        attack: true,
        ..Input::default()
    };
    let out = r.step(&fire, &hold, Some(foe));
    assert!(out.began.contains(&"ragnarok"));
    let mut spawned = 0;
    let mut lifted = 0f32;
    for _ in 0..tuning::RAGNAROK_END {
        let out = r.step(&Input::default(), &hold, Some(foe));
        spawned += out.spawned.len();
        lifted = lifted.max(r.player.position[1]);
        assert!(r.player.invulnerable > 0. || !matches!(r.kh3.mode, Mode::Volley { .. }));
    }
    assert_eq!(spawned, 24);
    assert!((lifted - tuning::VOLLEY_LIFT).abs() < 1.);
    assert!(matches!(r.kh3.mode, Mode::Air { .. }));
    // The bolts fly at the target.
    let close = r
        .kh3
        .bolts
        .iter()
        .filter(|b| {
            len3([
                b.pos[0] - foe.centre[0],
                b.pos[1] - foe.centre[1],
                b.pos[2] - foe.centre[2],
            ]) < 600.
        })
        .count();
    assert!(close > 12, "{close} of {} near", r.kh3.bolts.len());
}

#[test]
fn letting_go_cancels_the_aim_and_a_few_locks_fire_king_of_hearts() {
    let mut r = Rig::new(room(&[]));
    let foe = Foe {
        position: [0., 0., 900.],
        centre: [0., 80., 900.],
    };
    let hold = Pad {
        shotlock: true,
        ..Pad::default()
    };
    for _ in 0..tuning::AIM_HOLD + 20 {
        r.step(&Input::default(), &hold, Some(foe));
    }
    r.step(&Input::default(), &Pad::default(), Some(foe));
    assert!(r.kh3.aim.is_none() && r.kh3.mode == Mode::Ground);
    for _ in 0..tuning::AIM_HOLD + 20 {
        r.step(&Input::default(), &hold, Some(foe));
    }
    let locks = r.kh3.aim.unwrap().locks;
    assert!((2..10).contains(&locks), "{locks}");
    let out = r.step(
        &Input {
            attack: true,
            ..Input::default()
        },
        &hold,
        Some(foe),
    );
    assert!(out.began.contains(&"king_of_hearts"));
}

#[test]
fn airstep_speeds_follow_the_leg_length() {
    let cm = |v: f32| v * 60. / tuning::SCALE;
    let at = |d_cm: f32| cm(airstep_speed(d_cm * tuning::SCALE));
    assert!((at(1000.) - 6000.).abs() < 1.);
    assert!((at(4000.) - 8000.).abs() < 1., "2d for long legs");
    assert!((at(300.) - 3000.).abs() < 1., "capped near");
    assert!((at(20.) - 1200.).abs() < 1., "60 d for tiny legs");
}

#[test]
fn the_airstep_reaches_the_target_and_hops_back() {
    let mut r = Rig::new(room(&[]));
    let foe = Foe {
        position: [0., 0., 1200.],
        centre: [0., 80., 1200.],
    };
    let hold = Pad {
        shotlock: true,
        ..Pad::default()
    };
    for _ in 0..tuning::AIM_HOLD + 2 {
        r.step(&Input::default(), &hold, Some(foe));
    }
    r.step(
        &Input {
            guard: true,
            ..Input::default()
        },
        &hold,
        Some(foe),
    );
    assert!(matches!(r.kh3.mode, Mode::Airstep { .. }));
    let mut hopped = false;
    for _ in 0..60 {
        r.step(&Input::default(), &Pad::default(), Some(foe));
        if matches!(r.kh3.mode, Mode::HopBack { .. }) {
            hopped = true;
            break;
        }
        assert!(r.player.invulnerable > 0.);
    }
    assert!(hopped, "{:?} at {:?}", r.kh3.mode, r.player.position);
    assert!(r.player.position[2] > 1200. - 300.);
}

#[test]
fn a_held_jump_glides_after_the_apex_and_letting_go_falls() {
    let mut r = Rig::new(room(&[]));
    r.step(&press_jump(), &Pad::default(), None);
    let held = Input {
        jump_up: false,
        direction: [0., 1.],
        ..Input::default()
    };
    let mut glided = false;
    for _ in 0..60 {
        r.step(&held, &Pad::default(), None);
        if matches!(r.kh3.mode, Mode::Glide { .. }) {
            glided = true;
            break;
        }
    }
    assert!(glided);
    let z = r.player.position[2];
    for _ in 0..10 {
        r.step(&held, &Pad::default(), None);
    }
    assert!((r.player.position[2] - z - 10. * tuning::GLIDE).abs() < 1.);
    r.step(
        &Input {
            jump_up: true,
            ..Input::default()
        },
        &Pad::default(),
        None,
    );
    assert!(matches!(
        r.kh3.mode,
        Mode::Air {
            jump: Jump::None,
            ..
        }
    ));
}

#[test]
fn a_pole_in_the_slides_way_is_spun_round_then_left_by_a_superslide() {
    let pole = Pole {
        centre: [0., 500.],
        radius: 40.,
        bottom: 0.,
        top: 1200.,
    };
    let mut r = Rig::new(room(&[]));
    let poles = [pole];
    let tick = |r: &mut Rig, input: &Input| {
        let scene = Scene {
            foe: None,
            poles: &poles,
        };
        r.kh3.tick(
            &mut r.player,
            input,
            &Pad::default(),
            &r.p,
            &r.clips,
            &r.world,
            &scene,
        )
    };
    tick(&mut r, &press_jump());
    for _ in 0..12 {
        tick(&mut r, &Input::default());
    }
    // Slightly off line, so it spins one way.
    tick(
        &mut r,
        &Input {
            guard: true,
            direction: [0.05, 1.],
            jump_up: true,
            ..Input::default()
        },
    );
    let mut spun = false;
    for _ in 0..40 {
        tick(
            &mut r,
            &Input {
                jump_up: true,
                ..Input::default()
            },
        );
        if matches!(r.kh3.mode, Mode::Pole { .. }) {
            spun = true;
            break;
        }
    }
    assert!(spun, "{:?} at {:?}", r.kh3.mode, r.player.position);
    let mut slid = false;
    for _ in 0..(tuning::POLE_ATTACH + tuning::POLE_TURN * tuning::POLE_TURNS + 2) {
        tick(
            &mut r,
            &Input {
                jump_up: true,
                ..Input::default()
            },
        );
        if let Mode::Pole { .. } = r.kh3.mode {
            let d = (r.player.position[0] - pole.centre[0])
                .hypot(r.player.position[2] - pole.centre[1]);
            assert!(d < pole.radius + tuning::POLE_GAP + 120.);
        }
        if matches!(r.kh3.mode, Mode::Superslide { .. }) {
            slid = true;
            break;
        }
    }
    assert!(slid, "{:?}", r.kh3.mode);
}

#[test]
fn style_points_offer_the_element_used_most() {
    let mut k = Kh3::default();
    for _ in 0..4 {
        k.spell_released(crate::magic::FIRE);
    }
    assert!(k.offer.is_none());
    for _ in 0..4 {
        k.spell_released(crate::magic::BLIZZARD);
    }
    assert_eq!(k.offer.map(|o| o.0), Some(crate::magic::FIRE));
    k.grand_used();
    assert!(k.offer.is_none());
}

#[test]
fn snapshots_carry_what_the_joiner_shows() {
    let mut k = Kh3 {
        focus: 63.4,
        lean: [-1.5, 0.25],
        ..Kh3::default()
    };
    k.aim = Some(Aim {
        locks: 9,
        ..Aim::default()
    });
    let shown = k.shown();
    let mut j = Kh3::default();
    j.apply_shown(&shown);
    assert_eq!(j.focus, 63.);
    assert_eq!(j.aim.map(|a| a.locks), Some(9));
    assert_eq!(j.lean, [-1.5, 0.25]);
    assert_eq!(MODE_NAMES.len(), 22);
    for n in MODE_NAMES {
        assert!(mode_named(n).is_some());
    }
}

#[test]
fn armour_shrugs_off_light_reactions_only() {
    let k = Kh3 {
        mode: Mode::Superslide {
            tick: 3,
            pole: None,
        },
        ..Kh3::default()
    };
    assert!(k.shrugs(2) && k.shrugs(3) && !k.shrugs(4));
    assert!(!Kh3::default().shrugs(2));
}

#[test]
fn a_shotlock_held_through_a_roll_aims_once_it_ends_and_only_once() {
    let mut r = Rig::new(room(&[]));
    let foe = Foe {
        position: [0., 0., 900.],
        centre: [0., 80., 900.],
    };
    let hold = Pad {
        shotlock: true,
        ..Pad::default()
    };
    r.step(
        &Input {
            guard: true,
            direction: [1., 0.],
            ..Input::default()
        },
        &hold,
        Some(foe),
    );
    for _ in 0..tuning::AIM_HOLD {
        r.step(&Input::default(), &hold, Some(foe));
    }
    assert!(r.kh3.aim.is_none(), "not while rolling");
    for _ in 0..tuning::ROLL_TICKS {
        r.step(&Input::default(), &hold, Some(foe));
    }
    assert!(r.kh3.aim.is_some());
    // Fired: no new aim until the button is let go.
    r.step(
        &Input {
            attack: true,
            ..Input::default()
        },
        &hold,
        Some(foe),
    );
    for _ in 0..tuning::KOH_END + 2 {
        r.step(&Input::default(), &hold, Some(foe));
    }
    assert!(r.kh3.aim.is_none());
    r.step(&Input::default(), &Pad::default(), Some(foe));
    for _ in 0..tuning::AIM_HOLD {
        r.step(&Input::default(), &hold, Some(foe));
    }
    assert!(r.kh3.aim.is_some());
}

#[test]
fn a_superslide_does_not_pass_through_a_thin_wall() {
    // One wall plane across z = 300 (no thickness), 1000 tall.
    let mut r = Rig::new(room(&[([-1000., 300.], [1000., 300.], 1000.)]));
    r.player.position = [0., 200., 0.];
    r.player.yaw = 0.;
    r.kh3.mode = Mode::Superslide {
        tick: 0,
        pole: None,
    };
    r.kh3.shown = Some(r.player.action);
    for _ in 0..30 {
        r.step(
            &Input {
                jump_up: true,
                ..Input::default()
            },
            &Pad::default(),
            None,
        );
        assert!(r.player.position[2] < 300., "{:?}", r.player.position);
    }
}
