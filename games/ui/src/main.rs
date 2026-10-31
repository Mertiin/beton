//! Screen UI sample: Yoga playground-style phone frame with hover/click.

use beton::{run, style, Config, Context, Fit, Font, Game, TextureHandle, UiId};

const BODY: UiId = UiId(1);
const ICON: UiId = UiId(2);
const COUNT: UiId = UiId(3);
const TAB0: UiId = UiId(4);
const TAB1: UiId = UiId(5);
const TAB2: UiId = UiId(6);
const TAB3: UiId = UiId(7);

const DIGIT_CELL_W: u32 = 16;
const DIGIT_CELL_H: u32 = 20;
const DIGIT_ATLAS_W: u32 = DIGIT_CELL_W * 10;
const DIGIT_ATLAS_H: u32 = DIGIT_CELL_H;

struct UiDemo {
    font: Option<Font>,
    font_tex: Option<TextureHandle>,
    icon: Option<TextureHandle>,
    count: u32,
    tab: u32,
}

impl UiDemo {
    fn new() -> Self {
        Self {
            font: None,
            font_tex: None,
            icon: None,
            count: 0,
            tab: 0,
        }
    }
}

impl Game for UiDemo {
    fn boot(&mut self, ctx: &mut Context) {
        let (atlas, aw, ah) = digit_atlas_rgba();
        let font_tex = ctx
            .gfx
            .create_texture(aw, ah, &atlas)
            .expect("digit atlas");
        self.font_tex = Some(ctx.gfx.pin(font_tex).expect("pin font"));
        self.font = Some(Font::grid(font_tex, 10, 1, '0'));

        let mut icon_rgba = vec![0u8; 32 * 32 * 4];
        for y in 0..32u32 {
            for x in 0..32u32 {
                let i = ((y * 32 + x) * 4) as usize;
                let on = (x / 4 + y / 4) % 2 == 0;
                if on {
                    icon_rgba[i] = 80;
                    icon_rgba[i + 1] = 180;
                    icon_rgba[i + 2] = 255;
                    icon_rgba[i + 3] = 255;
                } else {
                    icon_rgba[i] = 40;
                    icon_rgba[i + 1] = 80;
                    icon_rgba[i + 2] = 140;
                    icon_rgba[i + 3] = 255;
                }
            }
        }
        let icon = ctx
            .gfx
            .create_texture(32, 32, &icon_rgba)
            .expect("icon");
        self.icon = Some(ctx.gfx.pin(icon).expect("pin icon"));
    }

    fn update(&mut self, ctx: &mut Context) {
        if ctx.ui.clicked(COUNT) || ctx.ui.clicked(ICON) || ctx.ui.clicked(BODY) {
            self.count = self.count.wrapping_add(1) % 100;
        }
        for (i, id) in [TAB0, TAB1, TAB2, TAB3].into_iter().enumerate() {
            if ctx.ui.clicked(id) {
                self.tab = i as u32;
            }
        }

        let font = self.font.as_ref().expect("boot");
        let icon = self.icon.as_ref().expect("boot").id();
        let count_hot = ctx.ui.hovered(COUNT) || ctx.ui.hovered(ICON) || ctx.ui.hovered(BODY);
        let count_str = format!("{:02}", self.count);

        let tab_color = |i: u32, hot: bool| -> [f32; 4] {
            if self.tab == i {
                [0.35, 0.55, 0.95, 1.0]
            } else if hot {
                [0.45, 0.5, 0.6, 1.0]
            } else {
                [0.28, 0.32, 0.4, 1.0]
            }
        };

        ctx.ui.node(
            UiId(0),
            style!(
                width: 250 pt,
                height: 475 pt,
                padding: 10 pt,
                background: Some([0.1, 0.11, 0.14, 1.0]),
            ),
            |ui| {
                ui.node(UiId(0), style!(flex: 1, row_gap: 10 pt), |ui| {
                    ui.node(
                        UiId(0),
                        style!(
                            height: 60 pt,
                            flex_direction: row,
                            align_items: center,
                            padding_inline: 8 pt,
                            gap: 12 pt,
                            background: Some([0.16, 0.18, 0.24, 1.0]),
                        ),
                        |ui| {
                            ui.image(
                                ICON,
                                icon,
                                style!(
                                    width: 40 pt,
                                    height: 40 pt,
                                    fit: Fit::Fill,
                                    color: if count_hot {
                                        [1.0, 1.0, 0.7, 1.0]
                                    } else {
                                        [1.0, 1.0, 1.0, 1.0]
                                    },
                                ),
                            );
                            ui.text(
                                COUNT,
                                font,
                                [20.0, 26.0],
                                style!(
                                    color: if count_hot {
                                        [1.0, 1.0, 0.7, 1.0]
                                    } else {
                                        [1.0, 1.0, 1.0, 1.0]
                                    },
                                ),
                                &count_str,
                            );
                        },
                    );
                    ui.node(
                        BODY,
                        style!(
                            flex: 1,
                            margin_inline: 10 pt,
                            background: Some(if count_hot {
                                [0.22, 0.38, 0.55, 1.0]
                            } else {
                                [0.18, 0.28, 0.4, 1.0]
                            }),
                        ),
                        |_| {},
                    );
                    ui.node(
                        UiId(0),
                        style!(
                            flex: 2,
                            margin_inline: 10 pt,
                            background: Some([0.2, 0.22, 0.28, 1.0]),
                        ),
                        |_| {},
                    );
                    ui.node(
                        UiId(0),
                        style!(
                            position: absolute,
                            width: 100 %,
                            bottom: 0 pt,
                            height: 64 pt,
                            flex_direction: row,
                            align_items: center,
                            justify_content: space_around,
                            background: Some([0.12, 0.14, 0.18, 0.96]),
                        ),
                        |ui| {
                            for (i, id) in [TAB0, TAB1, TAB2, TAB3].into_iter().enumerate() {
                                let hot = ui.hovered(id);
                                ui.node(
                                    id,
                                    style!(
                                        width: 40 pt,
                                        height: 40 pt,
                                        background: Some(tab_color(i as u32, hot)),
                                    ),
                                    |_| {},
                                );
                            }
                        },
                    );
                });
            },
        );
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
            title: "ui".into(),
            width: 720,
            height: 480,
            max_fps: Some(60),
        },
        UiDemo::new(),
    ) {
        eprintln!("{err}");
        std::process::exit(1);
    }
}
