//! Worm match. Hotseat from the menu, or `worms host [PORT]` / `worms connect HOST:PORT`.
//!
//! A/D or arrows move, W jumps, up/down aim, hold Space to charge and release to fire.
//! 1–9 or the badges pick a weapon. Q skips. R restarts.
//!
//! The desktop host simulates. Clients send input and paint snapshots. A browser can join
//! but cannot host.

mod draw;
mod font;
mod net;
mod particles;
mod sim;
mod terrain;
mod weapons;
mod worm;

use std::collections::HashMap;
use std::env;
use std::path::Path;

use beton::{
    run, step_particles, style, Camera, Config, Context, Font, Game, Key, NetEvent,
    PeerId, Projection, ShaderDesc, ShaderId, Sprite, TextureHandle, TextureId, UiId, Vec3,
    VertexShader,
};
use draw::{aim_uv, grid_uv, hand_col, power_uv, proj_draw, prop_uv, team_color};
use net::{
    decode_blast, decode_go, decode_input, decode_mask, decode_snap, decode_welcome, encode_blast,
    encode_go, encode_input, encode_mask, encode_need, encode_snap, encode_welcome, InputMsg,
    Snap, DEFAULT_PORT, TAG_BLAST, TAG_GO, TAG_INPUT, TAG_MASK, TAG_NEED, TAG_SNAP, TAG_WELCOME,
};
use particles::{emit_hit, emit_leaf, emit_spark};
use sim::{Control, Match, AIM_SPEED, DT, RETREAT_TIME, WORM_COUNT};
use terrain::{Terrain, MAP_H, MAP_W};
use weapons::Weapon;

