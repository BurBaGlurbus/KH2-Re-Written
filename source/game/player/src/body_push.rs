//! Battle objects pushing each other apart, translated from the PC
//! executable: each object's movement (FUN_1403b89a0 → FUN_1403b9090)
//! runs FUN_14040ad70 against every other active object's push shapes
//! (obj+0xa00, the model's kind-1 collision entries, FUN_1403ba970), with
//! the shape test FUN_1403c94c0, the separation FUN_1403c90b0, the weight
//! FUN_1403b5e00 and the immovable test FUN_1403ba7d0. The push is a
//! one-tick displacement on top of the object's own (Y-up here).

use crate::Vec3;

/// FUN_1403c90b0: the separation reaches this far past the touching
/// distance.
pub const MARGIN: f32 = 5.;
/// `DAT_1405b22e4`: the weight under motion range 44 (obj+0x190 bit 1).
pub const HEAVY: f32 = 10000.;

/// Collision shape kinds (FUN_1403c9b60): an upright cylinder, a sphere.
pub const CYLINDER: u8 = 0;
pub const SPHERE: u8 = 3;

/// One push shape placed in the scene.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shape {
    pub kind: u8,
    pub centre: Vec3,
    pub radius: f32,
    pub height: f32,
}

/// An object's side of the push.
#[derive(Clone, Debug, PartialEq)]
pub struct Body {
    pub shapes: Vec<Shape>,
    /// FUN_1403b5e00: objentry +0x50, or [`HEAVY`] under range 44.
    pub weight: f32,
    /// FUN_1403ba7d0 besides a zero weight: obj+0x120 bit 2, range 31
    /// (obj+0x18c bit 25), attached, or a mode with flag bit 2.
    pub fixed: bool,
}

impl Body {
    fn immovable(&self) -> bool {
        self.weight == 0. || self.fixed
    }

    fn shift(&mut self, d: Vec3) {
        for s in &mut self.shapes {
            for (c, d) in s.centre.iter_mut().zip(d) {
                *c += d;
            }
        }
    }
}

/// Kind 1: an upright cylinder of half height `height` (FUN_1403c94c0's
/// own branch and FUN_1403c9780). [`CYLINDER`] (kind 0) is tested as an
/// ellipsoid of the two half axes.
pub const UPRIGHT: u8 = 1;

/// FUN_1403c94c0 by the two kinds: two spheres compare the distance with
/// the summed radii; kinds 0 and 3 otherwise test the centres' offset over
/// the summed radii and heights against the unit sphere (FUN_1403c9890);
/// two upright cylinders their vertical gap and horizontal distance; an
/// upright cylinder against kind 0 or 3 is FUN_1403c9780.
pub fn overlap(a: &Shape, b: &Shape) -> bool {
    let d = [
        b.centre[0] - a.centre[0],
        b.centre[1] - a.centre[1],
        b.centre[2] - a.centre[2],
    ];
    let flat = d[0] * d[0] + d[2] * d[2];
    match (a.kind, b.kind) {
        (SPHERE, SPHERE) => {
            let r = a.radius + b.radius;
            d[0] * d[0] + d[1] * d[1] + d[2] * d[2] < r * r
        }
        (CYLINDER | SPHERE, CYLINDER | SPHERE) => {
            let (r, h) = (a.radius + b.radius, a.height + b.height);
            let n = [d[0] / r, d[1] / h, d[2] / r];
            n[0] * n[0] + n[1] * n[1] + n[2] * n[2] < 1.
        }
        (UPRIGHT, UPRIGHT) => {
            let r = a.radius + b.radius;
            d[1].abs() < a.height + b.height && flat < r * r
        }
        (CYLINDER | SPHERE, UPRIGHT) => cylinder_overlap(a, b, d[1].abs(), flat),
        (UPRIGHT, CYLINDER | SPHERE) => cylinder_overlap(b, a, d[1].abs(), flat),
        _ => false,
    }
}

/// FUN_1403c9780: the round shape `e` against the upright cylinder `c`,
/// `dy` and `flat` the centres' vertical gap and squared horizontal
/// distance. Above the cylinder's half height the round shape's horizontal
/// reach narrows as an ellipse's.
fn cylinder_overlap(e: &Shape, c: &Shape, dy: f32, flat: f32) -> bool {
    if e.height + c.height <= dy {
        return false;
    }
    let reach = if dy <= c.height {
        e.radius
    } else {
        let t = (dy - c.height) / e.height;
        (1. - t * t).abs().sqrt() * e.radius
    } + c.radius;
    flat < reach * reach
}

