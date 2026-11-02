//! Explicit research bind routes; no installed transform convention selected.
//! OpenKH route formulas and .NET 10 FMA ordering are attributed in NOTICE.
use crate::model_skeleton::BoneRecord;
use serde::{Deserialize, Serialize};
use std::io;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum PoseProfile {
    #[serde(rename = "openkh_legacy_bind_fma_v1")]
    Legacy,
    #[serde(rename = "openkh_modern_bind_fma_v1")]
    Modern,
}
#[derive(Serialize)]
pub struct ModelPose {
    pub profile: PoseProfile,
    pub arithmetic_profile: &'static str,
    pub matrix_bits: Vec<[u32; 16]>,
    pub position_bits: Vec<[u32; 3]>,
    pub runtime_verified: bool,
}
type Matrix = [[f32; 4]; 4];
const IDENTITY: Matrix = [
    [1., 0., 0., 0.],
    [0., 1., 0., 0.],
    [0., 0., 1., 0.],
    [0., 0., 0., 1.],
];
const MAX_ITEMS: usize = 262_144;
fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
fn finite<const N: usize>(values: [f32; N]) -> io::Result<[f32; N]> {
    if values.iter().all(|n| n.is_finite()) {
        Ok(values)
    } else {
        Err(invalid("nonfinite pose input/output"))
    }
}
fn transform4(v: [f32; 4], m: &Matrix) -> [f32; 4] {
    std::array::from_fn(|i| {
        let a = m[0][i] * v[0];
        let b = m[1][i].mul_add(v[1], a);
        let c = m[2][i].mul_add(v[2], b);
        m[3][i].mul_add(v[3], c)
    })
}
fn transform3(v: [f32; 3], m: &Matrix) -> [f32; 3] {
    std::array::from_fn(|i| {
        let a = m[0][i] * v[0];
        let b = m[1][i].mul_add(v[1], a);
        m[2][i].mul_add(v[2], b) + m[3][i]
    })
}
fn multiply(a: &Matrix, b: &Matrix) -> Matrix {
    a.map(|row| transform4(row, b))
}
fn translation(t: [f32; 3]) -> Matrix {
    let mut result = IDENTITY;
    result[3][..3].copy_from_slice(&t);
    result
}
fn axis_rotation(axis: usize, angle: f32) -> Matrix {
    let (s, c) = angle.sin_cos();
    match axis {
        0 => [
            [1., 0., 0., 0.],
            [0., c, s, 0.],
            [0., -s, c, 0.],
            [0., 0., 0., 1.],
        ],
        1 => [
            [c, 0., -s, 0.],
            [0., 1., 0., 0.],
            [s, 0., c, 0.],
            [0., 0., 0., 1.],
        ],
        _ => [
            [c, s, 0., 0.],
            [-s, c, 0., 0.],
            [0., 0., 1., 0.],
            [0., 0., 0., 1.],
        ],
    }
}
fn axis_quaternion(axis: usize, angle: f32) -> [f32; 4] {
    let (s, c) = (angle * 0.5).sin_cos();
    // Multiplication retains the signs of zero axis components.
    std::array::from_fn(|i| if i == 3 { c } else { f32::from(i == axis) * s })
}
fn quaternion_product(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    let permutations = [[3, 2, 1, 0], [2, 3, 0, 1], [1, 0, 3, 2]];
    let signs = [[1., -1., 1., -1.], [1., 1., -1., -1.], [-1., 1., 1., -1.]];
    let mut result = b.map(|n| n * a[3]);
    for axis in 0..3 {
        for i in 0..4 {
            result[i] = (b[permutations[axis][i]] * a[axis]).mul_add(signs[axis][i], result[i]);
        }
    }
    result
}
fn quaternion_matrix([x, y, z, w]: [f32; 4]) -> Matrix {
    let (xx, yy, zz, xy, xz, wz, wy, yz, wx) = (
        x * x,
        y * y,
        z * z,
        x * y,
        x * z,
        z * w,
        y * w,
        y * z,
        x * w,
    );
    [
        [1. - 2. * (yy + zz), 2. * (xy + wz), 2. * (xz - wy), 0.],
        [2. * (xy - wz), 1. - 2. * (zz + xx), 2. * (yz + wx), 0.],
        [2. * (xz + wy), 2. * (yz - wx), 1. - 2. * (yy + xx), 0.],
        [0., 0., 0., 1.],
    ]
}
fn bind_matrices(bones: &[BoneRecord], profile: PoseProfile) -> io::Result<Vec<Matrix>> {
    if bones.len() > 2048 {
        return Err(invalid("too many pose bones"));
    }
    let mut result: Vec<Matrix> = Vec::new();
    let mut rotations = Vec::new();
    let mut translations: Vec<[f32; 3]> = Vec::new();
    for (ordinal, bone) in bones.iter().enumerate() {
        let t = finite(bone.transform_bits.map(f32::from_bits))?;
        let parent = match profile {
            PoseProfile::Legacy => bone.header_i32[1],
            PoseProfile::Modern => i32::from(bone.header_i16[2]),
        };
        if parent >= 0 && parent as usize >= ordinal {
            return Err(invalid("pose parent must precede child"));
        }
        let position = [t[8], t[9], t[10]];
        let matrix = match profile {
            PoseProfile::Legacy => {
                let (parent_rotation, parent_translation) = if parent < 0 {
                    ([0., 0., 0., 1.], [0.; 3])
                } else {
                    (rotations[parent as usize], translations[parent as usize])
                };
                let rotated = transform3(position, &quaternion_matrix(parent_rotation));
                let absolute_translation =
                    std::array::from_fn(|i| parent_translation[i] + rotated[i]);
                let mut local_rotation = [0., 0., 0., 1.];
                for axis in [2, 1, 0] {
                    if t[4 + axis] != 0. {
                        local_rotation =
                            quaternion_product(local_rotation, axis_quaternion(axis, t[4 + axis]));
                    }
                }
                let absolute_rotation =
                    finite(quaternion_product(parent_rotation, local_rotation))?;
                rotations.push(absolute_rotation);
                translations.push(finite(absolute_translation)?);
                multiply(
                    &multiply(&IDENTITY, &quaternion_matrix(absolute_rotation)),
                    &translation(absolute_translation),
                )
            }
            PoseProfile::Modern => {
                let mut matrix = IDENTITY;
                for axis in 0..3 {
                    matrix[axis][axis] = t[axis];
                }
                for axis in 0..3 {
                    matrix = multiply(&matrix, &axis_rotation(axis, t[4 + axis]));
                }
                matrix = multiply(&matrix, &translation(position));
                if parent >= 0 {
                    matrix = multiply(&matrix, &result[parent as usize]);
                }
                matrix
            }
        };
        for row in matrix {
            finite(row)?;
        }
        result.push(matrix);
    }
    Ok(result)
}
#[derive(Clone, Serialize)]
struct Influence {
    matrix: usize,
    vertex_bits: [u32; 4],
}
/// Checked, retained coordinates and assignments; packet decoding happens once.
/// One flat list of influences, each vertex's ending at its `ends` entry.
#[derive(Clone, Default)]
pub struct Skinning {
    pub bones: Vec<BoneRecord>,
    influences: Vec<Influence>,
    ends: Vec<u32>,
}
impl Serialize for Skinning {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let assignments: Vec<&[Influence]> = self.assignments().collect();
        let mut s = serializer.serialize_struct("Skinning", 2)?;
        s.serialize_field("bones", &self.bones)?;
        s.serialize_field("assignments", &assignments)?;
        s.end()
    }
}
impl Skinning {
    pub fn read(bytes: &[u8]) -> io::Result<Self> {
        let structure = crate::model_skeleton::read_model_structure(bytes)?;
        let geometry = crate::model_geometry::read_geometry(bytes)?;
        let groups = crate::model_packets::read_packet_groups(bytes)?;
        let bones = structure
            .blocks
            .into_iter()
            .next()
            .ok_or_else(|| invalid("skin skeleton absent"))?
            .bones;
        let mut influences = Vec::new();
        let mut ends = Vec::new();
        for part in &geometry.parts {
            let group = groups
                .iter()
                .find(|g| {
                    g.block_offset == geometry.block_offset && g.group_ordinal == part.group_ordinal
                })
                .ok_or_else(|| invalid("pose group missing"))?;
            let data = group
                .packets
                .get(part.packet_ordinal)
                .and_then(|p| p.vpu_data.as_ref())
                .ok_or_else(|| invalid("pose type-1 packet missing"))?;
            for assignment in &part.assignments {
                if assignment.is_empty()
                    || assignment.len() > 32
                    || influences.len() + assignment.len() > MAX_ITEMS
                    || ends.len() == MAX_ITEMS
                {
                    return Err(invalid("pose assignments exceed bounded profile"));
                }
                for binding in assignment {
                    let matrix = usize::try_from(binding.matrix_index)
                        .ok()
                        .filter(|&i| i < bones.len())
                        .ok_or_else(|| invalid("pose matrix index out of range"))?;
                    let vertex = finite(
                        data.vertex_bits
                            .get(binding.vertex_index)
                            .ok_or_else(|| invalid("pose coordinate index out of range"))?
                            .map(f32::from_bits),
                    )?;
                    influences.push(Influence {
                        matrix,
                        vertex_bits: vertex.map(f32::to_bits),
                    });
                }
                ends.push(influences.len() as u32);
            }
        }
        Ok(Self {
            bones,
            influences,
            ends,
        })
    }
    /// Each vertex's influences.
    fn assignments(&self) -> impl Iterator<Item = &[Influence]> {
        let mut start = 0;
        self.ends.iter().map(move |&end| {
            let assignment = &self.influences[start..end as usize];
            start = end as usize;
            assignment
        })
    }
    /// The vertices under `matrices` (one per bone), in assignment order.
    fn skin(&self, matrices: &[Matrix]) -> io::Result<Vec<[f32; 3]>> {
        if matrices.len() != self.bones.len() {
            return Err(invalid("skin matrix count mismatch"));
        }
        let mut positions = Vec::with_capacity(self.ends.len());
        let mut all_finite = true;
        for assignment in self.assignments() {
            let mut position = [0.; 3];
            if let [only] = assignment {
                let v = only.vertex_bits.map(f32::from_bits);
                position = transform3([v[0], v[1], v[2]], &matrices[only.matrix]);
            } else {
                for influence in assignment {
                    let m = &matrices[influence.matrix];
                    let v = influence.vertex_bits.map(f32::from_bits);
                    for (i, sum) in position.iter_mut().enumerate() {
                        let a = m[0][i] * v[0];
                        let b = m[1][i].mul_add(v[1], a);
                        let c = m[2][i].mul_add(v[2], b);
                        *sum += m[3][i].mul_add(v[3], c);
                    }
                }
            }
            all_finite &= position.iter().all(|n| n.is_finite());
            positions.push(position);
        }
        if all_finite {
            Ok(positions)
        } else {
            Err(invalid("nonfinite pose input/output"))
        }
    }
    fn pose(
        &self,
        matrices: Vec<Matrix>,
        profile: PoseProfile,
        arithmetic_profile: &'static str,
    ) -> io::Result<ModelPose> {
        let positions = self
            .skin(&matrices)?
            .into_iter()
            .map(|p| p.map(f32::to_bits))
            .collect();
        Ok(ModelPose {
            profile,
            arithmetic_profile,
            matrix_bits: matrices
                .iter()
                .map(|m| std::array::from_fn(|i| m[i / 4][i % 4].to_bits()))
                .collect(),
            position_bits: positions,
            runtime_verified: false,
        })
    }
    /// Vertex positions skinned with caller-supplied model-space bone
    /// matrices (row-vector layout, one per bone), in assignment order.
    pub fn positions_with(&self, matrices: &[[[f32; 4]; 4]]) -> io::Result<Vec<[f32; 3]>> {
        self.skin(matrices)
    }
    /// Only the vertices of `pose_from_worlds`.
    pub fn positions_from_worlds(
        &self,
        worlds: &[crate::motion_eval::World],
    ) -> io::Result<Vec<[f32; 3]>> {
        let matrices: Vec<Matrix> = worlds
            .iter()
            .take(self.bones.len())
            .map(|w| w.matrix())
            .collect();
        self.skin(&matrices)
    }
    pub fn bind_pose(&self, profile: PoseProfile) -> io::Result<ModelPose> {
        self.pose(
            bind_matrices(&self.bones, profile)?,
            profile,
            "dotnet10_fma_order_std_sincos_candidate_v1",
        )
    }
    /// Explicit basic public FK profile. Scale, IK, root and events are not applied.
    pub fn sample_fk(
        &self,
        motion: &crate::motion::InterpolatedMotion,
        frame: u32,
    ) -> io::Result<ModelPose> {
        if let Some(raw) = &motion.raw {
            let (matrices, _) = raw.sample(frame as f32)?;
            return self.pose(matrices, PoseProfile::Modern, crate::raw_motion::PROFILE);
        }
        let locals = motion.sample_fk(&self.bones, frame)?;
        let mut rotations = Vec::new();
        let mut translations: Vec<[f32; 3]> = Vec::new();
        let mut matrices = Vec::new();
        for (i, (bone, local)) in self.bones.iter().zip(locals).enumerate() {
            let parent = bone.header_i32[1];
            if parent >= 0 && parent as usize >= i {
                return Err(invalid("motion parent must precede child"));
            }
            let (parent_rotation, parent_translation) = if parent < 0 {
                ([0., 0., 0., 1.], [0.; 3])
            } else {
                (rotations[parent as usize], translations[parent as usize])
            };
            let rotated = transform3(local.translation, &quaternion_matrix(parent_rotation));
            let absolute_translation = finite(std::array::from_fn(|axis| {
                parent_translation[axis] + rotated[axis]
            }))?;
            let mut local_rotation = [0., 0., 0., 1.];
            for axis in [2, 1, 0] {
                local_rotation =
                    quaternion_product(local_rotation, axis_quaternion(axis, local.rotation[axis]));
            }
            let absolute_rotation = finite(quaternion_product(parent_rotation, local_rotation))?;
            let matrix = multiply(
                &multiply(&IDENTITY, &quaternion_matrix(absolute_rotation)),
                &translation(absolute_translation),
            );
            for row in matrix {
                finite(row)?;
            }
            rotations.push(absolute_rotation);
            translations.push(absolute_translation);
            matrices.push(matrix);
        }
        self.pose(
            matrices,
            PoseProfile::Legacy,
            "openkh_basic_fk_dotnet10_fma_std_sincos_candidate_v1",
        )
    }
}
impl Skinning {
    /// Skins evaluated joints (one per model bone, e.g. a blend of two
    /// evaluations) the way `sample_evaluated` skins its own.
    pub fn pose_from_worlds(&self, worlds: &[crate::motion_eval::World]) -> io::Result<ModelPose> {
        let matrices = worlds
            .iter()
            .take(self.bones.len())
            .map(|w| w.matrix())
            .collect();
        self.pose(matrices, PoseProfile::Legacy, "motion_eval_blend_v1")
    }

