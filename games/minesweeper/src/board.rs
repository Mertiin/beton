//! Pure Minesweeper rules ported from the C# Toolkit.MiniGames reference.
//!
//! Cell state is packed in bytes: Unvisited=0, Cached1..8=1..8, Empty=16,
//! Flag=64, Bomb=128 (FLAG/BOMB combine via bit masks).

use beton::Rng;

const BOMB: u8 = 128;
const NON_BOMB_MASK: u8 = 127;
const FLAG: u8 = 64;
const EMPTY: u8 = 16;

/// Public cell view for rendering (bombs hidden until dead).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CellView {
    Unvisited,
    Number(u8),
    Empty,
    Flag,
    Bomb,
    /// Bomb that was also flagged (shown on loss).
    FlaggedBomb,
    /// Other raw bit combinations after loss.
    Raw(u8),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Sweep,
    Flag,
}

pub struct Board {
    width: usize,
    height: usize,
    mines: i16,
    placed_mines: i16,
    /// True after the first Sweep places mines (C# `playfield != null` after VerifyGenerated).
    mines_ready: bool,
    dead: bool,
    solved: bool,
    flags: i32,
    /// Row-major `width * height`, or `None` until first Flag/Sweep allocates.
    playfield: Option<Vec<u8>>,
    rng: Rng,
}

