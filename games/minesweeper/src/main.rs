//! Minesweeper: local solo or online battle royale via dedicated + lobby.

#![cfg_attr(windows, windows_subsystem = "windows")]

use std::path::Path;

use beton::{
    run, style, Camera, Config, Context, Font, Game, Key, NetEvent, NetRole, PeerId, Projection,
    Sprite, TextureHandle, UiId, Vec3,
};
use minesweeper::{
    cells_to_views, decode_board, decode_event, decode_go, decode_snap, decode_welcome,
    encode_hello, encode_input, lobby_base_url, public_server, Action, Board, BoardMsg, CellView,
    InputMsg, MatchEvent, SeatSnap, Snap, Welcome, BR_H, BR_W,
};

const ASSETS: &[&str] = &["assets/Roboto-Regular.ttf"];
const UI_FONT_ASSET: &str = "assets/Roboto-Regular.ttf";

const WORLD_HEIGHT: f32 = 20.0;
const DEFAULT_W: usize = 10;
const DEFAULT_H: usize = 10;
const DEFAULT_MINES: i16 = 10;
const MIN_SIZE: usize = 5;
const MAX_SIZE: usize = 30;
const BOARD_SPAN: f32 = 14.0;
const GAP: f32 = 0.12;

const DIGIT_CELL_W: u32 = 16;
const DIGIT_CELL_H: u32 = 20;
const DIGIT_ATLAS_W: u32 = DIGIT_CELL_W * 10;
const DIGIT_ATLAS_H: u32 = DIGIT_CELL_H;
const DIGIT_TRACKING: f32 = 0.08;

const UI_ROOT: UiId = UiId(1);
const UI_BAR: UiId = UiId(2);
const UI_EASY: UiId = UiId(10);
const UI_EASY_L: UiId = UiId(11);
const UI_MED: UiId = UiId(12);
const UI_MED_L: UiId = UiId(13);
const UI_HARD: UiId = UiId(14);
const UI_HARD_L: UiId = UiId(15);
const UI_W_DEC: UiId = UiId(20);
const UI_W_INC: UiId = UiId(21);
const UI_W_VAL: UiId = UiId(22);
const UI_H_DEC: UiId = UiId(30);
const UI_H_INC: UiId = UiId(31);
const UI_H_VAL: UiId = UiId(32);
const UI_M_DEC: UiId = UiId(40);
const UI_M_INC: UiId = UiId(41);
const UI_M_VAL: UiId = UiId(42);
const UI_NEW: UiId = UiId(50);
const UI_NEW_L: UiId = UiId(51);
const UI_LOCAL: UiId = UiId(60);
const UI_LOCAL_L: UiId = UiId(61);
const UI_JOIN: UiId = UiId(62);
const UI_JOIN_L: UiId = UiId(63);
const UI_BACK: UiId = UiId(66);
const UI_BACK_L: UiId = UiId(67);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Screen {
    Menu,
    Local,
    Online,
}

struct OnlineState {
    welcome: Option<Welcome>,
    snap: Option<Snap>,
    cells: Vec<CellView>,
    width: usize,
    height: usize,
    leader_token: String,
    #[allow(dead_code)]
    join_url: String,
    is_leader: bool,
    feed: String,
    hello_sent: bool,
}

impl OnlineState {
    fn new(join_url: String, leader_token: String) -> Self {
        Self {
            welcome: None,
            snap: None,
            cells: vec![CellView::Unvisited; BR_W * BR_H],
            width: BR_W,
            height: BR_H,
            leader_token,
            join_url,
            is_leader: false,
            feed: String::new(),
            hello_sent: false,
        }
    }

    fn my_seat(&self) -> Option<u8> {
        self.welcome.map(|w| w.seat)
    }

    fn my_roster(&self) -> Option<&SeatSnap> {
        let seat = self.my_seat()?;
        self.snap.as_ref()?.seats.iter().find(|s| s.seat == seat)
    }

    fn hide_active(&self) -> bool {
        self.my_roster().map(|s| s.hide_ms > 0).unwrap_or(false)
    }

    fn alive(&self) -> bool {
        self.my_roster().map(|s| s.alive != 0).unwrap_or(true)
    }
}

struct MinesweeperGame {
    white: Option<TextureHandle>,
    font_tex: Option<TextureHandle>,
    font: Option<Font>,
    ui_font: Option<Font>,
    board: Board,
    cfg_w: usize,
    cfg_h: usize,
    cfg_mines: i16,
    rmb_was: bool,
    screen: Screen,
    lobby_url: String,
    online: Option<OnlineState>,
    status: String,
}

