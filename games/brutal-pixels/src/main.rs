//! Side-view tower auto-battler on the beton API.

#![cfg_attr(windows, windows_subsystem = "windows")]

mod rules;

use std::collections::HashMap;
use std::path::Path;

use beton::{
    run, Align, Camera, Config, Context, Direction, Display, Fit, Font, Game, Justify, Key,
    Projection, Size, Sprite, Style, TextureHandle, TextureId, Ui, UiId, Vec3,
};
use rules::{
    ArcKind, PartKind, Phase, Run, Side, MAX_TURNS,
};

const ASSETS: &[&str] = &[
    "assets/foundation.png",
    "assets/wall.png",
    "assets/spikes.png",
    "assets/oil.png",
    "assets/cement.png",
    "assets/archer.png",
    "assets/mason.png",
    "assets/banner.png",
    "assets/ram.png",
    "assets/ballista.png",
    "assets/nest.png",
    "assets/lantern.png",
    "assets/chick.png",
    "assets/tower_player.png",
    "assets/tower_enemy.png",
    "assets/battlefield_bg.jpg",
    "assets/ui_wood_panel.jpg",
    "assets/Roboto-Regular.ttf",
];

const TOWER_PLAYER_ASSET: &str = "assets/tower_player.png";
const TOWER_ENEMY_ASSET: &str = "assets/tower_enemy.png";
const BATTLEFIELD_BG_ASSET: &str = "assets/battlefield_bg.jpg";
const UI_WOOD_ASSET: &str = "assets/ui_wood_panel.jpg";
const UI_FONT_ASSET: &str = "assets/Roboto-Regular.ttf";

const PART_ASSETS: &[(PartKind, &str)] = &[
    (PartKind::Foundation, "assets/foundation.png"),
    (PartKind::Wall, "assets/wall.png"),
    (PartKind::Spikes, "assets/spikes.png"),
    (PartKind::Oil, "assets/oil.png"),
    (PartKind::Cement, "assets/cement.png"),
    (PartKind::Archer, "assets/archer.png"),
    (PartKind::Mason, "assets/mason.png"),
    (PartKind::Banner, "assets/banner.png"),
    (PartKind::Ram, "assets/ram.png"),
    (PartKind::Ballista, "assets/ballista.png"),
    (PartKind::Nest, "assets/nest.png"),
    (PartKind::Lantern, "assets/lantern.png"),
    (PartKind::Chick, "assets/chick.png"),
];

const ID_HUD: UiId = UiId(1);
const ID_BAR: UiId = UiId(2);
const ID_FIGHT: UiId = UiId(3);
const ID_HINT: UiId = UiId(4);
const ID_RESULT: UiId = UiId(5);
const ID_RESTART: UiId = UiId(6);
const ID_TOWER: UiId = UiId(7);
const ID_REROLL: UiId = UiId(8);

const OFFER_BASE: u32 = 100;
const MODULE_BASE: u32 = 200;

const FONT_COLS: u32 = 16;
const FONT_ROWS: u32 = 6;
const FONT_FIRST: char = ' ';
const GLYPH_W: u32 = 5;
const GLYPH_H: u32 = 7;
const GLYPH_PAD: u32 = 1;
const FONT_CELL_W: u32 = GLYPH_W + GLYPH_PAD;
const FONT_CELL_H: u32 = GLYPH_H + GLYPH_PAD;

/// World: X left→right across the field, Y up.
/// Camera is framed each frame so the walk lane clears the shop dock.
const WORLD_H: f32 = 10.0;
const GROUND_Y: f32 = -0.55;
const TOWER_BASE_Y: f32 = -0.05;
const UI_TOP_H: f32 = 52.0;
const UI_HINT_H: f32 = 24.0;
const UI_BAR_H: f32 = 108.0;
const UI_BAR_FIGHT_H: f32 = 60.0;
/// Extra world units of gap between ground and the top of the dock.
const UI_GROUND_CLEARANCE: f32 = 2.2;

struct BrutalPixels {
    run: Run,
    seed: u64,
    font: Option<Font>,
    ui_font: Option<Font>,
    font_tex: Option<TextureHandle>,
    ui_font_tex: Option<TextureHandle>,
    icons: HashMap<PartKind, TextureHandle>,
    tower_player: Option<TextureHandle>,
    tower_enemy: Option<TextureHandle>,
    battlefield: Option<TextureHandle>,
    ui_wood: Option<TextureHandle>,
}

impl BrutalPixels {
    fn new() -> Self {
        let seed = 0xB07E_1u64;
        Self {
            run: Run::new(seed),
            seed,
            font: None,
            ui_font: None,
            font_tex: None,
            ui_font_tex: None,
            icons: HashMap::new(),
            tower_player: None,
            tower_enemy: None,
            battlefield: None,
            ui_wood: None,
        }
    }

    fn icon(&self, kind: PartKind) -> TextureId {
        self.icons[&kind].id()
    }

    fn restart(&mut self) {
        self.seed = self.seed.wrapping_add(0x9E37_79B9);
        self.run.restart(self.seed);
    }

