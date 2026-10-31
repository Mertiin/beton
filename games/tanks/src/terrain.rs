//! Bitmap dirt: collision mask + RGBA for drawing.

use std::sync::OnceLock;

use beton::Rng;

pub const MAP_W: u32 = 720;
pub const MAP_H: u32 = 405;

fn ground_image() -> &'static image::RgbaImage {
    static GROUND: OnceLock<image::RgbaImage> = OnceLock::new();
    GROUND.get_or_init(|| {
        image::load_from_memory(include_bytes!("../assets/ground.png"))
            .expect("ground.png")
            .to_rgba8()
    })
}

/// Solid/empty mask and matching RGBA image.
#[derive(Clone)]
pub struct Terrain {
    pub solid: Vec<bool>,
    pub rgba: Vec<u8>,
    pub dirty: bool,
}

impl Terrain {
    pub fn generate(seed: u64) -> Self {
        let mut rng = Rng::new(seed);
        let mut heights = vec![0.0f32; MAP_W as usize];
        let mid = MAP_H as f32 * 0.42;
        // Low + mid frequencies only — high-freq noise made the silhouette jagged.
        let waves = [
            (
                rng.range(0.006, 0.014),
                rng.range(22.0, 48.0),
                rng.range(0.0, 6.28),
            ),
            (
                rng.range(0.016, 0.032),
                rng.range(8.0, 18.0),
                rng.range(0.0, 6.28),
            ),
            (
                rng.range(0.034, 0.055),
                rng.range(2.0, 6.0),
                rng.range(0.0, 6.28),
            ),
        ];
        let floor = MAP_H as f32 * 0.12;
        let ceiling = MAP_H as f32 * 0.78;
        // Same hill count as the old 480-wide map, stretched across the wider field.
        let span = 480.0 / MAP_W as f32;
        let lift = MAP_H as f32 / 270.0;
        for (x, hgt) in heights.iter_mut().enumerate() {
            let xf = x as f32;
            let mut h = mid;
            for &(freq, amp, phase) in &waves {
                h += (xf * freq * span + phase).sin() * amp * lift;
            }
            *hgt = h.clamp(floor, ceiling);
        }
        // Box-blur the height field so hills roll instead of stair-step.
        for _ in 0..12 {
            let mut next = heights.clone();
            for x in 1..MAP_W as usize - 1 {
                next[x] = (heights[x - 1] + heights[x] * 2.0 + heights[x + 1]) * 0.25;
            }
            next[0] = (heights[0] * 3.0 + heights[1]) * 0.25;
            let last = MAP_W as usize - 1;
            next[last] = (heights[last] * 3.0 + heights[last - 1]) * 0.25;
            heights = next;
        }
        for h in heights.iter_mut() {
            *h = h.clamp(floor, ceiling);
        }

        let ground = ground_image();
        let gw = ground.width();
        let gh = ground.height();
        let mut solid = vec![false; (MAP_W * MAP_H) as usize];
        let mut rgba = vec![0u8; (MAP_W * MAP_H * 4) as usize];
        for y in 0..MAP_H {
            for x in 0..MAP_W {
                // Row 0 is the top of the texture (sky). Solid below the height field.
                let surface = heights[x as usize].round() as u32;
                let idx = (y * MAP_W + x) as usize;
                if y >= MAP_H - surface {
                    solid[idx] = true;
                    let pixel = ground.get_pixel(x % gw, y % gh);
                    let o = idx * 4;
                    rgba[o..o + 4].copy_from_slice(&[pixel[0], pixel[1], pixel[2], 255]);
                }
            }
        }
        let mut terrain = Self {
            solid,
            rgba,
            dirty: true,
        };
        terrain.paint_crust();
        terrain
    }