const UI_MENU: UiId = UiId(1);
const UI_TITLE: UiId = UiId(2);
const UI_START: UiId = UiId(3);
const UI_START_L: UiId = UiId(4);
const UI_HOST: UiId = UiId(5);
const UI_HOST_L: UiId = UiId(6);
const UI_JOIN: UiId = UiId(7);
const UI_JOIN_L: UiId = UiId(8);
const UI_STATUS: UiId = UiId(9);
const UI_LOBBY: UiId = UiId(10);
const UI_GO: UiId = UiId(11);
const UI_GO_L: UiId = UiId(12);
const UI_TRAY: UiId = UiId(22);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Screen {
    Menu,
    Lobby,
    Match,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Role {
    Offline,
    Host,
    Client,
}

enum BootAction {
    None,
    Host(u16),
    Connect(String),
}

struct Worms {
    screen: Screen,
    role: Role,
    boot: Option<BootAction>,
    status: String,
    white: Option<TextureHandle>,
    font_tex: Option<TextureHandle>,
    font: Option<Font>,
    worm_tex: Option<TextureHandle>,
    weapon_tex: Option<TextureHandle>,
    icon_tex: Option<TextureHandle>,
    prop_tex: Option<TextureHandle>,
    dirt_id: Option<TextureId>,
    dirt_pin: Option<TextureHandle>,
    weapons: Vec<Weapon>,
    shaders: HashMap<String, ShaderId>,
    ring: ShaderId,
    match_state: Option<Match>,
    terrain: Option<Terrain>,
    seed: u64,
    acc: f32,
    space_was: bool,
    arm_space: bool,
    my_id: Option<PeerId>,
    my_team: u8,
    seats: Vec<(PeerId, u8)>,
    remote: [Option<InputMsg>; 2],
    send_acc: f32,
    snap_acc: f32,
    blast_sent: usize,
    snap: Option<Snap>,
    shown: [(f32, f32); WORM_COUNT],
    have_shown: bool,
    leaf_acc: f32,
}

impl Worms {
    fn new(boot: BootAction) -> Self {
        Self {
            screen: Screen::Menu,
            role: Role::Offline,
            boot: Some(boot),
            status: "Hotseat, or host / join".into(),
            white: None,
            font_tex: None,
            font: None,
            worm_tex: None,
            weapon_tex: None,
            icon_tex: None,
            prop_tex: None,
            dirt_id: None,
            dirt_pin: None,
            weapons: Vec::new(),
            shaders: HashMap::new(),
            ring: ShaderId::SPRITE,
            match_state: None,
            terrain: None,
            seed: 0xA11C_E,
            acc: 0.0,
            space_was: false,
            arm_space: false,
            my_id: None,
            my_team: 0,
            seats: Vec::new(),
            remote: [None, None],
            send_acc: 0.0,
            snap_acc: 0.0,
            blast_sent: 0,
            snap: None,
            shown: [(0.0, 0.0); WORM_COUNT],
            have_shown: false,
            leaf_acc: 0.0,
        }
    }

    fn load_weapons(&mut self, ctx: &mut Context) -> Result<(), String> {
        self.weapons.clear();
        self.shaders.clear();
        let mut paths = ctx.gfx.list_dir("assets/weapons");
        paths.retain(|p| {
            p.extension()
                .and_then(|e| e.to_str())
                .map(|e| e.eq_ignore_ascii_case("txt"))
                .unwrap_or(false)
        });
        paths.sort();
        if paths.is_empty() {
            return Err("no weapons in assets/weapons".into());
        }
        for path in paths {
            let stem = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("weapon")
                .to_string();
            let bytes = ctx.gfx.read_bytes(&path)?;
            let text = String::from_utf8(bytes).map_err(|err| format!("{stem}: {err}"))?;
            let mut weapon = Weapon::parse(&stem, &text)?;
            weapon.shader = self.ensure_shader(ctx, &weapon)?;
            if weapon.effect == "ring" {
                self.ring = weapon.shader;
            }
            self.weapons.push(weapon);
        }
        Ok(())
    }

    fn ensure_shader(&mut self, ctx: &mut Context, weapon: &Weapon) -> Result<ShaderId, String> {
        if let Some(&id) = self.shaders.get(&weapon.effect) {
            return Ok(id);
        }
        let path = Path::new("assets/shaders").join(format!("{}.wgsl", weapon.effect));
        let bytes = ctx.gfx.read_bytes(&path)?;
        let source = String::from_utf8(bytes).map_err(|err| err.to_string())?;
        let id = ctx.gfx.create_shader(ShaderDesc {
            wgsl: &source,
            blend: weapon.blend,
            vertex: VertexShader::Engine,
            uniform_bytes: 16,
        })?;
        self.shaders.insert(weapon.effect.clone(), id);
        Ok(id)
    }

    fn write_time(&self, ctx: &mut Context) {
        let seconds = ctx.time;
        let bytes = [seconds.to_le_bytes(), [0; 4], [0; 4], [0; 4]].concat();
        for &id in self.shaders.values() {
            let _ = ctx.gfx.write_uniform(id, &bytes);
        }
    }

    fn begin_match(&mut self, ctx: &mut Context, seed: u64) {
        self.seed = seed;
        let mut terrain = Terrain::generate(seed);
        // Spawn on dirt first, then stamp pines away from worm columns.
        let mut state = Match::new(&terrain, seed);
        let spawns: Vec<f32> = state.worms.iter().map(|w| w.body.pos.x).collect();
        terrain.plant_ridge_trees(&spawns);
        self.upload_dirt(ctx, &terrain);
        state.grant_loadout(&self.weapons);
        self.snap = Some(Snap::from_match(&state));
        self.have_shown = false;
        self.blast_sent = 0;
        self.acc = 0.0;
        self.match_state = Some(state);
        self.terrain = Some(terrain);
        self.screen = Screen::Match;
        self.arm_space = !ctx.input.down(Key::Space);
        self.space_was = ctx.input.down(Key::Space);
        ctx.particles.clear();
    }

    fn upload_dirt(&mut self, ctx: &mut Context, terrain: &Terrain) {
        if let Some(id) = self.dirt_id {
            let _ = ctx.gfx.update_texture(id, &terrain.rgba);
        } else {
            let id = ctx
                .gfx
                .create_texture(MAP_W, MAP_H, &terrain.rgba)
                .expect("dirt");
            self.dirt_pin = Some(ctx.gfx.pin(id).expect("pin dirt"));
            self.dirt_id = Some(id);
        }
    }

    fn sync_dirt(&mut self, ctx: &mut Context) {
        let Some(terrain) = self.terrain.as_mut() else {
            return;
        };
        if !terrain.dirty {
            return;
        }
        terrain.dirty = false;
        if let Some(id) = self.dirt_id {
            let _ = ctx.gfx.update_texture(id, &terrain.rgba);
        }
    }

    fn try_host(&mut self, ctx: &mut Context, port: u16) {
        match ctx.net.host(port) {
            Ok(()) => {
                self.role = Role::Host;
                self.my_id = Some(PeerId::HOST);
                self.my_team = 0;
                self.seats.clear();
                self.screen = Screen::Lobby;
                self.status = format!("hosting :{port} — waiting");
            }
            Err(err) => {
                self.role = Role::Offline;
                self.status = err;
            }
        }
    }

    fn try_connect(&mut self, ctx: &mut Context, url: &str) {
        match ctx.net.connect(url) {
            Ok(()) => {
                self.role = Role::Client;
                self.my_team = 1;
                self.screen = Screen::Lobby;
                self.status = format!("connecting {url}");
            }
            Err(err) => {
                self.role = Role::Offline;
                self.status = err;
            }
        }
    }

    fn start_hosted(&mut self, ctx: &mut Context) {
        if self.seats.is_empty() {
            self.status = "waiting for a player".into();
            return;
        }
        let seed = self.seed.wrapping_add(0x9E37_79B9);
        self.begin_match(ctx, seed);
        let _ = ctx.net.broadcast(&encode_go(seed));
        self.status = "match".into();
    }

    fn handle_net(&mut self, ctx: &mut Context) {
        let events: Vec<NetEvent> = ctx.net.events().to_vec();
        for event in events {
            match event {
                NetEvent::Listening { port } => {
                    self.status = format!("listening :{port}");
                }
                NetEvent::Connected => {
                    self.status = "connected — waiting for the host".into();
                    self.role = Role::Client;
                }
                NetEvent::PeerJoined(id) => {
                    if self.role != Role::Host || self.seats.len() >= 3 {
                        continue;
                    }
                    let team = if self.seats.len() % 2 == 0 { 1 } else { 0 };
                    self.seats.push((id, team));
                    let _ = ctx.net.send(id, &encode_welcome(id, team));
                    if self.screen == Screen::Match {
                        let _ = ctx.net.send(id, &encode_go(self.seed));
                        if let Some(terrain) = &self.terrain {
                            let _ = ctx.net.send(id, &encode_mask(&terrain.pack_solid()));
                        }
                    }
                    if self.screen != Screen::Match {
                        self.status = "READY".into();
                    }
                }
                NetEvent::PeerLeft(id) => {
                    let team = self.seats.iter().find(|s| s.0 == id).map(|s| s.1);
                    self.seats.retain(|s| s.0 != id);
                    if self.role == Role::Host {
                        if let Some(team) = team {
                            let still = team == 0 || self.seats.iter().any(|s| s.1 == team);
                            if !still {
                                if let Some(state) = self.match_state.as_mut() {
                                    state.kill_team(team);
                                }
                                self.status = format!("team {} left", team + 1);
                            }
                        }
                    }
                }
                NetEvent::Message { from, bytes } => self.on_message(ctx, from, &bytes),
                NetEvent::Datagram { bytes, .. } => {
                    if self.role == Role::Client && bytes.first() == Some(&TAG_SNAP) {
                        if let Some(snap) = decode_snap(&bytes) {
                            self.snap = Some(snap);
                        }
                    }
                }
                NetEvent::Disconnected => {
                    self.role = Role::Offline;
                    self.screen = Screen::Menu;
                    self.status = "disconnected".into();
                    self.match_state = None;
                }
                NetEvent::Overflow => {
                    if self.role == Role::Client {
                        let _ = ctx.net.send(PeerId::HOST, &encode_need());
                        self.status = "asking for the map".into();
                    }
                }
            }
        }
    }

    fn on_message(&mut self, ctx: &mut Context, from: PeerId, bytes: &[u8]) {
        let Some(&tag) = bytes.first() else {
            return;
        };
        match tag {
            TAG_WELCOME => {
                if let Some(welcome) = decode_welcome(bytes) {
                    self.my_id = Some(PeerId(welcome.id));
                    self.my_team = welcome.team;
                    self.status = if welcome.team == 0 {
                        "READY — red".into()
                    } else {
                        "READY — blue".into()
                    };
                }
            }
            TAG_GO => {
                if let Some(seed) = decode_go(bytes) {
                    self.begin_match(ctx, seed);
                    self.role = Role::Client;
                }
            }
            TAG_BLAST => {
                if self.role == Role::Client {
                    if let Some(blast) = decode_blast(bytes) {
                        if let Some(terrain) = self.terrain.as_mut() {
                            terrain.stamp_circle(blast.x, blast.y, blast.radius);
                        }
                        if let Some(white) = self.white.as_ref().map(|t| t.id()) {
                            let (n, speed, life) = self.hit_spec(blast.weapon, blast.damage);
                            emit_hit(
                                &mut ctx.particles,
                                white,
                                self.ring,
                                Vec3::new(blast.x, blast.y, 0.3),
                                blast.radius,
                                n,
                                speed,
                                life,
                            );
                        }
                    }
                }
            }
            TAG_MASK => {
                if self.role == Role::Client {
                    if let Some(mask) = decode_mask(bytes) {
                        if let Some(terrain) = self.terrain.as_mut() {
                            let _ = terrain.unpack_solid(mask);
                        }
                    }
                }
            }
            TAG_NEED => {
                if self.role == Role::Host {
                    if let Some(terrain) = &self.terrain {
                        let _ = ctx.net.broadcast(&encode_mask(&terrain.pack_solid()));
                    }
                }
            }
            TAG_INPUT => {
                if self.role == Role::Host {
                    if let Some(msg) = decode_input(bytes) {
                        if let Some(team) = self.seats.iter().find(|s| s.0 == from).map(|s| s.1) {
                            self.remote[team as usize] = Some(msg);
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn read_control(&self, ctx: &Context) -> Control {
        let mut dir = 0.0;
        if ctx.input.down(Key::KeyA) || ctx.input.down(Key::ArrowLeft) {
            dir -= 1.0;
        }
        if ctx.input.down(Key::KeyD) || ctx.input.down(Key::ArrowRight) {
            dir += 1.0;
        }
        let mut aim = 0.0;
        if ctx.input.down(Key::ArrowUp) {
            aim += AIM_SPEED * DT;
        }
        if ctx.input.down(Key::ArrowDown) {
            aim -= AIM_SPEED * DT;
        }
        let mut weapon = None;
        for i in 0..self.weapons.len().min(9) {
            let key = match i {
                0 => Key::Digit1,
                1 => Key::Digit2,
                2 => Key::Digit3,
                3 => Key::Digit4,
                4 => Key::Digit5,
                5 => Key::Digit6,
                6 => Key::Digit7,
                7 => Key::Digit8,
                _ => Key::Digit9,
            };
            if ctx.input.pressed(key) || ctx.ui.clicked(UiId(40 + i as u32)) {
                weapon = Some(i);
            }
        }
        let down = self.arm_space && ctx.input.down(Key::Space);
        let pressed = down && !self.space_was;
        let released = self.arm_space && !ctx.input.down(Key::Space) && self.space_was;
        let pellets = self.pellets_selected();
        Control {
            dir,
            jump: ctx.input.pressed(Key::KeyW),
            aim,
            charge_held: down && !pellets,
            charge_released: if pellets { pressed } else { released },
            weapon,
            skip: ctx.input.pressed(Key::KeyQ),
        }
    }

    fn pellets_selected(&self) -> bool {
        let Some(state) = &self.match_state else {
            return false;
        };
        let index = if self.role == Role::Client {
            self.snap
                .as_ref()
                .map(|s| s.worms[s.active as usize].weapon as usize)
                .unwrap_or(0)
        } else {
            state.worms[state.active].weapon
        };
        self.weapons
            .get(index)
            .map(|w| w.behavior == weapons::Behavior::Pellets)
            .unwrap_or(false)
    }

    fn control_for(&mut self, local: Control, first: bool) -> Control {
        if self.role == Role::Client {
            return Control::default();
        }
        if self.role == Role::Offline {
            return if first { local } else { local.held_only() };
        }
        let team = self
            .match_state
            .as_ref()
            .map(|m| m.worms[m.active].team)
            .unwrap_or(0);
        let use_local = team == self.my_team && (local.any() || self.remote[team as usize].is_none());
        if use_local {
            return if first { local } else { local.held_only() };
        }
        let Some(msg) = self.remote[team as usize] else {
            return Control::default();
        };
        let control = Control {
            dir: msg.dir as f32,
            jump: first && msg.jump == 1,
            aim: if first && msg.aim_set == 1 { msg.aim } else { 0.0 },
            charge_held: msg.charge == 1,
            charge_released: first && msg.fire == 1,
            weapon: if first && msg.weapon != 255 {
                Some(msg.weapon as usize)
            } else {
                None
            },
            skip: first && msg.skip == 1,
        };
        if first {
            if let Some(msg) = self.remote[team as usize].as_mut() {
                msg.jump = 0;
                msg.fire = 0;
                msg.skip = 0;
                msg.weapon = 255;
                msg.aim_set = 0;
            }
        }
        control
    }

    fn hit_spec(&self, weapon: u8, damage: f32) -> (u32, f32, f32) {
        let spec = self.weapons.get(weapon as usize);
        let n = spec.map(|w| w.fragments).unwrap_or(8);
        let speed = spec.map(|w| w.fragment_speed).unwrap_or(40.0) * (1.0 + damage / 160.0);
        let life = spec.map(|w| w.fragment_life).unwrap_or(0.45);
        (n, speed, life)
    }

    fn flush_net(&mut self, ctx: &mut Context) {
        if self.role == Role::Client {
            return;
        }
        let Some(state) = &self.match_state else {
            return;
        };
        let white = self.white.as_ref().map(|t| t.id());
        let ring = self.ring;
        while self.blast_sent < state.blasts.len() {
            let blast = state.blasts[self.blast_sent];
            if let Some(white) = white {
                let (n, speed, life) = self.hit_spec(blast.weapon, blast.damage);
                emit_hit(
                    &mut ctx.particles,
                    white,
                    ring,
                    Vec3::new(blast.x, blast.y, 0.3),
                    blast.radius,
                    n,
                    speed,
                    life,
                );
            }
            if self.role == Role::Host {
                let _ = ctx.net.broadcast(&encode_blast(blast));
            }
            self.blast_sent += 1;
        }
        if self.role == Role::Host {
            self.snap_acc += ctx.dt;
            if self.snap_acc >= 1.0 / 20.0 {
                self.snap_acc = 0.0;
                if let Some(snap) = &self.snap {
                    let _ = ctx.net.broadcast_unreliable(&encode_snap(snap));
                }
            }
        }
    }

    fn send_input(&mut self, ctx: &mut Context, local: Control) {
        if self.role != Role::Client || self.screen != Screen::Match {
            return;
        }
        self.send_acc += ctx.dt;
        if self.send_acc < 0.1 && !local.any() {
            return;
        }
        self.send_acc = 0.0;
        let msg = InputMsg {
            dir: local.dir as i8,
            jump: u8::from(local.jump),
            aim: local.aim,
            aim_set: u8::from(local.aim != 0.0),
            charge: u8::from(local.charge_held),
            fire: u8::from(local.charge_released),
            weapon: local.weapon.map(|w| w as u8).unwrap_or(255),
            skip: u8::from(local.skip),
        };
        let _ = ctx.net.send(PeerId::HOST, &encode_input(&msg));
    }

    fn button(ui: &mut beton::Ui, id: UiId, label: UiId, font: &Font, text: &str, hot: bool) {
        ui.node(
            id,
            style!(
                width: 220 pt,
                height: 48 pt,
                justify_content: center,
                align_items: center,
                background: Some(if hot {
                    [0.35, 0.62, 0.38, 1.0]
                } else {
                    [0.18, 0.36, 0.62, 1.0]
                }),
            ),
            |ui| {
                ui.text(
                    label,
                    font,
                    [14.0, 20.0],
                    style!(color: [1.0, 1.0, 1.0, 1.0]),
                    text,
                );
            },
        );
    }

    fn update_menu(&mut self, ctx: &mut Context) {
        let Some(font) = self.font.clone() else {
            return;
        };
        let start_hot = ctx.ui.hovered(UI_START) || ctx.ui.hovered(UI_START_L);
        let host_hot = ctx.ui.hovered(UI_HOST) || ctx.ui.hovered(UI_HOST_L);
        let join_hot = ctx.ui.hovered(UI_JOIN) || ctx.ui.hovered(UI_JOIN_L);
        let [fb_w, fb_h] = ctx.fb_size;
        let status = self.status.clone();
        ctx.ui.node(
            UI_MENU,
            style!(
                position: absolute,
                width: (fb_w) pt,
                height: (fb_h) pt,
                top: 0 pt,
                left: 0 pt,
                flex_direction: column,
                justify_content: center,
                align_items: center,
                gap: 16 pt,
                background: Some([0.06, 0.1, 0.22, 1.0]),
            ),
            |ui| {
                ui.text(
                    UI_TITLE,
                    &font,
                    [28.0, 40.0],
                    style!(color: [0.95, 0.92, 0.8, 1.0]),
                    "WORMS",
                );
                Self::button(ui, UI_START, UI_START_L, &font, "START", start_hot);
                if !cfg!(target_arch = "wasm32") {
                    Self::button(ui, UI_HOST, UI_HOST_L, &font, "HOST", host_hot);
                }
                Self::button(ui, UI_JOIN, UI_JOIN_L, &font, "JOIN", join_hot);
                ui.text(
                    UI_STATUS,
                    &font,
                    [10.0, 14.0],
                    style!(color: [0.8, 0.86, 0.95, 1.0]),
                    &status,
                );
            },
        );
        if ctx.ui.clicked(UI_START) || ctx.ui.clicked(UI_START_L) || ctx.input.pressed(Key::Enter) {
            let _ = ctx.net.close();
            self.role = Role::Offline;
            self.my_team = 0;
            let seed = self.seed.wrapping_add(0x9E37_79B9);
            self.begin_match(ctx, seed);
        } else if !cfg!(target_arch = "wasm32")
            && (ctx.ui.clicked(UI_HOST) || ctx.ui.clicked(UI_HOST_L) || ctx.input.pressed(Key::KeyH))
        {
            self.try_host(ctx, DEFAULT_PORT);
        } else if ctx.ui.clicked(UI_JOIN) || ctx.ui.clicked(UI_JOIN_L) || ctx.input.pressed(Key::KeyC)
        {
            if let Some(url) = ask_connect_url() {
                self.try_connect(ctx, &url);
            }
        }
    }

    fn update_lobby(&mut self, ctx: &mut Context) {
        let Some(font) = self.font.clone() else {
            return;
        };
        let go_hot = ctx.ui.hovered(UI_GO) || ctx.ui.hovered(UI_GO_L);
        let [fb_w, fb_h] = ctx.fb_size;
        let status = self.status.clone();
        let host = self.role == Role::Host;
        ctx.ui.node(
            UI_LOBBY,
            style!(
                position: absolute,
                width: (fb_w) pt,
                height: (fb_h) pt,
                top: 0 pt,
                left: 0 pt,
                flex_direction: column,
                justify_content: center,
                align_items: center,
                gap: 14 pt,
                background: Some([0.06, 0.1, 0.22, 1.0]),
            ),
            |ui| {
                ui.text(
                    UI_TITLE,
                    &font,
                    [18.0, 26.0],
                    style!(color: [1.0, 1.0, 1.0, 1.0]),
                    "LOBBY",
                );
                ui.text(
                    UI_STATUS,
                    &font,
                    [10.0, 14.0],
                    style!(color: [0.85, 0.9, 1.0, 1.0]),
                    &status,
                );
                if host {
                    Self::button(ui, UI_GO, UI_GO_L, &font, "START", go_hot);
                }
            },
        );
        if host && (ctx.ui.clicked(UI_GO) || ctx.ui.clicked(UI_GO_L) || ctx.input.pressed(Key::Enter))
        {
            self.start_hosted(ctx);
        }
    }

    fn update_match(&mut self, ctx: &mut Context) {
        if ctx.input.pressed(Key::KeyR) && self.role != Role::Client {
            let seed = self.seed.wrapping_add(0x9E37_79B9);
            self.begin_match(ctx, seed);
            if self.role == Role::Host {
                let _ = ctx.net.broadcast(&encode_go(seed));
            }
            return;
        }
        let local = self.read_control(ctx);
        self.space_was = ctx.input.down(Key::Space);
        if !self.arm_space && !ctx.input.down(Key::Space) {
            self.arm_space = true;
        }
        self.send_input(ctx, local);

        if self.role != Role::Client {
            self.acc += ctx.dt.min(0.1);
            let mut n = 0;
            while self.acc >= DT && n < 4 {
                self.acc -= DT;
                let control = self.control_for(local, n == 0);
                let weapons = self.weapons.clone();
                if let (Some(state), Some(terrain)) =
                    (self.match_state.as_mut(), self.terrain.as_mut())
                {
                    state.step(terrain, &weapons, control, DT);
                }
                n += 1;
            }
            if let Some(state) = &self.match_state {
                self.snap = Some(Snap::from_match(state));
            }
        }
        self.flush_net(ctx);
        self.sync_dirt(ctx);
        self.draw_world(ctx);
        self.draw_hud(ctx);
    }

    fn draw_world(&mut self, ctx: &mut Context) {
        let Some(snap) = self.snap.clone() else {
            return;
        };
        let Some(white) = self.white.as_ref().map(|t| t.id()) else {
            return;
        };
        let Some(font) = self.font.clone() else {
            return;
        };
        self.write_time(ctx);
        step_particles(&mut ctx.particles, ctx.dt, |_| false);

        for i in 0..6 {
            let t = i as f32 / 6.0;
            let y = MAP_H as f32 * (0.15 + t * 0.85);
            let h = MAP_H as f32 / 6.0 + 2.0;
            let mut band = Sprite::new(white, Vec3::new(MAP_W as f32 * 0.5, y, -1.0), [MAP_W as f32, h]);
            let top = 0.10 + t * 0.18;
            band.color = [0.15 + top * 0.2, 0.22 + top * 0.35, 0.45 + top * 0.4, 1.0];
            ctx.scene.push(band);
        }

        if let Some(props) = self.prop_tex.as_ref().map(|t| t.id()) {
            for (i, x) in [80.0, 220.0, 360.0].into_iter().enumerate() {
                let mut cloud = Sprite::new(
                    props,
                    Vec3::new(x, 248.0 + (i as f32) * 4.0, -0.4),
                    [28.0, 12.0],
                );
                cloud.uv = prop_uv(32.0, 0.0, 32.0, 16.0);
                ctx.scene.push(cloud);
            }
        }

        if let Some(dirt) = self.dirt_id {
            ctx.scene.push(Sprite::new(
                dirt,
                Vec3::new(MAP_W as f32 * 0.5, MAP_H as f32 * 0.5, 0.0),
                [MAP_W as f32, MAP_H as f32],
            ));
        }

        let water = snap.water_y;
        let mut body = Sprite::new(
            white,
            Vec3::new(MAP_W as f32 * 0.5, water * 0.5, 0.2),
            [MAP_W as f32, water.max(1.0)],
        );
        body.color = [0.28, 0.52, 0.86, 1.0];
        ctx.scene.push(body);
        let mut x = 0.0;
        while x < MAP_W as f32 {
            let crest = water + ((x * 0.08) + ctx.time * 1.4).sin() * 2.2;
            let mut wave = Sprite::new(white, Vec3::new(x, crest, 0.22), [10.0, 2.4]);
            wave.color = [0.75, 0.88, 1.0, 0.95];
            ctx.scene.push(wave);
            x += 8.0;
        }

        if snap.crate_on == 1 {
            if let Some(tex) = self.weapon_tex.as_ref().map(|t| t.id()) {
                let mut crate_sprite = Sprite::new(
                    tex,
                    Vec3::new(snap.crate_x, snap.crate_y, 0.35),
                    [6.0, 6.0],
                );
                crate_sprite.uv = grid_uv(3, 6, 0, 1, false);
                ctx.scene.push(crate_sprite);
            }
        }

        self.leaf_acc += ctx.dt;
        if self.leaf_acc > 1.4 {
            self.leaf_acc = 0.0;
            let lx = (ctx.time * 37.0) % MAP_W as f32;
            emit_leaf(&mut ctx.particles, white, Vec3::new(lx, 250.0, -0.2));
        }

        let worm_tex = self.worm_tex.as_ref().map(|t| t.id());
        let weapon_tex = self.weapon_tex.as_ref().map(|t| t.id());
        for i in 0..WORM_COUNT {
            let w = &snap.worms[i];
            let (x, y) = self.shown_pos(i, w.x, w.y, ctx.dt);
            let alive = w.flags & 1 != 0;
            let grounded = w.flags & 2 != 0;
            let drowning = w.flags & 4 != 0;
            let z = if drowning { 0.12 } else { 0.4 };
            let team = if i < 4 { 0 } else { 1 };
            if !alive && grounded && !drowning {
                if let Some(props) = self.prop_tex.as_ref().map(|t| t.id()) {
                    let mut grave = Sprite::new(props, Vec3::new(x, y, z), [6.0, 6.0]);
                    grave.uv = prop_uv(32.0, 32.0, 16.0, 16.0);
                    ctx.scene.push(grave);
                }
            } else if let Some(tex) = worm_tex {
                let frame = if !grounded && alive {
                    3
                } else if w.vx.abs() > 1.0 {
                    1 + ((ctx.time * 8.0) as u32 % 2)
                } else {
                    0
                };
                // Sprite center sits so the cell floor matches the hitbox feet.
                let mut sprite = Sprite::new(tex, Vec3::new(x, y + 2.0, z), [10.0, 14.0]);
                sprite.uv = grid_uv(frame, 4, 0, 1, w.facing < 0.0);
                ctx.scene.push(sprite);
            } else {
                let mut box_sprite = Sprite::new(white, Vec3::new(x, y, z), [6.0, 10.0]);
                box_sprite.color = team_color(team);
                ctx.scene.push(box_sprite);
            }

            let aiming = snap.phase == 0 && i == snap.active as usize && alive;
            if aiming {
                let muzzle = Vec3::new(
                    x + w.aim.cos() * 6.0,
                    y + 1.0 + w.aim.sin() * 6.0,
                    0.45,
                );
                if let Some(tex) = weapon_tex {
                    let name = self
                        .weapons
                        .get(w.weapon as usize)
                        .map(|item| item.name.as_str())
                        .unwrap_or("");
                    let mut gun = Sprite::new(tex, muzzle, [8.0, 8.0]);
                    gun.uv = grid_uv(hand_col(name), 6, 0, 1, false);
                    gun.rotation = w.aim;
                    ctx.scene.push(gun);
                }
                let aim_dist = 40.0;
                let rx = x + w.aim.cos() * aim_dist;
                let ry = y + 1.0 + w.aim.sin() * aim_dist;
                if let Some(props) = self.prop_tex.as_ref().map(|t| t.id()) {
                    let mut cursor = Sprite::new(props, Vec3::new(rx, ry, 0.55), [10.0, 10.0]);
                    cursor.uv = aim_uv();
                    ctx.scene.push(cursor);
                    let t = (snap.charge / sim::CHARGE_TIME).clamp(0.0, 1.0);
                    if t > 0.02 {
                        let len = 10.0 + t * (aim_dist - 12.0);
                        let width = 2.5 + t * 3.5;
                        // Stack three tinted blobs for a red→orange gradient along the ray.
                        for (k, color) in [
                            [0.75, 0.12, 0.08, 0.85],
                            [0.95, 0.45, 0.12, 0.9],
                            [1.0, 0.85, 0.25, 0.95],
                        ]
                        .iter()
                        .enumerate()
                        {
                            let u = (k as f32 + 0.5) / 3.0;
                            let px = muzzle.x + w.aim.cos() * (len * u);
                            let py = muzzle.y + w.aim.sin() * (len * u);
                            let seg = len / 3.0 + 2.0;
                            let mut blob =
                                Sprite::new(props, Vec3::new(px, py, 0.5), [seg, width]);
                            blob.uv = power_uv();
                            blob.rotation = w.aim;
                            blob.color = *color;
                            ctx.scene.push(blob);
                        }
                    }
                }
            }

            let name = self
                .match_state
                .as_ref()
                .map(|m| m.worms[i].name)
                .unwrap_or("?");
            let color = team_color(team);
            let label = format!("{name}");
            let width = font.measure([3.4, 5.0], &label)[0];
            ctx.scene.text(
                &font,
                Vec3::new(x - width * 0.5, y + 12.4, 0.7),
                [3.4, 5.0],
                color,
                &label,
            );
            let hp = format!("{}", w.hp);
            let hw = font.measure([3.0, 4.4], &hp)[0];
            ctx.scene
                .text(&font, Vec3::new(x - hw * 0.5, y + 8.6, 0.7), [3.0, 4.4], color, &hp);
        }

        for proj in &snap.projs {
            let (col, size) = proj_draw(proj.kind);
            let rot = if proj.kind == 1 {
                ctx.time * 6.0
            } else {
                proj.vy.atan2(proj.vx)
            };
            // Soft yellow disc under the sheet cell so shots read without a neon bloom.
            let mut core = Sprite::new(white, Vec3::new(proj.x, proj.y, 0.54), [
                size[0] * 0.55,
                size[1] * 0.55,
            ]);
            core.color = [1.0, 0.9, 0.35, 0.85];
            ctx.scene.push(core);
            if let Some(tex) = weapon_tex {
                let mut shell = Sprite::new(tex, Vec3::new(proj.x, proj.y, 0.55), size);
                shell.uv = grid_uv(col, 6, 0, 1, false);
                shell.rotation = rot;
                ctx.scene.push(shell);
            }
            let trail = self
                .weapons
                .get(proj.weapon as usize)
                .map(|w| w.trail)
                .unwrap_or(0.0);
            if trail > 0.0 {
                emit_spark(&mut ctx.particles, white, Vec3::new(proj.x, proj.y, 0.55));
                emit_spark(
                    &mut ctx.particles,
                    white,
                    Vec3::new(proj.x - proj.vx * 0.012, proj.y - proj.vy * 0.012, 0.55),
                );
            }
        }

        let clock = clock_text(snap.timer);
        ctx.scene.text(
            &font,
            Vec3::new(8.0, 16.0, 0.9),
            [3.0, 4.5],
            [1.0, 1.0, 1.0, 1.0],
            &clock,
        );
        if snap.phase == 1 {
            let width = 36.0 * (snap.timer / RETREAT_TIME).clamp(0.0, 1.0);
            let mut bar = Sprite::new(white, Vec3::new(16.0 + width * 0.5, 12.0, 0.9), [width, 1.5]);
            bar.color = [1.0, 0.75, 0.3, 1.0];
            ctx.scene.push(bar);
        }
        let mut track = Sprite::new(white, Vec3::new(MAP_W as f32 - 40.0, 14.0, 0.9), [56.0, 6.0]);
        track.color = [0.05, 0.05, 0.08, 1.0];
        ctx.scene.push(track);
        let mag = (snap.wind.abs() / 48.0).clamp(0.0, 1.0) * 26.0;
        let cx = if snap.wind >= 0.0 {
            MAP_W as f32 - 40.0 + mag * 0.5
        } else {
            MAP_W as f32 - 40.0 - mag * 0.5
        };
        let mut fill = Sprite::new(white, Vec3::new(cx, 14.0, 0.91), [mag.max(1.0), 4.0]);
        fill.color = [0.85, 0.15, 0.12, 1.0];
        ctx.scene.push(fill);

        if snap.sudden == 1 {
            ctx.scene.text(
                &font,
                Vec3::new(MAP_W as f32 * 0.5 - 28.0, MAP_H as f32 - 16.0, 0.9),
                [3.0, 4.5],
                [0.7, 0.9, 1.0, 1.0],
                "WATER RISE",
            );
        }
        if snap.phase == 3 {
            let text = match snap.winner {
                0 => "RED WINS",
                1 => "BLUE WINS",
                _ => "DRAW",
            };
            ctx.scene.text(
                &font,
                Vec3::new(MAP_W as f32 * 0.5 - 24.0, MAP_H as f32 * 0.55, 0.95),
                [4.0, 6.0],
                [1.0, 1.0, 1.0, 1.0],
                text,
            );
        }
    }

    fn shown_pos(&mut self, index: usize, x: f32, y: f32, dt: f32) -> (f32, f32) {
        if self.role != Role::Client {
            return (x, y);
        }
        if !self.have_shown {
            self.shown[index] = (x, y);
            if index + 1 == WORM_COUNT {
                self.have_shown = true;
            }
            return (x, y);
        }
        let t = (dt * 12.0).min(1.0);
        let sx = self.shown[index].0 + (x - self.shown[index].0) * t;
        let sy = self.shown[index].1 + (y - self.shown[index].1) * t;
        self.shown[index] = (sx, sy);
        (sx, sy)
    }

    fn draw_hud(&mut self, ctx: &mut Context) {
        let Some(font) = self.font.clone() else {
            return;
        };
        let Some(snap) = self.snap.clone() else {
            return;
        };
        if snap.phase != 0 {
            return;
        }
        let icons = self.icon_tex.as_ref().map(|t| t.id());
        let n = self.weapons.len().min(4);
        ctx.ui.node(
            UI_TRAY,
            style!(
                position: absolute,
                bottom: 28 pt,
                left: 0 pt,
                width: 100 %,
                height: 48 pt,
                flex_direction: row,
                justify_content: center,
                align_items: center,
                gap: 8 pt,
            ),
            |ui| {
                for i in 0..n {
                    let selected = snap.worms[snap.active as usize].weapon as usize == i;
                    ui.node(
                        UiId(40 + i as u32),
                        style!(
                            width: 40 pt,
                            height: 44 pt,
                            flex_direction: column,
                            align_items: center,
                            background: Some(if selected {
                                [0.95, 0.82, 0.25, 1.0]
                            } else {
                                [0.08, 0.14, 0.28, 0.85]
                            }),
                        ),
                        |ui| {
                            if let Some(tex) = icons {
                                ui.image(
                                    UiId(60 + i as u32),
                                    tex,
                                    style!(
                                        width: 32 pt,
                                        height: 32 pt,
                                        uv: grid_uv(i as u32, 4, 0, 1, false),
                                    ),
                                );
                            }
                            let count = snap.worms[snap.active as usize].ammo.get(i).copied().unwrap_or(0);
                            ui.text(
                                UiId(70 + i as u32),
                                &font,
                                [8.0, 12.0],
                                style!(color: [1.0, 1.0, 1.0, 1.0]),
                                &count.to_string(),
                            );
                        },
                    );
                }
            },
        );
    }
}

impl Game for Worms {
    fn assets(&self) -> &[&'static str] {
        &[
            "assets/worm.png",
            "assets/weapons.png",
            "assets/icons.png",
            "assets/props.png",
            "assets/ground.png",
        ]
    }

    fn asset_dirs(&self) -> &[&'static str] {
        &["assets/weapons", "assets/shaders"]
    }

    fn boot(&mut self, ctx: &mut Context) {
        let white = ctx.gfx.white_texture();
        self.white = Some(ctx.gfx.pin(white).expect("white"));
        let (atlas, aw, ah) = font::atlas_rgba();
        let font_tex = ctx.gfx.create_texture(aw, ah, &atlas).expect("font");
        self.font_tex = Some(ctx.gfx.pin(font_tex).expect("pin font"));
        self.font = Some(font::make_font(font_tex));

        if let Ok(id) = ctx.gfx.load_texture(Path::new("assets/worm.png")) {
            self.worm_tex = ctx.gfx.pin(id);
        }
        if let Ok(id) = ctx.gfx.load_texture(Path::new("assets/weapons.png")) {
            self.weapon_tex = ctx.gfx.pin(id);
        }
        if let Ok(id) = ctx.gfx.load_texture(Path::new("assets/icons.png")) {
            self.icon_tex = ctx.gfx.pin(id);
        }
        if let Ok(id) = ctx.gfx.load_texture(Path::new("assets/props.png")) {
            self.prop_tex = ctx.gfx.pin(id);
        }
        if let Err(err) = self.load_weapons(ctx) {
            self.status = err;
        }
        *ctx.camera = Camera {
            eye: Vec3::new(MAP_W as f32 * 0.5, MAP_H as f32 * 0.5, 10.0),
            target: Vec3::new(MAP_W as f32 * 0.5, MAP_H as f32 * 0.5, 0.0),
            up: Vec3::Y,
            projection: Projection::Orthographic {
                height: MAP_H as f32,
                near: 0.1,
                far: 100.0,
            },
        };
    }

    fn update(&mut self, ctx: &mut Context) {
        if let Some(boot) = self.boot.take() {
            match boot {
                BootAction::Host(port) => self.try_host(ctx, port),
                BootAction::Connect(url) => self.try_connect(ctx, &url),
                BootAction::None => {}
            }
        }
        self.handle_net(ctx);
        match self.screen {
            Screen::Menu => self.update_menu(ctx),
            Screen::Lobby => self.update_lobby(ctx),
            Screen::Match => self.update_match(ctx),
        }
    }
}

fn clock_text(timer: f32) -> String {
    let secs = timer.max(0.0).ceil() as u32;
    format!("{:02}:{:02}", secs / 60, secs % 60)
}

fn to_ws_url(addr: &str) -> String {
    let addr = addr.trim();
    if addr.starts_with("ws://") || addr.starts_with("wss://") {
        addr.to_string()
    } else {
        format!("ws://{addr}")
    }
}

fn ask_connect_url() -> Option<String> {
    #[cfg(target_arch = "wasm32")]
    {
        let default = format!("127.0.0.1:{DEFAULT_PORT}");
        let window = web_sys::window()?;
        let entered = window
            .prompt_with_message_and_default(
                "Host (ip:port). On https pages use 127.0.0.1 or a wss:// URL.",
                &default,
            )
            .ok()??;
        let entered = entered.trim().to_string();
        if entered.is_empty() {
            None
        } else {
            Some(to_ws_url(&entered))
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        Some(to_ws_url(&format!("127.0.0.1:{DEFAULT_PORT}")))
    }
}

fn parse_args(args: &[String]) -> BootAction {
    match args.get(1).map(String::as_str) {
        Some("host") => {
            let port = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(DEFAULT_PORT);
            BootAction::Host(port)
        }
        Some("connect") => {
            let addr = args
                .get(2)
                .cloned()
                .unwrap_or_else(|| format!("127.0.0.1:{DEFAULT_PORT}"));
            BootAction::Connect(to_ws_url(&addr))
        }
        _ => BootAction::None,
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if let Err(err) = run(
        Config {
            title: "worms".into(),
            width: 960,
            height: 540,
            max_fps: Some(60),
        },
        Worms::new(parse_args(&args)),
    ) {
        eprintln!("{err}");
        std::process::exit(1);
    }
}
