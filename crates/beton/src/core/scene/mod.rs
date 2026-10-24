//! Immediate-mode sprites and bitmap text.

use crate::core::math::Vec3;
use crate::core::render::TextureId;

/// A textured quad in world space.
#[derive(Clone, Debug)]
pub struct Sprite {
    /// Texture to sample.
    pub texture: TextureId,
    /// World-space center.
    pub center: Vec3,
    /// Width and height in world units.
    pub size: [f32; 2],
    /// Rotation around the center, in radians. Positive turns a right-facing sprite nose-up.
    pub rotation: f32,
    /// UV rectangle: min x, min y, max x, max y. `[0, 0, 1, 1]` is the whole texture.
    pub uv: [f32; 4],
    /// Multiplied with the texel. White leaves the texture unchanged.
    pub color: [f32; 4],
}

impl Sprite {
    /// Full UV, no rotation, and white tint.
    pub fn new(texture: TextureId, center: Vec3, size: [f32; 2]) -> Self {
        Self {
            texture,
            center,
            size,
            rotation: 0.0,
            uv: [0.0, 0.0, 1.0, 1.0],
            color: [1.0, 1.0, 1.0, 1.0],
        }
    }
}

/// A fixed grid of glyphs in one texture.
#[derive(Clone, Debug)]
pub struct Font {
    /// Atlas texture.
    pub texture: TextureId,
    /// Glyphs across.
    pub columns: u32,
    /// Glyphs down.
    pub rows: u32,
    /// Character at grid index 0.
    pub first_char: char,
}

/// Sprites drawn this frame. Cleared by the engine before each `update`.
#[derive(Default)]
pub struct Scene {
    pub(crate) sprites: Vec<Sprite>,
}

impl Scene {
    pub(crate) fn clear(&mut self) {
        self.sprites.clear();
    }

    /// Queue a sprite for this frame.
    pub fn push(&mut self, sprite: Sprite) {
        self.sprites.push(sprite);
    }

    /// Draw `text` as one sprite per glyph, advancing by `glyph_size[0]` on X.
    pub fn text(
        &mut self,
        font: &Font,
        origin: Vec3,
        glyph_size: [f32; 2],
        color: [f32; 4],
        text: &str,
    ) {
        if font.columns == 0 || font.rows == 0 {
            return;
        }
        let cell_u = 1.0 / font.columns as f32;
        let cell_v = 1.0 / font.rows as f32;
        let first = font.first_char as u32;
        let mut x = origin.x;
        for ch in text.chars() {
            let index = ch as u32 as i32 - first as i32;
            if index >= 0 {
                let index = index as u32;
                let max = font.columns.saturating_mul(font.rows);
                if index < max {
                    let col = index % font.columns;
                    let row = index / font.columns;
                    // Inset half a texel's worth of the cell so neighbors do not bleed in.
                    let pad_u = cell_u * 0.02;
                    let pad_v = cell_v * 0.02;
                    let u0 = col as f32 * cell_u + pad_u;
                    let v0 = row as f32 * cell_v + pad_v;
                    let u1 = (col as f32 + 1.0) * cell_u - pad_u;
                    let v1 = (row as f32 + 1.0) * cell_v - pad_v;
                    self.sprites.push(Sprite {
                        texture: font.texture,
                        center: Vec3::new(x + glyph_size[0] * 0.5, origin.y, origin.z),
                        size: glyph_size,
                        rotation: 0.0,
                        uv: [u0, v0, u1, v1],
                        color,
                    });
                }
            }
            x += glyph_size[0];
        }
    }

    pub(crate) fn take_sorted(&mut self) -> Vec<Sprite> {
        let mut sprites = std::mem::take(&mut self.sprites);
        sprites.sort_by(|a, b| {
            a.center
                .z
                .partial_cmp(&b.center.z)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        sprites
    }
}
