//! Classic Tetris sample on the beton API.
//!
//! Single-player for now. Board logic lives in [`board`] so a later
//! battle-royale mode can share rules and sync state over `ctx.net`.

#![cfg_attr(windows, windows_subsystem = "windows")]

mod board;

use board::{Active, Board, PieceKind, COLS, HIDDEN, ROWS};
use beton::{
    run, Camera, Config, Context, Font, Game, Key, Projection, Sprite, TextureHandle, Vec3,
};

const WORLD_HEIGHT: f32 = 22.0;
const CELL: f32 = 0.85;
const GAP: f32 = 0.06;
const STEP: f32 = CELL + GAP;

const DIGIT_CELL_W: u32 = 16;
const DIGIT_CELL_H: u32 = 20;
const DIGIT_ATLAS_W: u32 = DIGIT_CELL_W * 10;
const DIGIT_ATLAS_H: u32 = DIGIT_CELL_H;
const DIGIT_TRACKING: f32 = 0.12;

struct Tetris {
    white: Option<TextureHandle>,
    font_tex: Option<TextureHandle>,
    font: Option<Font>,
    board: Board,
    das_left: f32,
    das_right: f32,
    soft_acc: f32,
    seed: u64,
}

impl Tetris {
    fn new() -> Self {
        let seed = 0x7e77_15u64;
        Self {
            white: None,
            font_tex: None,
            font: None,
            board: Board::new(seed),
            das_left: 0.0,
            das_right: 0.0,
            soft_acc: 0.0,
            seed,
        }
    }

    fn field_origin() -> (f32, f32) {
        let w = COLS as f32 * STEP - GAP;
        let h = ROWS as f32 * STEP - GAP;
        (-w * 0.5, h * 0.5)
    }

    fn cell_center(col: usize, row_visible: usize) -> Vec3 {
        let (ox, oy) = Self::field_origin();
        let x = ox + col as f32 * STEP + CELL * 0.5;
        let y = oy - row_visible as f32 * STEP - CELL * 0.5;
        Vec3::new(x, y, 0.0)
    }

    fn push_rect(
        ctx: &mut Context,
        white: beton::TextureId,
        center: Vec3,
        size: [f32; 2],
        color: [f32; 4],
    ) {
        let mut s = Sprite::new(white, center, size);
        s.color = color;
        ctx.scene.push(s);
    }

    fn draw_number(
        ctx: &mut Context,
        font: &Font,
        center: Vec3,
        value: u32,
        glyph: [f32; 2],
        color: [f32; 4],
    ) {
        let text = value.to_string();
        let n = text.len() as f32;
        let advance = glyph[0] + DIGIT_TRACKING;
        let width = if n > 0.0 {
            n * advance - DIGIT_TRACKING
        } else {
            0.0
        };
        let mut x = center.x - width * 0.5;
        for ch in text.chars() {
            let mut buf = [0u8; 4];
            let s = ch.encode_utf8(&mut buf);
            ctx.scene
                .text(font, Vec3::new(x, center.y, center.z), glyph, color, s);
            x += advance;
        }
    }

    fn draw_piece_cells(
        ctx: &mut Context,
        white: beton::TextureId,
        piece: &Active,
        color: [f32; 4],
        z: f32,
    ) {
        for (x, y) in piece.cells() {
            if y < HIDDEN as i32 || x < 0 || y >= (HIDDEN + ROWS) as i32 {
                continue;
            }
            let row = (y as usize).saturating_sub(HIDDEN);
            let mut c = Self::cell_center(x as usize, row);
            c.z = z;
            Self::push_rect(ctx, white, c, [CELL, CELL], color);
        }
    }

    fn draw_mini(
        ctx: &mut Context,
        white: beton::TextureId,
        kind: PieceKind,
        origin: Vec3,
        cell: f32,
    ) {
        let color = kind.color();
        // Local cells in 0..3; center the 4×4 box on origin.
        for (lx, ly) in kind.cells(0) {
            let cx = origin.x + (lx as f32 - 1.5) * (cell + 0.04);
            let cy = origin.y - (ly as f32 - 1.0) * (cell + 0.04);
            Self::push_rect(
                ctx,
                white,
                Vec3::new(cx, cy, origin.z),
                [cell, cell],
                color,
            );
        }
    }

