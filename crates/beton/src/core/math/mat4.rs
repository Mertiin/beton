//! Column-major 4x4 matrix for camera transforms.

use super::Vec3;

/// A column-major 4x4 matrix. Matches `wgpu` uniform layout.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mat4(pub [[f32; 4]; 4]);

impl Mat4 {
    /// The identity matrix.
    pub const IDENTITY: Self = Self([
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ]);

    /// Right-handed look-at. Camera looks from `eye` toward `target`. Forward is -Z in view space.
    pub fn look_at_rh(eye: Vec3, target: Vec3, up: Vec3) -> Self {
        let f = (target - eye).normalize();
        let s = f.cross(up).normalize();
        let u = s.cross(f);

        Self([
            [s.x, u.x, -f.x, 0.0],
            [s.y, u.y, -f.y, 0.0],
            [s.z, u.z, -f.z, 0.0],
            [-s.dot(eye), -u.dot(eye), f.dot(eye), 1.0],
        ])
    }

    /// Orthographic projection into clip space with z in `[0, 1]`.
    ///
    /// `height` is the visible world height. `aspect` is width / height.
    /// `near` and `far` are positive distances along the view forward (-Z).
    pub fn orthographic_rh(height: f32, aspect: f32, near: f32, far: f32) -> Self {
        let width = height * aspect;
        let left = -width * 0.5;
        let right = width * 0.5;
        let bottom = -height * 0.5;
        let top = height * 0.5;
        let rl = right - left;
        let tb = top - bottom;
        let fn_ = far - near;

        Self([
            [2.0 / rl, 0.0, 0.0, 0.0],
            [0.0, 2.0 / tb, 0.0, 0.0],
            [0.0, 0.0, -1.0 / fn_, 0.0],
            [
                -(right + left) / rl,
                -(top + bottom) / tb,
                -near / fn_,
                1.0,
            ],
        ])
    }

    /// Perspective projection into clip space with z in `[0, 1]`.
    ///
    /// `fov_y` is the vertical field of view in radians.
    pub fn perspective_rh(fov_y: f32, aspect: f32, near: f32, far: f32) -> Self {
        let f = 1.0 / (fov_y * 0.5).tan();
        let nf = 1.0 / (near - far);

        Self([
            [f / aspect, 0.0, 0.0, 0.0],
            [0.0, f, 0.0, 0.0],
            [0.0, 0.0, far * nf, -1.0],
            [0.0, 0.0, near * far * nf, 0.0],
        ])
    }

    /// Matrix product `self * other`.
    pub fn mul(self, other: Self) -> Self {
        let mut out = [[0.0f32; 4]; 4];
        for col in 0..4 {
            for row in 0..4 {
                out[col][row] = self.0[0][row] * other.0[col][0]
                    + self.0[1][row] * other.0[col][1]
                    + self.0[2][row] * other.0[col][2]
                    + self.0[3][row] * other.0[col][3];
            }
        }
        Self(out)
    }

    /// Transform a point (`w = 1`) and divide by the resulting `w`.
    pub fn project_point(self, p: Vec3) -> Vec3 {
        let x = self.0[0][0] * p.x + self.0[1][0] * p.y + self.0[2][0] * p.z + self.0[3][0];
        let y = self.0[0][1] * p.x + self.0[1][1] * p.y + self.0[2][1] * p.z + self.0[3][1];
        let z = self.0[0][2] * p.x + self.0[1][2] * p.y + self.0[2][2] * p.z + self.0[3][2];
        let w = self.0[0][3] * p.x + self.0[1][3] * p.y + self.0[2][3] * p.z + self.0[3][3];
        if w == 0.0 {
            Vec3::new(x, y, z)
        } else {
            Vec3::new(x / w, y / w, z / w)
        }
    }

    /// Flat column-major bytes for a uniform buffer.
    pub fn as_bytes(&self) -> &[u8] {
        bytemuck::bytes_of(&self.0)
    }
}
