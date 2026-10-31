//! KH3's animation on KH2's Sora. Each linked joint takes the KH3 bone's rotation change from its rest pose, after a
//! correction that turns the KH2 bone's bind direction onto the KH3 bone's rest direction (KH2 binds in a T-pose, KH3
//! rests in an A-pose). Joint positions follow KH2's own bone lengths; the hips sit where KH3's are, and the legs reach
//! KH3's ankles, so the feet plant. Hair and the hood keep KH2's own idle; every other joint its bind relation.
//!
//! The joints are a reading of P_EX100's bind skeleton; the bones are KH3's by name. KH3's cloth bones have nothing to
//! drive on this model.

use super::*;
use data::kh3::anim::BonePose;
use data::kh3::skeletal::Bone;
use data::kh3::sora::{self, Socket};
use data::motion_eval::World;

/// KH2's joint count for the model the links were read from.
const JOINTS: usize = 228;
const ROOT: usize = 0;
const HIPS: usize = 2;
const PELVIS: usize = 3;
/// The right wrist.
const WRIST: usize = 172;
/// What the other side adds to a left leg's or arm's joint.
const RIGHT_LEG: usize = 29;
const RIGHT_ARM: usize = 92;

/// (joint, the joint its direction points to, KH3's bone, the bone its direction points to).
const MIDDLE: &[(usize, Option<usize>, &str, Option<&str>)] = &[
    (ROOT, None, "center", None),
    (HIPS, None, "koshi", None),
    (65, Some(66), "hara1", Some("hara2")),
    (66, Some(67), "hara2", Some("mune")),
    (67, Some(68), "mune", Some("kubi")),
    (137, Some(138), "kubi", Some("atama")),
    (140, None, "atama", None),
];

const LEG: &[(usize, Option<usize>, &str, Option<&str>)] = &[
    (11, Some(12), "momo", Some("sune")),
    (12, Some(13), "sune", Some("ashi1")),
    (13, Some(17), "ashi1", Some("ashi2")),
    (17, None, "ashi2", None),
];

const ARM: &[(usize, Option<usize>, &str, Option<&str>)] = &[
    (75, Some(76), "kata", Some("ude1")),
    (78, Some(79), "ude1", Some("ude2")),
    (79, Some(80), "ude2", Some("te")),
    (80, Some(86), "te", Some("yubiC1")),
    (85, Some(86), "te", Some("yubiC1")),
    (88, Some(89), "yubiA1", Some("yubiA2")),
    (91, Some(92), "yubiA2", Some("yubiA3")),
    (94, Some(95), "yubiB1", Some("yubiB2")),
    (95, Some(96), "yubiB2", Some("yubiB3")),
    (96, None, "yubiB3", None),
    (98, Some(99), "yubiC1", Some("yubiC2")),
    (99, Some(100), "yubiC2", Some("yubiC3")),
    (100, None, "yubiC3", None),
    (102, Some(103), "yubiD1", Some("yubiD2")),
    (103, Some(104), "yubiD2", Some("yubiD3")),
    (104, None, "yubiD3", None),
    (106, Some(107), "yubiE1", Some("yubiE2")),
    (107, Some(108), "yubiE2", Some("yubiE3")),
    (108, None, "yubiE3", None),
];

/// The joints whose KH2 idle stays on: hair, the hood.
fn takes_idle(joint: usize) -> bool {
    (69..=73).contains(&joint) || (142..=163).contains(&joint)
}

#[derive(Clone, Copy)]
struct Link {
    bone: usize,
    correct: Quat,
}

struct Leg {
    thigh: usize,
    knee: usize,
    ankle: usize,
    bone: usize,
    upper: f32,
    lower: f32,
    /// KH2's ankle against KH3's, at rest.
    offset: Vec3,
}

fn model_t(t: [f32; 3]) -> Vec3 {
    Vec3::new(t[0], t[2], t[1])
}

fn model_q(q: [f32; 4]) -> Quat {
    Quat::from_xyzw(-q[0], -q[2], -q[1], q[3])
}

fn quat(q: [f32; 4]) -> Quat {
    Quat::from_array(q)
}

/// A pose of KH2's model and where KH3's effects find their bones on it.
#[derive(Default)]
pub struct Posed {
    pub joints: Vec<World>,
    /// The Keyblade's joint, KH2's model frame.
    pub weapon: Transform,
    /// KH3's bones at KH2's joints, in KH3's frame and centimetres.
    pub bones: Vec<BonePose>,
}

