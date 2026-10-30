//! The exe's vector unit conventions (PS2 libvu0 kept on PC): a matrix is
//! four columns (x, y, z axes, translation), `mul(a, b)` = a·b applies b
//! first, `apply(m, v)` = m·v. Angles are integers with 0x10000 = 360°
//! looked up in a 4096-entry sine table (index = angle >> 4).
pub type Vec4 = [f32; 4];
pub type Mat = [Vec4; 4];

pub const IDENTITY: Mat = [
    [1., 0., 0., 0.],
    [0., 1., 0., 0.],
    [0., 0., 1., 0.],
    [0., 0., 0., 1.],
];

pub fn apply(m: &Mat, v: Vec4) -> Vec4 {
    std::array::from_fn(|r| m[0][r] * v[0] + m[1][r] * v[1] + m[2][r] * v[2] + m[3][r] * v[3])
}

pub fn mul(a: &Mat, b: &Mat) -> Mat {
    std::array::from_fn(|c| apply(a, b[c]))
}

/// FUN_140478ce0: the inverse (rows hold the basis, row 3 the translation).
pub fn inverse(m: &Mat) -> Mat {
    let a: [f32; 16] = std::array::from_fn(|i| m[i / 4][i % 4]);
    let mut inv = [0f32; 16];
    inv[0] = a[5] * a[10] * a[15] - a[5] * a[11] * a[14] - a[9] * a[6] * a[15]
        + a[9] * a[7] * a[14]
        + a[13] * a[6] * a[11]
        - a[13] * a[7] * a[10];
    inv[4] = -a[4] * a[10] * a[15] + a[4] * a[11] * a[14] + a[8] * a[6] * a[15]
        - a[8] * a[7] * a[14]
        - a[12] * a[6] * a[11]
        + a[12] * a[7] * a[10];
    inv[8] = a[4] * a[9] * a[15] - a[4] * a[11] * a[13] - a[8] * a[5] * a[15]
        + a[8] * a[7] * a[13]
        + a[12] * a[5] * a[11]
        - a[12] * a[7] * a[9];
    inv[12] = -a[4] * a[9] * a[14] + a[4] * a[10] * a[13] + a[8] * a[5] * a[14]
        - a[8] * a[6] * a[13]
        - a[12] * a[5] * a[10]
        + a[12] * a[6] * a[9];
    inv[1] = -a[1] * a[10] * a[15] + a[1] * a[11] * a[14] + a[9] * a[2] * a[15]
        - a[9] * a[3] * a[14]
        - a[13] * a[2] * a[11]
        + a[13] * a[3] * a[10];
    inv[5] = a[0] * a[10] * a[15] - a[0] * a[11] * a[14] - a[8] * a[2] * a[15]
        + a[8] * a[3] * a[14]
        + a[12] * a[2] * a[11]
        - a[12] * a[3] * a[10];
    inv[9] = -a[0] * a[9] * a[15] + a[0] * a[11] * a[13] + a[8] * a[1] * a[15]
        - a[8] * a[3] * a[13]
        - a[12] * a[1] * a[11]
        + a[12] * a[3] * a[9];
    inv[13] = a[0] * a[9] * a[14] - a[0] * a[10] * a[13] - a[8] * a[1] * a[14]
        + a[8] * a[2] * a[13]
        + a[12] * a[1] * a[10]
        - a[12] * a[2] * a[9];
    inv[2] = a[1] * a[6] * a[15] - a[1] * a[7] * a[14] - a[5] * a[2] * a[15]
        + a[5] * a[3] * a[14]
        + a[13] * a[2] * a[7]
        - a[13] * a[3] * a[6];
    inv[6] = -a[0] * a[6] * a[15] + a[0] * a[7] * a[14] + a[4] * a[2] * a[15]
        - a[4] * a[3] * a[14]
        - a[12] * a[2] * a[7]
        + a[12] * a[3] * a[6];
    inv[10] = a[0] * a[5] * a[15] - a[0] * a[7] * a[13] - a[4] * a[1] * a[15]
        + a[4] * a[3] * a[13]
        + a[12] * a[1] * a[7]
        - a[12] * a[3] * a[5];
    inv[14] = -a[0] * a[5] * a[14] + a[0] * a[6] * a[13] + a[4] * a[1] * a[14]
        - a[4] * a[2] * a[13]
        - a[12] * a[1] * a[6]
        + a[12] * a[2] * a[5];
    inv[3] = -a[1] * a[6] * a[11] + a[1] * a[7] * a[10] + a[5] * a[2] * a[11]
        - a[5] * a[3] * a[10]
        - a[9] * a[2] * a[7]
        + a[9] * a[3] * a[6];
    inv[7] = a[0] * a[6] * a[11] - a[0] * a[7] * a[10] - a[4] * a[2] * a[11]
        + a[4] * a[3] * a[10]
        + a[8] * a[2] * a[7]
        - a[8] * a[3] * a[6];
    inv[11] = -a[0] * a[5] * a[11] + a[0] * a[7] * a[9] + a[4] * a[1] * a[11]
        - a[4] * a[3] * a[9]
        - a[8] * a[1] * a[7]
        + a[8] * a[3] * a[5];
    inv[15] = a[0] * a[5] * a[10] - a[0] * a[6] * a[9] - a[4] * a[1] * a[10]
        + a[4] * a[2] * a[9]
        + a[8] * a[1] * a[6]
        - a[8] * a[2] * a[5];
    let det = a[0] * inv[0] + a[1] * inv[4] + a[2] * inv[8] + a[3] * inv[12];
    let k = if det != 0. { 1. / det } else { 0. };
    std::array::from_fn(|r| std::array::from_fn(|c| inv[r * 4 + c] * k))
}

