//! Two-player Pong over the beton net channel.
//!
//! Host is the left paddle and simulates the ball. The first client is the right
//! paddle. `H` hosts port 7777, `C` connects to `ws://127.0.0.1:7777`.
//! Also: `netplay host [PORT]` / `netplay connect HOST:PORT`.
//!
//! Controls: W/S or ↑/↓ move your paddle (mouse Y also works while inside the window).
//!
//! Protocol (game-owned bytes):
//! - client → host: `[1, y_le_f32]` paddle
//! - host → all:    `[2, left_y, right_y, ball_x, ball_y, ball_vx, ball_vy, score_l_u8, score_r_u8]`
//! - host → client: `[3, peer_u32]` once (your id)

#![cfg_attr(windows, windows_subsystem = "windows")]

use std::env;

use beton::{
    run, Camera, Config, Context, Game, Key, NetEvent, NetRole, PeerId, Projection, Sprite,
    TextureHandle, TextureId, Vec3,
};

const WORLD_HEIGHT: f32 = 20.0;
const COURT_HALF_W: f32 = 14.0;
const PADDLE_X: f32 = 12.5;
const PADDLE_W: f32 = 0.45;
const PADDLE_H: f32 = 3.2;
const BALL_SIZE: f32 = 0.55;
const PADDLE_SPEED: f32 = 14.0;
const BALL_SPEED: f32 = 11.0;
const DEFAULT_PORT: u16 = 7777;
const SEND_HZ: f32 = 30.0;

const TAG_PADDLE: u8 = 1;
const TAG_STATE: u8 = 2;
const TAG_YOU: u8 = 3;

enum BootAction {
    None,
    Host(u16),
    Connect(String),
}

struct NetPlay {
    white: Option<TextureHandle>,
    boot: BootAction,
    status: String,
    send_accum: f32,
    online: bool,
    host_failed: bool,
    my_id: Option<PeerId>,
    /// Host only: which peer owns the right paddle.
    right_peer: Option<PeerId>,

    local_y: f32,
    left_y: f32,
    right_y: f32,
    ball: Vec3,
    ball_vel: Vec3,
    score_l: u8,
    score_r: u8,
    /// Ball only moves on the host while both paddles are live.
    playing: bool,
}

impl NetPlay {
    fn new(boot: BootAction) -> Self {
        Self {
            white: None,
            boot,
            status: "H host · C connect · W/S paddles".into(),
            send_accum: 0.0,
            online: false,
            host_failed: false,
            my_id: None,
            right_peer: None,
            local_y: 0.0,
            left_y: 0.0,
            right_y: 0.0,
            ball: Vec3::ZERO,
            ball_vel: Vec3::new(BALL_SPEED, BALL_SPEED * 0.35, 0.0),
            score_l: 0,
            score_r: 0,
            playing: false,
        }
    }

    fn reset_ball(&mut self, to_right: bool) {
        self.ball = Vec3::ZERO;
        let dir = if to_right { 1.0 } else { -1.0 };
        self.ball_vel = Vec3::new(dir * BALL_SPEED, BALL_SPEED * 0.4, 0.0);
    }

    fn clamp_paddle(y: f32) -> f32 {
        let limit = WORLD_HEIGHT * 0.5 - PADDLE_H * 0.5;
        y.clamp(-limit, limit)
    }

    fn encode_you(id: PeerId) -> Vec<u8> {
        let mut out = Vec::with_capacity(5);
        out.push(TAG_YOU);
        out.extend_from_slice(&id.0.to_le_bytes());
        out
    }

    fn decode_you(bytes: &[u8]) -> Option<PeerId> {
        if bytes.len() != 5 || bytes[0] != TAG_YOU {
            return None;
        }
        Some(PeerId(u32::from_le_bytes(bytes[1..5].try_into().ok()?)))
    }

    fn encode_paddle(y: f32) -> Vec<u8> {
        let mut out = Vec::with_capacity(5);
        out.push(TAG_PADDLE);
        out.extend_from_slice(&y.to_le_bytes());
        out
    }

    fn decode_paddle(bytes: &[u8]) -> Option<f32> {
        if bytes.len() != 5 || bytes[0] != TAG_PADDLE {
            return None;
        }
        Some(f32::from_le_bytes(bytes[1..5].try_into().ok()?))
    }

