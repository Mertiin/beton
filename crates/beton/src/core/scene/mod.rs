//! Immediate-mode sprites and bitmap text.

use std::collections::HashMap;
use std::path::Path;

use crate::core::math::Vec3;
use crate::core::render::{Gfx, TextureId};

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

/// One baked glyph in a TTF atlas.
#[derive(Clone, Copy, Debug)]
struct Glyph {
    uv: [f32; 4],
    /// Bitmap size in atlas pixels.
    size: [f32; 2],
    /// `fontdue` xmin (left bearing).
    xmin: f32,
    /// `fontdue` ymin (bitmap bottom relative to baseline).
    ymin: f32,
    /// Advance width in atlas pixels.
    advance: f32,
}

#[derive(Clone, Debug)]
enum FontKind {
    /// Fixed grid atlas (legacy digit strips, etc.).
    Grid {
        columns: u32,
        rows: u32,
        first_char: char,
    },
    /// Glyphs packed from a TrueType / OpenType face.
    Atlas {
        glyphs: HashMap<char, Glyph>,
        /// Pixel size used when baking (`px` passed to [`Font::from_ttf`]).
        px: f32,
    },
}

/// A font atlas: either a fixed grid or a baked TTF.
#[derive(Clone, Debug)]
pub struct Font {
    /// Atlas texture.
    pub texture: TextureId,
    kind: FontKind,
}

impl Font {
    /// Fixed grid of equal cells. Character at index 0 is `first_char`.
    pub fn grid(texture: TextureId, columns: u32, rows: u32, first_char: char) -> Self {
        Self {
            texture,
            kind: FontKind::Grid {
                columns,
                rows,
                first_char,
            },
        }
    }

    /// Bake a TrueType / OpenType face into an atlas and upload it.
    ///
    /// `px` is the pixel height used for rasterization. Pass the same idea to
    /// [`Scene::text`] / UI text as `glyph_size[1]`. Includes printable ASCII (32..=126).
    pub fn from_ttf(gfx: &mut Gfx, ttf: &[u8], px: f32) -> Result<Self, String> {
        let baked = bake_ttf(ttf, px, 512)?;
        let texture = gfx.create_texture(baked.width, baked.height, &baked.rgba)?;
        Ok(Self {
            texture,
            kind: FontKind::Atlas {
                glyphs: baked.glyphs,
                px: baked.px,
            },
        })
    }

    /// Load a `.ttf` / `.otf` path (uses preloaded [`crate::Game::assets`] bytes when present).
    pub fn load(gfx: &mut Gfx, path: &Path, px: f32) -> Result<Self, String> {
        let bytes = gfx.read_bytes(path)?;
        Self::from_ttf(gfx, &bytes, px)
    }

    /// Width and height of `text` when drawn at `glyph_size`.
    ///
    /// Grid fonts: `glyph_size` is the cell size. Atlas fonts: `glyph_size[1]` is the
    /// desired em height; `glyph_size[0]` is ignored.
    pub fn measure(&self, glyph_size: [f32; 2], text: &str) -> [f32; 2] {
        match &self.kind {
            FontKind::Grid { .. } => {
                let n = text.chars().count() as f32;
                [n * glyph_size[0], glyph_size[1]]
            }
            FontKind::Atlas { glyphs, px } => {
                let scale = if *px > 0.0 {
                    glyph_size[1] / px
                } else {
                    1.0
                };
                let mut w = 0.0f32;
                for ch in text.chars() {
                    if let Some(g) = glyphs.get(&ch) {
                        w += g.advance * scale;
                    } else if ch == ' ' {
                        w += glyph_size[1] * 0.33;
                    } else {
                        w += glyph_size[1] * 0.5;
                    }
                }
                [w, glyph_size[1]]
            }
        }
    }

    /// Emit glyphs for drawing. `offset` is top-left relative to pen at (0,0) with **Y down**.
    pub(crate) fn emit_glyphs_y_down(
        &self,
        text: &str,
        glyph_size: [f32; 2],
        mut emit: impl FnMut([f32; 4], [f32; 2], [f32; 2]),
    ) {
        match &self.kind {
            FontKind::Grid {
                columns,
                rows,
                first_char,
            } => {
                if *columns == 0 || *rows == 0 {
                    return;
                }
                let cell_u = 1.0 / *columns as f32;
                let cell_v = 1.0 / *rows as f32;
                let first = *first_char as u32;
                let mut x = 0.0f32;
                for ch in text.chars() {
                    let index = ch as u32 as i32 - first as i32;
                    if index >= 0 {
                        let index = index as u32;
                        let max = columns.saturating_mul(*rows);
                        if index < max {
                            let col = index % *columns;
                            let row = index / *columns;
                            let pad_u = cell_u * 0.02;
                            let pad_v = cell_v * 0.02;
                            let uv = [
                                col as f32 * cell_u + pad_u,
                                row as f32 * cell_v + pad_v,
                                (col as f32 + 1.0) * cell_u - pad_u,
                                (row as f32 + 1.0) * cell_v - pad_v,
                            ];
                            // Top-left of the cell in Y-down pen space.
                            emit(uv, [x, 0.0], glyph_size);
                        }
                    }
                    x += glyph_size[0];
                }
            }
            FontKind::Atlas { glyphs, px } => {
                let scale = if *px > 0.0 {
                    glyph_size[1] / px
                } else {
                    1.0
                };
                // Baseline near the bottom of the em box so caps sit in the middle.
                let baseline = glyph_size[1] * 0.8;
                let mut pen_x = 0.0f32;
                for ch in text.chars() {
                    if let Some(g) = glyphs.get(&ch) {
                        let w = g.size[0] * scale;
                        let h = g.size[1] * scale;
                        let left = pen_x + g.xmin * scale;
                        let top = baseline - (g.ymin + g.size[1]) * scale;
                        if w > 0.0 && h > 0.0 {
                            emit(g.uv, [left, top], [w, h]);
                        }
                        pen_x += g.advance * scale;
                    } else if ch == ' ' {
                        pen_x += glyph_size[1] * 0.33;
                    } else {
                        pen_x += glyph_size[1] * 0.5;
                    }
                }
            }
        }
    }
}