    fn field_x(norm: f32, half_w: f32) -> f32 {
        -half_w * 0.85 + norm * half_w * 1.7
    }

    fn handle_input(&mut self, ctx: &Context) {
        match self.run.phase {
            Phase::Prep => {
                if ctx.ui.clicked(ID_FIGHT) {
                    self.run.start_fight();
                    return;
                }
                if ctx.ui.clicked(ID_REROLL) {
                    self.run.reroll();
                }
                for i in 0..self.run.bar.len() {
                    if ctx.ui.clicked(UiId(OFFER_BASE + i as u32)) {
                        self.run.buy_offer(i);
                    }
                }
                for i in 0..self.run.player.modules.len() {
                    if ctx.ui.clicked(UiId(MODULE_BASE + i as u32)) {
                        // Second click on selected sells.
                        if self.run.selected_module == Some(i) {
                            self.run.sell_module(i);
                        } else {
                            self.run.select_module(i);
                        }
                    }
                }
            }
            Phase::Won | Phase::Lost => {
                if ctx.ui.clicked(ID_RESTART) || ctx.input.pressed(Key::KeyR) {
                    self.restart();
                }
            }
            Phase::Fight => {}
        }
    }

    fn draw_world(&self, ctx: &mut Context) {
        let fb = ctx.fb_size;
        let aspect = (fb[0] / fb[1].max(1.0)).max(0.5);
        let half_w = WORLD_H * aspect * 0.5;
        let white = ctx.gfx.white_texture();

        // Painted battlefield — road band aligned near GROUND_Y.
        if let Some(bg) = &self.battlefield {
            let bg_sprite = Sprite::new(
                bg.id(),
                Vec3::new(0.0, GROUND_Y + 3.6, -2.5),
                [half_w * 2.3, WORLD_H * 0.95],
            );
            ctx.scene.push(bg_sprite);
        } else {
            let mut sky = Sprite::new(white, Vec3::new(0.0, 4.0, -2.4), [half_w * 2.3, 7.0]);
            sky.color = [0.45, 0.78, 0.95, 1.0];
            ctx.scene.push(sky);
        }

        // Soft vignette bands so towers read against the art.
        let mut top_shade = Sprite::new(white, Vec3::new(0.0, GROUND_Y + 7.2, -2.0), [half_w * 2.3, 1.2]);
        top_shade.color = [0.1, 0.2, 0.35, 0.16];
        ctx.scene.push(top_shade);

        // Walk lane highlight on top of the painted road.
        let mut path_glow =
            Sprite::new(white, Vec3::new(0.0, GROUND_Y + 0.05, -0.7), [half_w * 1.5, 0.38]);
        path_glow.color = [1.0, 0.92, 0.55, 0.22];
        ctx.scene.push(path_glow);

        let player_x = Self::field_x(0.0, half_w);
        let enemy_x = Self::field_x(1.0, half_w);

        self.draw_tower_body(ctx, Side::Player, player_x);
        self.draw_tower_body(ctx, Side::Enemy, enemy_x);
        self.draw_modules(ctx, Side::Player, player_x);
        self.draw_modules(ctx, Side::Enemy, enemy_x);

        let hp_y = TOWER_BASE_Y + 4.15;
        if let Some(fight) = &self.run.fight {
            self.draw_hp_bar(
                ctx,
                white,
                Vec3::new(player_x, hp_y, 0.5),
                2.5,
                fight.player.hp,
                fight.player.max_hp,
            );
            self.draw_hp_bar(
                ctx,
                white,
                Vec3::new(enemy_x, hp_y, 0.5),
                2.5,
                fight.enemy.hp,
                fight.enemy.max_hp,
            );

            for u in &fight.units {
                let x = Self::field_x(u.x, half_w);
                let y = GROUND_Y + 0.62 + u.lane * 0.28;
                let size = match u.kind {
                    PartKind::Ram => 1.05,
                    _ => 0.82,
                };
                // Soft oval shadow under walker.
                let mut shadow = Sprite::new(white, Vec3::new(x, GROUND_Y + 0.12, 0.15), [size * 0.7, 0.18]);
                shadow.color = [0.2, 0.12, 0.06, 0.35];
                ctx.scene.push(shadow);
                let mut s = Sprite::new(
                    self.icon(u.kind),
                    Vec3::new(x, y, 0.2),
                    [size, size],
                );
                if u.side == Side::Enemy {
                    s.uv = [1.0, 0.0, 0.0, 1.0];
                }
                ctx.scene.push(s);
                self.draw_hp_bar(
                    ctx,
                    white,
                    Vec3::new(x, y + size * 0.62, 0.3),
                    size * 0.85,
                    u.hp,
                    u.max_hp,
                );
            }

            for shot in &fight.shots {
                let sx = Self::field_x(shot.start_x, half_w);
                let ex = Self::field_x(shot.end_x, half_w);
                let lift = match shot.arc {
                    ArcKind::Flat => 0.7,
                    ArcKind::Arc => 2.0,
                    ArcKind::Mortar => 3.2,
                };
                let t = shot.t.clamp(0.0, 1.0);
                let omt = 1.0 - t;
                let x = omt * omt * sx + 2.0 * omt * t * ((sx + ex) * 0.5) + t * t * ex;
                let mid_y = GROUND_Y + 2.1 + lift;
                let y = omt * omt * (GROUND_Y + 2.3)
                    + 2.0 * omt * t * mid_y
                    + t * t * (GROUND_Y + 1.5);
                // Cartoon projectile with outline.
                let mut outline = Sprite::new(white, Vec3::new(x, y, 0.58), [0.34, 0.34]);
                outline.color = [0.15, 0.1, 0.05, 1.0];
                ctx.scene.push(outline);
                let mut s = Sprite::new(white, Vec3::new(x, y, 0.6), [0.24, 0.24]);
                s.color = match shot.arc {
                    ArcKind::Flat => [1.0, 0.92, 0.25, 1.0],
                    ArcKind::Arc => [0.35, 0.95, 1.0, 1.0],
                    ArcKind::Mortar => [1.0, 0.5, 0.2, 1.0],
                };
                ctx.scene.push(s);
                let _ = shot.target;
            }
        } else {
            let php = self.run.player.max_hp();
            let ehp = self.run.enemy.max_hp();
            self.draw_hp_bar(ctx, white, Vec3::new(player_x, hp_y, 0.5), 2.5, php, php);
            self.draw_hp_bar(ctx, white, Vec3::new(enemy_x, hp_y, 0.5), 2.5, ehp, ehp);
        }
    }

