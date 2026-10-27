//! Golden values follow the executable's f32 operation order by hand.
use super::*;
use crate::script::{Instance, Memory, Thread};
use crate::world::{Object, World};

/// Run one trap against a work area that holds `floats` from word 0.
fn call(f: TrapFn, world: &mut World, floats: &[f32], args: &[Slot]) -> (Slot, Vec<f32>) {
    let mut obj = Object::bare(0);
    let mut header = vec![0u8; 44];
    header[..4].copy_from_slice(b"test");
    header[16..20].copy_from_slice(&64u32.to_le_bytes());
    header[20..24].copy_from_slice(&64u32.to_le_bytes());
    header[24..28].copy_from_slice(&64u32.to_le_bytes());
    let mut inst = Instance::new(0, &header).unwrap();
    let mut th = Thread::new(&inst, 0, 0, &[]);
    let base = inst.work_base();
    let mut memory = Memory::new(&mut inst, &mut th);
    for (k, v) in floats.iter().enumerate() {
        memory.write_f32(base + 4 * k as u32, *v);
    }
    let mut a = [Slot::default(); 16];
    a[..args.len()].copy_from_slice(args);
    let mut ctx = Ctx {
        world,
        me: &mut obj,
        env: &crate::env::NullEnv,
    };
    f(&mut ctx, &mut memory, &mut a).unwrap();
    let out: Vec<f32> = (0..floats.len())
        .map(|k| memory.read_f32(base + 4 * k as u32).unwrap())
        .collect();
    (a[0], out)
}

fn read_static(world: &World, handle: u32) -> [f32; 4] {
    [0, 1, 2, 3].map(|k| f32::from_bits(world.static_read(handle + 4 * k).unwrap()))
}

#[test]
fn add_and_sub_zero_nans_and_return_a_static_vector() {
    let mut world = World::default();
    let inst_base = crate::script::WORK_SEGMENT;
    let (r, mem) = call(
        vector_add,
        &mut world,
        &[1.0, 2.0, f32::NAN, 1.0, 0.5, 0.25, 4.0, 0.0],
        &[Slot::address(inst_base), Slot::address(inst_base + 16)],
    );
    assert_eq!(r.tag, crate::script::TAG_ADR);
    assert_eq!(read_static(&world, r.value), [1.5, 2.25, 4.0, 1.0]);
    assert_eq!(mem[2], 0.0, "the NaN lane was cleared in the operand");
    let (r, _) = call(
        vector_sub,
        &mut world,
        &[1.0, 2.0, 3.0, 1.0, 0.5, 0.25, 4.0, 0.0],
        &[Slot::address(inst_base), Slot::address(inst_base + 16)],
    );
    assert_eq!(read_static(&world, r.value), [0.5, 1.75, -1.0, 1.0]);
}

#[test]
fn normalize_writes_the_length_to_w_and_skips_zero_vectors() {
    let mut world = World::default();
    let b = crate::script::WORK_SEGMENT;
    let (r, mem) = call(
        vector_normalize,
        &mut world,
        &[3.0, 0.0, 4.0, 9.0],
        &[Slot::address(b)],
    );
    assert_eq!(r, Slot::float(5.0));
    let inv = 1.0f32 / 5.0;
    assert_eq!(mem, [3.0 * inv, 0.0, 4.0 * inv, 5.0]);
    let (r, mem) = call(
        vector_normalize,
        &mut world,
        &[0.0, 0.0, 0.0, 7.0],
        &[Slot::address(b)],
    );
    assert_eq!(r, Slot::float(0.0));
    assert_eq!(mem, [0.0, 0.0, 0.0, 0.0]);
}

#[test]
fn rotation_about_y_follows_the_matrix_layout() {
    // A quarter turn takes +z to +x (x' = sin*z) and +x to -z.
    let q = std::f32::consts::FRAC_PI_2;
    let v = rotate_y(&[0.0, 5.0, 2.0, 1.0], q);
    assert!((v[0] - 2.0).abs() < 1e-5 && v[1] == 5.0 && v[2].abs() < 1e-5 && v[3] == 1.0);
    let v = rotate_y(&[2.0, 0.0, 0.0, 1.0], q);
    assert!(v[0].abs() < 1e-5 && (v[2] + 2.0).abs() < 1e-5);
    // A zero angle is an exact copy.
    assert_eq!(rotate_y(&[1.0, 2.0, 3.0, 4.0], 0.0), [1.0, 2.0, 3.0, 4.0]);
}