    fn handle_input(&mut self, ctx: &Context) {
        if ctx.input.pressed(Key::KeyR) {
            self.seed = self.seed.wrapping_add(1);
            self.board.reset(self.seed);
            self.das_left = 0.0;
            self.das_right = 0.0;
            return;
        }
        if self.board.over {
            return;
        }

        if ctx.input.pressed(Key::KeyZ) {
            self.board.try_rotate(-1);
        }
        if ctx.input.pressed(Key::ArrowUp)
            || ctx.input.pressed(Key::KeyW)
            || ctx.input.pressed(Key::KeyX)
        {
            self.board.try_rotate(1);
        }
        if ctx.input.pressed(Key::Space) {
            self.board.hard_drop();
        }
        if ctx.input.pressed(Key::KeyC) || ctx.input.pressed(Key::ShiftLeft) {
            self.board.hold();
        }

        // Horizontal DAS: immediate on press, then repeat.
        const DAS_DELAY: f32 = 0.17;
        const DAS_REPEAT: f32 = 0.04;
        let left = ctx.input.down(Key::ArrowLeft) || ctx.input.down(Key::KeyA);
        let right = ctx.input.down(Key::ArrowRight) || ctx.input.down(Key::KeyD);
        if left {
            if ctx.input.pressed(Key::ArrowLeft) || ctx.input.pressed(Key::KeyA) {
                self.board.try_shift(-1);
                self.das_left = 0.0;
            } else {
                self.das_left += ctx.dt;
                if self.das_left >= DAS_DELAY {
                    self.das_left -= DAS_REPEAT;
                    self.board.try_shift(-1);
                }
            }
        } else {
            self.das_left = 0.0;
        }
        if right {
            if ctx.input.pressed(Key::ArrowRight) || ctx.input.pressed(Key::KeyD) {
                self.board.try_shift(1);
                self.das_right = 0.0;
            } else {
                self.das_right += ctx.dt;
                if self.das_right >= DAS_DELAY {
                    self.das_right -= DAS_REPEAT;
                    self.board.try_shift(1);
                }
            }
        } else {
            self.das_right = 0.0;
        }

        let soft = ctx.input.down(Key::ArrowDown) || ctx.input.down(Key::KeyS);
        if soft {
            self.soft_acc += ctx.dt;
            while self.soft_acc >= 0.04 {
                self.soft_acc -= 0.04;
                if !self.board.soft_drop() {
                    break;
                }
            }
        } else {
            self.soft_acc = 0.0;
        }
    }

    fn draw(&self, ctx: &mut Context) {
        let white = self.white.as_ref().expect("boot").id();
        let font = self.font.as_ref().expect("boot");

        // Backdrop.
        Self::push_rect(
            ctx,
            white,
            Vec3::new(0.0, 0.0, -2.0),
            [40.0, WORLD_HEIGHT],
            [0.08, 0.09, 0.12, 1.0],
        );

        let (ox, oy) = Self::field_origin();
        let field_w = COLS as f32 * STEP - GAP;
        let field_h = ROWS as f32 * STEP - GAP;
        Self::push_rect(
            ctx,
            white,
            Vec3::new(ox + field_w * 0.5, oy - field_h * 0.5, -1.0),
            [field_w + 0.35, field_h + 0.35],
            [0.14, 0.15, 0.18, 1.0],
        );
        Self::push_rect(
            ctx,
            white,
            Vec3::new(ox + field_w * 0.5, oy - field_h * 0.5, -0.8),
            [field_w, field_h],
            [0.05, 0.06, 0.08, 1.0],
        );

        // Locked cells (visible rows only).
        for row in 0..ROWS {
            for col in 0..COLS {
                if let Some(kind) = self.board.cell(col, row + HIDDEN) {
                    let c = Self::cell_center(col, row);
                    Self::push_rect(ctx, white, c, [CELL, CELL], kind.color());
                }
            }
        }

        // Ghost + active.
        if let Some(active) = self.board.active {
            if let Some(gy) = self.board.ghost_y() {
                let mut ghost = active;
                ghost.y = gy;
                let mut color = active.kind.color();
                color[3] = 0.25;
                Self::draw_piece_cells(ctx, white, &ghost, color, 0.2);
            }
            Self::draw_piece_cells(ctx, white, &active, active.kind.color(), 0.5);
        }

        // Hold / next panels.
        let panel_x = ox + field_w + 2.2;
        Self::push_rect(
            ctx,
            white,
            Vec3::new(panel_x, oy - 1.2, -0.5),
            [3.2, 3.0],
            [0.12, 0.13, 0.16, 1.0],
        );
        if let Some(hold) = self.board.hold {
            Self::draw_mini(ctx, white, hold, Vec3::new(panel_x, oy - 1.0, 0.0), 0.45);
        }

        let mut ny = oy - 5.0;
        for kind in self.board.next.iter().take(3) {
            Self::push_rect(
                ctx,
                white,
                Vec3::new(panel_x, ny, -0.5),
                [3.2, 2.4],
                [0.12, 0.13, 0.16, 1.0],
            );
            Self::draw_mini(ctx, white, *kind, Vec3::new(panel_x, ny + 0.15, 0.0), 0.4);
            ny -= 2.8;
        }

        // Score / lines / level.
        let glyph = [0.45, 0.58];
        let label = [0.65, 0.68, 0.72, 1.0];
        Self::draw_number(
            ctx,
            font,
            Vec3::new(-6.5, 9.5, 1.0),
            self.board.score,
            glyph,
            label,
        );
        Self::draw_number(
            ctx,
            font,
            Vec3::new(-6.5, 8.5, 1.0),
            self.board.lines,
            [0.35, 0.45],
            [0.50, 0.55, 0.60, 1.0],
        );
        Self::draw_number(
            ctx,
            font,
            Vec3::new(-6.5, 7.7, 1.0),
            self.board.level,
            [0.35, 0.45],
            [0.50, 0.55, 0.60, 1.0],
        );

        if self.board.over {
            Self::push_rect(
                ctx,
                white,
                Vec3::new(ox + field_w * 0.5, oy - field_h * 0.5, 2.0),
                [field_w, field_h],
                [0.05, 0.04, 0.06, 0.55],
            );
        }
    }
}

