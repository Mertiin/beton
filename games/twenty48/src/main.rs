//! Classic 2048 sample on the beton API.

#![cfg_attr(windows, windows_subsystem = "windows")]

mod board;

use board::{Board, Dir};
use beton::{
    run, Camera, Config, Context, Font, Game, Key, Projection, Sprite, TextureHandle, Vec3,
};

const WORLD_HEIGHT: f32 = 20.0;

const BOARD_SIZE: f32 = 14.0;
const GAP: f32 = 0.5;
const CELL: f32 = (BOARD_SIZE - GAP * 5.0) / 4.0;
/// Extra horizontal gap between digit sprites (in addition to glyph width).
const DIGIT_TRACKING: f32 = 0.22;

/// Digit atlas: 10 cells of 16x20 (each source pixel scaled 4x from 4x5).
const DIGIT_CELL_W: u32 = 16;
const DIGIT_CELL_H: u32 = 20;
const DIGIT_ATLAS_W: u32 = DIGIT_CELL_W * 10;
const DIGIT_ATLAS_H: u32 = DIGIT_CELL_H;

struct Twenty48 {
    white: Option<TextureHandle>,
    font_tex: Option<TextureHandle>,
    font: Option<Font>,
    board: Board,
}

impl Twenty48 {
    fn new() -> Self {
        Self {
            white: None,
            font_tex: None,
            font: None,
            board: Board::new(1),
        }
    }

    fn cell_center(row: usize, col: usize) -> Vec3 {
        let origin_x = -BOARD_SIZE * 0.5 + GAP + CELL * 0.5;
        let origin_y = BOARD_SIZE * 0.5 - GAP - CELL * 0.5 - 1.0;
        let x = origin_x + col as f32 * (CELL + GAP);
        let y = origin_y - row as f32 * (CELL + GAP);
        Vec3::new(x, y, 0.0)
    }

    fn push_rect(ctx: &mut Context, white: beton::TextureId, center: Vec3, size: [f32; 2], color: [f32; 4], z: f32) {
        let mut s = Sprite::new(white, Vec3::new(center.x, center.y, z), size);
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
        z: f32,
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
            ctx.scene.text(font, Vec3::new(x, center.y, z), glyph, color, s);
            x += advance;
        }
    }
}