    fn encode_state(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(27);
        out.push(TAG_STATE);
        out.extend_from_slice(&self.left_y.to_le_bytes());
        out.extend_from_slice(&self.right_y.to_le_bytes());
        out.extend_from_slice(&self.ball.x.to_le_bytes());
        out.extend_from_slice(&self.ball.y.to_le_bytes());
        out.extend_from_slice(&self.ball_vel.x.to_le_bytes());
        out.extend_from_slice(&self.ball_vel.y.to_le_bytes());
        out.push(self.score_l);
        out.push(self.score_r);
        out
    }

    fn decode_state(&mut self, bytes: &[u8]) {
        if bytes.len() != 27 || bytes[0] != TAG_STATE {
            return;
        }
        self.left_y = f32::from_le_bytes(bytes[1..5].try_into().unwrap());
        self.right_y = f32::from_le_bytes(bytes[5..9].try_into().unwrap());
        self.ball.x = f32::from_le_bytes(bytes[9..13].try_into().unwrap());
        self.ball.y = f32::from_le_bytes(bytes[13..17].try_into().unwrap());
        self.ball_vel.x = f32::from_le_bytes(bytes[17..21].try_into().unwrap());
        self.ball_vel.y = f32::from_le_bytes(bytes[21..25].try_into().unwrap());
        self.score_l = bytes[25];
        self.score_r = bytes[26];
        self.playing = true;
    }

    fn try_host(&mut self, ctx: &mut Context, port: u16) {
        match ctx.net.host(port) {
            Ok(()) => {
                self.online = true;
                self.host_failed = false;
                self.my_id = Some(PeerId::HOST);
                self.status = format!("hosting :{port} — waiting for opponent");
                self.reset_ball(true);
            }
            Err(err) => {
                self.host_failed = true;
                self.online = false;
                self.my_id = None;
                self.status = err;
            }
        }
    }

    fn try_connect(&mut self, ctx: &mut Context, url: &str) {
        match ctx.net.connect(url) {
            Ok(()) => {
                self.online = true;
                self.host_failed = false;
                self.my_id = None;
                self.status = format!("connecting {url}");
            }
            Err(err) => {
                self.online = false;
                self.my_id = None;
                self.status = err;
            }
        }
    }

    fn handle_events(&mut self, ctx: &mut Context) {
        let events: Vec<NetEvent> = ctx.net.events().to_vec();
        for event in events {
            match event {
                NetEvent::Listening { port } => {
                    self.status = format!("listening :{port}");
                    self.online = true;
                }
                NetEvent::Connected => {
                    self.status = "connected — waiting for seat".into();
                    self.online = true;
                }
                NetEvent::PeerJoined(id) => {
                    if self.right_peer.is_none() {
                        self.right_peer = Some(id);
                        self.playing = true;
                        self.reset_ball(true);
                        self.status = "opponent joined — play!".into();
                    } else {
                        self.status = format!("extra peer {} ignored (2-player)", id.0);
                    }
                    let _ = ctx.net.send(id, &Self::encode_you(id));
                }
                NetEvent::PeerLeft(id) => {
                    if self.right_peer == Some(id) {
                        self.right_peer = None;
                        self.playing = false;
                        self.right_y = 0.0;
                        self.status = "opponent left".into();
                    }
                }
                NetEvent::Message { from, bytes } => match ctx.net.role() {
                    NetRole::Hosting => {
                        if self.right_peer == Some(from) {
                            if let Some(y) = Self::decode_paddle(&bytes) {
                                self.right_y = Self::clamp_paddle(y);
                            }
                        }
                    }
                    NetRole::Client => {
                        if let Some(id) = Self::decode_you(&bytes) {
                            self.my_id = Some(id);
                            self.status = "you are right paddle".into();
                        } else if bytes.first() == Some(&TAG_STATE) {
                            self.decode_state(&bytes);
                        }
                    }
                    NetRole::Offline => {}
                },
                NetEvent::Disconnected => {
                    self.online = false;
                    self.playing = false;
                    self.my_id = None;
                    self.right_peer = None;
                    self.status = "disconnected".into();
                }
                NetEvent::Overflow => {
                    self.status = "net overflow".into();
                }
            }
        }
    }

