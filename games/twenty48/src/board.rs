//! Pure 2048 rules: slide, merge once, spawn, win, and lock detection.

use beton::Rng;

const SIZE: usize = 4;
const N: usize = SIZE * SIZE;
const WIN_TILE: u32 = 2048;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dir {
    Left,
    Right,
    Up,
    Down,
}

pub struct Board {
    cells: [u32; N],
    score: u32,
    won: bool,
    over: bool,
    rng: Rng,
}

impl Board {
    pub fn new(seed: u64) -> Self {
        let mut board = Self {
            cells: [0; N],
            score: 0,
            won: false,
            over: false,
            rng: Rng::new(seed),
        };
        board.spawn();
        board.spawn();
        board
    }

    pub fn reset(&mut self) {
        self.cells = [0; N];
        self.score = 0;
        self.won = false;
        self.over = false;
        self.spawn();
        self.spawn();
    }

    pub fn cells(&self) -> &[u32; N] {
        &self.cells
    }

    pub fn score(&self) -> u32 {
        self.score
    }

    pub fn won(&self) -> bool {
        self.won
    }

    pub fn over(&self) -> bool {
        self.over
    }

    /// Apply a slide. Spawns only when the board changed. Returns whether anything moved.
    pub fn move_dir(&mut self, dir: Dir) -> bool {
        if self.over {
            return false;
        }
        let (changed, gained) = self.slide(dir);
        if !changed {
            return false;
        }
        self.score += gained;
        if self.cells.iter().any(|&v| v >= WIN_TILE) {
            self.won = true;
        }
        self.spawn();
        self.over = !self.any_move();
        true
    }

    fn slide(&mut self, dir: Dir) -> (bool, u32) {
        let mut changed = false;
        let mut gained = 0u32;
        for i in 0..SIZE {
            let mut line = match dir {
                Dir::Left => [self.get(i, 0), self.get(i, 1), self.get(i, 2), self.get(i, 3)],
                Dir::Right => [self.get(i, 3), self.get(i, 2), self.get(i, 1), self.get(i, 0)],
                Dir::Up => [self.get(0, i), self.get(1, i), self.get(2, i), self.get(3, i)],
                Dir::Down => [self.get(3, i), self.get(2, i), self.get(1, i), self.get(0, i)],
            };
            let before = line;
            let score = slide_line(&mut line);
            if line != before {
                changed = true;
                gained += score;
                match dir {
                    Dir::Left => {
                        self.set(i, 0, line[0]);
                        self.set(i, 1, line[1]);
                        self.set(i, 2, line[2]);
                        self.set(i, 3, line[3]);
                    }
                    Dir::Right => {
                        self.set(i, 3, line[0]);
                        self.set(i, 2, line[1]);
                        self.set(i, 1, line[2]);
                        self.set(i, 0, line[3]);
                    }
                    Dir::Up => {
                        self.set(0, i, line[0]);
                        self.set(1, i, line[1]);
                        self.set(2, i, line[2]);
                        self.set(3, i, line[3]);
                    }
                    Dir::Down => {
                        self.set(3, i, line[0]);
                        self.set(2, i, line[1]);
                        self.set(1, i, line[2]);
                        self.set(0, i, line[3]);
                    }
                }
            }
        }
        (changed, gained)
    }

    fn any_move(&self) -> bool {
        if self.cells.iter().any(|&v| v == 0) {
            return true;
        }
        for r in 0..SIZE {
            for c in 0..SIZE {
                let v = self.get(r, c);
                if c + 1 < SIZE && self.get(r, c + 1) == v {
                    return true;
                }
                if r + 1 < SIZE && self.get(r + 1, c) == v {
                    return true;
                }
            }
        }
        false
    }

    fn spawn(&mut self) {
        let empties: Vec<usize> = self
            .cells
            .iter()
            .enumerate()
            .filter_map(|(i, &v)| if v == 0 { Some(i) } else { None })
            .collect();
        if empties.is_empty() {
            return;
        }
        let idx = (self.rng.range(0.0, empties.len() as f32) as usize).min(empties.len() - 1);
        let value = if self.rng.range(0.0, 1.0) < 0.9 { 2 } else { 4 };
        self.cells[empties[idx]] = value;
    }

