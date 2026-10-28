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
    assert_eq!(MODE_NAMES.len(), 33);
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

// Hit reactions, Final Blow, the Risk Dodge, Focus Syphon and the cues.

fn idle_input() -> Input {
    Input {
        jump_up: true,
        ..Input::default()
    }
}

#[test]
fn every_attack_gives_the_reference_s_damage_kind() {
    use hurt::{Knock as K, Source as S, strike};
    let cases = [
        (
            S::Swing {
                finisher: false,
                air: false,
            },
            K::Small,
            0.8,
        ),
        (
            S::Swing {
                finisher: true,
                air: false,
            },
            K::Blow,
            1.2,
        ),
        (
            S::Swing {
                finisher: true,
                air: true,
            },
            K::Blow,
            0.8,
        ),
        (
            S::Flow {
                record: record::SONIC,
                hit: 0,
            },
            K::Small,
            1.0,
        ),
        (
            S::Flow {
                record: record::SONIC,
                hit: 1,
            },
            K::Toss,
            1.5,
        ),
        (
            S::Flow {
                record: record::WALL_ATTACK,
                hit: 0,
            },
            K::Toss,
            1.5,
        ),
        (
            S::Flow {
                record: record::SPIN,
                hit: 2,
            },
            K::VerticalRoll,
            1.0,
        ),
        (S::Launch, K::Toss, 0.6),
        (S::Counter, K::VerticalRoll, 1.6),
        (S::Chase { hit: 0 }, K::Small, 0.3),
        (S::Chase { hit: 1 }, K::SideSpin, 0.6),
        (S::Bolt { ragnarok: false }, K::SideSpin, 0.8),
        (S::Bolt { ragnarok: true }, K::Toss, 0.9),
        (
            S::Spell {
                heavy: false,
                grand: false,
            },
            K::Small,
            1.0,
        ),
        (
            S::Spell {
                heavy: true,
                grand: false,
            },
            K::Parabola,
            1.0,
        ),
        (
            S::Spell {
                heavy: false,
                grand: true,
            },
            K::Parabola,
            1.0,
        ),
    ];
    for (source, knock, scale) in cases {
        let s = strike(source);
        assert_eq!((s.knock, s.scale), (knock, scale), "{}", source.name());
    }
    assert!(!K::Small.heavy() && !K::Beat.heavy() && K::Toss.heavy() && K::Blow.heavy());
}

#[test]
fn a_flinch_slides_holds_him_until_its_change_point_then_lets_him_act() {
    let mut r = Rig::new(room(&[]));
    let strike = Strike {
        knock: Knock::Small,
        scale: 1.,
    };
    assert!(r.kh3.hurt(&mut r.player, strike, [0., 1.], true, &r.clips));
    assert!(matches!(r.kh3.mode, Mode::Hurt { air: false, .. }));
    let z0 = r.player.position[2];
    // Stunned: a held stick and every button do nothing until 0.4667 s.
    let mash = Input {
        attack: true,
        jump: true,
        direction: [1., 0.],
        ..Input::default()
    };
    for _ in 1..tuning::SMALL_ACT {
        r.step(&mash, &Pad::default(), None);
        assert_eq!(r.kh3.mode.name(), "flinch");
    }
    let slid = r.player.position[2] - z0;
    // 550 cm/s, x0.9 a tick: about a tenth of a metre-second's worth.
    let expect = tuning::SMALL_KNOCK * 0.9 / (1. - 0.9);
    assert!((slid - expect).abs() < expect * 0.1, "{slid} vs {expect}");
    assert!(r.player.position[0].abs() < 1e-3, "no steering while hurt");
    r.step(&idle_input(), &Pad::default(), None);
    r.step(
        &Input {
            direction: [1., 0.],
            jump_up: true,
            ..Input::default()
        },
        &Pad::default(),
        None,
    );
    assert_eq!(r.kh3.mode, Mode::Ground, "the stick ends it at the change");
}

