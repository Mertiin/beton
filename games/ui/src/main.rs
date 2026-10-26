//! Screen UI sample: HUD with hover/click, plus an overlay menu.

use beton::{
    run, Align, Config, Context, Direction, Display, Fit, Font, Game, Justify, Size, Style,
    TextureHandle, UiId,
};

const HUD: UiId = UiId(1);
const ICON: UiId = UiId(2);
const COUNT: UiId = UiId(3);
const OPEN: UiId = UiId(4);
const DIMMER: UiId = UiId(5);
const SETTINGS: UiId = UiId(6);
const BACK: UiId = UiId(7);

const DIGIT_CELL_W: u32 = 16;
const DIGIT_CELL_H: u32 = 20;
const DIGIT_ATLAS_W: u32 = DIGIT_CELL_W * 10;
const DIGIT_ATLAS_H: u32 = DIGIT_CELL_H;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Screen {
    Play,
    Settings,
}

struct UiDemo {
    font: Option<Font>,
    font_tex: Option<TextureHandle>,
    icon: Option<TextureHandle>,
    count: u32,
    screen: Screen,
}

impl UiDemo {
    fn new() -> Self {
        Self {
            font: None,
            font_tex: None,
            icon: None,
            count: 0,
            screen: Screen::Play,
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
        if ctx.ui.clicked(OPEN) {
            self.screen = Screen::Settings;
        }
        if ctx.ui.clicked(BACK) {
            self.screen = Screen::Play;
        }
        if ctx.ui.clicked(COUNT) || ctx.ui.clicked(ICON) {
            self.count = self.count.wrapping_add(1) % 100;
        }

        let font = self.font.as_ref().expect("boot");
        let icon = self.icon.as_ref().expect("boot").id();
        let open_hot = ctx.ui.hovered(OPEN);
        let count_hot = ctx.ui.hovered(COUNT) || ctx.ui.hovered(ICON);
        let back_hot = ctx.ui.hovered(BACK);

        let count_str = format!("{:02}", self.count);

        ctx.ui.node(
            HUD,
            Style {
                display: Display::Flex,
                direction: Direction::Row,
                width: Size::Px(ctx.fb_size[0]),
                height: Size::Px(72.0),
                padding: Style::edges(12.0),
                gap: 16.0,
                align: Align::Center,
                justify: Justify::Start,
                background: Some([0.12, 0.14, 0.18, 0.92]),
                ..Style::default()
            },
            |ui| {
                ui.image(
                    ICON,
                    icon,
                    Style {
                        width: Size::Px(48.0),
                        height: Size::Px(48.0),
                        fit: Fit::Fill,
                        color: if count_hot {
                            [1.0, 1.0, 0.7, 1.0]
                        } else {
                            [1.0, 1.0, 1.0, 1.0]
                        },
                        ..Style::default()
                    },
                );
                ui.text(
                    COUNT,
                    font,
                    [24.0, 30.0],
                    Style {
                        color: if count_hot {
                            [1.0, 1.0, 0.7, 1.0]
                        } else {
                            [1.0, 1.0, 1.0, 1.0]
                        },
                        ..Style::default()
                    },
                    &count_str,
                );
                ui.node(
                    OPEN,
                    Style {
                        width: Size::Px(140.0),
                        height: Size::Px(40.0),
                        margin: [0.0, 0.0, 0.0, 24.0],
                        background: Some(if open_hot {
                            [0.35, 0.55, 0.95, 1.0]
                        } else {
                            [0.22, 0.38, 0.72, 1.0]
                        }),
                        ..Style::default()
                    },
                    |_| {},
                );
            },
        );

        if self.screen == Screen::Settings {
            // Pull back over the HUD so the dimmer covers the whole window.
            ctx.ui.node(
                DIMMER,
                Style {
                    display: Display::Flex,
                    direction: Direction::Column,
                    width: Size::Px(ctx.fb_size[0]),
                    height: Size::Px(ctx.fb_size[1]),
                    margin: [-72.0, 0.0, 0.0, 0.0],
                    justify: Justify::Center,
                    align: Align::Center,
                    background: Some([0.0, 0.0, 0.0, 0.55]),
                    ..Style::default()
                },
                |ui| {
                    ui.node(
                        SETTINGS,
                        Style {
                            display: Display::Flex,
                            direction: Direction::Column,
                            width: Size::Px(280.0),
                            height: Size::Px(160.0),
                            padding: Style::edges(20.0),
                            justify: Justify::Center,
                            align: Align::Center,
                            background: Some([0.16, 0.18, 0.24, 1.0]),
                            ..Style::default()
                        },
                        |ui| {
                            ui.node(
                                BACK,
                                Style {
                                    width: Size::Px(120.0),
                                    height: Size::Px(44.0),
                                    background: Some(if back_hot {
                                        [0.9, 0.45, 0.35, 1.0]
                                    } else {
                                        [0.7, 0.3, 0.25, 1.0]
                                    }),
                                    ..Style::default()
                                },
                                |_| {},
                            );
                        },
                    );
                },
            );
        }
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
