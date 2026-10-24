//! Per-frame keyboard and pointer snapshot.

use std::collections::HashSet;

/// Physical key code from `winit`. The crate pins the `winit` major version.
pub use winit::keyboard::KeyCode as Key;

/// Keys held, keys that went down this frame, and the cursor position.
#[derive(Debug, Default)]
pub struct Input {
    down: HashSet<Key>,
    pressed: HashSet<Key>,
    /// Cursor in physical framebuffer pixels (origin top-left, Y down).
    cursor_px: [f32; 2],
    /// True when the cursor is over the window.
    cursor_inside: bool,
}

impl Input {
    pub(crate) fn begin_frame(&mut self) {
        self.pressed.clear();
    }

    pub(crate) fn key_event(&mut self, key: Key, is_pressed: bool) {
        if is_pressed {
            if self.down.insert(key) {
                self.pressed.insert(key);
            }
        } else {
            self.down.remove(&key);
        }
    }

    pub(crate) fn cursor_moved(&mut self, x: f32, y: f32) {
        self.cursor_px = [x, y];
        self.cursor_inside = true;
    }

    pub(crate) fn cursor_left(&mut self) {
        self.cursor_inside = false;
    }

    /// True while the key is held.
    pub fn down(&self, key: Key) -> bool {
        self.down.contains(&key)
    }

    /// True if the key went down this frame.
    pub fn pressed(&self, key: Key) -> bool {
        self.pressed.contains(&key)
    }

    /// Cursor in physical framebuffer pixels (origin top-left, Y down).
    pub fn cursor_px(&self) -> [f32; 2] {
        self.cursor_px
    }

    /// True when the cursor is over the window.
    pub fn cursor_inside(&self) -> bool {
        self.cursor_inside
    }
}