#[test]
fn a_toss_peaks_where_the_reference_says_then_downs_him_until_he_gets_up() {
    let mut r = Rig::new(room(&[]));
    let strike = Strike {
        knock: Knock::Toss,
        scale: 1.,
    };
    r.kh3.hurt(&mut r.player, strike, [0., 1.], true, &r.clips);
    let mut peak: f32 = 0.;
    let mut downed = None;
    for tick in 1..200 {
        r.step(&idle_input(), &Pad::default(), None);
        peak = peak.max(r.player.position[1]);
        // No invulnerability in the air: a juggle goes on.
        if matches!(r.kh3.mode, Mode::Hurt { .. }) {
            assert!(!r.kh3.untouchable(&r.player, &r.clips));
        }
        if matches!(r.kh3.mode, Mode::Down { .. }) {
            downed = Some(tick);
            break;
        }
    }
    // 1500 cm/s at x3 gravity: 383 cm up.
    let want = 1500. * 1500. / (2. * 2940.) * tuning::SCALE;
    assert!((peak - want).abs() < want * 0.05, "{peak} vs {want}");
    // Up 0.51 s, a 0.1 s hang, down about 0.88 s.
    let downed = downed.expect("down");
    assert!((80..=95).contains(&downed), "{downed}");
    assert!(r.kh3.untouchable(&r.player, &r.clips), "down [duel choice]");
    for _ in 1..tuning::DOWN_END {
        r.step(&idle_input(), &Pad::default(), None);
        assert_eq!(r.kh3.mode.name(), "down");
    }
    r.step(&idle_input(), &Pad::default(), None);
    assert_eq!(r.kh3.mode, Mode::Ground);
    assert!(r.player.invulnerable > 0., "the wake guard");
}

#[test]
fn a_launch_goes_higher_and_ends_in_a_fall() {
    let mut r = Rig::new(room(&[]));
    r.kh3.hurt(
        &mut r.player,
        Strike {
            knock: Knock::Launch,
            scale: 1.,
        },
        [0., 1.],
        true,
        &r.clips,
    );
    let mut peak: f32 = 0.;
    let mut fell = false;
    for _ in 0..200 {
        r.step(&idle_input(), &Pad::default(), None);
        peak = peak.max(r.player.position[1]);
        if matches!(r.kh3.mode, Mode::Air { .. }) {
            fell = true;
            break;
        }
    }
    let want = 2500. * 2500. / (2. * 2940.) * tuning::SCALE;
    assert!((peak - want).abs() < want * 0.05, "{peak} vs {want}");
    assert!(fell, "the launch ends in the air: {:?}", r.kh3.mode);
}

#[test]
fn aerial_recovery_needs_the_ability_and_a_fifth_of_a_second() {
    for ability in [false, true] {
        let mut r = Rig::new(room(&[]));
        if ability {
            r.player
                .abilities
                .set(crate::abilities::bit::AERIAL_RECOVERY);
        }
        r.kh3.hurt(
            &mut r.player,
            Strike {
                knock: Knock::Toss,
                scale: 1.,
            },
            [0., 1.],
            true,
            &r.clips,
        );
        r.step(&press_jump(), &Pad::default(), None);
        assert!(matches!(r.kh3.mode, Mode::Hurt { .. }), "too early");
        for _ in 1..tuning::RECOVERY_FROM {
            r.step(&idle_input(), &Pad::default(), None);
        }
        r.step(&press_jump(), &Pad::default(), None);
        assert_eq!(
            matches!(r.kh3.mode, Mode::Recover { .. }),
            ability,
            "{:?}",
            r.kh3.mode
        );
        if ability {
            assert!(r.kh3.untouchable(&r.player, &r.clips));
        }
    }
}

