//! What flowmotion holds on to: walls, found through the world's own wall
//! solver, and poles, the narrow pillars among a room's wall triangles.
//! The reference marks only some walls as wall-runnable; here every wall
//! steeper than the floor limit is [duel choice: the arenas carry no such
//! marks].

use crate::{Vec3, World};

/// A wall met by a probe: its outward normal (XZ, unit) and the point of
/// its surface nearest the probe.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Wall {
    pub normal: [f32; 2],
    pub surface: [f32; 2],
}

/// The wall a body circle of `radius` would meet `reach` along `dir` (XZ,
/// unit) from `at`, over the vertical span `low..high`: the solver pushes
/// the probe circle out, and the push is the wall's normal.
pub fn wall_near(
    world: &impl World,
    at: Vec3,
    dir: [f32; 2],
    reach: f32,
    radius: f32,
    span: (f32, f32),
) -> Option<Wall> {
    let probe = [at[0] + dir[0] * reach, at[2] + dir[1] * reach];
    let out = world.resolve_walls(probe, radius, span.0, span.1);
    let d = [out[0] - probe[0], out[1] - probe[1]];
    let len = d[0].hypot(d[1]);
    if len < 1e-3 {
        return None;
    }
    let normal = [d[0] / len, d[1] / len];
    Some(Wall {
        normal,
        surface: [out[0] - normal[0] * radius, out[1] - normal[1] * radius],
    })
}

/// A pole: an upright pillar Sora can spin around.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pole {
    pub centre: [f32; 2],
    pub radius: f32,
    pub bottom: f32,
    pub top: f32,
}

/// [duel choice] What counts as a pole: a group of touching wall triangles
/// no wider than this across, at least this narrow, this tall, and not
/// much longer one way than the other.
pub const POLE_WIDEST: f32 = 250.;
pub const POLE_NARROWEST: f32 = 10.;
pub const POLE_SHORTEST: f32 = 250.;
const POLE_ROUNDNESS: f32 = 2.5;
/// Triangles closer than this (XZ boxes) belong to one group.
const TOUCH: f32 = 4.;
/// Rooms with more wall triangles than this are not searched.
const MOST_WALLS: usize = 20_000;

/// The poles among a room's wall triangles.
pub fn find_poles(walls: &[[Vec3; 3]]) -> Vec<Pole> {
    if walls.len() > MOST_WALLS {
        return Vec::new();
    }
    let boxes: Vec<[f32; 6]> = walls
        .iter()
        .map(|t| {
            let mut b = [f32::MAX, f32::MAX, f32::MAX, f32::MIN, f32::MIN, f32::MIN];
            for v in t {
                for k in 0..3 {
                    b[k] = b[k].min(v[k]);
                    b[k + 3] = b[k + 3].max(v[k]);
                }
            }
            b
        })
        .collect();
    // Union-find over triangles whose XZ boxes touch.
    let mut parent: Vec<usize> = (0..walls.len()).collect();
    fn root(parent: &mut [usize], mut i: usize) -> usize {
        while parent[i] != i {
            parent[i] = parent[parent[i]];
            i = parent[i];
        }
        i
    }
    for i in 0..boxes.len() {
        for j in i + 1..boxes.len() {
            let (a, b) = (&boxes[i], &boxes[j]);
            let touch = a[0] <= b[3] + TOUCH
                && b[0] <= a[3] + TOUCH
                && a[2] <= b[5] + TOUCH
                && b[2] <= a[5] + TOUCH;
            if touch {
                let (ri, rj) = (root(&mut parent, i), root(&mut parent, j));
                if ri != rj {
                    parent[ri] = rj;
                }
            }
        }
    }
    let mut groups: std::collections::BTreeMap<usize, [f32; 6]> = Default::default();
    for (i, b) in boxes.iter().enumerate() {
        let r = root(&mut parent, i);
        let g =
            groups
                .entry(r)
                .or_insert([f32::MAX, f32::MAX, f32::MAX, f32::MIN, f32::MIN, f32::MIN]);
        for k in 0..3 {
            g[k] = g[k].min(b[k]);
            g[k + 3] = g[k + 3].max(b[k + 3]);
        }
    }
    groups
        .values()
        .filter_map(|g| {
            let (w, d, h) = (g[3] - g[0], g[5] - g[2], g[4] - g[1]);
            let (wide, narrow) = (w.max(d), w.min(d));
            (wide <= POLE_WIDEST
                && narrow >= POLE_NARROWEST
                && h >= POLE_SHORTEST
                && wide <= narrow * POLE_ROUNDNESS)
                .then(|| Pole {
                    centre: [(g[0] + g[3]) * 0.5, (g[2] + g[5]) * 0.5],
                    radius: wide * 0.5,
                    bottom: g[1],
                    top: g[4],
                })
        })
        .collect()
}