    fn move_local_paddle(&mut self, ctx: &Context) {
        let mut dy = 0.0;
        if ctx.input.down(Key::KeyW) || ctx.input.down(Key::ArrowUp) {
            dy += 1.0;
        }
        if ctx.input.down(Key::KeyS) || ctx.input.down(Key::ArrowDown) {
            dy -= 1.0;
        }
        if dy != 0.0 {
            self.local_y = Self::clamp_paddle(self.local_y + dy * PADDLE_SPEED * ctx.dt);
        } else if ctx.input.cursor_inside() {
            let world = ctx.camera.screen_to_world(
                ctx.input.cursor_px(),
                ctx.fb_size[0],
                ctx.fb_size[1],
            );
            self.local_y = Self::clamp_paddle(world.y);
        }

        match ctx.net.role() {
            NetRole::Hosting => self.left_y = self.local_y,
            NetRole::Client => {
                // Keep local paddle snappy; state still carries the authoritative echo.
                self.right_y = self.local_y;
            }
            NetRole::Offline => {}
        }
    }

    fn step_ball(&mut self, dt: f32) {
        if !self.playing {
            return;
        }
        self.ball.x += self.ball_vel.x * dt;
        self.ball.y += self.ball_vel.y * dt;

        let half_h = WORLD_HEIGHT * 0.5 - BALL_SIZE * 0.5;
        if self.ball.y > half_h {
            self.ball.y = half_h;
            self.ball_vel.y = -self.ball_vel.y.abs();
        } else if self.ball.y < -half_h {
            self.ball.y = -half_h;
            self.ball_vel.y = self.ball_vel.y.abs();
        }

        let paddle_half = PADDLE_H * 0.5;
        let ball_half = BALL_SIZE * 0.5;

        // Left paddle (host).
        if self.ball_vel.x < 0.0
            && self.ball.x - ball_half <= -PADDLE_X + PADDLE_W * 0.5
            && self.ball.x + ball_half >= -PADDLE_X - PADDLE_W * 0.5
            && (self.ball.y - self.left_y).abs() <= paddle_half + ball_half
        {
            self.ball.x = -PADDLE_X + PADDLE_W * 0.5 + ball_half;
            self.ball_vel.x = self.ball_vel.x.abs();
            let offset = (self.ball.y - self.left_y) / paddle_half;
            self.ball_vel.y = offset * BALL_SPEED;
        }

        // Right paddle (client).
        if self.ball_vel.x > 0.0
            && self.ball.x + ball_half >= PADDLE_X - PADDLE_W * 0.5
            && self.ball.x - ball_half <= PADDLE_X + PADDLE_W * 0.5
            && (self.ball.y - self.right_y).abs() <= paddle_half + ball_half
        {
            self.ball.x = PADDLE_X - PADDLE_W * 0.5 - ball_half;
            self.ball_vel.x = -self.ball_vel.x.abs();
            let offset = (self.ball.y - self.right_y) / paddle_half;
            self.ball_vel.y = offset * BALL_SPEED;
        }

        if self.ball.x < -COURT_HALF_W {
            self.score_r = self.score_r.saturating_add(1);
            self.reset_ball(true);
        } else if self.ball.x > COURT_HALF_W {
            self.score_l = self.score_l.saturating_add(1);
            self.reset_ball(false);
        }
    }

    fn relay(&mut self, ctx: &mut Context) {
        match ctx.net.role() {
            NetRole::Hosting => {
                let _ = ctx.net.broadcast(&self.encode_state());
            }
            NetRole::Client => {
                let _ = ctx.net.send(PeerId::HOST, &Self::encode_paddle(self.local_y));
            }
            NetRole::Offline => {}
        }
    }