#[test]
fn armour_and_a_command_s_first_tenth_shrug_off_light_hits_only() {
    let light = Strike {
        knock: Knock::Small,
        scale: 1.,
    };
    let heavy = Strike {
        knock: Knock::Blow,
        scale: 1.,
    };
    let player = Player::new([0.; 3], 0.);
    let k = Kh3 {
        mode: Mode::Superslide {
            tick: 3,
            pole: None,
        },
        ..Kh3::default()
    };
    assert!(k.shrugs_strike(&player, light) && !k.shrugs_strike(&player, heavy));
    let mut swinging = Player::new([0.; 3], 0.);
    swinging.action = Action::Attack(0);
    swinging.frame = tuning::COMMAND_ARMOUR - 1;
    assert!(Kh3::default().shrugs_strike(&swinging, light));
    swinging.frame = tuning::COMMAND_ARMOUR;
    assert!(!Kh3::default().shrugs_strike(&swinging, light));
}

#[test]
fn another_hit_on_a_body_in_the_air_keeps_its_motion() {
    let mut r = Rig::new(room(&[]));
    let toss = Strike {
        knock: Knock::Toss,
        scale: 1.,
    };
    assert!(r.kh3.hurt(&mut r.player, toss, [0., 1.], true, &r.clips));
    for _ in 0..5 {
        r.step(&idle_input(), &Pad::default(), None);
    }
    let frame = r.player.frame;
    // A bolt from behind: speed again, no new motion, voice or facing.
    let yaw = r.player.yaw;
    assert!(!r.kh3.hurt(&mut r.player, toss, [0., -1.], true, &r.clips));
    assert_eq!((r.player.frame, r.player.yaw), (frame, yaw));
    assert!(r.kh3.velocity[1] > 0.);
}

#[test]
fn a_counter_that_blows_him_away_offers_final_blow_which_warps_to_him() {
    let mut r = Rig::new(room(&[]));
    let foe = Foe {
        position: [0., 0., 1200.],
        centre: [0., 80., 1200.],
    };
    let counter = hurt::Source::Counter;
    r.kh3.struck(counter, hurt::strike(counter));
    assert!(r.kh3.chase_offer > 0);
    r.step(
        &Input {
            attack: true,
            ..idle_input()
        },
        &Pad::default(),
        Some(foe),
    );
    assert_eq!(r.kh3.mode.name(), "final_blow");
    assert!(r.player.invulnerable > 0.);
    let mut warped = false;
    for _ in 0..tuning::CHASE_WARP + 2 {
        let out = r.step(&idle_input(), &Pad::default(), Some(foe));
        warped |= out.began.contains(&"chase_warp");
    }
    assert!(warped);
    let d = (foe.position[2] - r.player.position[2]).abs();
    assert!((d - tuning::CHASE_SHORT).abs() < 5., "{d}");
}

#[test]
fn a_guarded_heavy_hit_offers_the_risk_dodge_to_behind_the_attacker() {
    let mut r = Rig::new(room(&[]));
    let attacker = [0., 0., 300.];
    r.kh3.guarded(
        &mut r.player,
        Strike {
            knock: Knock::Small,
            scale: 1.,
        },
        [0., -1.],
        attacker,
        false,
    );
    assert!(r.kh3.risk_offer.is_none(), "a light hit offers nothing");
    r.kh3.guarded(
        &mut r.player,
        Strike {
            knock: Knock::Blow,
            scale: 1.2,
        },
        [0., -1.],
        attacker,
        false,
    );
    assert!(r.kh3.risk_offer.is_some());
    r.player.action = Action::GuardRecoil;
    r.player.frame = 5;
    r.kh3.mode = Mode::Kh2;
    r.step(&press_jump(), &Pad::default(), None);
    assert_eq!(r.kh3.mode.name(), "risk_dodge");
    for _ in 0..tuning::RISK_SLIDE_TICKS + 1 {
        assert!(r.kh3.untouchable(&r.player, &r.clips));
        r.step(&idle_input(), &Pad::default(), None);
    }
    assert_eq!(r.kh3.mode, Mode::Ground);
    assert!(
        r.player.position[2] > attacker[2],
        "{:?}",
        r.player.position
    );
    // Turned round to face him, with the counter offered.
    assert!(r.player.yaw.cos() < -0.99);
    assert!(r.kh3.counter > 0);
}

