//! KH3's secondary motion on the component-space pose: KineDriver (helper bones) and Bonamik (hair, hood, sleeves,
//! coat, chains). From StrawHatPro's `kh-anim` (MIT), a transcription of the executable; the data comes from the
//! player's own install (`data::kh3::rig`).
#![forbid(unsafe_code)]
#![allow(clippy::all, dead_code, unused_imports, unused_variables, unused_mut)]

pub mod blend;
pub mod bonamik;
pub mod json;
pub mod kine;
pub mod pack;

pub use glam;
use glam::{Quat, Vec3};

pub use bonamik::{Bonamik, BonamikHost, BonamikOptions, Jitter};
pub use pack::{Bone, Skeleton};

/// One bone transform: UE's `FTransform` (scale, then rotation, then translation).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Trs {
    pub translation: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}

impl Trs {
    pub const IDENTITY: Trs = Trs {
        translation: Vec3::ZERO,
        rotation: Quat::IDENTITY,
        scale: Vec3::ONE,
    };

    /// `child * parent` in UE's order: this transform expressed in the parent's parent space.
    pub fn then(&self, parent: &Trs) -> Trs {
        Trs {
            translation: parent.translation + parent.rotation * (parent.scale * self.translation),
            rotation: (parent.rotation * self.rotation).normalize(),
            scale: parent.scale * self.scale,
        }
    }

    /// The transform that, followed by `parent`, gives this one (`FTransform::GetRelativeTransform`).
    pub fn relative_to(&self, parent: &Trs) -> Trs {
        let one = |x: f32| if x.abs() <= 1.0e-8 { 0.0 } else { 1.0 / x };
        let reciprocal = Vec3::new(
            one(parent.scale.x),
            one(parent.scale.y),
            one(parent.scale.z),
        );
        let inverse = parent.rotation.conjugate();
        Trs {
            translation: (inverse * (self.translation - parent.translation)) * reciprocal,
            rotation: (inverse * self.rotation).normalize(),
            scale: self.scale * reciprocal,
        }
    }

    pub fn transform_point(&self, p: Vec3) -> Vec3 {
        self.translation + self.rotation * (self.scale * p)
    }
}
