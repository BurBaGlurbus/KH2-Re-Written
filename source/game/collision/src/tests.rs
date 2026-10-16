use super::*;
use data::room_collision::{CollisionMesh, Node as CNode, Polygon, RoomCollision};

/// Sora's body (P_EX100 BAR type 23 entry 0: kind 4, r 90, h 90, ph 115).
const SORA: Body = Body {
    radius: 90.,
    height: 90.,
    offset: 115.,
    mode: 1,
    class: 0,
};

const FLOOR: u32 = 0x11;
const WALL: u32 = 0x12;

/// A 2000-square floor at y = 0 and a wall plane x = 300 facing −x.
fn room(wall_attr: u32) -> RoomCollision {
    let v = |x: f32, y: f32, z: f32| [x, y, z, 1.].map(f32::to_bits);
    let vertex_bits = vec![
        v(-1000., 0., -1000.),
        v(1000., 0., -1000.),
        v(1000., 0., 1000.),
        v(-1000., 0., 1000.),
        // Wall at x = 300 from y = 0 up to y = −400 (Y down).
        v(300., 0., -1000.),
        v(300., 0., 1000.),
        v(300., -400., 1000.),
        v(300., -400., -1000.),
    ];
    let plane_bits = vec![
        [0., -1., 0., 0.].map(f32::to_bits),
        // Normal −x: signed distance = −x + 300.
        [-1., 0., 0., 300.].map(f32::to_bits),
    ];
    let polygons = vec![
        Polygon {
            ground: 3,
            floor_level: 0,
            vertices: [0, 1, 2, 3],
            plane: 0,
            bounding_box: -1,
            attribute: 0,
        },
        Polygon {
            ground: 0,
            floor_level: 0,
            vertices: [4, 7, 6, 5],
            plane: 1,
            bounding_box: -1,
            attribute: 1,
        },
    ];
    RoomCollision {
        version: 1,
        depth: 0,
        kind: 0,
        table_spans: [[0; 2]; 8],
        nodes: vec![CNode {
            children: [-1; 8],
            bounds: [-2000, -2000, -2000, 2000, 2000, 2000],
            mesh_range: [0, 2],
        }],
        meshes: vec![
            CollisionMesh {
                bounds: [-1000, -10, -1000, 1000, 10, 1000],
                polygon_range: [0, 1],
                visibility: 0,
                group: 0,
                padding: 0,
            },
            CollisionMesh {
                bounds: [299, -400, -1000, 301, 0, 1000],
                polygon_range: [1, 2],
                visibility: 0,
                group: 1,
                padding: 0,
            },
        ],
        polygons,
        vertex_bits,
        plane_bits,
        bounding_boxes: vec![],
        attributes: vec![FLOOR, wall_attr],
    }
}

#[test]
fn walking_stays_on_the_floor() {
    let w = World::new(&room(WALL), !0).unwrap();
    let s = solve(
        &w,
        &SORA,
        &Query {
            start: [0., 0., 0.],
            displacement: [10., 0., 5.],
            airborne: false,
            extra_snap: 0.,
        },
    );
    assert!(!s.no_floor);
    assert_eq!(s.floor_polygon, Some(0));
    assert_eq!(s.position, [10., 0., 5.]);
    assert_eq!(floor_info(&w, &s).normal, [0., -1., 0.]);
    assert_eq!(floor_info(&w, &s).ground, 3);
}

#[test]
fn wall_pushes_the_sphere_back_with_clearance() {
    let w = World::new(&room(WALL), !0).unwrap();
    let s = solve(
        &w,
        &SORA,
        &Query {
            start: [180., 0., 0.],
            displacement: [60., 0., 0.],
            airborne: false,
            extra_snap: 0.,
        },
    );
    // The sphere stops r + 0.008 off the wall plane.
    assert!(
        (s.position[0] - (300. - 90. - CLEARANCE)).abs() < 1e-3,
        "{s:?}"
    );
    // The knee sphere did the push; knee pushes are not counted, so the
    // sweep reports no touch (FUN_140174640).
    assert!(!s.touched);
}

#[test]
fn hidden_group_and_non_solid_walls_do_not_block() {
    for (mask, attr) in [(1u64, WALL), (!0, 0x02)] {
        let w = World::new(&room(attr), mask).unwrap();
        let s = solve(
            &w,
            &SORA,
            &Query {
                start: [180., 0., 0.],
                displacement: [60., 0., 0.],
                airborne: false,
                extra_snap: 0.,
            },
        );
        assert_eq!(s.position[0], 240.);
    }
}

#[test]
fn snap_down_limit_is_offset_minus_height() {
    // A floor 24 below snaps; 26 below is a walk-off.
    for (drop, falls) in [(24., false), (26., true)] {
        let w = World::new(&room(WALL), !0).unwrap();
        let s = solve(
            &w,
            &SORA,
            &Query {
                start: [0., -drop, 0.],
                displacement: [5., 0., 0.],
                airborne: false,
                extra_snap: 0.,
            },
        );
        assert_eq!(s.no_floor, falls, "drop {drop}");
        if !falls {
            assert_eq!(s.position[1], 0.);
        }
    }
}

#[test]
fn air_landing_needs_the_feet_behind_the_plane() {
    let w = World::new(&room(WALL), !0).unwrap();
    let above = solve(
        &w,
        &SORA,
        &Query {
            start: [0., -50., 0.],
            displacement: [0., 10., 0.],
            airborne: true,
            extra_snap: 0.,
        },
    );
    assert!(above.no_floor);
    assert_eq!(above.position[1], -40.);
    let through = solve(
        &w,
        &SORA,
        &Query {
            start: [0., -5., 0.],
            displacement: [0., 10., 0.],
            airborne: true,
            extra_snap: 0.,
        },
    );
    assert!(!through.no_floor);
    assert_eq!(through.position[1], 0.);
}

#[test]
fn ray_misses_without_floor() {
    let w = World::new(&room(WALL), 2).unwrap();
    let s = solve(
        &w,
        &SORA,
        &Query {
            start: [0., 0., 0.],
            displacement: [5., 0., 0.],
            airborne: false,
            extra_snap: 0.,
        },
    );
    assert!(s.ray_missed && s.no_floor);
    assert_eq!(s.floor_y, NO_FLOOR);
}

#[test]
fn projection_keeps_length_along_a_slope() {
    // A 45° floor: −normal (down) = (0.7071, 0.7071, 0).
    let h = std::f32::consts::FRAC_1_SQRT_2;
    let p = project_on_floor([10., 0., 0.], [h, h, 0.]);
    assert!((dot(p, p).sqrt() - 10.).abs() < 1e-4);
    assert!(dot(p, [h, h, 0.]).abs() < 1e-4);
}

#[test]
fn the_camera_ray_keeps_nibble_four_polygons_and_reports_normal_and_surface() {
    let (from, to) = ([0., -100., 0.], [600., -100., 0.]);
    // The body query skips nibble-4 polygons; the camera query clears the filter.
    let w = World::new(&room(0x14), !0).unwrap();
    assert!(w.ray(from, to, 0xf).is_none());
    let hit = w.camera_ray(from, to).unwrap();
    assert_eq!(hit.point, [300., -100., 0.]);
    assert_eq!(hit.normal, [-1., 0., 0.]);
    assert_eq!(hit.surface, 4);
    let w = World::new(&room(0x15), !0).unwrap();
    assert_eq!(w.camera_ray(from, to).unwrap().surface, 5);
    // Back faces are not hit.
    assert!(w.camera_ray(to, from).is_none());
}
