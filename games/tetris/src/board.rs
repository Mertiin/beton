//! Single-player Tetris rules.
//!
//! Kept free of rendering so a later battle-royale layer can replicate or hash
//! this state over the net channel without pulling in `Context`.

use beton::Rng;

pub const COLS: usize = 10;
pub const ROWS: usize = 20;
/// Hidden rows above the visible field (spawn buffer).
pub const HIDDEN: usize = 2;
pub const TOTAL_ROWS: usize = ROWS + HIDDEN;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PieceKind {
    I,
    O,
    T,
    S,
    Z,
    J,
    L,
}

impl PieceKind {
    pub const ALL: [PieceKind; 7] = [
        PieceKind::I,
        PieceKind::O,
        PieceKind::T,
        PieceKind::S,
        PieceKind::Z,
        PieceKind::J,
        PieceKind::L,
    ];

    pub fn color(self) -> [f32; 4] {
        match self {
            PieceKind::I => [0.20, 0.85, 0.90, 1.0],
            PieceKind::O => [0.95, 0.85, 0.25, 1.0],
            PieceKind::T => [0.70, 0.35, 0.90, 1.0],
            PieceKind::S => [0.35, 0.85, 0.40, 1.0],
            PieceKind::Z => [0.95, 0.30, 0.35, 1.0],
            PieceKind::J => [0.30, 0.45, 0.95, 1.0],
            PieceKind::L => [0.95, 0.55, 0.20, 1.0],
        }
    }

