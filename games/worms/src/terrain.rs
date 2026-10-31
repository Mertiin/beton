//! Bitmap dirt: collision mask + RGBA. 960×540; mask resync uses chunked net packets.

use std::sync::OnceLock;

use beton::Rng;

pub const MAP_W: u32 = 960;
pub const MAP_H: u32 = 540;

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
    pub fn empty() -> Self {
        let n = (MAP_W * MAP_H) as usize;
        Self {
            solid: vec![false; n],
            rgba: vec![0; n * 4],
            dirty: true,
        }
    }

    pub fn generate(seed: u64) -> Self {
        let mut rng = Rng::new(seed);
        let mut heights = vec![0.0f32; MAP_W as usize];
        let mid = 240.0;
        let waves = [
            (
                rng.range(0.004, 0.010),
                rng.range(40.0, 90.0),
                rng.range(0.0, 6.28),
            ),
            (
                rng.range(0.012, 0.024),
                rng.range(14.0, 32.0),
                rng.range(0.0, 6.28),
            ),
            (
                rng.range(0.026, 0.045),
                rng.range(4.0, 12.0),
                rng.range(0.0, 6.28),
            ),
        ];
        let floor = 128.0;
        let ceiling = 420.0;
        for (x, hgt) in heights.iter_mut().enumerate() {
            let xf = x as f32;
            let mut h = mid;
            for &(freq, amp, phase) in &waves {
                h += (xf * freq + phase).sin() * amp;
            }
            *hgt = h.clamp(floor, ceiling);
        }
        for _ in 0..8 {
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

        let mut terrain = Self::empty();
        for y in 0..MAP_H {
            for x in 0..MAP_W {
                let surface = heights[x as usize].round() as u32;
                if y >= MAP_H - surface {
                    let idx = (y * MAP_W + x) as usize;
                    terrain.solid[idx] = true;
                }
            }
        }
        terrain.rebuild_rgba();
        terrain
    }

    /// Fill every solid cell from the dirt tile and repaint the grass lip.
    pub fn rebuild_rgba(&mut self) {
        let ground = ground_image();
        let gw = ground.width();
        let gh = ground.height();
        self.rgba.fill(0);
        for row in 0..MAP_H {
            for col in 0..MAP_W {
                let idx = (row * MAP_W + col) as usize;
                if !self.solid[idx] {
                    continue;
                }
                let pixel = ground.get_pixel(col % gw, row % gh);
                let o = idx * 4;
                self.rgba[o..o + 4].copy_from_slice(&[pixel[0], pixel[1], pixel[2], 255]);
            }
        }
        self.paint_crust();
        self.dirty = true;
    }

    /// Bright green lip along the current surface, including crater rims.
    fn paint_crust(&mut self) {
        const GRASS: [u8; 4] = [64, 196, 52, 255];
        const GRASS_DK: [u8; 4] = [36, 150, 40, 255];
        for col in 0..MAP_W as i32 {
            let mut depth = 0u32;
            for row in 0..MAP_H as i32 {
                if !self.solid_at(col, row) {
                    continue;
                }
                let color = if (col as u32 + depth) % 3 == 0 {
                    GRASS_DK
                } else {
                    GRASS
                };
                let o = ((row as u32 * MAP_W + col as u32) * 4) as usize;
                self.rgba[o..o + 4].copy_from_slice(&color);
                depth += 1;
                if depth == 4 {
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

    #[cfg(test)]
    pub fn set_solid(&mut self, col: i32, row: i32, on: bool) {
        if col < 0 || row < 0 || col >= MAP_W as i32 || row >= MAP_H as i32 {
            return;
        }
        let idx = (row as u32 * MAP_W + col as u32) as usize;
        self.solid[idx] = on;
        self.dirty = true;
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
                return Some(MAP_H as f32 - row as f32);
            }
        }
        None
    }

    /// Solid where the cell center is below `y` (world, Y-up).
    #[cfg(test)]
    pub fn fill_below(&mut self, y: f32) {
        for row in 0..MAP_H as i32 {
            let py = MAP_H as f32 - 1.0 - row as f32 - 0.5;
            if py >= y {
                continue;
            }
            for col in 0..MAP_W as i32 {
                self.set_solid(col, row, true);
            }
        }
    }

    /// Clear solid cells whose centers lie in the world rect.
    #[cfg(test)]
    pub fn clear_rect(&mut self, x0: f32, y0: f32, x1: f32, y1: f32) {
        let c0 = x0.floor() as i32;
        let c1 = x1.ceil() as i32;
        for col in c0..c1 {
            for row in 0..MAP_H as i32 {
                let py = MAP_H as f32 - 1.0 - row as f32 - 0.5;
                let px = col as f32 + 0.5;
                if px >= x0 && px < x1 && py >= y0 && py < y1 {
                    self.set_solid(col, row, false);
                }
            }
        }
    }

    pub fn stamp_circle(&mut self, cx: f32, cy: f32, radius: f32) {
        if radius < 1.0 {
            return;
        }
        let r2 = radius * radius;
        let min_c = (cx - radius).floor() as i32;
        let max_c = (cx + radius).ceil() as i32;
        let min_row = (MAP_H as f32 - 1.0 - (cy + radius)).floor() as i32;
        let max_row = (MAP_H as f32 - 1.0 - (cy - radius)).ceil() as i32;
        let mut hit = false;
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
                    hit = true;
                }
            }
        }
        if hit {
            self.paint_crust();
            self.dirty = true;
        }
    }

    /// Paint a pine into the solid mask (diggable like dirt). `base_y` is ground surface.
    pub fn stamp_tree(&mut self, base_x: f32, base_y: f32) {
        const TRUNK: [u8; 4] = [110, 72, 40, 255];
        const GREEN: [[u8; 4]; 2] = [[40, 150, 48, 255], [56, 170, 60, 255]];
        // Trunk rooted into the dirt so it does not float (taller than before).
        for dy in -4..22 {
            for dx in -2..=2 {
                let wx = base_x + dx as f32;
                let wy = base_y + dy as f32 + 0.5;
                let (c, r) = Self::world_to_cell(wx, wy);
                if c < 0 || r < 0 || c >= MAP_W as i32 || r >= MAP_H as i32 {
                    continue;
                }
                let idx = (r as u32 * MAP_W + c as u32) as usize;
                self.solid[idx] = true;
                let o = idx * 4;
                self.rgba[o..o + 4].copy_from_slice(&TRUNK);
            }
        }
        // Three stacked canopy triangles above the trunk (larger silhouette).
        let layers: [(f32, f32); 3] = [(22.0, 12.0), (34.0, 15.0), (46.0, 17.0)];
        for (i, &(top, half)) in layers.iter().enumerate() {
            let color = GREEN[i % 2];
            for row in 0..(half as i32) {
                let t = row as f32 / half.max(1.0);
                let span = 3.0 + t * (half * 0.65);
                let wy = base_y + top - row as f32;
                for dx in (-span as i32)..=(span as i32) {
                    let wx = base_x + dx as f32;
                    let (c, r) = Self::world_to_cell(wx, wy);
                    if c < 0 || r < 0 || c >= MAP_W as i32 || r >= MAP_H as i32 {
                        continue;
                    }
                    let idx = (r as u32 * MAP_W + c as u32) as usize;
                    self.solid[idx] = true;
                    let o = idx * 4;
                    self.rgba[o..o + 4].copy_from_slice(&color);
                }
            }
        }
        self.dirty = true;
    }

    /// Stamp several pines on the ridge. Skip columns near `avoid_x` (worm spawns).
    pub fn plant_ridge_trees(&mut self, avoid_x: &[f32]) {
        let fracs = [0.14, 0.28, 0.40, 0.58, 0.72, 0.86];
        for &f in &fracs {
            let x = MAP_W as f32 * f;
            if avoid_x.iter().any(|&wx| (wx - x).abs() < 20.0) {
                continue;
            }
            let y = self.surface_y(x as i32).unwrap_or(80.0);
            self.stamp_tree(x, y);
        }
    }

    #[cfg(test)]
    pub fn count_solid(&self) -> usize {
        self.solid.iter().filter(|&&s| s).count()
    }

    pub fn pack_solid(&self) -> Vec<u8> {
        let n = (MAP_W * MAP_H) as usize;
        let mut out = vec![0u8; n.div_ceil(8)];
        for (i, &on) in self.solid.iter().enumerate() {
            if on {
                out[i / 8] |= 1 << (i % 8);
            }
        }
        out
    }

    pub fn unpack_solid(&mut self, bytes: &[u8]) -> bool {
        let n = (MAP_W * MAP_H) as usize;
        if bytes.len() < n.div_ceil(8) {
            return false;
        }
        for i in 0..n {
            self.solid[i] = bytes[i / 8] & (1 << (i % 8)) != 0;
        }
        self.rebuild_rgba();
        true
    }

    #[cfg(test)]
    pub fn checksum(&self) -> u32 {
        let mut h = 2_166_136_261u32;
        for (i, &on) in self.solid.iter().enumerate() {
            if on {
                h ^= i as u32;
                h = h.wrapping_mul(1_677_7619);
            }
        }
        h
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seeded_map_sits_above_the_water_line() {
        let t = Terrain::generate(0xC0FFEE);
        for x in 0..MAP_W as i32 {
            let y = t.surface_y(x).expect("column");
            assert!(y > 100.0, "surface {y} too low at {x}");
        }
    }

    #[test]
    fn stamp_clears_pixels_and_roundtrips() {
        let mut t = Terrain::generate(1);
        let before = t.count_solid();
        t.stamp_circle(MAP_W as f32 * 0.5, 200.0, 28.0);
        assert!(t.count_solid() < before);
        let packed = t.pack_solid();
        assert_eq!(packed.len(), (MAP_W * MAP_H / 8) as usize);
        let mut other = Terrain::empty();
        assert!(other.unpack_solid(&packed));
        assert_eq!(other.checksum(), t.checksum());
    }
}