impl Board {
    pub fn new(width: usize, height: usize, mines: i16) -> Self {
        Self {
            width: width.max(1),
            height: height.max(1),
            mines: mines.max(1),
            placed_mines: 0,
            mines_ready: false,
            dead: false,
            solved: false,
            flags: 0,
            playfield: None,
            rng: Rng::new(0x41_4e_53_57),
        }
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn height(&self) -> usize {
        self.height
    }

    #[allow(dead_code)]
    pub fn mines(&self) -> i16 {
        self.mines
    }

    #[allow(dead_code)]
    pub fn placed_mines(&self) -> i16 {
        self.placed_mines
    }

    pub fn is_dead(&self) -> bool {
        self.dead
    }

    pub fn is_solved(&self) -> bool {
        self.solved
    }

    #[allow(dead_code)]
    pub fn is_generated(&self) -> bool {
        self.mines_ready
    }

    #[allow(dead_code)]
    pub fn is_done(&self) -> bool {
        (self.dead || self.solved) && self.is_generated()
    }

    #[allow(dead_code)]
    pub fn flags(&self) -> i32 {
        self.flags
    }

    /// Remaining mines estimate: configured mines minus placed flags.
    pub fn mines_left(&self) -> i32 {
        self.mines as i32 - self.flags
    }

    pub fn resize(&mut self, width: usize, height: usize, mines: i16) {
        self.width = width.max(1);
        self.height = height.max(1);
        self.mines = mines.max(1);
        self.restart();
    }

    #[allow(dead_code)]
    pub fn set_mines(&mut self, mines: i16) {
        self.mines = mines.max(1);
        self.restart();
    }

    pub fn restart(&mut self) {
        self.playfield = None;
        self.mines_ready = false;
        self.dead = false;
        self.solved = false;
        self.flags = 0;
        self.placed_mines = 0;
        // New seed each restart so layouts vary.
        self.rng = Rng::new(self.rng.range(1.0, u32::MAX as f32) as u64);
    }

    pub fn cell(&self, x: usize, y: usize) -> CellView {
        let Some(pf) = self.playfield.as_ref() else {
            return CellView::Unvisited;
        };
        let raw = pf[self.idx(x, y)];
        let shown = if self.dead {
            raw
        } else {
            raw & NON_BOMB_MASK
        };
        classify(shown, self.dead)
    }

    pub fn do_action(&mut self, x: i32, y: i32, action: Action) {
        if self.dead || self.solved {
            return;
        }
        match action {
            Action::Flag => self.toggle_flag(x, y),
            Action::Sweep => self.sweep(x, y),
        }
        if self.is_solved_check() {
            self.solved = true;
        }
    }

    fn toggle_flag(&mut self, x: i32, y: i32) {
        if !self.in_bounds(x, y) {
            return;
        }
        // C# assumes playfield exists; allocate an empty field so pre-sweep flags work
        // without placing mines yet (mines still defer to first Sweep).
        self.ensure_playfield();
        let i = self.idx(x as usize, y as usize);
        let pf = self.playfield.as_mut().unwrap();
        let val = pf[i];
        if (val != 0 && val <= 8) || val == EMPTY {
            return;
        }
        if has_flag(val, FLAG) {
            pf[i] = val.wrapping_sub(FLAG);
            self.flags -= 1;
        } else {
            pf[i] = val.wrapping_add(FLAG);
            self.flags += 1;
        }
    }

    fn sweep(&mut self, x: i32, y: i32) {
        if !self.in_bounds(x, y) {
            return;
        }
        self.verify_generated(x as usize, y as usize);
        let val = self.playfield.as_ref().unwrap()[self.idx(x as usize, y as usize)];
        if has_flag(val, FLAG) {
            return;
        }
        if has_flag(val, BOMB) {
            self.dead = true;
            return;
        }
        if val == 0 {
            self.sweep_recursive(x, y);
        } else if val <= 8 && self.has_flags_nearby(x, y, val as i32) {
            self.clear_non_flag_nearby(x, y);
        }
    }

    fn is_solved_check(&self) -> bool {
        let Some(pf) = self.playfield.as_ref() else {
            return false;
        };
        for &c in pf.iter() {
            if c == 0 || c == FLAG {
                return false;
            }
        }
        true
    }

    fn has_flags_nearby(&self, x: i32, y: i32, flags: i32) -> bool {
        flags
            == self.is_flag_int(x - 1, y + 1)
                + self.is_flag_int(x - 1, y)
                + self.is_flag_int(x - 1, y - 1)
                + self.is_flag_int(x + 1, y + 1)
                + self.is_flag_int(x + 1, y)
                + self.is_flag_int(x + 1, y - 1)
                + self.is_flag_int(x, y + 1)
                + self.is_flag_int(x, y - 1)
    }

    fn is_flag_int(&self, x: i32, y: i32) -> i32 {
        if !self.in_bounds(x, y) {
            return 0;
        }
        let pf = self.playfield.as_ref().unwrap();
        if has_flag(pf[self.idx(x as usize, y as usize)], FLAG) {
            1
        } else {
            0
        }
    }

    fn is_flag(&self, x: i32, y: i32) -> bool {
        if !self.in_bounds(x, y) {
            return false;
        }
        let pf = self.playfield.as_ref().unwrap();
        has_flag(pf[self.idx(x as usize, y as usize)], FLAG)
    }

    fn sweep_if_not_flag(&mut self, x: i32, y: i32) {
        if self.is_flag(x, y) {
            return;
        }
        if self.is_mine(x, y) {
            self.dead = true;
            return;
        }
        self.sweep_recursive(x, y);
    }

    fn clear_non_flag_nearby(&mut self, x: i32, y: i32) {
        self.sweep_if_not_flag(x - 1, y + 1);
        self.sweep_if_not_flag(x - 1, y);
        self.sweep_if_not_flag(x - 1, y - 1);
        self.sweep_if_not_flag(x + 1, y + 1);
        self.sweep_if_not_flag(x + 1, y);
        self.sweep_if_not_flag(x + 1, y - 1);
        self.sweep_if_not_flag(x, y + 1);
        self.sweep_if_not_flag(x, y - 1);
    }

    fn sweep_recursive(&mut self, x: i32, y: i32) {
        if !self.in_bounds(x, y) {
            return;
        }
        let i = self.idx(x as usize, y as usize);
        let val = self.playfield.as_ref().unwrap()[i];
        if val != 0 {
            return;
        }
        let mines_nearby = self.nearby_mines(x, y);
        if mines_nearby == 0 {
            self.playfield.as_mut().unwrap()[i] = EMPTY;
            self.sweep_recursive(x - 1, y + 1);
            self.sweep_recursive(x - 1, y);
            self.sweep_recursive(x - 1, y - 1);
            self.sweep_recursive(x + 1, y + 1);
            self.sweep_recursive(x + 1, y);
            self.sweep_recursive(x + 1, y - 1);
            self.sweep_recursive(x, y + 1);
            self.sweep_recursive(x, y - 1);
        } else {
            self.playfield.as_mut().unwrap()[i] = mines_nearby as u8;
        }
    }

    fn nearby_mines(&self, x: i32, y: i32) -> i32 {
        self.is_mine_int(x - 1, y + 1)
            + self.is_mine_int(x - 1, y)
            + self.is_mine_int(x - 1, y - 1)
            + self.is_mine_int(x + 1, y + 1)
            + self.is_mine_int(x + 1, y)
            + self.is_mine_int(x + 1, y - 1)
            + self.is_mine_int(x, y + 1)
            + self.is_mine_int(x, y - 1)
    }

    fn is_mine_int(&self, x: i32, y: i32) -> i32 {
        if self.is_mine(x, y) {
            1
        } else {
            0
        }
    }

    fn is_mine(&self, x: i32, y: i32) -> bool {
        if !self.in_bounds(x, y) {
            return false;
        }
        let pf = self.playfield.as_ref().unwrap();
        has_flag(pf[self.idx(x as usize, y as usize)], BOMB)
    }

    fn ensure_playfield(&mut self) {
        if self.playfield.is_none() {
            self.playfield = Some(vec![0u8; self.width * self.height]);
        }
    }

    fn verify_generated(&mut self, click_x: usize, click_y: usize) {
        if self.mines_ready {
            return;
        }
        self.ensure_playfield();
        let pf = self.playfield.as_mut().unwrap();
        // Preserve existing FLAG bits from pre-sweep marking; only place BOMB on empty cells.
        let mut mines_left = self.mines;
        let mut attempts = 0i32;
        while mines_left > 0 {
            let x_pos = (self.rng.range(0.0, self.width as f32) as usize).min(self.width - 1);
            let y_pos = (self.rng.range(0.0, self.height as f32) as usize).min(self.height - 1);
            let chebyshev = chebyshev(x_pos, y_pos, click_x, click_y);
            let i = y_pos * self.width + x_pos;
            let cell = pf[i];
            if chebyshev < 2 || has_flag(cell, BOMB) {
                attempts += 1;
                if attempts > 1000 {
                    break;
                }
                continue;
            }
            // C# writes BOMB over 0; if the cell was flagged, keep FLAG|BOMB.
            pf[i] = cell | BOMB;
            mines_left -= 1;
            attempts = 0;
        }
        self.placed_mines = self.mines - mines_left;
        self.mines_ready = true;
    }

    fn in_bounds(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && (x as usize) < self.width && (y as usize) < self.height
    }

    fn idx(&self, x: usize, y: usize) -> usize {
        y * self.width + x
    }
}

fn has_flag(b: u8, val: u8) -> bool {
    (b & val) == val
}

fn chebyshev(ax: usize, ay: usize, bx: usize, by: usize) -> i32 {
    let dx = (ax as i32 - bx as i32).abs();
    let dy = (ay as i32 - by as i32).abs();
    dx.max(dy)
}

fn classify(shown: u8, dead: bool) -> CellView {
    if shown == 0 {
        return CellView::Unvisited;
    }
    if shown >= 1 && shown <= 8 {
        return CellView::Number(shown);
    }
    if shown == EMPTY {
        return CellView::Empty;
    }
    if shown == FLAG {
        return CellView::Flag;
    }
    if dead && has_flag(shown, BOMB) && has_flag(shown, FLAG) {
        return CellView::FlaggedBomb;
    }
    if dead && has_flag(shown, BOMB) {
        return CellView::Bomb;
    }
    if has_flag(shown, FLAG) {
        return CellView::Flag;
    }
    CellView::Raw(shown)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_sweep_safe_neighborhood() {
        let mut board = Board::new(10, 10, 10);
        board.do_action(4, 4, Action::Sweep);
        assert!(board.is_generated());
        assert!(!board.is_dead());
        // 3x3 around click must not contain bombs (hidden until dead, so force raw check).
        let pf = board.playfield.as_ref().unwrap();
        for dy in -1i32..=1 {
            for dx in -1i32..=1 {
                let x = (4 + dx) as usize;
                let y = (4 + dy) as usize;
                let raw = pf[y * 10 + x];
                assert!(!has_flag(raw, BOMB), "mine at neighbor {x},{y}");
            }
        }
    }

    #[test]
    fn flag_before_sweep_does_not_place_mines() {
        let mut board = Board::new(10, 10, 10);
        board.do_action(0, 0, Action::Flag);
        assert!(!board.is_generated());
        assert_eq!(board.flags(), 1);
        assert_eq!(board.cell(0, 0), CellView::Flag);
    }

    #[test]
    fn chord_requires_matching_flag_count() {
        let mut board = Board::new(5, 5, 1);
        // Force a tiny known layout: place manually after first generate.
        board.verify_generated(0, 0);
        let pf = board.playfield.as_mut().unwrap();
        for c in pf.iter_mut() {
            *c = 0;
        }
        // Mine at (2,2); reveal (1,1) as number 1.
        pf[2 * 5 + 2] = BOMB;
        pf[1 * 5 + 1] = 1;
        board.placed_mines = 1;

        // Chord without flags should not clear neighbors.
        board.do_action(1, 1, Action::Sweep);
        assert_eq!(board.playfield.as_ref().unwrap()[1 * 5 + 0], 0);

        // Flag the mine, then chord.
        board.do_action(2, 2, Action::Flag);
        assert_eq!(board.flags(), 1);
        board.do_action(1, 1, Action::Sweep);
        // Neighbor (0,0) should flood/reveal (zero mines nearby from this partial board —
        // only one mine at 2,2 so (0,0) nearby count is 0 → EMPTY).
        let c = board.playfield.as_ref().unwrap()[0];
        assert!(c == EMPTY || (1..=8).contains(&c));
    }

    #[test]
    fn win_when_no_unvisited_or_flag_only() {
        let mut board = Board::new(2, 2, 1);
        board.playfield = Some(vec![EMPTY, 1, BOMB, EMPTY]);
        board.placed_mines = 1;
        assert!(board.is_solved_check());
        // Flag-only cell blocks win.
        board.playfield = Some(vec![EMPTY, 1, BOMB, FLAG]);
        assert!(!board.is_solved_check());
    }
}