    /// Four cells for rotation 0..3 in a local 4×4 box (origin top-left of the box).
    pub fn cells(self, rot: u8) -> [(i32, i32); 4] {
        let r = (rot & 3) as usize;
        match self {
            PieceKind::O => [(1, 1), (2, 1), (1, 2), (2, 2)],
            PieceKind::I => match r {
                0 => [(0, 1), (1, 1), (2, 1), (3, 1)],
                1 => [(2, 0), (2, 1), (2, 2), (2, 3)],
                2 => [(0, 2), (1, 2), (2, 2), (3, 2)],
                _ => [(1, 0), (1, 1), (1, 2), (1, 3)],
            },
            PieceKind::T => match r {
                0 => [(1, 0), (0, 1), (1, 1), (2, 1)],
                1 => [(1, 0), (1, 1), (2, 1), (1, 2)],
                2 => [(0, 1), (1, 1), (2, 1), (1, 2)],
                _ => [(1, 0), (0, 1), (1, 1), (1, 2)],
            },
            PieceKind::S => match r {
                0 => [(1, 0), (2, 0), (0, 1), (1, 1)],
                1 => [(1, 0), (1, 1), (2, 1), (2, 2)],
                2 => [(1, 1), (2, 1), (0, 2), (1, 2)],
                _ => [(0, 0), (0, 1), (1, 1), (1, 2)],
            },
            PieceKind::Z => match r {
                0 => [(0, 0), (1, 0), (1, 1), (2, 1)],
                1 => [(2, 0), (1, 1), (2, 1), (1, 2)],
                2 => [(0, 1), (1, 1), (1, 2), (2, 2)],
                _ => [(1, 0), (0, 1), (1, 1), (0, 2)],
            },
            PieceKind::J => match r {
                0 => [(0, 0), (0, 1), (1, 1), (2, 1)],
                1 => [(1, 0), (2, 0), (1, 1), (1, 2)],
                2 => [(0, 1), (1, 1), (2, 1), (2, 2)],
                _ => [(1, 0), (1, 1), (0, 2), (1, 2)],
            },
            PieceKind::L => match r {
                0 => [(2, 0), (0, 1), (1, 1), (2, 1)],
                1 => [(1, 0), (1, 1), (1, 2), (2, 2)],
                2 => [(0, 1), (1, 1), (2, 1), (0, 2)],
                _ => [(0, 0), (1, 0), (1, 1), (1, 2)],
            },
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Active {
    pub kind: PieceKind,
    pub rot: u8,
    pub x: i32,
    pub y: i32,
}

impl Active {
    pub fn cells(&self) -> [(i32, i32); 4] {
        let mut out = self.kind.cells(self.rot);
        for c in &mut out {
            c.0 += self.x;
            c.1 += self.y;
        }
        out
    }
}

pub struct Board {
    /// `None` = empty. Index `row * COLS + col`, row 0 is top (including hidden).
    cells: [Option<PieceKind>; TOTAL_ROWS * COLS],
    pub active: Option<Active>,
    pub hold: Option<PieceKind>,
    hold_used: bool,
    bag: Vec<PieceKind>,
    pub next: Vec<PieceKind>,
    pub score: u32,
    pub lines: u32,
    pub level: u32,
    pub over: bool,
    gravity_acc: f32,
    lock_acc: f32,
    rng: Rng,
}

impl Board {
    pub fn new(seed: u64) -> Self {
        let mut board = Self {
            cells: [None; TOTAL_ROWS * COLS],
            active: None,
            hold: None,
            hold_used: false,
            bag: Vec::new(),
            next: Vec::new(),
            score: 0,
            lines: 0,
            level: 1,
            over: false,
            gravity_acc: 0.0,
            lock_acc: 0.0,
            rng: Rng::new(seed),
        };
        for _ in 0..5 {
            let k = board.draw_from_bag();
            board.next.push(k);
        }
        board.spawn();
        board
    }

    pub fn reset(&mut self, seed: u64) {
        *self = Self::new(seed);
    }

    pub fn cell(&self, col: usize, row: usize) -> Option<PieceKind> {
        if col >= COLS || row >= TOTAL_ROWS {
            return None;
        }
        self.cells[row * COLS + col]
    }

    fn set_cell(&mut self, col: i32, row: i32, kind: PieceKind) {
        if col < 0 || row < 0 {
            return;
        }
        let (c, r) = (col as usize, row as usize);
        if c < COLS && r < TOTAL_ROWS {
            self.cells[r * COLS + c] = Some(kind);
        }
    }

    fn draw_from_bag(&mut self) -> PieceKind {
        if self.bag.is_empty() {
            self.bag.extend_from_slice(&PieceKind::ALL);
            // Fisher–Yates
            for i in (1..self.bag.len()).rev() {
                let j = (self.rng.range(0.0, (i + 1) as f32) as usize).min(i);
                self.bag.swap(i, j);
            }
        }
        self.bag.pop().unwrap()
    }

    fn take_next(&mut self) -> PieceKind {
        let kind = self.next.remove(0);
        let filled = self.draw_from_bag();
        self.next.push(filled);
        kind
    }

    fn spawn_kind(&mut self, kind: PieceKind) -> bool {
        let active = Active {
            kind,
            rot: 0,
            x: 3,
            y: 0,
        };
        if !self.fits(&active) {
            self.over = true;
            self.active = None;
            return false;
        }
        self.active = Some(active);
        self.hold_used = false;
        self.gravity_acc = 0.0;
        self.lock_acc = 0.0;
        true
    }

    fn spawn(&mut self) {
        let kind = self.take_next();
        self.spawn_kind(kind);
    }

    fn fits(&self, piece: &Active) -> bool {
        for (x, y) in piece.cells() {
            if x < 0 || x >= COLS as i32 || y >= TOTAL_ROWS as i32 {
                return false;
            }
            if y >= 0 {
                if self.cells[y as usize * COLS + x as usize].is_some() {
                    return false;
                }
            }
        }
        true
    }

    fn on_ground(&self, piece: &Active) -> bool {
        let mut down = *piece;
        down.y += 1;
        !self.fits(&down)
    }

    pub fn try_shift(&mut self, dx: i32) -> bool {
        let Some(mut piece) = self.active else {
            return false;
        };
        piece.x += dx;
        if self.fits(&piece) {
            self.active = Some(piece);
            self.lock_acc = 0.0;
            true
        } else {
            false
        }
    }

    pub fn try_rotate(&mut self, dir: i32) -> bool {
        let Some(piece) = self.active else {
            return false;
        };
        if piece.kind == PieceKind::O {
            return true;
        }
        let from = piece.rot;
        let to = ((piece.rot as i32 + dir).rem_euclid(4)) as u8;
        for (kx, ky) in wall_kicks(piece.kind, from, to) {
            let mut next = piece;
            next.rot = to;
            next.x += kx;
            next.y += ky;
            if self.fits(&next) {
                self.active = Some(next);
                self.lock_acc = 0.0;
                return true;
            }
        }
        false
    }

    pub fn soft_drop(&mut self) -> bool {
        let Some(mut piece) = self.active else {
            return false;
        };
        piece.y += 1;
        if self.fits(&piece) {
            self.active = Some(piece);
            self.score += 1;
            self.lock_acc = 0.0;
            true
        } else {
            false
        }
    }

    pub fn hard_drop(&mut self) {
        let Some(mut piece) = self.active else {
            return;
        };
        let mut dist = 0;
        loop {
            piece.y += 1;
            if !self.fits(&piece) {
                piece.y -= 1;
                break;
            }
            dist += 1;
        }
        self.score += dist * 2;
        self.active = Some(piece);
        self.lock_piece();
    }

    pub fn hold(&mut self) {
        if self.over || self.hold_used {
            return;
        }
        let Some(active) = self.active.take() else {
            return;
        };
        self.hold_used = true;
        let swap = self.hold.replace(active.kind);
        match swap {
            Some(kind) => {
                if !self.spawn_kind(kind) {
                    // spawn failed — already over
                }
            }
            None => self.spawn(),
        }
    }

    fn lock_piece(&mut self) {
        let Some(piece) = self.active.take() else {
            return;
        };
        for (x, y) in piece.cells() {
            self.set_cell(x, y, piece.kind);
        }
        let cleared = self.clear_lines();
        if cleared > 0 {
            self.lines += cleared;
            self.score += line_score(cleared, self.level);
            self.level = 1 + self.lines / 10;
        }
        self.spawn();
    }

    fn clear_lines(&mut self) -> u32 {
        let mut wrote = TOTAL_ROWS as i32 - 1;
        let mut cleared = 0u32;
        for read in (0..TOTAL_ROWS as i32).rev() {
            let full = (0..COLS).all(|c| self.cells[read as usize * COLS + c].is_some());
            if full {
                cleared += 1;
                continue;
            }
            if wrote != read {
                for c in 0..COLS {
                    self.cells[wrote as usize * COLS + c] =
                        self.cells[read as usize * COLS + c];
                }
            }
            wrote -= 1;
        }
        while wrote >= 0 {
            for c in 0..COLS {
                self.cells[wrote as usize * COLS + c] = None;
            }
            wrote -= 1;
        }
        cleared
    }

    /// Gravity step. `dt` in seconds.
    pub fn tick(&mut self, dt: f32) {
        if self.over || self.active.is_none() {
            return;
        }
        let interval = gravity_interval(self.level);
        self.gravity_acc += dt;
        while self.gravity_acc >= interval {
            self.gravity_acc -= interval;
            if !self.soft_drop_gravity() {
                break;
            }
        }

        if let Some(piece) = self.active {
            if self.on_ground(&piece) {
                self.lock_acc += dt;
                if self.lock_acc >= 0.5 {
                    self.lock_piece();
                }
            } else {
                self.lock_acc = 0.0;
            }
        }
    }

    fn soft_drop_gravity(&mut self) -> bool {
        let Some(mut piece) = self.active else {
            return false;
        };
        piece.y += 1;
        if self.fits(&piece) {
            self.active = Some(piece);
            true
        } else {
            false
        }
    }

    pub fn ghost_y(&self) -> Option<i32> {
        let mut piece = self.active?;
        loop {
            piece.y += 1;
            if !self.fits(&piece) {
                return Some(piece.y - 1);
            }
        }
    }
}

fn gravity_interval(level: u32) -> f32 {
    let n = level.saturating_sub(1) as i32;
    (0.8 * 0.85_f32.powi(n)).clamp(0.05, 0.8)
}

fn line_score(cleared: u32, level: u32) -> u32 {
    let base = match cleared {
        1 => 100,
        2 => 300,
        3 => 500,
        4 => 800,
        _ => 0,
    };
    base * level
}

fn wall_kicks(kind: PieceKind, from: u8, to: u8) -> [(i32, i32); 5] {
    // Simplified SRS-style offsets (including identity).
    let _ = (from, to);
    if kind == PieceKind::I {
        [
            (0, 0),
            (-2, 0),
            (1, 0),
            (-2, 1),
            (1, -2),
        ]
    } else {
        [
            (0, 0),
            (-1, 0),
            (-1, -1),
            (0, 2),
            (-1, 2),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spawn_and_hard_drop_locks() {
        let mut b = Board::new(1);
        assert!(b.active.is_some());
        b.hard_drop();
        assert!(b.active.is_some() || b.over);
        assert!(b.cells.iter().any(|c| c.is_some()));
    }

    #[test]
    fn clear_full_row() {
        let mut b = Board::new(2);
        b.active = None;
        let row = TOTAL_ROWS - 1;
        for c in 0..COLS {
            b.cells[row * COLS + c] = Some(PieceKind::O);
        }
        assert_eq!(b.clear_lines(), 1);
        assert!(b.cells[row * COLS..].iter().all(|c| c.is_none()));
    }
}