#[test]
fn angle_wrap_uses_two_branches() {
    use std::f32::consts::PI;
    assert!((wrap_angle(3.0 * PI) - PI).abs() < 1e-5 || (wrap_angle(3.0 * PI) + PI).abs() < 1e-5);
    assert_eq!(wrap_angle(0.0), 0.0);
    assert!((wrap_angle(-0.5) + 0.5).abs() < 1e-6);
}

#[test]
fn random_follows_the_lcg_and_the_documented_expressions() {
    let mut world = World::default();
    // state 1 -> 69069 = 0x10dcd.
    let (r, _) = call(random_getf, &mut world, &[], &[Slot::float(360.0)]);
    assert_eq!(r, Slot::float(69069.0f32 * RANDOM_SCALE * 360.0));
    let (r, _) = call(
        random_range,
        &mut world,
        &[],
        &[Slot::float(60.0), Slot::float(90.0)],
    );
    let state = 69069u32.wrapping_mul(0x10dcd);
    assert_eq!(
        r,
        Slot::float((90.0 - 60.0) * state as f32 * RANDOM_SCALE + 60.0)
    );
    let before = world.rng.0;
    let (r, _) = call(random_get, &mut world, &[], &[Slot::int(2)]);
    let next = before.wrapping_mul(0x10dcd);
    assert_eq!(
        r.as_int(),
        (u64::from(next) / (0x1_0000_0000u64.div_ceil(2))) as i32
    );
}

#[test]
fn fix_rad_zeroes_non_finite_angles() {
    let mut world = World::default();
    let (r, _) = call(fix_rad, &mut world, &[], &[Slot::float(f32::INFINITY)]);
    assert_eq!(r, Slot::float(0.0));
}

#[test]
fn euler_angles_pick_the_solution_nearest_the_reference() {
    use std::f32::consts::{FRAC_PI_2, PI};
    let identity = [[1., 0., 0.], [0., 1., 0.], [0., 0., 1.]];
    let e = euler_xyz(&identity, [0.; 3]);
    assert!(e.iter().all(|a| a.abs() < 1.0e-6));
    // A quarter turn about z: the heading comes from row 0.
    let yaw = [[0., 1., 0.], [-1., 0., 0.], [0., 0., 1.]];
    let e = euler_xyz(&yaw, [0.; 3]);
    assert!((e[2] - FRAC_PI_2).abs() < 1.0e-6 && e[0].abs() < 1.0e-6 && e[1].abs() < 1.0e-6);
    // The same rotation seen from a reference near the other solution.
    let e = euler_xyz(&identity, [PI, PI, PI]);
    assert!(e.iter().all(|a| (a - PI).abs() < 1.0e-5));
}

#[test]
fn strike_raid_rotation_for_forward_z_and_up_y() {
    let mut world = World::default();
    // forward (0,0,1) at word 0, up hint (0,1,0) at word 4.
    let floats = [0., 0., 1., 0., 0., 1., 0., 0.];
    let (r, out) = {
        let mut obj = Object::bare(0);
        let mut header = vec![0u8; 44];
        header[..4].copy_from_slice(b"test");
        for at in [16, 20, 24] {
            header[at..at + 4].copy_from_slice(&64u32.to_le_bytes());
        }
        let mut inst = Instance::new(0, &header).unwrap();
        let mut th = Thread::new(&inst, 0, 0, &[]);
        let base = inst.work_base();
        let mut memory = Memory::new(&mut inst, &mut th);
        for (k, v) in floats.iter().enumerate() {
            memory.write_f32(base + 4 * k as u32, *v);
        }
        let mut a = [Slot::default(); 16];
        a[0] = Slot::address(base);
        a[1] = Slot::address(base + 16);
        let mut ctx = Ctx {
            world: &mut world,
            me: &mut obj,
            env: &crate::env::NullEnv,
        };
        strike_raid_calc_xyzrot(&mut ctx, &mut memory, &mut a).unwrap();
        let out: Vec<f32> = (0..8)
            .map(|k| memory.read_f32(base + 4 * k as u32).unwrap())
            .collect();
        (a[0], out)
    };
    let angles = read_static(&world, r.value);
    assert!((angles[0] - std::f32::consts::PI).abs() < 1.0e-5);
    assert!(angles[1].abs() < 1.0e-5 && angles[2].abs() < 1.0e-5 && angles[3] == 0.0);
    // The up hint was replaced by cross(forward, side) = (0, 1, 0).
    assert_eq!(&out[4..7], &[0., 1., 0.]);
}