impl MinesweeperGame {
    fn new() -> Self {
        Self {
            white: None,
            font_tex: None,
            font: None,
            ui_font: None,
            board: Board::new(DEFAULT_W, DEFAULT_H, DEFAULT_MINES),
            cfg_w: DEFAULT_W,
            cfg_h: DEFAULT_H,
            cfg_mines: DEFAULT_MINES,
            rmb_was: false,
            screen: Screen::Menu,
            lobby_url: lobby_base_url(),
            online: None,
            status: String::new(),
        }
    }

    fn max_mines(width: usize, height: usize) -> i16 {
        let cells = width.saturating_mul(height);
        let reserved = if cells > 9 { 9 } else { 1 };
        cells.saturating_sub(reserved).max(1) as i16
    }

    fn clamp_mines(width: usize, height: usize, mines: i16) -> i16 {
        mines.clamp(1, Self::max_mines(width, height))
    }

    fn apply_config(&mut self) {
        self.cfg_w = self.cfg_w.clamp(MIN_SIZE, MAX_SIZE);
        self.cfg_h = self.cfg_h.clamp(MIN_SIZE, MAX_SIZE);
        self.cfg_mines = Self::clamp_mines(self.cfg_w, self.cfg_h, self.cfg_mines);
        self.board.resize(self.cfg_w, self.cfg_h, self.cfg_mines);
        self.rmb_was = false;
    }

    fn set_preset(&mut self, w: usize, h: usize, mines: i16) {
        self.cfg_w = w;
        self.cfg_h = h;
        self.cfg_mines = mines;
        self.apply_config();
    }

    fn join_server(&mut self, ctx: &mut Context) {
        match public_server(&self.lobby_url) {
            Ok(room) => {
                self.status = format!("joining {}…", room.name);
                self.begin_online(ctx, room.join_url, String::new());
            }
            Err(e) => self.status = e,
        }
    }

    fn begin_online(&mut self, ctx: &mut Context, join_url: String, leader_token: String) {
        if let Err(e) = ctx.net.connect(&join_url) {
            self.status = e;
            return;
        }
        self.online = Some(OnlineState::new(join_url, leader_token));
        self.screen = Screen::Online;
        self.status = "connecting…".into();
    }

    fn board_dims(&self) -> (usize, usize) {
        if self.screen == Screen::Online {
            if let Some(o) = &self.online {
                return (o.width, o.height);
            }
            return (BR_W, BR_H);
        }
        (self.board.width(), self.board.height())
    }

    fn cell_size(&self) -> f32 {
        let (w, h) = self.board_dims();
        let n = w.max(h) as f32;
        (BOARD_SPAN - GAP * (n + 1.0)) / n
    }

    fn cell_center(&self, x: usize, y: usize) -> Vec3 {
        let (bw, bh) = self.board_dims();
        let w = bw as f32;
        let h = bh as f32;
        let cell = self.cell_size();
        let step = cell + GAP;
        let board_w = w * step - GAP;
        let board_h = h * step - GAP;
        let origin_x = -board_w * 0.5 + cell * 0.5;
        let origin_y = board_h * 0.5 - cell * 0.5 + 0.4;
        Vec3::new(
            origin_x + x as f32 * step,
            origin_y - y as f32 * step,
            0.0,
        )
    }

    fn hit_cell(&self, world: Vec3) -> Option<(usize, usize)> {
        let (bw, bh) = self.board_dims();
        let cell = self.cell_size();
        let half = cell * 0.5;
        for y in 0..bh {
            for x in 0..bw {
                let c = self.cell_center(x, y);
                if (world.x - c.x).abs() <= half && (world.y - c.y).abs() <= half {
                    return Some((x, y));
                }
            }
        }
        None
    }