    fn draw_tower_body(&self, ctx: &mut Context, side: Side, x: f32) {
        let tex = match side {
            Side::Player => self.tower_player.as_ref().map(|t| t.id()),
            Side::Enemy => self.tower_enemy.as_ref().map(|t| t.id()),
        };
        let Some(tex) = tex else { return };
        let white = ctx.gfx.white_texture();
        // Ground blob shadow.
        let mut shadow = Sprite::new(white, Vec3::new(x, GROUND_Y + 0.08, -0.05), [2.2, 0.28]);
        shadow.color = [0.2, 0.12, 0.05, 0.4];
        ctx.scene.push(shadow);
        // Keep tower feet on the road and clear of the shop dock.
        let body = Sprite::new(
            tex,
            Vec3::new(x, TOWER_BASE_Y + 1.75, -0.15),
            [2.7, 4.2],
        );
        ctx.scene.push(body);
    }

    fn draw_modules(&self, ctx: &mut Context, side: Side, tower_x: f32) {
        let loadout = match side {
            Side::Player => &self.run.player,
            Side::Enemy => {
                if let Some(f) = &self.run.fight {
                    &f.enemy.loadout
                } else {
                    &self.run.enemy
                }
            }
        };
        let white = ctx.gfx.white_texture();
        let face = match side {
            Side::Player => 1.45,
            Side::Enemy => -1.45,
        };
        for (i, m) in loadout.modules.iter().enumerate() {
            let y = TOWER_BASE_Y + 0.85 + i as f32 * 0.75;
            // Cartoon plank with dark outline.
            let mut plank_out = Sprite::new(
                white,
                Vec3::new(tower_x + face * 0.5, y - 0.4, 0.04),
                [1.4, 0.26],
            );
            plank_out.color = [0.25, 0.14, 0.06, 1.0];
            ctx.scene.push(plank_out);
            let mut plank = Sprite::new(
                white,
                Vec3::new(tower_x + face * 0.5, y - 0.38, 0.05),
                [1.28, 0.16],
            );
            plank.color = [0.72, 0.48, 0.24, 1.0];
            ctx.scene.push(plank);
            let mut s = Sprite::new(
                self.icon(m.kind),
                Vec3::new(tower_x + face, y, 0.12),
                [0.9, 0.9],
            );
            if side == Side::Enemy {
                s.uv = [1.0, 0.0, 0.0, 1.0];
            }
            if m.level >= 2 {
                s.color = [1.0, 0.95, 0.55, 1.0];
            }
            ctx.scene.push(s);
        }
    }