#[test]
fn an_emptied_gauge_makes_the_shotlock_focus_syphon() {
    let mut r = Rig::new(room(&[]));
    let foe = Foe {
        position: [0., 0., 900.],
        centre: [0., 80., 900.],
    };
    r.kh3.focus = 0.;
    r.step(&idle_input(), &Pad::default(), Some(foe));
    assert!(r.kh3.focus_empty);
    let hold = Pad {
        shotlock: true,
        ..Pad::default()
    };
    let mut locks = 0;
    for _ in 0..400 {
        r.step(&idle_input(), &hold, Some(foe));
        if let Some(a) = r.kh3.aim {
            assert!(a.syphon);
            locks = a.locks;
        }
        if locks >= tuning::SYPHON_LOCKS {
            break;
        }
    }
    assert_eq!(locks, tuning::SYPHON_LOCKS);
    let before = r.kh3.focus;
    r.step(
        &Input {
            attack: true,
            ..idle_input()
        },
        &hold,
        Some(foe),
    );
    assert_eq!(r.kh3.mode.name(), "focus_syphon");
    let mut gained = false;
    for _ in 0..tuning::SYPHON_TICKS + 1 {
        let out = r.step(&idle_input(), &hold, Some(foe));
        gained |= out.cues.contains(&"syphon_gain");
    }
    assert!(gained);
    let gain = r.kh3.focus - before;
    let want = (tuning::SYPHON_RATE * f32::from(tuning::SYPHON_LOCKS)).trunc();
    assert!((gain - want).abs() < 1.5, "{gain} vs {want}");
    assert!(r.kh3.focus_empty, "until the gauge is full again");
}

#[test]
fn every_move_and_moment_has_its_kh2_stand_in_and_nothing_stacks() {
    use cue::{Cue, EVENTS, of, periodic};
    // Events that a KH2 motion's own triggers voice and sound alone.
    let motion_only = [
        "dodge_roll",
        "airstep_hop",
        "launch",
        "flinch",
        "air_flinch",
        "blown",
        "beaten",
        "tossed",
    ];
    for e in EVENTS {
        assert_eq!(of(e).is_empty(), motion_only.contains(&e), "{e}");
    }
    // Every move the layer names (bar locomotion and the KH2 side) is an
    // event.
    for m in MODE_NAMES {
        if !matches!(m, "kh2" | "ground" | "air" | "land" | "takeoff") {
            assert!(EVENTS.contains(&m), "{m}");
        }
    }
    // Sounds sit in the banks the duel loads.
    for e in EVENTS {
        for c in of(e) {
            if let Cue::Sound(id) = c {
                assert!(matches!(id / 1000, 0 | 500 | 999), "{e}: {id}");
            }
        }
    }
    // A wall run's step once per period, a dash's afterimage likewise.
    let steps = (0..120)
        .filter(|&tick| {
            periodic(&Mode::WallRun {
                normal: [1., 0.],
                up: true,
                side: 0.,
                tick,
                idle: 0,
            })
            .is_some()
        })
        .count();
    assert_eq!(steps, (119 / cue::STEP_EVERY) as usize);
    assert!(periodic(&Mode::Ground).is_none());
}

#[test]
fn the_layer_s_cues_name_known_events() {
    // A wall run, a lock: what the layer emits has an entry.
    let mut r = Rig::new(room(&[([-1000., 300.], [1000., 300.], 2000.)]));
    let foe = Foe {
        position: [0., 0., -900.],
        centre: [0., 80., -900.],
    };
    let hold = Pad {
        shotlock: true,
        ..Pad::default()
    };
    let mut seen = Vec::new();
    for _ in 0..60 {
        let out = r.step(&idle_input(), &hold, Some(foe));
        seen.extend(out.began);
        seen.extend(out.cues);
    }
    assert!(seen.contains(&"aim") && seen.contains(&"lock"), "{seen:?}");
    for e in seen {
        assert!(cue::EVENTS.contains(&e), "{e}");
    }
}