    fn get(&self, row: usize, col: usize) -> u32 {
        self.cells[row * SIZE + col]
    }

    fn set(&mut self, row: usize, col: usize, value: u32) {
        self.cells[row * SIZE + col] = value;
    }
}

/// Compact toward index 0, then merge each equal pair once, then compact again.
fn slide_line(line: &mut [u32; SIZE]) -> u32 {
    let mut buf = [0u32; SIZE];
    let mut n = 0;
    for &v in line.iter() {
        if v != 0 {
            buf[n] = v;
            n += 1;
        }
    }
    let mut score = 0u32;
    let mut i = 0;
    while i + 1 < n {
        if buf[i] == buf[i + 1] {
            buf[i] *= 2;
            score += buf[i];
            for j in i + 1..n - 1 {
                buf[j] = buf[j + 1];
            }
            buf[n - 1] = 0;
            n -= 1;
        }
        i += 1;
    }
    *line = buf;
    score
}

#[cfg(test)]
mod tests {
    use super::*;

    fn board_from(cells: [u32; N], seed: u64) -> Board {
        Board {
            cells,
            score: 0,
            won: false,
            over: false,
            rng: Rng::new(seed),
        }
    }

    #[test]
    fn merge_once_on_four_twos() {
        let mut line = [2, 2, 2, 2];
        let score = slide_line(&mut line);
        assert_eq!(line, [4, 4, 0, 0]);
        assert_eq!(score, 8);
    }

    #[test]
    fn merge_once_on_two_fours_and_eight() {
        let mut line = [4, 4, 8, 0];
        let score = slide_line(&mut line);
        assert_eq!(line, [8, 8, 0, 0]);
        assert_eq!(score, 8);
    }

    #[test]
    fn blocked_slide_does_not_change_or_spawn() {
        let mut board = board_from(
            [
                2, 4, 8, 16, //
                4, 8, 16, 32, //
                8, 16, 32, 64, //
                16, 32, 64, 128,
            ],
            1,
        );
        let before = board.cells;
        assert!(!board.move_dir(Dir::Left));
        assert_eq!(board.cells, before);
        assert_eq!(board.score, 0);
    }

    #[test]
    fn spawn_only_after_real_move() {
        let mut board = board_from(
            [
                0, 0, 0, 2, //
                0, 0, 0, 0, //
                0, 0, 0, 0, //
                0, 0, 0, 0,
            ],
            42,
        );
        let non_zero_before = board.cells.iter().filter(|&&v| v != 0).count();
        assert!(board.move_dir(Dir::Left));
        let non_zero_after = board.cells.iter().filter(|&&v| v != 0).count();
        assert_eq!(non_zero_before, 1);
        assert_eq!(non_zero_after, 2);
        assert_eq!(board.get(0, 0), 2);
    }

    #[test]
    fn full_locked_board_is_over_after_failed_moves() {
        let mut board = board_from(
            [
                2, 4, 8, 16, //
                4, 8, 16, 32, //
                8, 16, 32, 64, //
                16, 32, 64, 128,
            ],
            7,
        );
        assert!(!board.any_move());
        // Force the over flag the same way gameplay does after a move that fills the board.
        board.over = !board.any_move();
        assert!(board.over);
        assert!(!board.move_dir(Dir::Up));
    }

    #[test]
    fn win_when_reaching_2048() {
        let mut board = board_from(
            [
                1024, 1024, 0, 0, //
                0, 0, 0, 0, //
                0, 0, 0, 0, //
                0, 0, 0, 0,
            ],
            3,
        );
        assert!(board.move_dir(Dir::Left));
        assert!(board.won);
        assert!(board.cells.iter().any(|&v| v == 2048));
        assert_eq!(board.score, 2048);
    }
}
