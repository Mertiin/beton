//! Sprite sheets and simple frame clips.

use crate::core::render::TextureId;
use crate::core::scene::Sprite;

/// A grid of frames in one texture.
#[derive(Clone, Debug)]
pub struct SpriteSheet {
    /// Atlas texture.
    pub texture: TextureId,
    /// Frames across.
    pub columns: u32,
    /// Frames down.
    pub rows: u32,
}

impl SpriteSheet {
    /// Total cells in the grid.
    pub fn frame_count(&self) -> u32 {
        self.columns.saturating_mul(self.rows)
    }

    /// UV rect for a cell index (row-major). Out-of-range indices wrap.
    pub fn uv(&self, frame: u32) -> [f32; 4] {
        if self.columns == 0 || self.rows == 0 {
            return [0.0, 0.0, 1.0, 1.0];
        }
        let count = self.frame_count();
        let frame = if count == 0 { 0 } else { frame % count };
        let col = frame % self.columns;
        let row = frame / self.columns;
        let cell_u = 1.0 / self.columns as f32;
        let cell_v = 1.0 / self.rows as f32;
        let pad_u = cell_u * 0.02;
        let pad_v = cell_v * 0.02;
        [
            col as f32 * cell_u + pad_u,
            row as f32 * cell_v + pad_v,
            (col as f32 + 1.0) * cell_u - pad_u,
            (row as f32 + 1.0) * cell_v - pad_v,
        ]
    }

    /// Write a static sheet frame onto a sprite (`texture` + `uv`).
    pub fn apply(&self, sprite: &mut Sprite, frame: u32) {
        sprite.texture = self.texture;
        sprite.uv = self.uv(frame);
    }
}

/// A looping or one-shot clip over sheet frame indices.
#[derive(Clone, Debug)]
pub struct Animation {
    /// Sheet the frames index into.
    pub sheet: SpriteSheet,
    /// Cell indices, in play order. May repeat (e.g. `[0, 1, 2, 1]`).
    pub frames: Vec<u32>,
    /// Frames advanced per second.
    pub fps: f32,
    /// When true, restart after the last frame.
    pub looping: bool,
    time: f32,
    finished: bool,
}

impl Animation {
    /// Build a looping clip.
    pub fn new(sheet: SpriteSheet, frames: impl Into<Vec<u32>>, fps: f32) -> Self {
        Self {
            sheet,
            frames: frames.into(),
            fps,
            looping: true,
            time: 0.0,
            finished: false,
        }
    }

    /// Build a one-shot clip that sticks on the last frame.
    pub fn once(sheet: SpriteSheet, frames: impl Into<Vec<u32>>, fps: f32) -> Self {
        let mut anim = Self::new(sheet, frames, fps);
        anim.looping = false;
        anim
    }

    /// Same sheet, frames, fps, and looping with a fresh playback clock.
    ///
    /// Use this to spawn many objects from one template without locking their
    /// animation times together (unlike [`Clone`], which copies `time`).
    pub fn instance(&self) -> Self {
        Self {
            sheet: self.sheet.clone(),
            frames: self.frames.clone(),
            fps: self.fps,
            looping: self.looping,
            time: 0.0,
            finished: false,
        }
    }

    /// Advance the clip by `dt` seconds.
    pub fn tick(&mut self, dt: f32) {
        if self.finished || self.frames.is_empty() || self.fps <= 0.0 {
            return;
        }
        self.time += dt;
        if !self.looping {
            let last = (self.frames.len() as f32 - 1.0) / self.fps;
            if self.time >= last {
                self.time = last;
                self.finished = true;
            }
        }
    }

    /// Sheet cell for the current time.
    pub fn frame_index(&self) -> u32 {
        if self.frames.is_empty() {
            return 0;
        }
        let i = if self.looping {
            ((self.time * self.fps) as usize) % self.frames.len()
        } else {
            ((self.time * self.fps) as usize).min(self.frames.len() - 1)
        };
        self.frames[i]
    }

    /// UV rect for the current frame.
    pub fn uv(&self) -> [f32; 4] {
        self.sheet.uv(self.frame_index())
    }

    /// Texture used by this clip.
    pub fn texture(&self) -> TextureId {
        self.sheet.texture
    }

    /// True after a one-shot clip reaches its end.
    pub fn finished(&self) -> bool {
        self.finished
    }

    /// Restart from the first frame.
    pub fn reset(&mut self) {
        self.time = 0.0;
        self.finished = false;
    }

    /// Write the current frame onto a sprite.
    pub fn apply(&self, sprite: &mut Sprite) {
        sprite.texture = self.sheet.texture;
        sprite.uv = self.uv();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sheet_uv_first_and_last_cell() {
        let sheet = SpriteSheet {
            texture: TextureId::from_raw(0),
            columns: 3,
            rows: 1,
        };
        let a = sheet.uv(0);
        let b = sheet.uv(2);
        assert!(a[0] < a[2]);
        assert!(b[0] > a[0]);
        assert!(b[2] <= 1.0 + 1e-5);
    }

    #[test]
    fn animation_loops_through_frames() {
        let sheet = SpriteSheet {
            texture: TextureId::from_raw(0),
            columns: 3,
            rows: 1,
        };
        let mut anim = Animation::new(sheet, [0, 1, 2, 1], 10.0);
        assert_eq!(anim.frame_index(), 0);
        anim.tick(0.1);
        assert_eq!(anim.frame_index(), 1);
        anim.tick(0.1);
        assert_eq!(anim.frame_index(), 2);
        anim.tick(0.1);
        assert_eq!(anim.frame_index(), 1);
        anim.tick(0.1);
        assert_eq!(anim.frame_index(), 0);
    }

    #[test]
    fn instance_resets_playback_time() {
        let sheet = SpriteSheet {
            texture: TextureId::from_raw(0),
            columns: 3,
            rows: 1,
        };
        let mut template = Animation::new(sheet, [0, 1, 2], 10.0);
        template.tick(0.2);
        assert_eq!(template.frame_index(), 2);
        let fresh = template.instance();
        assert_eq!(fresh.frame_index(), 0);
        assert!(!fresh.finished());
    }
}