    fn draw_court(&self, ctx: &mut Context, white: TextureId) {
        let mut sky = Sprite::new(white, Vec3::new(0.0, 0.0, -1.0), [COURT_HALF_W * 2.2, WORLD_HEIGHT]);
        sky.color = if self.host_failed && !self.online {
            [0.25, 0.08, 0.08, 1.0]
        } else {
            [0.08, 0.09, 0.12, 1.0]
        };
        ctx.scene.push(sky);

        // Center line dashes.
        let mut y = -WORLD_HEIGHT * 0.5 + 0.6;
        while y < WORLD_HEIGHT * 0.5 {
            let mut dash = Sprite::new(white, Vec3::new(0.0, y, -0.5), [0.18, 0.7]);
            dash.color = [0.35, 0.38, 0.45, 1.0];
            ctx.scene.push(dash);
            y += 1.4;
        }

        let left_y = if self.my_id == Some(PeerId::HOST) {
            self.local_y
        } else {
            self.left_y
        };
        let right_y = if self.my_id.is_some() && self.my_id != Some(PeerId::HOST) {
            self.local_y
        } else {
            self.right_y
        };

        let mut left = Sprite::new(
            white,
            Vec3::new(-PADDLE_X, left_y, 0.0),
            [PADDLE_W, PADDLE_H],
        );
        left.color = [0.95, 0.40, 0.35, 1.0];
        ctx.scene.push(left);

        let mut right = Sprite::new(
            white,
            Vec3::new(PADDLE_X, right_y, 0.0),
            [PADDLE_W, PADDLE_H],
        );
        right.color = [0.35, 0.75, 0.95, 1.0];
        ctx.scene.push(right);

        if self.online {
            let mut ball = Sprite::new(white, self.ball, [BALL_SIZE, BALL_SIZE]);
            ball.color = [0.95, 0.92, 0.55, 1.0];
            ctx.scene.push(ball);
        }

        // Score as small blocks (no font atlas in this sample).
        for i in 0..self.score_l.min(8) {
            let mut pip = Sprite::new(
                white,
                Vec3::new(-3.0 - i as f32 * 0.55, WORLD_HEIGHT * 0.5 - 0.8, 0.5),
                [0.4, 0.4],
            );
            pip.color = [0.95, 0.40, 0.35, 1.0];
            ctx.scene.push(pip);
        }
        for i in 0..self.score_r.min(8) {
            let mut pip = Sprite::new(
                white,
                Vec3::new(3.0 + i as f32 * 0.55, WORLD_HEIGHT * 0.5 - 0.8, 0.5),
                [0.4, 0.4],
            );
            pip.color = [0.35, 0.75, 0.95, 1.0];
            ctx.scene.push(pip);
        }
    }
}

impl Game for NetPlay {
    fn boot(&mut self, ctx: &mut Context) {
        let white = ctx
            .gfx
            .create_texture(1, 1, &[255, 255, 255, 255])
            .expect("white");
        self.white = Some(ctx.gfx.pin(white).expect("pin white"));

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

        match std::mem::replace(&mut self.boot, BootAction::None) {
            BootAction::Host(port) => self.try_host(ctx, port),
            BootAction::Connect(url) => self.try_connect(ctx, &url),
            BootAction::None => {}
        }
    }

    fn update(&mut self, ctx: &mut Context) {
        let white: TextureId = self.white.as_ref().expect("boot").id();

        if ctx.input.pressed(Key::KeyH) && ctx.net.role() == NetRole::Offline {
            self.try_host(ctx, DEFAULT_PORT);
        }
        if ctx.input.pressed(Key::KeyC) && ctx.net.role() == NetRole::Offline {
            self.try_connect(ctx, &format!("ws://127.0.0.1:{DEFAULT_PORT}"));
        }

        self.handle_events(ctx);
        self.move_local_paddle(ctx);

        if ctx.net.role() == NetRole::Hosting {
            self.step_ball(ctx.dt);
        }

        self.send_accum += ctx.dt;
        if self.send_accum >= 1.0 / SEND_HZ {
            self.send_accum = 0.0;
            self.relay(ctx);
        }

        self.draw_court(ctx, white);
        let _ = &self.status;
    }
}

fn parse_args(args: &[String]) -> BootAction {
    match args.get(1).map(String::as_str) {
        Some("host") => {
            let port = args
                .get(2)
                .and_then(|s| s.parse().ok())
                .unwrap_or(DEFAULT_PORT);
            BootAction::Host(port)
        }
        Some("connect") => {
            let addr = args
                .get(2)
                .cloned()
                .unwrap_or_else(|| format!("127.0.0.1:{DEFAULT_PORT}"));
            let url = if addr.starts_with("ws://") {
                addr
            } else {
                format!("ws://{addr}")
            };
            BootAction::Connect(url)
        }
        _ => BootAction::None,
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let boot = parse_args(&args);
    if let Err(err) = run(
        Config {
            title: "netplay pong".into(),
            width: 900,
            height: 600,
            max_fps: Some(60),
        },
        NetPlay::new(boot),
    ) {
        eprintln!("{err}");
        std::process::exit(1);
    }
}