pub struct Retarget {
    parents: Vec<i32>,
    bind_q: Vec<Quat>,
    bind_t: Vec<Vec3>,
    links: Vec<Option<Link>>,
    legs: [Leg; 2],
    hip_bones: [usize; 2],
    /// KH3's skeleton at rest, in KH2's axes.
    rest_q: Vec<Quat>,
    bone_parents: Vec<i32>,
    /// By KH3 bone: the joint its effects sit on.
    joint_of: Vec<Option<usize>>,
    socket: Option<(usize, BonePose)>,
    hand_bone: usize,
    /// KH2's hand against KH3's.
    hand_scale: f32,
    /// Sora's size against KH3's, the layer's.
    scale: f32,
}

impl Retarget {
    /// `bind`: KH2's bind joints; None when this is not the skeleton the links were read from, or KH3's lacks a bone.
    pub fn new(
        bind: &[[[f32; 4]; 4]],
        parents: &[i32],
        bones: &[Bone],
        sockets: &[Socket],
        scale: f32,
    ) -> Option<Self> {
        if bind.len() != JOINTS || parents.len() != JOINTS {
            return None;
        }
        let matrices: Vec<Mat4> = bind.iter().map(Mat4::from_cols_array_2d).collect();
        let bind_q: Vec<Quat> = matrices
            .iter()
            .map(|m| m.to_scale_rotation_translation().1)
            .collect();
        let bind_t: Vec<Vec3> = matrices.iter().map(|m| m.w_axis.truncate()).collect();
        let local: Vec<BonePose> = bones
            .iter()
            .map(|b| BonePose {
                rotation: b.rotation,
                translation: b.translation,
                scale: b.scale,
            })
            .collect();
        let mut rest = Vec::new();
        sora::to_component(bones, &local, &mut rest);
        let rest_q: Vec<Quat> = rest.iter().map(|p| model_q(p.rotation)).collect();
        let rest_t: Vec<Vec3> = rest.iter().map(|p| model_t(p.translation)).collect();
        let bone = |name: &str| bones.iter().position(|b| b.name == name);
        let mut links = vec![None; JOINTS];
        let mut joint_of = vec![None; bones.len()];
        let mut link = |joint: usize,
                        tip: Option<usize>,
                        name: &str,
                        to: Option<&str>,
                        previous: Option<Quat>|
         -> Option<Quat> {
            let at = bone(name)?;
            let correct = match (tip, to) {
                (Some(tip), Some(to)) => Quat::from_rotation_arc(
                    (bind_t[tip] - bind_t[joint]).normalize_or_zero(),
                    (rest_t[bone(to)?] - rest_t[at]).normalize_or_zero(),
                ),
                // The end of a chain turns as the bone before it.
                _ => previous.unwrap_or(Quat::IDENTITY),
            };
            links[joint] = Some(Link { bone: at, correct });
            joint_of[at] = Some(joint);
            Some(correct)
        };
        let mut previous = None;
        for &(joint, tip, name, to) in MIDDLE {
            previous = Some(link(joint, tip, name, to, None)?);
        }
        for (side, leg, arm) in [("L_", 0, 0), ("R_", RIGHT_LEG, RIGHT_ARM)] {
            for &(joint, tip, name, to) in LEG {
                let to = to.map(|t| format!("{side}{t}"));
                previous = Some(link(
                    joint + leg,
                    tip.map(|t| t + leg),
                    &format!("{side}{name}"),
                    to.as_deref(),
                    previous,
                )?);
            }
            for &(joint, tip, name, to) in ARM {
                let to = to.map(|t| format!("{side}{t}"));
                previous = Some(link(
                    joint + arm,
                    tip.map(|t| t + arm),
                    &format!("{side}{name}"),
                    to.as_deref(),
                    previous,
                )?);
            }
        }
        let leg = |side: &str, add: usize| -> Option<Leg> {
            let (thigh, knee, ankle) = (11 + add, 12 + add, 13 + add);
            let at = bone(&format!("{side}ashi1"))?;
            Some(Leg {
                thigh,
                knee,
                ankle,
                bone: at,
                upper: bind_t[knee].distance(bind_t[thigh]),
                lower: bind_t[ankle].distance(bind_t[knee]),
                offset: bind_t[ankle] - rest_t[at] * scale,
            })
        };
        let hand_bone = bone("R_te")?;
        let hand_scale = bind_t[WRIST + 6].distance(bind_t[WRIST])
            / rest_t[bone("R_yubiC1")?].distance(rest_t[hand_bone])
            / scale;
        let socket = sockets
            .iter()
            .find(|s| s.name == "R_wep_skt")
            .and_then(|s| {
                Some((
                    bone(&s.bone)?,
                    BonePose {
                        rotation: sora::quat_of_rotator(s.rotation),
                        translation: s.location,
                        scale: s.scale,
                    },
                ))
            });
        Some(Self {
            parents: parents.to_vec(),
            legs: [leg("L_", 0)?, leg("R_", RIGHT_LEG)?],
            hip_bones: [bone("L_momo")?, bone("R_momo")?],
            bind_q,
            bind_t,
            links,
            rest_q,
            bone_parents: bones.iter().map(|b| b.parent).collect(),
            joint_of,
            socket,
            hand_bone,
            hand_scale,
            scale,
        })
    }

