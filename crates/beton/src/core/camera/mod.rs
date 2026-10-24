//! View and projection.

use crate::core::math::{Mat4, Vec3};

/// How the world is projected into the window.
#[derive(Clone, Copy, Debug)]
pub enum Projection {
    /// Visible world height. Width comes from the window aspect.
    Orthographic {
        /// Visible world height.
        height: f32,
        /// Near plane distance along the view forward.
        near: f32,
        /// Far plane distance along the view forward.
        far: f32,
    },
    /// Vertical field of view in radians.
    Perspective {
        /// Vertical field of view in radians.
        fov_y: f32,
        /// Near plane distance along the view forward.
        near: f32,
        /// Far plane distance along the view forward.
        far: f32,
    },
}

/// A camera in world space.
#[derive(Clone, Debug)]
pub struct Camera {
    /// Eye position.
    pub eye: Vec3,
    /// Point the camera looks at.
    pub target: Vec3,
    /// World up, usually +Y.
    pub up: Vec3,
    /// Orthographic or perspective.
    pub projection: Projection,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            eye: Vec3::new(0.0, 0.0, 10.0),
            target: Vec3::ZERO,
            up: Vec3::Y,
            projection: Projection::Orthographic {
                height: 20.0,
                near: 0.1,
                far: 100.0,
            },
        }
    }
}

impl Camera {
    /// Combined view-projection matrix for the given window aspect (`width / height`).
    pub fn view_proj(&self, aspect: f32) -> Mat4 {
        let aspect = if aspect <= 0.0 { 1.0 } else { aspect };
        let view = Mat4::look_at_rh(self.eye, self.target, self.up);
        let proj = match self.projection {
            Projection::Orthographic { height, near, far } => {
                Mat4::orthographic_rh(height, aspect, near, far)
            }
            Projection::Perspective { fov_y, near, far } => {
                Mat4::perspective_rh(fov_y, aspect, near, far)
            },
        };
        proj.mul(view)
    }

    /// Map a framebuffer pixel (origin top-left, Y down) to a world-space XY point
    /// on the camera's look plane. Orthographic only; perspective returns the target XY.
    pub fn screen_to_world(&self, px: [f32; 2], fb_w: f32, fb_h: f32) -> Vec3 {
        let fb_w = fb_w.max(1.0);
        let fb_h = fb_h.max(1.0);
        let ndc_x = (px[0] / fb_w) * 2.0 - 1.0;
        let ndc_y = 1.0 - (px[1] / fb_h) * 2.0;
        let aspect = fb_w / fb_h;
        match self.projection {
            Projection::Orthographic { height, .. } => {
                let half_h = height * 0.5;
                let half_w = half_h * aspect;
                // Default camera looks from +Z at origin: screen XY ≈ world XY.
                Vec3::new(
                    self.target.x + ndc_x * half_w,
                    self.target.y + ndc_y * half_h,
                    self.target.z,
                )
            }
            Projection::Perspective { .. } => self.target,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn orthographic_origin_maps_to_ndc_center() {
        let camera = Camera {
            eye: Vec3::new(0.0, 0.0, 10.0),
            target: Vec3::ZERO,
            up: Vec3::Y,
            projection: Projection::Orthographic {
                height: 20.0,
                near: 0.1,
                far: 100.0,
            },
        };
        let clip = camera.view_proj(1.0).project_point(Vec3::ZERO);
        assert!(clip.x.abs() < 1e-4, "x = {}", clip.x);
        assert!(clip.y.abs() < 1e-4, "y = {}", clip.y);
        assert!(clip.z > 0.0 && clip.z < 1.0, "z = {}", clip.z);
    }

    #[test]
    fn perspective_point_on_look_axis_maps_to_ndc_center() {
        let camera = Camera {
            eye: Vec3::new(0.0, 0.0, 10.0),
            target: Vec3::ZERO,
            up: Vec3::Y,
            projection: Projection::Perspective {
                fov_y: std::f32::consts::FRAC_PI_2,
                near: 0.1,
                far: 100.0,
            },
        };
        // A point between the eye and the target, on the look axis.
        let point = Vec3::new(0.0, 0.0, 5.0);
        let clip = camera.view_proj(1.0).project_point(point);
        assert!(clip.x.abs() < 1e-4, "x = {}", clip.x);
        assert!(clip.y.abs() < 1e-4, "y = {}", clip.y);
        assert!(clip.z > 0.0 && clip.z < 1.0, "z = {}", clip.z);
    }
}