    fn draw_hp_bar(
        &self,
        ctx: &mut Context,
        white: TextureId,
        center: Vec3,
        width: f32,
        hp: i32,
        max_hp: i32,
    ) {
        let frac = if max_hp > 0 {
            (hp as f32 / max_hp as f32).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let mut outline = Sprite::new(white, center, [width + 0.08, 0.28]);
        outline.color = [0.12, 0.08, 0.05, 1.0];
        ctx.scene.push(outline);
        let mut back = Sprite::new(
            white,
            Vec3::new(center.x, center.y, center.z + 0.005),
            [width, 0.2],
        );
        back.color = [0.25, 0.18, 0.14, 1.0];
        ctx.scene.push(back);
        if frac > 0.0 {
            let w = width * frac;
            let mut fill = Sprite::new(
                white,
                Vec3::new(center.x - (width - w) * 0.5, center.y, center.z + 0.01),
                [w, 0.16],
            );
            fill.color = if frac > 0.35 {
                [0.35, 0.92, 0.4, 1.0]
            } else {
                [1.0, 0.4, 0.28, 1.0]
            };
            ctx.scene.push(fill);
        }
    }

    fn frame_camera(&self, ctx: &mut Context, bar_h: f32) {
        let fb_h = ctx.fb_size[1].max(1.0);
        let bottom_frac = (UI_HINT_H + bar_h) / fb_h;
        // World Y grows upward; dock eats the bottom of the screen. Aim so
        // GROUND_Y sits above the dock top by UI_GROUND_CLEARANCE.
        // y_dock_top = target_y - H/2 + bottom_frac*H
        // GROUND_Y = y_dock_top + clearance
        // => target_y = GROUND_Y - clearance + H*(0.5 - bottom_frac)
        let target_y = GROUND_Y - UI_GROUND_CLEARANCE + WORLD_H * (0.5 - bottom_frac);
        ctx.camera.eye = Vec3::new(0.0, target_y + 0.2, 14.0);
        ctx.camera.target = Vec3::new(0.0, target_y, 0.0);
        ctx.camera.projection = Projection::Orthographic {
            height: WORLD_H,
            near: 0.1,
            far: 100.0,
        };
    }
}

fn labeled_button(
    ui: &mut Ui,
    id: UiId,
    font: &Font,
    label: &str,
    w: f32,
    h: f32,
    hot: bool,
    color: [f32; 4],
    hot_color: [f32; 4],
) {
    // Dark rim + fill reads like a mobile-game CTA.
    ui.node(
        id,
        Style {
            display: Display::Flex,
            direction: Direction::Row,
            width: Size::Px(w),
            height: Size::Px(h),
            padding: Style::edges(3.0),
            justify: Justify::Center,
            align: Align::Center,
            background: Some([0.18, 0.1, 0.05, 1.0]),
            ..Style::default()
        },
        |ui| {
            ui.node(
                UiId(id.0 + 40_000),
                Style {
                    display: Display::Flex,
                    direction: Direction::Row,
                    width: Size::Px(w - 6.0),
                    height: Size::Px(h - 6.0),
                    justify: Justify::Center,
                    align: Align::Center,
                    background: Some(if hot { hot_color } else { color }),
                    ..Style::default()
                },
                |ui| {
                    ui.text(
                        UiId(id.0 + 50_000),
                        font,
                        [0.0, 20.0],
                        Style {
                            color: [1.0, 1.0, 1.0, 1.0],
                            ..Style::default()
                        },
                        label,
                    );
                },
            );
        },
    );
}

fn hud_chip(ui: &mut Ui, id: UiId, font: &Font, label: &str, fill: [f32; 4], text: [f32; 4]) {
    ui.node(
        id,
        Style {
            display: Display::Flex,
            direction: Direction::Row,
            height: Size::Px(40.0),
            padding: [0.0, 14.0, 0.0, 14.0],
            justify: Justify::Center,
            align: Align::Center,
            background: Some([0.2, 0.12, 0.06, 1.0]),
            ..Style::default()
        },
        |ui| {
            ui.node(
                UiId(id.0 + 1),
                Style {
                    display: Display::Flex,
                    direction: Direction::Row,
                    height: Size::Px(34.0),
                    padding: [0.0, 12.0, 0.0, 12.0],
                    justify: Justify::Center,
                    align: Align::Center,
                    background: Some(fill),
                    ..Style::default()
                },
                |ui| {
                    ui.text(
                        UiId(id.0 + 2),
                        font,
                        [0.0, 20.0],
                        Style {
                            color: text,
                            ..Style::default()
                        },
                        label,
                    );
                },
            );
        },
    );
}

impl Game for BrutalPixels {
    fn assets(&self) -> &[&'static str] {
        ASSETS
    }