    /// `pose_from_worlds` with the exe's head look-at (look-02 section 1):
    /// `look(origin)` receives the look bone's world origin and returns the
    /// world point to face and the blend weight.
    pub fn pose_from_worlds_look(
        &self,
        worlds: &[crate::motion_eval::World],
        bone: usize,
        look: impl FnOnce([f32; 3]) -> Option<([f32; 3], f32)>,
    ) -> io::Result<ModelPose> {
        let mut matrices: Vec<Matrix> = worlds
            .iter()
            .take(self.bones.len())
            .map(|w| w.matrix())
            .collect();
        if bone < matrices.len() {
            let m = &matrices[bone];
            if let Some((point, weight)) = look([m[3][0], m[3][1], m[3][2]]) {
                let parents: Vec<i32> = self.bones.iter().map(|b| b.header_i32[1]).collect();
                apply_look(&mut matrices, &parents, bone, point, weight);
            }
        }
        self.pose(
            matrices,
            PoseProfile::Legacy,
            "motion_eval_ik_constraints_look_v1",
        )
    }

    /// Full motion evaluation (IK helpers, constraints, IK chains and
    /// expressions; see motion_eval). Returns the skinned pose and the
    /// evaluation, whose root translation drives root motion.
    pub fn sample_evaluated(
        &self,
        motion: &crate::motion::InterpolatedMotion,
        frame: f32,
    ) -> io::Result<(ModelPose, crate::motion_eval::Evaluated)> {
        if let Some(raw) = &motion.raw {
            let (matrices, root_translation) = raw.sample(frame)?;
            let pose = self.pose(matrices, PoseProfile::Modern, crate::raw_motion::PROFILE)?;
            // Raw matrices are already supplied in ModelPose. Local quaternion
            // joints/IK stats are absent, rather than approximate decompositions.
            return Ok((
                pose,
                crate::motion_eval::Evaluated {
                    joints: Vec::new(),
                    root_translation,
                    stats: Default::default(),
                    chains: Vec::new(),
                },
            ));
        }
        let evaluated = crate::motion_eval::evaluate(&self.bones, motion, frame)?;
        let matrices = evaluated.joints[..self.bones.len()]
            .iter()
            .map(|w| w.matrix())
            .collect();
        let pose = self.pose(
            matrices,
            PoseProfile::Legacy,
            "motion_eval_ik_constraints_v1",
        )?;
        Ok((pose, evaluated))
    }
}
fn sub3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn len3(a: [f32; 3]) -> f32 {
    (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt()
}
fn norm3(a: [f32; 3]) -> [f32; 3] {
    let l = len3(a);
    if l > 1e-12 {
        [a[0] / l, a[1] / l, a[2] / l]
    } else {
        a
    }
}
fn cross3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn scale3(a: [f32; 3], s: f32) -> [f32; 3] {
    [a[0] * s, a[1] * s, a[2] * s]
}
/// General 4x4 inverse (Gauss-Jordan); `None` when singular.
fn inverse(m: &Matrix) -> Option<Matrix> {
    let mut a = *m;
    let mut r = IDENTITY;
    for c in 0..4 {
        let pivot = (c..4).max_by(|&i, &j| a[i][c].abs().total_cmp(&a[j][c].abs()))?;
        if a[pivot][c].abs() < 1e-12 {
            return None;
        }
        a.swap(c, pivot);
        r.swap(c, pivot);
        let d = 1. / a[c][c];
        for k in 0..4 {
            a[c][k] *= d;
            r[c][k] *= d;
        }
        for i in 0..4 {
            if i != c {
                let f = a[i][c];
                for k in 0..4 {
                    a[i][k] -= f * a[c][k];
                    r[i][k] -= f * r[c][k];
                }
            }
        }
    }
    Some(r)
}

/// The anb evaluator's look branch (look-02 section 1): the look bone's
/// local forward axis (row 1) turns toward `point` by `weight`, the basis
/// is re-orthonormalised keeping the bone origin, and the change carries
/// to the bone's whole subtree (row-vector matrices, world space).
pub fn apply_look(matrices: &mut [Matrix], parents: &[i32], bone: usize, point: [f32; 3], w: f32) {
    let parent = parents.get(bone).copied().unwrap_or(-1);
    let wp = if parent >= 0 {
        matrices[parent as usize]
    } else {
        IDENTITY
    };
    let (Some(inv_wp), Some(inv_wb)) = (inverse(&wp), inverse(&matrices[bone])) else {
        return;
    };
    let wb = matrices[bone];
    let l = multiply(&wb, &inv_wp);
    let t = [l[3][0], l[3][1], l[3][2]];
    let row = |i: usize| [l[i][0], l[i][1], l[i][2]];
    let (lx, ly, lz) = (row(0), row(1), row(2));
    let (sx, sy, sz) = (len3(lx), len3(ly), len3(lz));
    if sx < 1e-6 || sy < 1e-6 || sz < 1e-6 {
        return;
    }
    let pp = transform3(point, &inv_wp);
    let aim = norm3(sub3(pp, t));
    let y = norm3([
        ly[0] / sy * (1. - w) + aim[0] * w,
        ly[1] / sy * (1. - w) + aim[1] * w,
        ly[2] / sy * (1. - w) + aim[2] * w,
    ]);
    let z = norm3(cross3(norm3(lx), y));
    let x = norm3(cross3(y, z));
    let z = norm3(cross3(x, y));
    let [x, y, z] = [scale3(x, sx), scale3(y, sy), scale3(z, sz)];
    let l2: Matrix = [
        [x[0], x[1], x[2], 0.],
        [y[0], y[1], y[2], 0.],
        [z[0], z[1], z[2], 0.],
        [t[0], t[1], t[2], 1.],
    ];
    let wb2 = multiply(&l2, &wp);
    let c = multiply(&inv_wb, &wb2);
    let mut subtree = vec![false; matrices.len()];
    subtree[bone] = true;
    for i in bone + 1..matrices.len() {
        let p = parents.get(i).copied().unwrap_or(-1);
        if p >= 0 && subtree[p as usize] {
            subtree[i] = true;
            matrices[i] = multiply(&matrices[i], &c);
        }
    }
    matrices[bone] = wb2;
}

#[cfg(test)]
mod look_tests {
    use super::*;

    fn at(x: f32, y: f32, z: f32) -> Matrix {
        let mut m = IDENTITY;
        m[3] = [x, y, z, 1.];
        m
    }

    #[test]
    fn weight_zero_changes_nothing_and_weight_one_aims_the_forward_axis() {
        let parents = [-1, 0, 1];
        let base = vec![at(0., 0., 0.), at(0., 10., 0.), at(0., 5., 0.)];
        let mut m = base.clone();
        apply_look(&mut m, &parents, 1, [0., 10., 100.], 0.);
        for (a, b) in m.iter().zip(&base) {
            for r in 0..4 {
                for c in 0..4 {
                    assert!((a[r][c] - b[r][c]).abs() < 1e-4);
                }
            }
        }
        let mut m = base.clone();
        apply_look(&mut m, &parents, 1, [0., 10., 100.], 1.);
        // Row 1 (forward) now points along +z; the bone origin stays.
        assert!((m[1][1][2] - 1.).abs() < 1e-4, "{:?}", m[1][1]);
        assert!(m[1][3][1] - 10. < 1e-4);
        // The child's origin was carried by the same rotation.
        assert!(m[2][3][0].abs() > 1e-3 || m[2][3][2].abs() > 1e-3);
    }
}

pub fn read_pose(bytes: &[u8], profile: PoseProfile) -> io::Result<ModelPose> {
    Skinning::read(bytes)?.bind_pose(profile)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn bone(parent: i32, scale: f32, x: f32) -> BoneRecord {
        BoneRecord {
            header_i32: [0, parent, 0, 0],
            header_i16: [0, 0, parent as i16, 0, 0, 0, 0, 0],
            transform_bits: [scale, scale, scale, 0., 0., 0., 0., 0., x, 0., 0., 0.]
                .map(f32::to_bits),
        }
    }
    fn skinning(bones: Vec<BoneRecord>, assignments: Vec<Vec<Influence>>) -> Skinning {
        let mut ends = Vec::new();
        let mut influences = Vec::new();
        for assignment in assignments {
            influences.extend(assignment);
            ends.push(influences.len() as u32);
        }
        Skinning {
            bones,
            influences,
            ends,
        }
    }
    #[test]
    fn weighted_vertices_sum_as_the_full_transform_does() {
        let v = [1.5, -2.25, 0.125, 0.75].map(f32::to_bits);
        let w = [-3.5, 0.5, 7., 0.25].map(f32::to_bits);
        let skin = skinning(
            vec![bone(-1, 1., 10.), bone(0, 1., 1.)],
            vec![vec![
                Influence {
                    matrix: 0,
                    vertex_bits: v,
                },
                Influence {
                    matrix: 1,
                    vertex_bits: w,
                },
            ]],
        );
        let turn = |k: f32| {
            let mut m = IDENTITY;
            m[0] = [k.cos(), k.sin(), 0.1, 0.];
            m[1] = [-k.sin(), k.cos(), 0.2, 0.];
            m[3] = [k, 2. * k, 3. * k, 1.];
            m
        };
        let matrices = [turn(0.3), turn(1.7)];
        let a = transform4(v.map(f32::from_bits), &matrices[0]);
        let b = transform4(w.map(f32::from_bits), &matrices[1]);
        let expected: [f32; 3] = std::array::from_fn(|i| 0. + a[i] + b[i]);
        let got = skin.positions_with(&matrices).unwrap();
        assert_eq!(got[0].map(f32::to_bits), expected.map(f32::to_bits));
    }
    #[test]
    fn skinning_serializes_as_one_list_per_vertex() {
        let skin = skinning(
            Vec::new(),
            vec![
                vec![Influence {
                    matrix: 1,
                    vertex_bits: [1, 2, 3, 4],
                }],
                vec![
                    Influence {
                        matrix: 0,
                        vertex_bits: [5, 6, 7, 8],
                    },
                    Influence {
                        matrix: 2,
                        vertex_bits: [9, 10, 11, 12],
                    },
                ],
            ],
        );
        assert_eq!(
            serde_json::to_value(&skin).unwrap(),
            serde_json::json!({"bones": [], "assignments": [
                [{"matrix": 1, "vertex_bits": [1, 2, 3, 4]}],
                [{"matrix": 0, "vertex_bits": [5, 6, 7, 8]},
                 {"matrix": 2, "vertex_bits": [9, 10, 11, 12]}],
            ]})
        );
    }
    #[test]
    fn retained_skinning_reuses_single_and_weighted_bindings_without_packets() {
        let skin = skinning(
            vec![bone(-1, 1., 10.), bone(0, 1., 1.)],
            vec![
                vec![Influence {
                    matrix: 1,
                    vertex_bits: [2., 0., 0., 0.].map(f32::to_bits),
                }],
                vec![
                    Influence {
                        matrix: 0,
                        vertex_bits: [1., 0., 0., 0.5].map(f32::to_bits),
                    },
                    Influence {
                        matrix: 1,
                        vertex_bits: [2., 0., 0., 0.5].map(f32::to_bits),
                    },
                ],
            ],
        );
        let pose = skin.bind_pose(PoseProfile::Legacy).unwrap();
        assert_eq!(pose.position_bits[0].map(f32::from_bits), [13., 0., 0.]);
        assert_eq!(pose.position_bits[1].map(f32::from_bits), [13.5, 0., 0.]);
        assert!(skin.pose(vec![], PoseProfile::Legacy, "authored").is_err());
        assert_eq!(
            skin.bind_pose(PoseProfile::Legacy).unwrap().position_bits,
            pose.position_bits
        );
    }
    #[test]
    fn raw_source_matrices_skin_directly_without_fabricated_parent_or_ik_poses() {
        let skin = skinning(
            vec![bone(-1, 1., 100.), bone(0, 1., 200.)],
            vec![vec![Influence {
                matrix: 1,
                vertex_bits: [2., 0., 0., 0.].map(f32::to_bits),
            }]],
        );
        let mut bytes = vec![0; 0x90 + 224];
        let body = &mut bytes[0x90..];
        body[..4].copy_from_slice(&1u32.to_le_bytes());
        body[8..12].copy_from_slice(&224u32.to_le_bytes());
        for (at, value) in [(16, 2u32), (32, 0), (36, 1), (40, 96)] {
            body[at..at + 4].copy_from_slice(&value.to_le_bytes());
        }
        for (matrix, x) in [10f32, 20.].into_iter().enumerate() {
            let at = 96 + matrix * 64;
            for (word, value) in [(0, 1f32), (5, 1.), (10, 1.), (15, 1.), (12, x)] {
                body[at + word * 4..at + word * 4 + 4].copy_from_slice(&value.to_le_bytes());
            }
        }
        let motion = crate::motion::read_interpolated(&bytes).unwrap();
        assert!(motion.sample_fk(&skin.bones, 0).is_err());
        assert!(crate::motion_eval::evaluate(&skin.bones, &motion, 0.).is_err());
        let (pose, metadata) = skin.sample_evaluated(&motion, 0.).unwrap();
        assert_eq!(pose.position_bits[0].map(f32::from_bits), [22., 0., 0.]);
        assert_eq!(pose.matrix_bits[1][12], 20f32.to_bits());
        assert!(metadata.joints.is_empty());
        assert_eq!(
            skin.sample_fk(&motion, 0).unwrap().position_bits,
            pose.position_bits
        );
    }
    #[test]
    fn distinct_parent_scale_routes_and_homogeneous_weights() {
        let b = [bone(-1, 2., 10.), bone(0, 1., 1.)];
        let legacy = bind_matrices(&b, PoseProfile::Legacy).unwrap();
        let modern = bind_matrices(&b, PoseProfile::Modern).unwrap();
        assert_eq!(legacy[1][3][0], 11.);
        assert_eq!(modern[1][3][0], 12.);
        assert_eq!(transform3([2., 0., 0.], &legacy[1])[0], 13.);
        assert_eq!(transform3([2., 0., 0.], &modern[1])[0], 16.);
        assert_eq!(
            transform4([1., 0., 0., 0.5], &legacy[0])[0]
                + transform4([2., 0., 0., 0.5], &legacy[1])[0],
            13.5
        );
        assert_eq!(
            transform4([1., 0., 0., 0.5], &modern[0])[0]
                + transform4([2., 0., 0., 0.5], &modern[1])[0],
            17.
        );
    }
    #[test]
    fn fused_vector_order_retains_small_cancellation_result() {
        let mut m = [[0.; 4]; 4];
        m[0][0] = -f32::from_bits(0x3f800002);
        m[1][0] = f32::from_bits(0x3f800001);
        let result = transform4([1., f32::from_bits(0x3f800001), 0., 0.], &m);
        assert_eq!(result[0].to_bits(), 0x28800000);
    }
    #[test]
    fn quaternion_order_and_matrix_preserve_authored_axis_permutation() {
        let a = [0.5; 4];
        let b = [0., 0., 1., 0.];
        assert_eq!(quaternion_product(a, b), [0.5, -0.5, 0.5, -0.5]);
        assert_eq!(quaternion_product(b, a), [-0.5, 0.5, 0.5, -0.5]);
        assert_eq!(
            transform3([1., 2., 3.], &quaternion_matrix(a)),
            [3., 1., 2.]
        );
    }
    #[test]
    fn rejects_overflow_and_unknown_profiles_without_substitution() {
        assert!(
            bind_matrices(
                &[bone(-1, 1., f32::MAX), bone(0, 1., f32::MAX)],
                PoseProfile::Legacy
            )
            .is_err()
        );
        assert!(
            bind_matrices(
                &[bone(-1, f32::MAX, 0.), bone(0, f32::MAX, 0.)],
                PoseProfile::Modern
            )
            .is_err()
        );
        assert!(serde_json::from_str::<PoseProfile>("\"unknown\"").is_err());
        assert!(read_pose(&[], PoseProfile::Legacy).is_err());
    }
    #[test]
    fn rejects_forward_parents_nonfinite_and_oversized_bone_arrays() {
        for profile in [PoseProfile::Legacy, PoseProfile::Modern] {
            assert!(bind_matrices(&[bone(0, 1., 0.)], profile).is_err());
            assert!(bind_matrices(&[bone(-1, f32::NAN, 0.)], profile).is_err());
            let bones: Vec<_> = (0..2049).map(|_| bone(-1, 1., 0.)).collect();
            assert!(bind_matrices(&bones, profile).is_err());
        }
    }
}