    fn cell_at(&self, x: usize, y: usize) -> CellView {
        if self.screen == Screen::Online {
            if let Some(o) = &self.online {
                if o.hide_active() {
                    return CellView::Unvisited;
                }
                let i = y * o.width + x;
                return o.cells.get(i).copied().unwrap_or(CellView::Unvisited);
            }
        }
        self.board.cell(x, y)
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

    fn ui_blocks_board(ctx: &Context) -> bool {
        ctx.ui.hovered(UI_BAR)
            || ctx.ui.hovered(UI_EASY)
            || ctx.ui.hovered(UI_MED)
            || ctx.ui.hovered(UI_HARD)
            || ctx.ui.hovered(UI_NEW)
            || ctx.ui.hovered(UI_W_DEC)
            || ctx.ui.hovered(UI_W_INC)
            || ctx.ui.hovered(UI_H_DEC)
            || ctx.ui.hovered(UI_H_INC)
            || ctx.ui.hovered(UI_M_DEC)
            || ctx.ui.hovered(UI_M_INC)
            || ctx.ui.hovered(UI_BACK)
            || ctx.ui.hovered(UI_LOCAL)
            || ctx.ui.hovered(UI_JOIN)
    }

    fn handle_net(&mut self, ctx: &mut Context) {
        if self.online.is_none() {
            return;
        }
        let events: Vec<NetEvent> = ctx.net.events().to_vec();
        let mut send_hello = false;
        let mut disconnect = false;
        for ev in events {
            let Some(online) = self.online.as_mut() else {
                break;
            };
            match ev {
                NetEvent::Connected => {
                    self.status = "connected".into();
                    if !online.hello_sent {
                        send_hello = true;
                        online.hello_sent = true;
                    }
                }
                NetEvent::Message { bytes, .. } | NetEvent::Datagram { bytes, .. } => {
                    if let Some(w) = decode_welcome(&bytes) {
                        online.welcome = Some(w);
                        online.is_leader = w.leader != 0;
                        online.width = w.width as usize;
                        online.height = w.height as usize;
                        online.cells =
                            vec![CellView::Unvisited; online.width * online.height];
                        self.status = format!("seat {}", w.seat);
                    } else if let Some(seed) = decode_go(&bytes) {
                        self.status = format!("GO {seed}");
                        online.feed = "match started".into();
                    } else if let Some(snap) = decode_snap(&bytes) {
                        online.snap = Some(snap);
                    } else if let Some(board) = decode_board(&bytes) {
                        apply_board(online, &board);
                    } else if let Some(ev) = decode_event(&bytes) {
                        online.feed = format_event(&ev);
                    }
                }
                NetEvent::Disconnected => {
                    disconnect = true;
                }
                _ => {}
            }
        }
        if send_hello {
            if let Some(online) = &self.online {
                let tok = online.leader_token.clone();
                let _ = ctx.net.send(PeerId::HOST, &encode_hello(&tok));
            }
        }
        if disconnect {
            self.status = "disconnected".into();
            self.online = None;
            self.screen = Screen::Menu;
        }
    }

    fn handle_online_input(&mut self, ctx: &mut Context) {
        if Self::ui_blocks_board(ctx) {
            self.rmb_was = ctx.input.secondary_down();
            return;
        }
        let Some(online) = self.online.as_ref() else {
            return;
        };
        if !online.alive() {
            self.rmb_was = ctx.input.secondary_down();
            return;
        }
        let playing = online
            .snap
            .as_ref()
            .map(|s| s.phase == 1)
            .unwrap_or(false);
        if !playing {
            self.rmb_was = ctx.input.secondary_down();
            return;
        }

        let world = ctx
            .camera
            .screen_to_world(ctx.input.cursor_px(), ctx.fb_size[0], ctx.fb_size[1]);
        let hit = self.hit_cell(world);

        if ctx.input.pointer_pressed() {
            if let Some((x, y)) = hit {
                let msg = InputMsg {
                    x: x as u8,
                    y: y as u8,
                    action: Action::Sweep,
                };
                let _ = ctx.net.send(PeerId::HOST, &encode_input(&msg));
            }
        }
        let rmb = ctx.input.secondary_down();
        if rmb && !self.rmb_was {
            if let Some((x, y)) = hit {
                let msg = InputMsg {
                    x: x as u8,
                    y: y as u8,
                    action: Action::Flag,
                };
                let _ = ctx.net.send(PeerId::HOST, &encode_input(&msg));
            }
        }
        self.rmb_was = rmb;
    }

    fn handle_local_settings(&mut self, ctx: &Context) {
        if ctx.ui.clicked(UI_EASY) || ctx.ui.clicked(UI_EASY_L) {
            self.set_preset(9, 9, 10);
            return;
        }
        if ctx.ui.clicked(UI_MED) || ctx.ui.clicked(UI_MED_L) {
            self.set_preset(16, 16, 40);
            return;
        }
        if ctx.ui.clicked(UI_HARD) || ctx.ui.clicked(UI_HARD_L) {
            self.set_preset(24, 16, 60);
            return;
        }
        if ctx.ui.clicked(UI_NEW) || ctx.ui.clicked(UI_NEW_L) {
            self.apply_config();
            return;
        }
        let mut changed = false;
        if ctx.ui.clicked(UI_W_DEC) {
            self.cfg_w = self.cfg_w.saturating_sub(1).max(MIN_SIZE);
            changed = true;
        }
        if ctx.ui.clicked(UI_W_INC) {
            self.cfg_w = (self.cfg_w + 1).min(MAX_SIZE);
            changed = true;
        }
        if ctx.ui.clicked(UI_H_DEC) {
            self.cfg_h = self.cfg_h.saturating_sub(1).max(MIN_SIZE);
            changed = true;
        }
        if ctx.ui.clicked(UI_H_INC) {
            self.cfg_h = (self.cfg_h + 1).min(MAX_SIZE);
            changed = true;
        }
        if ctx.ui.clicked(UI_M_DEC) {
            self.cfg_mines = (self.cfg_mines - 1).max(1);
            changed = true;
        }
        if ctx.ui.clicked(UI_M_INC) {
            self.cfg_mines = self.cfg_mines.saturating_add(1);
            changed = true;
        }
        if changed {
            self.apply_config();
        }
    }

    fn handle_local_input(&mut self, ctx: &Context) {
        self.handle_local_settings(ctx);
        if ctx.input.pressed(Key::KeyR) {
            self.apply_config();
            return;
        }
        if ctx.input.pressed(Key::Digit1) {
            self.set_preset(9, 9, 10);
            return;
        }
        if ctx.input.pressed(Key::Digit2) {
            self.set_preset(16, 16, 40);
            return;
        }
        if ctx.input.pressed(Key::Digit3) {
            self.set_preset(24, 16, 60);
            return;
        }
        if Self::ui_blocks_board(ctx) {
            self.rmb_was = ctx.input.secondary_down();
            return;
        }
        let world = ctx
            .camera
            .screen_to_world(ctx.input.cursor_px(), ctx.fb_size[0], ctx.fb_size[1]);
        let hit = self.hit_cell(world);
        if ctx.input.pointer_pressed() {
            if let Some((x, y)) = hit {
                self.board.do_action(x as i32, y as i32, Action::Sweep);
            }
        }
        let rmb = ctx.input.secondary_down();
        if rmb && !self.rmb_was {
            if let Some((x, y)) = hit {
                self.board.do_action(x as i32, y as i32, Action::Flag);
            }
        }
        self.rmb_was = rmb;
    }

    fn draw_menu(&mut self, ctx: &mut Context) {
        let ui_font = self.ui_font.as_ref().expect("boot");
        let [fb_w, fb_h] = ctx.fb_size;
        let local_hot = ctx.ui.hovered(UI_LOCAL) || ctx.ui.hovered(UI_LOCAL_L);
        let join_hot = ctx.ui.hovered(UI_JOIN) || ctx.ui.hovered(UI_JOIN_L);
        ctx.ui.node(
            UI_ROOT,
            style!(
                position: absolute,
                width: (fb_w) pt,
                height: (fb_h) pt,
                top: 0 pt,
                left: 0 pt,
                flex_direction: column,
                justify_content: center,
                align_items: center,
                gap: 12 pt,
                background: Some([0.10, 0.11, 0.14, 0.92]),
            ),
            |ui| {
                ui.text(
                    UiId(0),
                    ui_font,
                    [0.0, 28.0],
                    style!(color: [0.95, 0.96, 0.98, 1.0]),
                    "Minesweeper BR",
                );
                menu_btn(ui, ui_font, UI_JOIN, UI_JOIN_L, "JOIN SERVER", join_hot);
                menu_btn(ui, ui_font, UI_LOCAL, UI_LOCAL_L, "LOCAL", local_hot);
                if !self.status.is_empty() {
                    ui.text(
                        UiId(0),
                        ui_font,
                        [0.0, 14.0],
                        style!(color: [0.75, 0.78, 0.85, 1.0]),
                        &self.status,
                    );
                }
            },
        );
    }

    fn draw_settings_bar(&self, ctx: &mut Context) {
        let ui_font = self.ui_font.as_ref().expect("boot");
        let [fb_w, fb_h] = ctx.fb_size;
        let easy = self.cfg_w == 9 && self.cfg_h == 9 && self.cfg_mines == 10;
        let med = self.cfg_w == 16 && self.cfg_h == 16 && self.cfg_mines == 40;
        let hard = self.cfg_w == 24 && self.cfg_h == 16 && self.cfg_mines == 60;
        ctx.ui.node(
            UI_ROOT,
            style!(
                position: absolute,
                width: (fb_w) pt,
                height: (fb_h) pt,
                top: 0 pt,
                left: 0 pt,
                flex_direction: column,
                justify_content: flex_end,
                align_items: stretch,
            ),
            |ui| {
                ui.node(
                    UI_BAR,
                    style!(
                        flex_direction: row,
                        align_items: center,
                        justify_content: center,
                        gap: 8 pt,
                        padding: 10 pt,
                        background: Some([0.10, 0.11, 0.14, 0.96]),
                    ),
                    |ui| {
                        let easy_hot = ui.hovered(UI_EASY) || ui.hovered(UI_EASY_L);
                        let med_hot = ui.hovered(UI_MED) || ui.hovered(UI_MED_L);
                        let hard_hot = ui.hovered(UI_HARD) || ui.hovered(UI_HARD_L);
                        let new_hot = ui.hovered(UI_NEW) || ui.hovered(UI_NEW_L);
                        let back_hot = ui.hovered(UI_BACK) || ui.hovered(UI_BACK_L);
                        chip(ui, ui_font, UI_EASY, UI_EASY_L, "Easy", easy_hot, easy);
                        chip(ui, ui_font, UI_MED, UI_MED_L, "Med", med_hot, med);
                        chip(ui, ui_font, UI_HARD, UI_HARD_L, "Hard", hard_hot, hard);
                        stepper(ui, ui_font, "W", UI_W_DEC, UI_W_VAL, UI_W_INC, self.cfg_w as u32);
                        stepper(ui, ui_font, "H", UI_H_DEC, UI_H_VAL, UI_H_INC, self.cfg_h as u32);
                        stepper(
                            ui,
                            ui_font,
                            "Mines",
                            UI_M_DEC,
                            UI_M_VAL,
                            UI_M_INC,
                            self.cfg_mines as u32,
                        );
                        chip(ui, ui_font, UI_NEW, UI_NEW_L, "New", new_hot, false);
                        chip(ui, ui_font, UI_BACK, UI_BACK_L, "Menu", back_hot, false);
                    },
                );
            },
        );
    }

    fn draw_online_hud(&self, ctx: &mut Context) {
        let ui_font = self.ui_font.as_ref().expect("boot");
        let [fb_w, fb_h] = ctx.fb_size;
        let online = self.online.as_ref();
        let roster = online.and_then(|o| o.my_roster());
        let hp = roster.map(|r| r.hp).unwrap_or(3);
        let clears = roster.map(|r| r.clears).unwrap_or(0);
        let deadline_ms = roster.map(|r| r.deadline_ms).unwrap_or(0);
        let alive_n = online
            .and_then(|o| o.snap.as_ref())
            .map(|s| s.seats.iter().filter(|x| x.alive != 0).count())
            .unwrap_or(0);
        let phase = online.and_then(|o| o.snap.as_ref()).map(|s| s.phase).unwrap_or(0);
        let lobby_cd = online
            .and_then(|o| o.snap.as_ref())
            .map(|s| s.lobby_countdown_ms)
            .unwrap_or(0);
        let players_n = online
            .and_then(|o| o.snap.as_ref())
            .map(|s| s.seats.len())
            .unwrap_or(0);
        let feed = online.map(|o| o.feed.as_str()).unwrap_or("");
        let back_hot = ctx.ui.hovered(UI_BACK) || ctx.ui.hovered(UI_BACK_L);

        ctx.ui.node(
            UI_ROOT,
            style!(
                position: absolute,
                width: (fb_w) pt,
                height: (fb_h) pt,
                top: 0 pt,
                left: 0 pt,
                flex_direction: column,
                justify_content: space_between,
                padding: 10 pt,
            ),
            |ui| {
                ui.node(
                    UiId(0),
                    style!(
                        flex_direction: row,
                        gap: 12 pt,
                        align_items: center,
                    ),
                    |ui| {
                        ui.text(
                            UiId(0),
                            ui_font,
                            [0.0, 18.0],
                            style!(color: [0.95, 0.4, 0.4, 1.0]),
                            &format!("HP {hp}"),
                        );
                        ui.text(
                            UiId(0),
                            ui_font,
                            [0.0, 18.0],
                            style!(color: [0.85, 0.88, 0.92, 1.0]),
                            &format!("T {:.1}", deadline_ms as f32 / 1000.0),
                        );
                        ui.text(
                            UiId(0),
                            ui_font,
                            [0.0, 18.0],
                            style!(color: [0.85, 0.88, 0.92, 1.0]),
                            &format!("Clears {clears}"),
                        );
                        ui.text(
                            UiId(0),
                            ui_font,
                            [0.0, 18.0],
                            style!(color: [0.85, 0.88, 0.92, 1.0]),
                            &format!("Alive {alive_n}"),
                        );
                        ui.text(
                            UiId(0),
                            ui_font,
                            [0.0, 18.0],
                            style!(color: [0.85, 0.88, 0.92, 1.0]),
                            &format!("Players {players_n}"),
                        );
                    },
                );
                if phase == 0 {
                    let wait = if lobby_cd > 0 {
                        format!("Starting in {:.0}s", lobby_cd as f32 / 1000.0)
                    } else if players_n < 2 {
                        "Waiting for players…".into()
                    } else {
                        "Get ready…".into()
                    };
                    ui.text(
                        UiId(0),
                        ui_font,
                        [0.0, 20.0],
                        style!(color: [0.95, 0.85, 0.45, 1.0]),
                        &wait,
                    );
                }
                if !feed.is_empty() {
                    ui.text(
                        UiId(0),
                        ui_font,
                        [0.0, 14.0],
                        style!(color: [0.95, 0.85, 0.45, 1.0]),
                        feed,
                    );
                }
                ui.node(
                    UI_BAR,
                    style!(
                        flex_direction: row,
                        gap: 8 pt,
                        justify_content: center,
                        padding: 8 pt,
                        background: Some([0.10, 0.11, 0.14, 0.92]),
                    ),
                    |ui| {
                        chip(ui, ui_font, UI_BACK, UI_BACK_L, "Leave", back_hot, false);
                    },
                );
            },
        );
    }

    fn draw_board(&self, ctx: &mut Context) {
        let white = self.white.as_ref().expect("boot").id();
        let font = self.font.as_ref().expect("boot");
        let (bw, bh) = self.board_dims();
        let cell = self.cell_size();
        let w = bw as f32;
        let h = bh as f32;
        let step = cell + GAP;
        let board_w = w * step - GAP;
        let board_h = h * step - GAP;

        Self::push_rect(
            ctx,
            white,
            Vec3::new(0.0, 0.0, -2.0),
            [40.0, WORLD_HEIGHT],
            [0.18, 0.20, 0.24, 1.0],
        );
        Self::push_rect(
            ctx,
            white,
            Vec3::new(0.0, 0.4, -1.0),
            [board_w + 0.45, board_h + 0.45],
            [0.28, 0.32, 0.38, 1.0],
        );

        let glyph = [cell * 0.42, cell * 0.55];
        let hide = self
            .online
            .as_ref()
            .map(|o| o.hide_active())
            .unwrap_or(false);
        for y in 0..bh {
            for x in 0..bw {
                let center = self.cell_center(x, y);
                let view = self.cell_at(x, y);
                let (bg, fg, show_num) = cell_style(view);
                let bg = if hide {
                    [0.12, 0.12, 0.16, 1.0]
                } else {
                    bg
                };
                Self::push_rect(ctx, white, center, [cell, cell], bg);
                if hide {
                    continue;
                }
                match show_num {
                    Some(n) => {
                        Self::draw_number(
                            ctx,
                            font,
                            Vec3::new(center.x, center.y, 1.0),
                            n as u32,
                            glyph,
                            fg,
                        );
                    }
                    None if matches!(view, CellView::Flag | CellView::FlaggedBomb) => {
                        Self::push_rect(
                            ctx,
                            white,
                            Vec3::new(center.x - cell * 0.12, center.y, 1.0),
                            [cell * 0.08, cell * 0.55],
                            [0.15, 0.12, 0.10, 1.0],
                        );
                        Self::push_rect(
                            ctx,
                            white,
                            Vec3::new(center.x + cell * 0.05, center.y + cell * 0.12, 1.1),
                            [cell * 0.32, cell * 0.22],
                            [0.90, 0.25, 0.22, 1.0],
                        );
                    }
                    None if matches!(view, CellView::Bomb) => {
                        Self::push_rect(
                            ctx,
                            white,
                            Vec3::new(center.x, center.y, 1.0),
                            [cell * 0.45, cell * 0.45],
                            [0.08, 0.08, 0.10, 1.0],
                        );
                    }
                    _ => {}
                }
            }
        }

        if self.screen == Screen::Local {
            let mines_left = self.board.mines_left();
            let hud_color = if self.board.is_solved() {
                [0.35, 0.85, 0.45, 1.0]
            } else if self.board.is_dead() {
                [0.95, 0.35, 0.30, 1.0]
            } else {
                [0.85, 0.88, 0.92, 1.0]
            };
            Self::draw_number(
                ctx,
                font,
                Vec3::new(0.0, 9.0, 2.0),
                mines_left.unsigned_abs().min(999),
                [0.55, 0.7],
                hud_color,
            );
        }
    }
}

impl Game for MinesweeperGame {
    fn assets(&self) -> &[&'static str] {
        ASSETS
    }

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
        self.ui_font =
            Some(Font::load(ctx.gfx, Path::new(UI_FONT_ASSET), 48.0).expect("ui font"));
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
        // Smoke / CI: `MINES_AUTO_JOIN=1` connects to the public server on boot.
        if std::env::var("MINES_AUTO_JOIN")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false)
        {
            self.join_server(ctx);
        }
    }

    fn update(&mut self, ctx: &mut Context) {
        match self.screen {
            Screen::Menu => {
                self.draw_menu(ctx);
                if ctx.ui.clicked(UI_JOIN) || ctx.ui.clicked(UI_JOIN_L) {
                    self.join_server(ctx);
                } else if ctx.ui.clicked(UI_LOCAL) || ctx.ui.clicked(UI_LOCAL_L) {
                    self.screen = Screen::Local;
                }
            }
            Screen::Local => {
                self.handle_local_input(ctx);
                if ctx.ui.clicked(UI_BACK) || ctx.ui.clicked(UI_BACK_L) {
                    self.screen = Screen::Menu;
                }
                self.draw_board(ctx);
                self.draw_settings_bar(ctx);
            }
            Screen::Online => {
                self.handle_net(ctx);
                if ctx.ui.clicked(UI_BACK) || ctx.ui.clicked(UI_BACK_L) {
                    ctx.net.close();
                    self.online = None;
                    self.screen = Screen::Menu;
                }
                self.handle_online_input(ctx);
                self.draw_board(ctx);
                self.draw_online_hud(ctx);
                let _ = ctx.net.role() == NetRole::Client;
            }
        }
    }
}