    /// KH2's joints for KH3's pose; `idle`: this frame of KH2's own idle.
    pub fn pose(&self, component: &[BonePose], idle: &[World], out: &mut Posed) {
        let k = self.scale;
        let change = |bone: usize| model_q(component[bone].rotation) * self.rest_q[bone].inverse();
        let turned = |joint: usize| {
            self.links[joint].map(|l| (change(l.bone) * l.correct * self.bind_q[joint]).normalize())
        };
        let local = |joint: usize, parent: usize| -> (Quat, Vec3) {
            let (q, t, pq, pt) = match (takes_idle(joint), idle.get(joint), idle.get(parent)) {
                (true, Some(a), Some(b)) => (
                    quat(a.rotation),
                    Vec3::from_array(a.translation),
                    quat(b.rotation),
                    Vec3::from_array(b.translation),
                ),
                _ => (
                    self.bind_q[joint],
                    self.bind_t[joint],
                    self.bind_q[parent],
                    self.bind_t[parent],
                ),
            };
            let inverse = pq.inverse();
            (inverse * q, inverse * (t - pt))
        };
        // KH2's pelvis sits between KH3's hip joints.
        let hips = turned(HIPS).unwrap_or(Quat::IDENTITY);
        let target = (model_t(component[self.hip_bones[0]].translation)
            + model_t(component[self.hip_bones[1]].translation))
            * 0.5
            * k;
        let drop = self.bind_q[HIPS].inverse() * (self.bind_t[PELVIS] - self.bind_t[HIPS]);
        let root = target - hips * drop;
        let mut q = vec![Quat::IDENTITY; JOINTS];
        let mut t = vec![Vec3::ZERO; JOINTS];
        let mut knees: [Option<Quat>; 2] = [None; 2];
        for i in 0..JOINTS {
            let (follow_q, follow_t) = match usize::try_from(self.parents[i]) {
                Ok(p) => {
                    let (lq, lt) = local(i, p);
                    (q[p] * lq, t[p] + q[p] * lt)
                }
                Err(_) => (Quat::IDENTITY, root),
            };
            t[i] = follow_t;
            q[i] = turned(i).unwrap_or(follow_q);
            for (leg, knee) in self.legs.iter().zip(&mut knees) {
                if i == leg.thigh {
                    let at = target_of(leg, component, &change, k);
                    let (thigh, shin) = self.reach(leg, t[i], q[i], &turned, at);
                    q[i] = thigh;
                    *knee = Some(shin);
                } else if i == leg.knee
                    && let Some(shin) = *knee
                {
                    q[i] = shin;
                }
            }
        }
        out.joints.clear();
        out.joints.extend((0..JOINTS).map(|i| World {
            rotation: q[i].to_array(),
            translation: t[i].to_array(),
            // Some joints are kept scaled to nothing: what they carry is not worn.
            scale: idle.get(i).map_or([1.; 3], |w| w.scale),
        }));
        // The Keyblade: KH3's socket from the hand, KH2's hand size.
        let weapon = self.socket.as_ref().map(|(bone, socket)| {
            let world = sora::compose(&component[*bone], socket);
            let grip = model_t(world.translation) - model_t(component[self.hand_bone].translation);
            let turn = model_q(world.rotation);
            let at = t[WRIST] + grip * k * self.hand_scale + turn * GRIP;
            (*bone, at, (turn * blade_frame()).normalize())
        });
        out.weapon = weapon.map_or(Transform::default(), |(_, at, rotation)| Transform {
            translation: at,
            rotation,
            scale: Vec3::ONE,
        });
        // The bones for the effects: each on its joint, the rest at their rest offset from it.
        out.bones.clear();
        for (b, pose) in component.iter().enumerate() {
            let at = match (self.joint_of[b], usize::try_from(self.bone_parents[b])) {
                _ if weapon.is_some_and(|w| w.0 == b) => weapon.map_or(Vec3::ZERO, |w| w.1) / k,
                (Some(j), _) => t[j] / k,
                (None, Ok(p)) => {
                    let offset = model_t(pose.translation) - model_t(component[p].translation);
                    model_t(out.bones[p].translation) + offset
                }
                (None, Err(_)) => model_t(pose.translation),
            };
            out.bones.push(BonePose {
                rotation: pose.rotation,
                translation: [at.x, at.z, at.y],
                scale: pose.scale,
            });
        }
    }