impl Game for Tetris {
    fn boot(&mut self, ctx: &mut Context) {
        let white = ctx
            .gfx
            .create_texture(1, 1, &[255, 255, 255, 255])
            .expect("white");
        let (atlas, aw, ah) = digit_atlas_rgba();
        let font_id = ctx.gfx.create_texture(aw, ah, &atlas).expect("font");

        self.white = Some(ctx.gfx.pin(white).expect("pin white"));
        self.font_tex = Some(ctx.gfx.pin(font_id).expect("pin font"));
        self.font = Some(Font::grid(font_id, 10, 1, '0'));

        *ctx.camera = Camera {
            eye: Vec3::new(0.0, 0.0, 10.0),
            target: Vec3::ZERO,
            up: Vec3::Y,
            projection: Projection::Orthographic {
                height: WORLD_HEIGHT,
                near: 0.1,
                far: 100.0,
            },
        };
    }

    fn update(&mut self, ctx: &mut Context) {
        self.handle_input(ctx);
        if !self.board.over {
            self.board.tick(ctx.dt);
        }
        self.draw(ctx);
    }
}

fn digit_atlas_rgba() -> (Vec<u8>, u32, u32) {
    const DIGITS: [[u8; 5]; 10] = [
        [0b1111, 0b1001, 0b1001, 0b1001, 0b1111],
        [0b0010, 0b0110, 0b0010, 0b0010, 0b0111],
        [0b1111, 0b0001, 0b1111, 0b1000, 0b1111],
        [0b1111, 0b0001, 0b0111, 0b0001, 0b1111],
        [0b1001, 0b1001, 0b1111, 0b0001, 0b0001],
        [0b1111, 0b1000, 0b1111, 0b0001, 0b1111],
        [0b1111, 0b1000, 0b1111, 0b1001, 0b1111],
        [0b1111, 0b0001, 0b0010, 0b0100, 0b0100],
        [0b1111, 0b1001, 0b1111, 0b1001, 0b1111],
        [0b1111, 0b1001, 0b1111, 0b0001, 0b1111],
    ];
    const SCALE: u32 = 4;
    const SRC_W: u32 = 4;
    const SRC_H: u32 = 5;

    let mut rgba = vec![0u8; (DIGIT_ATLAS_W * DIGIT_ATLAS_H * 4) as usize];
    for (d, pattern) in DIGITS.iter().enumerate() {
        for row in 0..SRC_H {
            for col in 0..SRC_W {
                let on = (pattern[row as usize] >> (SRC_W - 1 - col)) & 1 == 1;
                if !on {
                    continue;
                }
                for sy in 0..SCALE {
                    for sx in 0..SCALE {
                        let x = d as u32 * DIGIT_CELL_W + col * SCALE + sx;
                        let y = row * SCALE + sy;
                        let i = ((y * DIGIT_ATLAS_W + x) * 4) as usize;
                        rgba[i] = 255;
                        rgba[i + 1] = 255;
                        rgba[i + 2] = 255;
                        rgba[i + 3] = 255;
                    }
                }
            }
        }
    }
    (rgba, DIGIT_ATLAS_W, DIGIT_ATLAS_H)
}

fn main() {
    if let Err(err) = run(
        Config {
            title: "tetris".into(),
            width: 720,
            height: 840,
            max_fps: Some(60),
        },
        Tetris::new(),
    ) {
        eprintln!("{err}");
        std::process::exit(1);
    }
}