fn apply_board(online: &mut OnlineState, board: &BoardMsg) {
    if online.my_seat() != Some(board.seat) {
        return;
    }
    online.width = board.width as usize;
    online.height = board.height as usize;
    online.cells = cells_to_views(board);
}

fn format_event(ev: &MatchEvent) -> String {
    match ev {
        MatchEvent::Cleared { seat, hide_target } => {
            format!("P{seat} cleared — hid P{hide_target}")
        }
        MatchEvent::Damaged { seat, reason, hp } => {
            format!("P{seat} -1hp ({reason:?}) → {hp}")
        }
        MatchEvent::Eliminated { seat } => format!("P{seat} OUT"),
        MatchEvent::Winner { seat: Some(s) } => format!("Winner P{s}"),
        MatchEvent::Winner { seat: None } => "Draw".into(),
    }
}

fn menu_btn(ui: &mut beton::Ui, font: &Font, id: UiId, label: UiId, text: &str, hot: bool) {
    ui.node(
        id,
        style!(
            flex_direction: row,
            width: 280 pt,
            height: 44 pt,
            justify_content: center,
            align_items: center,
            background: Some(if hot {
                [0.35, 0.55, 0.85, 1.0]
            } else {
                [0.26, 0.30, 0.38, 1.0]
            }),
        ),
        |ui| {
            ui.text(
                label,
                font,
                [0.0, 18.0],
                style!(color: [0.95, 0.96, 0.98, 1.0]),
                text,
            );
        },
    );
}