impl Game for Twenty48 {
    fn boot(&mut self, ctx: &mut Context) {
        let white = ctx
            .gfx
            .create_texture(1, 1, &[255, 255, 255, 255])
            .expect("white");
        let (atlas, aw, ah) = digit_atlas_rgba();
        let font_id = ctx.gfx.create_texture(aw, ah, &atlas).expect("font");

        self.white = Some(ctx.gfx.pin(white).expect("pin white"));
        self.font_tex = Some(ctx.gfx.pin(font_id).expect("pin font"));
        self.font = Some(Font {
            texture: font_id,
            columns: 10,
            rows: 1,
            first_char: '0',
        });

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
        if ctx.input.pressed(Key::KeyR) {
            self.board.reset();
        }

        if !self.board.over() {
            let dir = if ctx.input.pressed(Key::ArrowLeft) || ctx.input.pressed(Key::KeyA) {
                Some(Dir::Left)
            } else if ctx.input.pressed(Key::ArrowRight) || ctx.input.pressed(Key::KeyD) {
                Some(Dir::Right)
            } else if ctx.input.pressed(Key::ArrowUp) || ctx.input.pressed(Key::KeyW) {
                Some(Dir::Up)
            } else if ctx.input.pressed(Key::ArrowDown) || ctx.input.pressed(Key::KeyS) {
                Some(Dir::Down)
            } else {
                None
            };
            if let Some(dir) = dir {
                self.board.move_dir(dir);
            }
        }

        let white = self.white.as_ref().expect("boot").id();
        let font = self.font.clone().expect("boot");

        // Page background.
        Self::push_rect(
            ctx,
            white,
            Vec3::ZERO,
            [40.0, WORLD_HEIGHT],
            [0.98, 0.97, 0.94, 1.0],
            -2.0,
        );

        // Board plate.
        let board_y = -1.0;
        Self::push_rect(
            ctx,
            white,
            Vec3::new(0.0, board_y, -1.0),
            [BOARD_SIZE, BOARD_SIZE],
            [0.73, 0.68, 0.63, 1.0],
            -1.0,
        );

        // Win gold border.
        if self.board.won() && !self.board.over() {
            Self::push_rect(
                ctx,
                white,
                Vec3::new(0.0, board_y, -1.5),
                [BOARD_SIZE + 0.5, BOARD_SIZE + 0.5],
                [0.90, 0.75, 0.20, 1.0],
                -1.5,
            );
        }

        let cells = *self.board.cells();
        for row in 0..4 {
            for col in 0..4 {
                let center = Self::cell_center(row, col);
                let value = cells[row * 4 + col];
                let (tile_color, digit_color) = tile_colors(value);
                Self::push_rect(
                    ctx,
                    white,
                    center,
                    [CELL, CELL],
                    tile_color,
                    0.0,
                );
                if value != 0 {
                    let digits = digit_count(value);
                    let glyph_w = if digits <= 2 {
                        CELL * 0.28
                    } else if digits == 3 {
                        CELL * 0.22
                    } else {
                        CELL * 0.17
                    };
                    let glyph_h = glyph_w * (DIGIT_CELL_H as f32 / DIGIT_CELL_W as f32);
                    Self::draw_number(
                        ctx,
                        &font,
                        Vec3::new(center.x, center.y, 1.0),
                        value,
                        [glyph_w, glyph_h],
                        digit_color,
                        1.0,
                    );
                }
            }
        }

        // Score HUD (single number centered above the board).
        let glyph = [0.55, 0.7];
        Self::draw_number(
            ctx,
            &font,
            Vec3::new(0.0, 8.5, 2.0),
            self.board.score(),
            glyph,
            [0.47, 0.43, 0.40, 1.0],
            2.0,
        );

        if self.board.over() {
            Self::push_rect(
                ctx,
                white,
                Vec3::new(0.0, board_y, 3.0),
                [BOARD_SIZE, BOARD_SIZE],
                [0.15, 0.12, 0.10, 0.55],
                3.0,
            );
        }
    }
}

fn digit_count(mut n: u32) -> u32 {
    if n == 0 {
        return 1;
    }
    let mut c = 0;
    while n > 0 {
        n /= 10;
        c += 1;
    }
    c
}

fn tile_colors(value: u32) -> ([f32; 4], [f32; 4]) {
    let dark = [0.47, 0.43, 0.40, 1.0];
    let light = [0.98, 0.96, 0.95, 1.0];
    let empty = [0.80, 0.75, 0.71, 1.0];
    match value {
        0 => (empty, dark),
        2 => ([0.93, 0.89, 0.85, 1.0], dark),
        4 => ([0.93, 0.88, 0.78, 1.0], dark),
        8 => ([0.95, 0.69, 0.47, 1.0], light),
        16 => ([0.96, 0.58, 0.39, 1.0], light),
        32 => ([0.96, 0.49, 0.37, 1.0], light),
        64 => ([0.96, 0.37, 0.23, 1.0], light),
        128 => ([0.93, 0.81, 0.45, 1.0], light),
        256 => ([0.93, 0.80, 0.38, 1.0], light),
        512 => ([0.93, 0.78, 0.31, 1.0], light),
        1024 => ([0.93, 0.77, 0.25, 1.0], light),
        2048 => ([0.93, 0.76, 0.18, 1.0], light),
        _ => ([0.24, 0.23, 0.20, 1.0], light),
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
            title: "2048".into(),
            width: 720,
            height: 800,
            max_fps: Some(60),
        },
        Twenty48::new(),
    ) {
        eprintln!("{err}");
        std::process::exit(1);
    }
}
