//! worldmap::ObjCamera (FUN_140397b80 / FUN_140397c70) and the blend every
//! worldmap::Camera shares (FUN_14038eb50 / FUN_14038ebc0). Engine
//! coordinates (Y down), one update per 60 Hz tick (`DAT_140717480` = 1).
type V3 = [f32; 3];

fn add(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

/// FUN_140141ae0(out, a, b, t) = `b * (1 − t) + a * t`.
fn mix(a: V3, b: V3, t: f32) -> V3 {
    [0, 1, 2].map(|i| b[i] * (1. - t) + a[i] * t)
}

fn ease(t: f32) -> f32 {
    // sin(t * π * 0.5) (DAT_140623b78, DAT_140623a4c).
    (t.clamp(0., 1.) * std::f32::consts::PI * 0.5).sin()
}

/// Eye and target offsets from the cursor (0x1407490f0 / 0x1407490e0).
pub const EYE: V3 = [0., -2300., -3000.];
pub const TARGET: V3 = [0., -500., 0.];
/// Near The World That Never Was symbol (0x140749110 / 0x140749100).
pub const EH_EYE: V3 = [0., -790., -8580.];
pub const EH_TARGET: V3 = [0., -1760., -3180.];
/// Far south (0x140749120): the eye offset the cursor's depth blends to.
pub const SOUTH_EYE: V3 = [0., -2500., -2000.];
/// +0x88: 65 degrees, times `DAT_1405b15b4` (π / 180).
pub const FOV_DEGREES: f32 = 65.;
/// +0x54: blend length in ticks.
pub const BLEND_TICKS: f32 = 60.;
/// 0x1405c2b88..0x1405c2b94 and `DAT_1405b26e4`.
const SOUTH_RANGE: f32 = 6000.;
const EH_RANGE: f32 = 8000.;
const SOUTH_START: f32 = -8000.;
const SOUTH_LIMIT: f32 = -14000.;
const EH_NEAR: f32 = 5000.;

/// One camera pose (FUN_14019d3d0's eye and target, FUN_14019dde0's fov
/// in radians).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose {
    pub eye: V3,
    pub target: V3,
    pub fov: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ObjCamera {
    /// +0x08 / +0x18 / +0x48: where a blend starts.
    from: Pose,
    /// +0x50 / +0x54.
    elapsed: f32,
    length: f32,
    /// +0x4c bit 0: cleared when another camera slot took over
    /// (FUN_14038ebb0), so the next update blends from the current camera.
    current: bool,
}

impl ObjCamera {
    /// FUN_140397b80: created already current, so it does not blend in.
    pub fn new(now: Pose) -> Self {
        Self {
            from: now,
            elapsed: BLEND_TICKS,
            length: BLEND_TICKS,
            current: true,
        }
    }

    /// FUN_14038ebb0: another camera slot is in use.
    pub fn suspend(&mut self) {
        self.current = false;
    }

    /// FUN_140397c70 then FUN_14038ebc0. `cursor` is the cursor's
    /// position, `eh_symbol` the World That Never Was symbol's position
    /// while that world is open and its record flag bit 1 is clear (else
    /// `None`), `now` the engine camera this tick.
    pub fn update(&mut self, cursor: V3, eh_symbol: Option<V3>, now: Pose) -> Pose {
        let mut p = cursor;
        if p[2] < SOUTH_LIMIT {
            p[2] = SOUTH_LIMIT;
        }
        let (mut eye, target) = match eh_symbol {
            None => (EYE, TARGET),
            Some(s) => {
                let d = (s[0] - p[0]).hypot(s[2] - p[2]);
                let t = ease((d - EH_NEAR) / EH_RANGE);
                (mix(EYE, EH_EYE, t), mix(TARGET, EH_TARGET, t))
            }
        };
        if p[2] < SOUTH_START {
            let t = ease((p[2] - SOUTH_LIMIT) / SOUTH_RANGE);
            eye = mix(eye, SOUTH_EYE, t);
        }
        let mut goal = Pose {
            eye: add(p, eye),
            target: add(p, target),
            fov: FOV_DEGREES.to_radians(),
        };
        // FUN_14038ebc0.
        if !self.current {
            self.from = now;
            // The re-initialisation also takes the engine's current fov.
            goal.fov = now.fov;
            self.elapsed = 0.;
        }
        let mut out = goal;
        if self.elapsed < self.length {
            self.elapsed += 1.;
            if self.elapsed < self.length {
                let s = (self.elapsed / self.length * std::f32::consts::FRAC_PI_2).sin();
                out.eye = mix(goal.eye, self.from.eye, s);
                out.target = mix(goal.target, self.from.target, s);
            }
        }
        self.current = true;
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: Pose = Pose {
        eye: [0.; 3],
        target: [0., 0., 1.],
        fov: 1.,
    };

    #[test]
    fn follows_the_cursor_with_its_offsets() {
        let mut c = ObjCamera::new(NOW);
        let pose = c.update([100., -400., 50.], None, NOW);
        assert_eq!(pose.eye, [100., -2700., -2950.]);
        assert_eq!(pose.target, [100., -900., 50.]);
        assert!((pose.fov - 65f32.to_radians()).abs() < 1e-6);
    }

    #[test]
    fn south_edge_clamps_and_blends_the_eye() {
        let mut c = ObjCamera::new(NOW);
        let pose = c.update([0., 0., -20000.], None, NOW);
        assert_eq!(pose.eye, [0., -2500., -16000.]);
        let half = c.update([0., 0., -11000.], None, NOW);
        assert!(half.eye[1] < -2300. && half.eye[1] > -2500.);
    }

    #[test]
    fn near_the_eh_symbol_the_close_offsets_win() {
        let mut c = ObjCamera::new(NOW);
        let pose = c.update([0., 0., 0.], Some([1000., 0., 0.]), NOW);
        assert_eq!(pose.eye, EH_EYE);
        assert_eq!(pose.target, EH_TARGET);
        let far = c.update([0., 0., 0.], Some([20000., 0., 0.]), NOW);
        assert_eq!(far.eye, EYE);
    }

    #[test]
    fn a_suspended_camera_blends_back_in() {
        let mut c = ObjCamera::new(NOW);
        c.suspend();
        let first = c.update([0., 0., 0.], None, NOW);
        assert_eq!(first.fov, NOW.fov);
        // After one tick the pose is mostly the old camera's.
        assert!(first.eye[1] > -2300. * 0.1);
        for _ in 0..70 {
            c.update([0., 0., 0.], None, NOW);
        }
        assert_eq!(c.update([0., 0., 0.], None, NOW).eye, EYE);
    }
}