fn chip(
    ui: &mut beton::Ui,
    font: &Font,
    id: UiId,
    label: UiId,
    text: &str,
    hot: bool,
    active: bool,
) {
    let bg = if active {
        [0.30, 0.55, 0.85, 1.0]
    } else if hot {
        [0.40, 0.45, 0.55, 1.0]
    } else {
        [0.26, 0.30, 0.38, 1.0]
    };
    ui.node(
        id,
        style!(
            flex_direction: row,
            height: 34 pt,
            padding_inline: 10 pt,
            justify_content: center,
            align_items: center,
            background: Some(bg),
        ),
        |ui| {
            ui.text(
                label,
                font,
                [0.0, 16.0],
                style!(color: [0.95, 0.96, 0.98, 1.0]),
                text,
            );
        },
    );
}

fn stepper(
    ui: &mut beton::Ui,
    font: &Font,
    label: &str,
    dec: UiId,
    val: UiId,
    inc: UiId,
    value: u32,
) {
    ui.node(
        UiId(0),
        style!(
            flex_direction: row,
            align_items: center,
            gap: 4 pt,
            height: 34 pt,
        ),
        |ui| {
            ui.text(
                UiId(0),
                font,
                [0.0, 14.0],
                style!(color: [0.75, 0.78, 0.85, 1.0]),
                label,
            );
            let dec_hot = ui.hovered(dec);
            ui.node(
                dec,
                style!(
                    width: 28 pt,
                    height: 28 pt,
                    justify_content: center,
                    align_items: center,
                    background: Some(if dec_hot {
                        [0.45, 0.35, 0.35, 1.0]
                    } else {
                        [0.32, 0.28, 0.30, 1.0]
                    }),
                ),
                |ui| {
                    ui.text(
                        UiId(0),
                        font,
                        [0.0, 18.0],
                        style!(color: [1.0, 0.85, 0.85, 1.0]),
                        "-",
                    );
                },
            );
            ui.node(
                val,
                style!(
                    width: 36 pt,
                    height: 28 pt,
                    justify_content: center,
                    align_items: center,
                    background: Some([0.16, 0.18, 0.22, 1.0]),
                ),
                |ui| {
                    ui.text(
                        UiId(0),
                        font,
                        [0.0, 16.0],
                        style!(color: [0.95, 0.96, 0.98, 1.0]),
                        &value.to_string(),
                    );
                },
            );
            let inc_hot = ui.hovered(inc);
            ui.node(
                inc,
                style!(
                    width: 28 pt,
                    height: 28 pt,
                    justify_content: center,
                    align_items: center,
                    background: Some(if inc_hot {
                        [0.35, 0.45, 0.35, 1.0]
                    } else {
                        [0.28, 0.32, 0.30, 1.0]
                    }),
                ),
                |ui| {
                    ui.text(
                        UiId(0),
                        font,
                        [0.0, 18.0],
                        style!(color: [0.85, 1.0, 0.85, 1.0]),
                        "+",
                    );
                },
            );
        },
    );
}