/// The pole within `reach` of the point `at` (its height inside the
/// pole's), nearest first.
pub fn pole_near(poles: &[Pole], at: Vec3, reach: f32) -> Option<Pole> {
    poles
        .iter()
        .filter(|p| (p.bottom..=p.top).contains(&at[1]))
        .map(|p| {
            let d = (at[0] - p.centre[0]).hypot(at[2] - p.centre[1]) - p.radius;
            (d, *p)
        })
        .filter(|(d, _)| *d <= reach)
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, p)| p)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TriangleWorld;

    /// A square box of walls `half` either side of (cx, cz), `h` tall.
    fn column(cx: f32, cz: f32, half: f32, h: f32) -> Vec<[Vec3; 3]> {
        let c = [
            [cx - half, cz - half],
            [cx + half, cz - half],
            [cx + half, cz + half],
            [cx - half, cz + half],
        ];
        let mut out = Vec::new();
        for k in 0..4 {
            let (a, b) = (c[k], c[(k + 1) % 4]);
            out.push([[a[0], 0., a[1]], [b[0], 0., b[1]], [b[0], h, b[1]]]);
            out.push([[a[0], 0., a[1]], [b[0], h, b[1]], [a[0], h, a[1]]]);
        }
        out
    }

    #[test]
    fn narrow_tall_groups_are_poles_and_long_walls_are_not() {
        let mut walls = column(500., 0., 40., 600.);
        // A long wall: 2000 along x.
        walls.push([
            [-1000., 0., -800.],
            [1000., 0., -800.],
            [1000., 500., -800.],
        ]);
        walls.push([
            [-1000., 0., -800.],
            [1000., 500., -800.],
            [-1000., 500., -800.],
        ]);
        // A short block (a crate) is not a pole.
        walls.extend(column(-500., 0., 40., 100.));
        let poles = find_poles(&walls);
        assert_eq!(poles.len(), 1);
        let p = poles[0];
        assert!((p.centre[0] - 500.).abs() < 1e-3 && p.centre[1].abs() < 1e-3);
        assert!((p.radius - 40.).abs() < 1e-3 && p.top == 600.);
        let near = pole_near(&poles, [500., 100., 130.], 100.);
        assert_eq!(near, Some(p));
        assert_eq!(pole_near(&poles, [500., 100., 300.], 100.), None);
        assert_eq!(
            pole_near(&poles, [500., 700., 100.], 100.),
            None,
            "above it"
        );
    }

    #[test]
    fn a_probe_finds_the_wall_and_its_normal() {
        // A wall in the plane z = 100 from x -500..500, 400 tall.
        let world = TriangleWorld::new(
            [
                (
                    [[-500., 0., 100.], [500., 0., 100.], [500., 400., 100.]],
                    false,
                    true,
                ),
                (
                    [[-500., 0., 100.], [500., 400., 100.], [-500., 400., 100.]],
                    false,
                    true,
                ),
            ],
            0.7,
        );
        let wall = wall_near(&world, [0., 0., 70.], [0., 1.], 10., 30., (10., 150.)).unwrap();
        assert!((wall.normal[1] + 1.).abs() < 1e-4, "{wall:?}");
        assert!((wall.surface[1] - 100.).abs() < 1e-3);
        assert_eq!(
            wall_near(&world, [0., 0., -200.], [0., 1.], 10., 30., (10., 150.)),
            None
        );
        // Above the wall's top nothing is met.
        assert_eq!(
            wall_near(&world, [0., 450., 70.], [0., 1.], 10., 30., (460., 600.)),
            None
        );
    }
}