struct BakedFont {
    width: u32,
    height: u32,
    rgba: Vec<u8>,
    glyphs: HashMap<char, Glyph>,
    px: f32,
}

fn bake_ttf(ttf: &[u8], px: f32, atlas_w: u32) -> Result<BakedFont, String> {
    let px = px.max(1.0);
    let settings = fontdue::FontSettings {
        scale: px,
        ..fontdue::FontSettings::default()
    };
    let font = fontdue::Font::from_bytes(ttf, settings)
        .map_err(|err| format!("failed to parse font: {err}"))?;

    let chars: Vec<char> = (32u8..=126).map(|b| b as char).collect();
    const PAD: u32 = 2;
    let mut shelf_x = PAD;
    let mut shelf_y = PAD;
    let mut shelf_h = 0u32;
    let mut positions: Vec<(char, u32, u32, fontdue::Metrics, Vec<u8>)> = Vec::new();

    for ch in chars {
        let (metrics, bitmap) = font.rasterize(ch, px);
        let gw = metrics.width as u32;
        let gh = metrics.height as u32;
        if gw == 0 || gh == 0 {
            positions.push((ch, 0, 0, metrics, bitmap));
            continue;
        }
        if shelf_x + gw + PAD > atlas_w {
            shelf_y += shelf_h + PAD;
            shelf_x = PAD;
            shelf_h = 0;
        }
        if shelf_y + gh + PAD > atlas_w {
            let next = atlas_w.saturating_mul(2).max(1024);
            if next == atlas_w {
                return Err("font atlas is too large".into());
            }
            return bake_ttf(ttf, px, next);
        }
        positions.push((ch, shelf_x, shelf_y, metrics, bitmap));
        shelf_x += gw + PAD;
        shelf_h = shelf_h.max(gh);
    }
    let atlas_h = (shelf_y + shelf_h + PAD).next_power_of_two().max(64);
    let atlas_w = atlas_w.max(64);

    let mut rgba = vec![0u8; (atlas_w * atlas_h * 4) as usize];
    let mut glyphs = HashMap::new();
    for (ch, x, y, metrics, bitmap) in positions {
        let gw = metrics.width as u32;
        let gh = metrics.height as u32;
        if gw > 0 && gh > 0 {
            for row in 0..gh {
                for col in 0..gw {
                    let src = (row * gw + col) as usize;
                    let dst = (((y + row) * atlas_w + (x + col)) * 4) as usize;
                    let a = bitmap[src];
                    rgba[dst] = 255;
                    rgba[dst + 1] = 255;
                    rgba[dst + 2] = 255;
                    rgba[dst + 3] = a;
                }
            }
        }
        glyphs.insert(
            ch,
            Glyph {
                uv: [
                    x as f32 / atlas_w as f32,
                    y as f32 / atlas_h as f32,
                    (x + gw) as f32 / atlas_w as f32,
                    (y + gh) as f32 / atlas_h as f32,
                ],
                size: [gw as f32, gh as f32],
                xmin: metrics.xmin as f32,
                ymin: metrics.ymin as f32,
                advance: metrics.advance_width,
            },
        );
    }

    Ok(BakedFont {
        width: atlas_w,
        height: atlas_h,
        rgba,
        glyphs,
        px,
    })
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

    /// Draw `text` as one sprite per glyph.
    ///
    /// `origin` is the left of the first glyph. For grid fonts, `origin.y` is the cell
    /// center and `glyph_size` is the cell size. For TTF fonts, `origin.y` is the vertical
    /// center of the em box and `glyph_size[1]` is the em height.
    pub fn text(
        &mut self,
        font: &Font,
        origin: Vec3,
        glyph_size: [f32; 2],
        color: [f32; 4],
        text: &str,
    ) {
        // emit uses Y-down offsets from a pen at the top of the em box (y=0).
        // Shift so the em box is centered on origin.y in Y-up world space.
        let top_y_up = origin.y + glyph_size[1] * 0.5;
        font.emit_glyphs_y_down(text, glyph_size, |uv, offset, size| {
            let cx = origin.x + offset[0] + size[0] * 0.5;
            let cy = top_y_up - (offset[1] + size[1] * 0.5);
            self.sprites.push(Sprite {
                texture: font.texture,
                center: Vec3::new(cx, cy, origin.z),
                size,
                rotation: 0.0,
                uv,
                color,
            });
        });
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