fn cell_style(view: CellView) -> ([f32; 4], [f32; 4], Option<u8>) {
    let hidden = [0.55, 0.62, 0.72, 1.0];
    let revealed = [0.78, 0.82, 0.88, 1.0];
    let dark = [0.20, 0.22, 0.26, 1.0];
    match view {
        CellView::Unvisited => (hidden, dark, None),
        CellView::Empty => (revealed, dark, None),
        CellView::Number(n) => (revealed, number_color(n), Some(n)),
        CellView::Flag => (hidden, dark, None),
        CellView::Bomb => ([0.92, 0.45, 0.40, 1.0], dark, None),
        CellView::FlaggedBomb => ([0.55, 0.75, 0.45, 1.0], dark, None),
        CellView::Raw(_) => ([0.40, 0.40, 0.45, 1.0], dark, None),
    }
}

fn number_color(n: u8) -> [f32; 4] {
    match n {
        1 => [0.20, 0.35, 0.85, 1.0],
        2 => [0.15, 0.55, 0.25, 1.0],
        3 => [0.85, 0.20, 0.20, 1.0],
        4 => [0.25, 0.15, 0.55, 1.0],
        5 => [0.55, 0.15, 0.15, 1.0],
        6 => [0.15, 0.55, 0.55, 1.0],
        7 => [0.10, 0.10, 0.10, 1.0],
        _ => [0.45, 0.45, 0.45, 1.0],
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
            title: "minesweeper".into(),
            width: 900,
            height: 900,
            max_fps: Some(60),
        },
        MinesweeperGame::new(),
    ) {
        eprintln!("{err}");
        std::process::exit(1);
    }
}