/// FUN_1403c90b0: the offset from `a` toward `b` that takes them apart
/// along the ground; coincident centres push along +X.
pub fn separation(a: &Shape, b: &Shape) -> Vec3 {
    let d = [
        b.centre[0] - a.centre[0],
        b.centre[1] - a.centre[1],
        b.centre[2] - a.centre[2],
    ];
    let (r, h) = (a.radius + b.radius, a.height + b.height);
    let n = [d[0] / r, d[1] / h, d[2] / r];
    let reach = r + MARGIN;
    let ground = n[0] * n[0] + n[2] * n[2];
    if ground <= 0. {
        return [reach, 0., 0.];
    }
    let s = ((1. - n[1] * n[1]) / ground).abs().sqrt() - 1.;
    [n[0] * s * reach, 0., n[2] * s * reach]
}

/// FUN_14040ad70(mine, theirs, acc): for each of their shapes, the first
/// of mine touching it separates the two by weight. Returns what they move
/// (added to their obj+0xa48) and what this object gives back (taken off
/// its displacement `acc`), summed over the pairs.
pub fn resolve(mine: &mut Body, theirs: &mut Body, acc: Vec3) -> (Vec3, Vec3) {
    let mut moved = [0.; 3];
    let mut back = [0.; 3];
    for j in 0..theirs.shapes.len() {
        let Some(i) = (0..mine.shapes.len()).find(|&i| overlap(&mine.shapes[i], &theirs.shapes[j]))
        else {
            continue;
        };
        let d = separation(&mine.shapes[i], &theirs.shapes[j]);
        let give = match (theirs.immovable(), mine.immovable()) {
            (false, false) => {
                let (wa, wb) = (mine.weight, theirs.weight);
                let c = d.map(|v| v * (wa / (wb + wa)));
                theirs.shift(c);
                (0..3).for_each(|k| moved[k] += c[k]);
                c.map(|v| v * (wb / wa))
            }
            (false, true) => {
                theirs.shift(d);
                (0..3).for_each(|k| moved[k] += d[k]);
                [0.; 3]
            }
            (true, true) => acc,
            (true, false) => d,
        };
        mine.shift(give.map(|v| -v));
        (0..3).for_each(|k| back[k] += give[k]);
    }
    (moved, back)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn body(x: f32, weight: f32) -> Body {
        Body {
            shapes: vec![Shape {
                kind: CYLINDER,
                centre: [x, 85., 0.],
                radius: 35.,
                height: 55.,
            }],
            weight,
            fixed: false,
        }
    }

    #[test]
    fn overlapping_bodies_part_by_weight() {
        // Sora (100) 40 from a body of weight 300: the separation is
        // n·(1/n − 1)·(70 + 5) = 32.14 along x; the other takes a quarter,
        // Sora gives back the rest.
        let mut sora = body(0., 100.);
        let mut other = body(40., 300.);
        let (moved, back) = resolve(&mut sora, &mut other, [0.; 3]);
        assert!((moved[0] - 8.035_714).abs() < 1e-3, "{moved:?}");
        assert!((back[0] - 24.107_143).abs() < 1e-3, "{back:?}");
        // An immovable one takes nothing; Sora gives all of it back.
        let mut sora = body(0., 100.);
        let mut wall = body(40., 0.);
        let (moved, back) = resolve(&mut sora, &mut wall, [0.; 3]);
        assert_eq!(moved, [0.; 3]);
        assert!((back[0] - 32.142_857).abs() < 1e-3);
        // Apart: nothing.
        let mut far = body(200., 100.);
        let mut sora = body(0., 100.);
        assert_eq!(resolve(&mut sora, &mut far, [0.; 3]), ([0.; 3], [0.; 3]));
    }

    #[test]
    fn upright_cylinders_test_their_gap_and_reach() {
        let shape = |kind, centre, radius, height| Shape {
            kind,
            centre,
            radius,
            height,
        };
        // Two cylinders (r 10, half height 50): a vertical gap under the
        // summed half heights and a reach under the summed radii.
        let a = shape(UPRIGHT, [0., 0., 0.], 10., 50.);
        assert!(overlap(&a, &shape(UPRIGHT, [19., 99., 0.], 10., 50.)));
        assert!(!overlap(&a, &shape(UPRIGHT, [19., 100., 0.], 10., 50.)));
        assert!(!overlap(&a, &shape(UPRIGHT, [20., 0., 0.], 10., 50.)));
        // A sphere (r 20) beside the cylinder's side reaches 30 out; above
        // its top (dy 60, 10 into the sphere's 20) only 10 + 20·√0.75.
        let s = |x, y| shape(SPHERE, [x, y, 0.], 20., 20.);
        assert!(overlap(&s(29., 40.), &a));
        assert!(!overlap(&s(30., 40.), &a));
        let reach = 10. + 20. * 0.75f32.sqrt();
        assert!(overlap(&a, &s(reach - 0.1, 60.)));
        assert!(!overlap(&a, &s(reach + 0.1, 60.)));
        assert!(!overlap(&a, &s(0., 70.)));
    }
}