    /// The thigh and shin that put the ankle on `at`, the knee to the side the animation bends it.
    fn reach(
        &self,
        leg: &Leg,
        hip: Vec3,
        thigh: Quat,
        turned: &dyn Fn(usize) -> Option<Quat>,
        at: Vec3,
    ) -> (Quat, Quat) {
        let knee_local =
            self.bind_q[leg.thigh].inverse() * (self.bind_t[leg.knee] - self.bind_t[leg.thigh]);
        let ankle_local =
            self.bind_q[leg.knee].inverse() * (self.bind_t[leg.ankle] - self.bind_t[leg.knee]);
        let shin = turned(leg.knee).unwrap_or(Quat::IDENTITY);
        let knee = hip + thigh * knee_local;
        let ankle = knee + shin * ankle_local;
        let to = at - hip;
        let reach = to.length().clamp(
            (leg.upper - leg.lower).abs() + 1e-3,
            leg.upper + leg.lower - 1e-3,
        );
        let along = to.normalize_or_zero();
        if along == Vec3::ZERO {
            return (thigh, shin);
        }
        let x = (leg.upper * leg.upper - leg.lower * leg.lower + reach * reach) / (2. * reach);
        let h = (leg.upper * leg.upper - x * x).max(0.).sqrt();
        let side = (knee - hip) - along * (knee - hip).dot(along);
        // A straight leg: its bend goes forward.
        let side = side.try_normalize().unwrap_or(thigh * Vec3::Z);
        let new_knee = hip + along * x + side * h;
        let first = Quat::from_rotation_arc(
            (knee - hip).normalize_or_zero(),
            (new_knee - hip).normalize_or_zero(),
        );
        let new_ankle = hip + along * reach;
        let second = Quat::from_rotation_arc(
            (first * (ankle - knee)).normalize_or_zero(),
            (new_ankle - new_knee).normalize_or_zero(),
        );
        (
            (first * thigh).normalize(),
            (second * first * shin).normalize(),
        )
    }
}

fn target_of(leg: &Leg, component: &[BonePose], change: &dyn Fn(usize) -> Quat, k: f32) -> Vec3 {
    model_t(component[leg.bone].translation) * k + change(leg.bone) * leg.offset
}

/// KH2's Keyblade against KH3's socket, read off both models (each has its grip at its origin and its shaft along one
/// axis, the teeth to one side): where KH2's shaft goes,
const GRIP: Vec3 = Vec3::new(-0.025, 1.029, 3.739);

/// and how KH2's model turns onto KH3's.
fn blade_frame() -> Quat {
    Quat::from_xyzw(0.80445, -0.57576, -0.08186, -0.1211)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_leg_reaches_its_ankle() {
        // Hip at the origin, the bones straight down.
        let mut bind = vec![Mat4::IDENTITY.to_cols_array_2d(); JOINTS];
        let mut parents = vec![-1; JOINTS];
        for (i, p) in parents.iter_mut().enumerate().skip(1) {
            *p = i as i32 - 1;
        }
        let at = |y: f32| Mat4::from_translation(Vec3::new(0., y, 0.)).to_cols_array_2d();
        bind[11] = at(0.);
        bind[12] = at(-40.);
        bind[13] = at(-80.);
        let leg = Leg {
            thigh: 11,
            knee: 12,
            ankle: 13,
            bone: 0,
            upper: 40.,
            lower: 40.,
            offset: Vec3::ZERO,
        };
        let matrices: Vec<Mat4> = bind.iter().map(Mat4::from_cols_array_2d).collect();
        let retarget = Retarget {
            parents,
            bind_q: vec![Quat::IDENTITY; JOINTS],
            bind_t: matrices.iter().map(|m| m.w_axis.truncate()).collect(),
            links: vec![None; JOINTS],
            legs: [Leg { ..leg_copy(&leg) }, Leg { ..leg_copy(&leg) }],
            hip_bones: [0, 0],
            rest_q: Vec::new(),
            bone_parents: Vec::new(),
            joint_of: Vec::new(),
            socket: None,
            hand_bone: 0,
            hand_scale: 1.,
            scale: 1.,
        };
        let target = Vec3::new(10., -60., 20.);
        let same = |_: usize| Some(Quat::IDENTITY);
        let (thigh, shin) = retarget.reach(&leg, Vec3::ZERO, Quat::IDENTITY, &same, target);
        let knee = thigh * Vec3::new(0., -40., 0.);
        let ankle = knee + shin * Vec3::new(0., -40., 0.);
        assert!(ankle.distance(target) < 1e-2, "{ankle}");
        assert!((knee.length() - 40.).abs() < 1e-2);
    }

    fn leg_copy(leg: &Leg) -> Leg {
        Leg {
            thigh: leg.thigh,
            knee: leg.knee,
            ankle: leg.ankle,
            bone: leg.bone,
            upper: leg.upper,
            lower: leg.lower,
            offset: leg.offset,
        }
    }
}
