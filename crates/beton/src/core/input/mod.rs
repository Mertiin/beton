//! Per-frame keyboard and pointer snapshot.

use std::collections::HashSet;

/// Physical key code from `winit`. The crate pins the `winit` major version.
pub use winit::keyboard::KeyCode as Key;

/// Keys held, keys that went down / up this frame, and the cursor position.
#[derive(Debug, Default)]
pub struct Input {
    down: HashSet<Key>,
    pressed: HashSet<Key>,
    released: HashSet<Key>,
    /// Cursor in physical framebuffer pixels (origin top-left, Y down).
    cursor_px: [f32; 2],
    /// True when the cursor is over the window.
    cursor_inside: bool,
    /// Left mouse button currently held.
    pointer_down: bool,
    /// Left mouse button went down this frame.
    pointer_pressed: bool,
    /// Left mouse button went up this frame.
    pointer_released: bool,
    /// Right mouse button currently held.
    secondary_down: bool,
    /// Middle mouse button currently held.
    middle_down: bool,
    /// Vertical scroll this frame (positive = wheel up / zoom in).
    scroll_y: f32,
}

impl Input {
    pub(crate) fn begin_frame(&mut self) {
        self.pressed.clear();
        self.released.clear();
        self.pointer_pressed = false;
        self.pointer_released = false;
        self.scroll_y = 0.0;
    }

    pub(crate) fn key_event(&mut self, key: Key, is_pressed: bool) {
        if is_pressed {
            if self.down.insert(key) {
                self.pressed.insert(key);
            }
        } else if self.down.remove(&key) {
            self.released.insert(key);
        }
    }

    pub(crate) fn pointer_event(&mut self, is_pressed: bool) {
        if is_pressed {
            if !self.pointer_down {
                self.pointer_pressed = true;
            }
            self.pointer_down = true;
        } else if self.pointer_down {
            self.pointer_down = false;
            self.pointer_released = true;
        }
    }

    pub(crate) fn secondary_event(&mut self, is_pressed: bool) {
        self.secondary_down = is_pressed;
    }

    pub(crate) fn middle_event(&mut self, is_pressed: bool) {
        self.middle_down = is_pressed;
    }

    pub(crate) fn scroll(&mut self, dy: f32) {
        self.scroll_y += dy;
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

    /// True if the key went up this frame.
    pub fn released(&self, key: Key) -> bool {
        self.released.contains(&key)
    }

    /// True while the left mouse button is held.
    pub fn pointer_down(&self) -> bool {
        self.pointer_down
    }

    /// True if the left mouse button went down this frame.
    pub fn pointer_pressed(&self) -> bool {
        self.pointer_pressed
    }

    /// True if the left mouse button went up this frame.
    pub fn pointer_released(&self) -> bool {
        self.pointer_released
    }

    /// True while the right mouse button is held.
    pub fn secondary_down(&self) -> bool {
        self.secondary_down
    }

    /// True while the middle mouse button is held.
    pub fn middle_down(&self) -> bool {
        self.middle_down
    }

    /// Vertical scroll accumulated this frame (positive = wheel up).
    pub fn scroll_y(&self) -> f32 {
        self.scroll_y
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