pub fn scale(v: Vec4, s: f32) -> Vec4 {
    v.map(|x| x * s)
}

/// FUN_140478d80: xyz normalised (w kept).
pub fn normalize(v: Vec4) -> Vec4 {
    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if l > 0. {
        [v[0] / l, v[1] / l, v[2] / l, v[3]]
    } else {
        v
    }
}

fn table_sin(angle: i32) -> f32 {
    let index = ((angle as i64 >> 4) & 0xfff) as f32;
    (index * std::f32::consts::TAU / 4096.).sin()
}

/// (sin, cos) of an integer angle (FUN_1404dc860 and siblings).
pub fn sin_cos(angle: i32) -> (f32, f32) {
    (table_sin(angle), table_sin(angle.wrapping_add(0x4000)))
}

pub fn rot_x(angle: i32) -> Mat {
    let (s, c) = sin_cos(angle);
    [
        [1., 0., 0., 0.],
        [0., c, s, 0.],
        [0., -s, c, 0.],
        [0., 0., 0., 1.],
    ]
}

pub fn rot_y(angle: i32) -> Mat {
    let (s, c) = sin_cos(angle);
    [
        [c, 0., -s, 0.],
        [0., 1., 0., 0.],
        [s, 0., c, 0.],
        [0., 0., 0., 1.],
    ]
}

pub fn rot_z(angle: i32) -> Mat {
    let (s, c) = sin_cos(angle);
    [
        [c, s, 0., 0.],
        [-s, c, 0., 0.],
        [0., 0., 1., 0.],
        [0., 0., 0., 1.],
    ]
}

/// FUN_1404be820: Rz·Ry·Rx from integer angles.
pub fn rot_xyz(a: [i32; 3]) -> Mat {
    mul(&mul(&rot_z(a[2]), &rot_y(a[1])), &rot_x(a[0]))
}

/// FUN_1404e01f0: Ry·Rx·Rz from integer angles (Z applied first).
pub fn rot_zxy(a: [i32; 3]) -> Mat {
    mul(&mul(&rot_y(a[1]), &rot_x(a[0])), &rot_z(a[2]))
}

/// FUN_1404e0150: Rz·Rx·Ry from integer angles (Y applied first).
pub fn rot_yxz(a: [i32; 3]) -> Mat {
    mul(&mul(&rot_z(a[2]), &rot_x(a[0])), &rot_y(a[1]))
}

/// FUN_1404e00b0 (pppMatrixYZX): Rx·Rz·Ry from integer angles (Y applied
/// first, then Z, then X).
pub fn rot_yzx(a: [i32; 3]) -> Mat {
    mul(&mul(&rot_x(a[0]), &rot_z(a[2])), &rot_y(a[1]))
}

/// FUN_14013f3a0 (FUN_140478ce0): the inverse of a rigid matrix, the 3×3
/// part transposed and the translation −Rᵀt. The exe's copy keeps the
/// input's last w.
pub fn rigid_inverse(m: &Mat) -> Mat {
    let mut out = IDENTITY;
    for (c, col) in out.iter_mut().take(3).enumerate() {
        for (r, x) in col.iter_mut().take(3).enumerate() {
            *x = m[r][c];
        }
    }
    let rt = apply(&out, [m[3][0], m[3][1], m[3][2], m[3][3]]);
    out[3] = [-rt[0], -rt[1], -rt[2], rt[3]];
    out
}

/// Float-radian rotations (instance matrix, FUN_1404b1280 → libvu0 RotMatrixX/Y/Z).
pub fn rot_radians(m: &Mat, r: [f32; 3]) -> Mat {
    let rx = {
        let (s, c) = r[0].sin_cos();
        [
            [1., 0., 0., 0.],
            [0., c, s, 0.],
            [0., -s, c, 0.],
            [0., 0., 0., 1.],
        ]
    };
    let ry = {
        let (s, c) = r[1].sin_cos();
        [
            [c, 0., -s, 0.],
            [0., 1., 0., 0.],
            [s, 0., c, 0.],
            [0., 0., 0., 1.],
        ]
    };
    let rz = {
        let (s, c) = r[2].sin_cos();
        [
            [c, s, 0., 0.],
            [-s, c, 0., 0.],
            [0., 0., 1., 0.],
            [0., 0., 0., 1.],
        ]
    };
    mul(&rz, &mul(&ry, &mul(&rx, m)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn column_convention_and_rotations() {
        let t = [
            [1., 0., 0., 0.],
            [0., 1., 0., 0.],
            [0., 0., 1., 0.],
            [5., 6., 7., 1.],
        ];
        assert_eq!(apply(&t, [1., 1., 1., 1.]), [6., 7., 8., 1.]);
        // A quarter turn about Z takes +X to +Y.
        let p = apply(&rot_z(0x4000), [1., 0., 0., 1.]);
        assert!((p[0]).abs() < 1e-6 && (p[1] - 1.).abs() < 1e-6);
        // mul applies the right operand first.
        let m = mul(&t, &rot_z(0x4000));
        let q = apply(&m, [1., 0., 0., 1.]);
        assert!((q[0] - 5.).abs() < 1e-6 && (q[1] - 7.).abs() < 1e-6);
    }
}