    fn boot(&mut self, ctx: &mut Context) {
        let (atlas, aw, ah) = font_atlas_rgba();
        let font_tex = ctx
            .gfx
            .create_texture(aw, ah, &atlas)
            .expect("font atlas");
        self.font_tex = Some(ctx.gfx.pin(font_tex).expect("pin font"));
        self.font = Some(Font::grid(font_tex, FONT_COLS, FONT_ROWS, FONT_FIRST));

        let ui_font = Font::load(ctx.gfx, Path::new(UI_FONT_ASSET), 48.0).expect("ui font");
        self.ui_font_tex = Some(ctx.gfx.pin(ui_font.texture).expect("pin ui font"));
        self.ui_font = Some(ui_font);

        for &(kind, path) in PART_ASSETS {
            let id = ctx
                .gfx
                .load_texture(Path::new(path))
                .unwrap_or_else(|e| panic!("load {path}: {e}"));
            self.icons.insert(kind, ctx.gfx.pin(id).expect("pin icon"));
        }

        let player_id = ctx
            .gfx
            .load_texture(Path::new(TOWER_PLAYER_ASSET))
            .unwrap_or_else(|e| panic!("load {TOWER_PLAYER_ASSET}: {e}"));
        self.tower_player = Some(ctx.gfx.pin(player_id).expect("pin tower player"));
        let enemy_id = ctx
            .gfx
            .load_texture(Path::new(TOWER_ENEMY_ASSET))
            .unwrap_or_else(|e| panic!("load {TOWER_ENEMY_ASSET}: {e}"));
        self.tower_enemy = Some(ctx.gfx.pin(enemy_id).expect("pin tower enemy"));

        let bg_id = ctx
            .gfx
            .load_texture(Path::new(BATTLEFIELD_BG_ASSET))
            .unwrap_or_else(|e| panic!("load {BATTLEFIELD_BG_ASSET}: {e}"));
        self.battlefield = Some(ctx.gfx.pin(bg_id).expect("pin battlefield"));
        let wood_id = ctx
            .gfx
            .load_texture(Path::new(UI_WOOD_ASSET))
            .unwrap_or_else(|e| panic!("load {UI_WOOD_ASSET}: {e}"));
        self.ui_wood = Some(ctx.gfx.pin(wood_id).expect("pin ui wood"));

        *ctx.camera = Camera {
            eye: Vec3::new(0.0, 2.0, 14.0),
            target: Vec3::new(0.0, 1.8, 0.0),
            up: Vec3::new(0.0, 1.0, 0.0),
            projection: Projection::Orthographic {
                height: WORLD_H,
                near: 0.1,
                far: 100.0,
            },
        };
    }