#[test]
fn div_multiplies_every_lane_by_the_reciprocal() {
    let mut world = World::default();
    let base = crate::script::WORK_SEGMENT;
    let (r, _) = call(
        vector_div,
        &mut world,
        &[2.0, -4.0, 6.0, 1.0],
        &[Slot::address(base), Slot::float(4.0)],
    );
    assert_eq!(read_static(&world, r.value), [0.5, -1.0, 1.5, 0.25]);
}

#[test]
fn an_engine_constant_lands_where_the_engine_puts_it() {
    // Sephiroth's Meteor spot, engine (-750, -600, 2250): 600 up, and on the
    // scene's -Z side, as the map is turned.
    assert_eq!(native([-750., -600., 2250., 1.]), [-750., 600., -2250., 1.]);
    // The turn is its own inverse.
    let v = [1., 2., 3., 4.];
    assert_eq!(native(native(v)), v);
    assert_eq!(native_local(native_local(v)), v);
}

#[test]
fn an_engine_offset_under_an_engine_heading_lands_as_the_scene_places_it() {
    // Engine: p + R(psi) l, with R(psi) taking +Z to (sin psi, 0, cos psi).
    // Scene: native(p) + RotY(native_yaw(psi)) native_local(l).
    let rot = |a: f32, v: [f32; 3]| {
        let (s, c) = a.sin_cos();
        [v[0] * c + v[2] * s, v[1], -v[0] * s + v[2] * c]
    };
    let close = |a: [f32; 3], b: [f32; 3]| (0..3).all(|k| (a[k] - b[k]).abs() < 1e-3);
    let p = [10., -20., 30.];
    for &psi in &[0.0f32, 0.7, -2.1, 3.0] {
        for l in [[0., 0., 100.], [50., -10., 0.], [3., 4., -5.]] {
            let r = rot(psi, l);
            let engine = native([p[0] + r[0], p[1] + r[1], p[2] + r[2], 1.]);
            let pb = native([p[0], p[1], p[2], 1.]);
            let lb = native_local([l[0], l[1], l[2], 0.]);
            let rb = rot(native_yaw(psi), [lb[0], lb[1], lb[2]]);
            let scene = [pb[0] + rb[0], pb[1] + rb[1], pb[2] + rb[2]];
            assert!(
                close([engine[0], engine[1], engine[2]], scene),
                "psi {psi} l {l:?}"
            );
        }
        assert!((native_yaw(native_yaw(psi)) - psi).abs() < 1e-5);
    }
}

#[test]
fn vector_to_angle_gives_pitch_and_yaw_and_zeroes_nan_lanes() {
    // FUN_1403b7710: pitch atan2(-y, |xz|), yaw atan2(x, z); FUN_14041d980
    // zeroes the argument's NaN lanes in place first.
    let mut world = World::default();
    let base = crate::script::WORK_SEGMENT;
    let (r, work) = call(
        vector_to_angle,
        &mut world,
        &[0., -1., 1., f32::NAN],
        &[Slot::address(base)],
    );
    let a = read_static(&world, r.value);
    assert!((a[0] - std::f32::consts::FRAC_PI_4).abs() < 1e-6, "{a:?}");
    assert_eq!([a[1], a[2], a[3]], [0., 0., 1.]);
    assert_eq!(work[3], 0.);
}