    /// Dark lip and a grass strip along the current surface, including crater rims.
    fn paint_crust(&mut self) {
        const LIP: [u8; 4] = [62, 42, 24, 255];
        const GRASS: [u8; 4] = [118, 170, 58, 255];
        const GRASS_DK: [u8; 4] = [84, 136, 44, 255];
        for col in 0..MAP_W as i32 {
            let mut depth = 0u32;
            for row in 0..MAP_H as i32 {
                if !self.solid_at(col, row) {
                    continue;
                }
                let color = if depth == 0 {
                    LIP
                } else if (col as u32 + depth) % 3 == 0 {
                    GRASS_DK
                } else {
                    GRASS
                };
                let o = ((row as u32 * MAP_W + col as u32) * 4) as usize;
                self.rgba[o..o + 4].copy_from_slice(&color);
                depth += 1;
                if depth == 8 {
                    break;
                }
            }
        }
    }

    pub fn solid_at(&self, col: i32, row: i32) -> bool {
        if col < 0 || row < 0 || col >= MAP_W as i32 || row >= MAP_H as i32 {
            return false;
        }
        self.solid[(row as u32 * MAP_W + col as u32) as usize]
    }

    /// World Y-up: origin bottom-left of the map.
    pub fn world_to_cell(x: f32, y: f32) -> (i32, i32) {
        let col = x.floor() as i32;
        let row = (MAP_H as f32 - 1.0 - y).floor() as i32;
        (col, row)
    }

    pub fn surface_y(&self, col: i32) -> Option<f32> {
        if col < 0 || col >= MAP_W as i32 {
            return None;
        }
        for row in 0..MAP_H as i32 {
            if self.solid_at(col, row) {
                // Top of this solid pixel in world Y.
                return Some(MAP_H as f32 - row as f32);
            }
        }
        None
    }

    pub fn stamp_circle(&mut self, cx: f32, cy: f32, radius: f32) {
        let r = radius.max(1.0);
        let r2 = r * r;
        let min_c = (cx - r).floor() as i32;
        let max_c = (cx + r).ceil() as i32;
        let min_r_world = cy - r;
        let max_r_world = cy + r;
        let min_row = (MAP_H as f32 - 1.0 - max_r_world).floor() as i32;
        let max_row = (MAP_H as f32 - 1.0 - min_r_world).ceil() as i32;
        for row in min_row..=max_row {
            for col in min_c..=max_c {
                if col < 0 || row < 0 || col >= MAP_W as i32 || row >= MAP_H as i32 {
                    continue;
                }
                let px = col as f32 + 0.5;
                let py = MAP_H as f32 - 1.0 - row as f32 - 0.5;
                let dx = px - cx;
                let dy = py - cy;
                if dx * dx + dy * dy > r2 {
                    continue;
                }
                let idx = (row as u32 * MAP_W + col as u32) as usize;
                if self.solid[idx] {
                    self.solid[idx] = false;
                    let o = idx * 4;
                    self.rgba[o] = 0;
                    self.rgba[o + 1] = 0;
                    self.rgba[o + 2] = 0;
                    self.rgba[o + 3] = 0;
                    self.dirty = true;
                }
            }
        }
        if self.dirty {
            self.paint_crust();
        }
    }

    pub fn count_solid(&self) -> usize {
        self.solid.iter().filter(|&&s| s).count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seeded_map_has_hill_and_downturn() {
        let t = Terrain::generate(0xC0FFEE);
        let mid = MAP_H as f32 * 0.42;
        let mut above = false;
        let mut below = false;
        for x in 0..MAP_W as i32 {
            let Some(y) = t.surface_y(x) else {
                continue;
            };
            if y > mid + 8.0 {
                above = true;
            }
            if y < mid - 8.0 {
                below = true;
            }
        }
        assert!(above, "expected a hill above midline");
        assert!(below, "expected a downturn below midline");
    }

    #[test]
    fn stamp_clears_pixels() {
        let mut t = Terrain::generate(1);
        let before = t.count_solid();
        t.stamp_circle(MAP_W as f32 * 0.5, MAP_H as f32 * 0.35, 18.0);
        let after = t.count_solid();
        assert!(after < before);
        let mut big = Terrain::generate(1);
        big.stamp_circle(MAP_W as f32 * 0.5, MAP_H as f32 * 0.35, 32.0);
        assert!(big.count_solid() < after);
    }
}