    fn update(&mut self, ctx: &mut Context) {
        let fb = ctx.fb_size;
        let fighting = self.run.phase == Phase::Fight;
        let bar_h = if fighting { UI_BAR_FIGHT_H } else { UI_BAR_H };
        self.frame_camera(ctx, bar_h);

        self.handle_input(ctx);

        match self.run.phase {
            Phase::Prep | Phase::Won | Phase::Lost => ctx.set_continuous(false),
            Phase::Fight => {
                ctx.set_continuous(true);
                self.run.step_fight(ctx.dt);
            }
        }

        self.draw_world(ctx);

        let font = self.font.clone().expect("boot");
        let ui_font = self.ui_font.clone().expect("boot");
        let wood = self.ui_wood.as_ref().map(|t| t.id());
        let tier = rules::shop_tier(self.run.turn);

        // Floating HUD chips (gold / round / hp) — mobile battler style.
        let gold_label = format!("GOLD  {}", self.run.gold);
        let round_label = if fighting {
            format!("ROUND  {}/{}  FIGHT", self.run.turn, MAX_TURNS)
        } else {
            format!("ROUND  {}/{}  T{}", self.run.turn, MAX_TURNS, tier)
        };
        let hp_label = format!("HP  {}", self.run.player.max_hp());
        ctx.ui.node(
            ID_HUD,
            Style {
                display: Display::Flex,
                direction: Direction::Row,
                width: Size::Px(fb[0]),
                height: Size::Px(UI_TOP_H),
                padding: [8.0, 16.0, 0.0, 16.0],
                align: Align::Center,
                justify: Justify::SpaceBetween,
                gap: 10.0,
                ..Style::default()
            },
            |ui| {
                hud_chip(
                    ui,
                    UiId(10),
                    &ui_font,
                    &gold_label,
                    [0.95, 0.78, 0.22, 1.0],
                    [0.28, 0.16, 0.05, 1.0],
                );
                hud_chip(
                    ui,
                    UiId(13),
                    &ui_font,
                    &round_label,
                    [0.95, 0.9, 0.78, 1.0],
                    [0.28, 0.16, 0.08, 1.0],
                );
                hud_chip(
                    ui,
                    UiId(16),
                    &ui_font,
                    &hp_label,
                    [0.45, 0.82, 0.4, 1.0],
                    [0.08, 0.22, 0.08, 1.0],
                );
                // Round pips
                ui.node(
                    UiId(19),
                    Style {
                        display: Display::Flex,
                        direction: Direction::Row,
                        height: Size::Px(40.0),
                        gap: 5.0,
                        align: Align::Center,
                        padding: [0.0, 8.0, 0.0, 8.0],
                        background: Some([0.2, 0.12, 0.06, 0.85]),
                        ..Style::default()
                    },
                    |ui| {
                        for r in 1..=MAX_TURNS {
                            let done = r < self.run.turn || self.run.phase == Phase::Won;
                            let cur = r == self.run.turn && self.run.phase != Phase::Won;
                            ui.node(
                                UiId(20 + r),
                                Style {
                                    width: Size::Px(12.0),
                                    height: Size::Px(12.0),
                                    background: Some(if done {
                                        [0.35, 0.85, 0.4, 1.0]
                                    } else if cur {
                                        [1.0, 0.55, 0.18, 1.0]
                                    } else {
                                        [0.55, 0.42, 0.28, 1.0]
                                    }),
                                    ..Style::default()
                                },
                                |_| {},
                            );
                        }
                    },
                );
            },
        );

        let used = UI_TOP_H + UI_HINT_H + bar_h;
        let spacer = (fb[1] - used).max(0.0);
        ctx.ui.node(
            UiId(30),
            Style {
                width: Size::Px(fb[0]),
                height: Size::Px(spacer),
                ..Style::default()
            },
            |_| {},
        );

        let recipes = rules::active_recipe_names(&self.run.player);
        let combo_hint = if recipes.is_empty() {
            String::new()
        } else {
            format!("  ·  {}", recipes.join(" + "))
        };
        let hint = match self.run.phase {
            Phase::Prep => {
                if self.run.selected_module.is_some() {
                    "Tap module again to sell (+1 gold)".to_string()
                } else if !combo_hint.is_empty() {
                    format!("Combos{combo_hint}")
                } else {
                    "Build your tower, then Fight".to_string()
                }
            }
            Phase::Fight => "Battle in progress".to_string(),
            Phase::Won => "Victory!".to_string(),
            Phase::Lost => "Defeat".to_string(),
        };
        ctx.ui.node(
            ID_HINT,
            Style {
                display: Display::Flex,
                direction: Direction::Row,
                width: Size::Px(fb[0]),
                height: Size::Px(UI_HINT_H),
                justify: Justify::Center,
                align: Align::Center,
                background: Some([0.18, 0.12, 0.08, 0.82]),
                ..Style::default()
            },
            |ui| {
                ui.text(
                    UiId(15),
                    &ui_font,
                    [0.0, 16.0],
                    Style {
                        color: [1.0, 0.93, 0.75, 1.0],
                        ..Style::default()
                    },
                    &hint,
                );
            },
        );

        let fight_hot = ctx.ui.hovered(ID_FIGHT);
        // Fixed-height shop dock (no giant panel image — that was covering the map).
        let _ = wood;
        ctx.ui.node(
            ID_BAR,
            Style {
                display: Display::Flex,
                direction: Direction::Row,
                width: Size::Px(fb[0]),
                height: Size::Px(bar_h),
                padding: Style::edges(8.0),
                gap: 10.0,
                align: Align::Center,
                justify: Justify::Center,
                background: Some([0.28, 0.16, 0.08, 0.96]),
                ..Style::default()
            },
            |ui| {
                        ui.text(
                            UiId(11),
                            &ui_font,
                            [0.0, 16.0],
                            Style {
                                color: [1.0, 0.9, 0.65, 1.0],
                                ..Style::default()
                            },
                            "TOWER",
                        );
                        for (i, m) in self.run.player.modules.iter().enumerate() {
                            let id = UiId(MODULE_BASE + i as u32);
                            let selected = self.run.selected_module == Some(i);
                            let hot = ui.hovered(id);
                            let card_h = if fighting { 44.0 } else { 84.0 };
                            let icon_s = if fighting { 26.0 } else { 36.0 };
                            ui.node(
                                id,
                                Style {
                                    display: Display::Flex,
                                    direction: Direction::Column,
                                    width: Size::Px(if fighting { 56.0 } else { 74.0 }),
                                    height: Size::Px(card_h),
                                    padding: Style::edges(4.0),
                                    gap: 2.0,
                                    align: Align::Center,
                                    justify: Justify::Center,
                                    background: Some(if selected {
                                        [1.0, 0.78, 0.25, 1.0]
                                    } else if hot {
                                        [1.0, 0.93, 0.7, 1.0]
                                    } else {
                                        [0.96, 0.86, 0.62, 1.0]
                                    }),
                                    ..Style::default()
                                },
                                |ui| {
                                    ui.image(
                                        UiId(MODULE_BASE + 40 + i as u32),
                                        self.icon(m.kind),
                                        Style {
                                            width: Size::Px(icon_s),
                                            height: Size::Px(icon_s),
                                            fit: Fit::Contain,
                                            ..Style::default()
                                        },
                                    );
                                    if !fighting {
                                        let label = if m.level >= 2 {
                                            format!("{} L2", m.kind.short())
                                        } else {
                                            m.kind.short().to_string()
                                        };
                                        ui.text(
                                            UiId(MODULE_BASE + 60 + i as u32),
                                            &font,
                                            [9.0, 12.0],
                                            Style {
                                                color: [0.25, 0.15, 0.08, 1.0],
                                                ..Style::default()
                                            },
                                            &label,
                                        );
                                    }
                                },
                            );
                        }
                        if self.run.player.modules.is_empty() {
                            ui.text(
                                UiId(12),
                                &ui_font,
                                [0.0, 16.0],
                                Style {
                                    color: [0.9, 0.82, 0.65, 1.0],
                                    ..Style::default()
                                },
                                "Empty",
                            );
                        }

                        if !fighting {
                            ui.node(
                                ID_TOWER,
                                Style {
                                    width: Size::Px(6.0),
                                    height: Size::Px(72.0),
                                    background: Some([0.35, 0.22, 0.12, 1.0]),
                                    ..Style::default()
                                },
                                |_| {},
                            );

                            ui.text(
                                UiId(33),
                                &ui_font,
                                [0.0, 16.0],
                                Style {
                                    color: [1.0, 0.9, 0.65, 1.0],
                                    ..Style::default()
                                },
                                "SHOP",
                            );
                            for (i, offer) in self.run.bar.iter().enumerate() {
                                let id = UiId(OFFER_BASE + i as u32);
                                let hot = ui.hovered(id) && self.run.phase == Phase::Prep;
                                ui.node(
                                    id,
                                    Style {
                                        display: Display::Flex,
                                        direction: Direction::Column,
                                        width: Size::Px(84.0),
                                        height: Size::Px(88.0),
                                        padding: Style::edges(4.0),
                                        gap: 2.0,
                                        align: Align::Center,
                                        justify: Justify::Center,
                                        background: Some(if hot {
                                            [1.0, 0.88, 0.45, 1.0]
                                        } else {
                                            [0.92, 0.62, 0.22, 1.0]
                                        }),
                                        ..Style::default()
                                    },
                                    |ui| {
                                        if let Some(kind) = offer {
                                            ui.image(
                                                UiId(OFFER_BASE + 40 + i as u32),
                                                self.icon(*kind),
                                                Style {
                                                    width: Size::Px(38.0),
                                                    height: Size::Px(38.0),
                                                    fit: Fit::Contain,
                                                    ..Style::default()
                                                },
                                            );
                                            let line =
                                                format!("{} ${}", kind.short(), kind.cost());
                                            ui.text(
                                                UiId(OFFER_BASE + 60 + i as u32),
                                                &font,
                                                [9.0, 12.0],
                                                Style {
                                                    color: [0.2, 0.1, 0.04, 1.0],
                                                    ..Style::default()
                                                },
                                                &line,
                                            );
                                        } else {
                                            ui.text(
                                                UiId(OFFER_BASE + 60 + i as u32),
                                                &ui_font,
                                                [0.0, 18.0],
                                                Style {
                                                    color: [0.45, 0.28, 0.12, 1.0],
                                                    ..Style::default()
                                                },
                                                "—",
                                            );
                                        }
                                    },
                                );
                            }

                            if self.run.phase == Phase::Prep {
                                let reroll_cost = self.run.reroll_cost();
                                let can_reroll = self.run.gold >= reroll_cost;
                                let reroll_hot = ui.hovered(ID_REROLL) && can_reroll;
                                let reroll_label = format!("Reroll ${}", reroll_cost);
                                labeled_button(
                                    ui,
                                    ID_REROLL,
                                    &ui_font,
                                    &reroll_label,
                                    126.0,
                                    52.0,
                                    reroll_hot,
                                    if can_reroll {
                                        [0.3, 0.55, 0.9, 1.0]
                                    } else {
                                        [0.4, 0.4, 0.45, 1.0]
                                    },
                                    if can_reroll {
                                        [0.4, 0.68, 1.0, 1.0]
                                    } else {
                                        [0.5, 0.5, 0.55, 1.0]
                                    },
                                );
                                labeled_button(
                                    ui,
                                    ID_FIGHT,
                                    &ui_font,
                                    "FIGHT!",
                                    120.0,
                                    52.0,
                                    fight_hot,
                                    [0.88, 0.22, 0.18, 1.0],
                                    [1.0, 0.4, 0.28, 1.0],
                                );
                            }
                        }
            },
        );

        if matches!(self.run.phase, Phase::Won | Phase::Lost) {
            let win = self.run.phase == Phase::Won;
            let restart_hot = ctx.ui.hovered(ID_RESTART);
            ctx.ui.node(
                ID_RESULT,
                Style {
                    display: Display::Flex,
                    direction: Direction::Column,
                    width: Size::Px(fb[0]),
                    height: Size::Px(fb[1]),
                    margin: [-fb[1], 0.0, 0.0, 0.0],
                    justify: Justify::Center,
                    align: Align::Center,
                    gap: 18.0,
                    background: Some([0.0, 0.0, 0.0, 0.7]),
                    ..Style::default()
                },
                |ui| {
                    ui.text(
                        UiId(70),
                        &ui_font,
                        [0.0, 48.0],
                        Style {
                            color: if win {
                                [0.45, 1.0, 0.55, 1.0]
                            } else {
                                [1.0, 0.45, 0.35, 1.0]
                            },
                            ..Style::default()
                        },
                        if win { "Victory!" } else { "Defeat" },
                    );
                    labeled_button(
                        ui,
                        ID_RESTART,
                        &ui_font,
                        "Play again",
                        200.0,
                        54.0,
                        restart_hot,
                        [0.35, 0.55, 0.9, 1.0],
                        [0.5, 0.7, 1.0, 1.0],
                    );
                },
            );
        }
    }
}

fn font_atlas_rgba() -> (Vec<u8>, u32, u32) {
    let glyphs = ascii_5x7();
    let atlas_w = FONT_COLS * FONT_CELL_W;
    let atlas_h = FONT_ROWS * FONT_CELL_H;
    let mut rgba = vec![0u8; (atlas_w * atlas_h * 4) as usize];
    for (i, rows) in glyphs.iter().enumerate() {
        let col = (i as u32) % FONT_COLS;
        let row = (i as u32) / FONT_COLS;
        if row >= FONT_ROWS {
            break;
        }
        for gy in 0..GLYPH_H {
            let bits = rows[gy as usize];
            for gx in 0..GLYPH_W {
                if (bits >> (GLYPH_W - 1 - gx)) & 1 == 0 {
                    continue;
                }
                let x = col * FONT_CELL_W + gx;
                let y = row * FONT_CELL_H + gy;
                let idx = ((y * atlas_w + x) * 4) as usize;
                rgba[idx] = 255;
                rgba[idx + 1] = 255;
                rgba[idx + 2] = 255;
                rgba[idx + 3] = 255;
            }
        }
    }
    (rgba, atlas_w, atlas_h)
}

fn ascii_5x7() -> Vec<[u8; 7]> {
    let mut out = vec![[0u8; 7]; 64];
    let put = |out: &mut Vec<[u8; 7]>, ch: char, rows: [u8; 7]| {
        let i = ch as usize - 32;
        if i < out.len() {
            out[i] = rows;
        }
    };
    put(&mut out, '0', [0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110]);
    put(&mut out, '1', [0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110]);
    put(&mut out, '2', [0b01110, 0b10001, 0b00001, 0b00110, 0b01000, 0b10000, 0b11111]);
    put(&mut out, '3', [0b01110, 0b10001, 0b00001, 0b00110, 0b00001, 0b10001, 0b01110]);
    put(&mut out, '4', [0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010]);
    put(&mut out, '5', [0b11111, 0b10000, 0b11110, 0b00001, 0b00001, 0b10001, 0b01110]);
    put(&mut out, '6', [0b00110, 0b01000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110]);
    put(&mut out, '7', [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000]);
    put(&mut out, '8', [0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110]);
    put(&mut out, '9', [0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00010, 0b01100]);
    put(&mut out, 'A', [0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001]);
    put(&mut out, 'B', [0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110]);
    put(&mut out, 'C', [0b01110, 0b10001, 0b10000, 0b10000, 0b10000, 0b10001, 0b01110]);
    put(&mut out, 'D', [0b11110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11110]);
    put(&mut out, 'E', [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111]);
    put(&mut out, 'F', [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000]);
    put(&mut out, 'G', [0b01110, 0b10001, 0b10000, 0b10111, 0b10001, 0b10001, 0b01110]);
    put(&mut out, 'H', [0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001]);
    put(&mut out, 'I', [0b01110, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110]);
    put(&mut out, 'J', [0b00111, 0b00010, 0b00010, 0b00010, 0b00010, 0b10010, 0b01100]);
    put(&mut out, 'K', [0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001]);
    put(&mut out, 'L', [0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111]);
    put(&mut out, 'M', [0b10001, 0b11011, 0b10101, 0b10001, 0b10001, 0b10001, 0b10001]);
    put(&mut out, 'N', [0b10001, 0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001]);
    put(&mut out, 'O', [0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110]);
    put(&mut out, 'P', [0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000]);
    put(&mut out, 'Q', [0b01110, 0b10001, 0b10001, 0b10001, 0b10101, 0b10010, 0b01101]);
    put(&mut out, 'R', [0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001]);
    put(&mut out, 'S', [0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110]);
    put(&mut out, 'T', [0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100]);
    put(&mut out, 'U', [0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110]);
    put(&mut out, 'V', [0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100]);
    put(&mut out, 'W', [0b10001, 0b10001, 0b10001, 0b10001, 0b10101, 0b10101, 0b01010]);
    put(&mut out, 'X', [0b10001, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001, 0b10001]);
    put(&mut out, 'Y', [0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100]);
    put(&mut out, 'Z', [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b11111]);
    put(&mut out, '/', [0b00001, 0b00010, 0b00100, 0b00100, 0b01000, 0b10000, 0b10000]);
    put(&mut out, '$', [0b00100, 0b01111, 0b10100, 0b01110, 0b00101, 0b11110, 0b00100]);
    put(&mut out, '-', [0b00000, 0b00000, 0b00000, 0b11111, 0b00000, 0b00000, 0b00000]);
    put(&mut out, '(', [0b00010, 0b00100, 0b01000, 0b01000, 0b01000, 0b00100, 0b00010]);
    put(&mut out, ')', [0b01000, 0b00100, 0b00010, 0b00010, 0b00010, 0b00100, 0b01000]);
    put(&mut out, '+', [0b00000, 0b00100, 0b00100, 0b11111, 0b00100, 0b00100, 0b00000]);
    put(&mut out, ' ', [0u8; 7]);
    out
}

fn main() {
    if let Err(err) = run(
        Config {
            title: "brutal-pixels".into(),
            width: 1200,
            height: 720,
            max_fps: Some(60),
        },
        BrutalPixels::new(),
    ) {
        eprintln!("{err}");
        std::process::exit(1);
    }
}
