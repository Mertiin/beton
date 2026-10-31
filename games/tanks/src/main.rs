//! Hotseat two-tank duel with Liero-style dirt and file-defined weapons.

#![cfg_attr(windows, windows_subsystem = "windows")]

mod particles;
mod sim;
mod terrain;
mod weapons;

use std::collections::HashMap;
use std::path::Path;

use particles::{emit_blast, emit_fragments, emit_trail};
use sim::{
    apply_blast, barrel_sprite_pose, muzzle_point, settle_tanks, step_shell, MatchState, Shell,
    TurnPhase, AIM_SPEED, CHARGE_TIME, FUEL_BUDGET, MAX_HP, TANK_H, TANK_W,
};

/// `panzer3.png` is 640×304. Two body rows of 4×160×72, then a 160×160 gun.
/// The gun is drawn behind the hull so the breech stays inside the turret.
const SHEET_COLS: u32 = 4;
const CELL_W: f32 = 160.0;
const CELL_H: f32 = 72.0;
const SHEET_W: f32 = 640.0;
const SHEET_H: f32 = 304.0;
const GUN_BOX: f32 = 160.0;
const BODY_W: f32 = 80.0;
const BODY_H: f32 = BODY_W * CELL_H / CELL_W;
const GUN_SIZE: f32 = BODY_W * GUN_BOX / CELL_W;
/// Breech, inside the mantlet, as a fraction of a right-facing cell.
const TURRET_U: f32 = 114.0 / 160.0;
const TURRET_V: f32 = 24.0 / 72.0;
/// World units of travel per tread frame.
const TREAD_STEP: f32 = 4.0;

/// Sit the treads on the same ground line as the collision box.
fn body_center(pos: Vec3) -> Vec3 {
    Vec3::new(pos.x, pos.y + (BODY_H - TANK_H) * 0.5, pos.z)
}

/// Breech position on the turret. `side == 1` faces left.
fn gun_pivot(pos: Vec3, side: usize) -> Vec3 {
    let c = body_center(pos);
    let u = if side == 1 { 1.0 - TURRET_U } else { TURRET_U };
    Vec3::new(
        c.x + (u - 0.5) * BODY_W,
        c.y + (0.5 - TURRET_V) * BODY_H,
        0.05,
    )
}

fn body_uv(col: u32, row: u32, flip: bool) -> [f32; 4] {
    let u0 = col as f32 * CELL_W / SHEET_W;
    let u1 = (col as f32 + 1.0) * CELL_W / SHEET_W;
    let v0 = row as f32 * CELL_H / SHEET_H;
    let v1 = (row as f32 + 1.0) * CELL_H / SHEET_H;
    if flip {
        [u1, v0, u0, v1]
    } else {
        [u0, v0, u1, v1]
    }
}

fn gun_uv() -> [f32; 4] {
    let top = CELL_H * 2.0;
    [0.0, top / SHEET_H, GUN_BOX / SHEET_W, (top + GUN_BOX) / SHEET_H]
}

fn ride_bob(travel: f32, moving: bool) -> f32 {
    if moving {
        (travel * 0.9).sin() * 0.3
    } else {
        0.0
    }
}
use terrain::{Terrain, MAP_H, MAP_W};
use weapons::Weapon;

use beton::{
    run, step_particles, style, Camera, Config, Context, Font, Game, GameObject, GameObjectId,
    Key, Projection, Rng, ShaderDesc, ShaderId, Sprite, TextureHandle, TextureId, UiId, Vec3,
    VertexShader,
};

const ASSET_DIRS: &[&str] = &["assets/weapons", "assets/shaders"];

const UI_MENU_ROOT: UiId = UiId(1);
const UI_TITLE: UiId = UiId(2);
const UI_START_BTN: UiId = UiId(3);
const UI_START_LABEL: UiId = UiId(4);
const UI_HUD: UiId = UiId(10);
const UI_TURN: UiId = UiId(11);
const UI_WEAPON: UiId = UiId(12);
const UI_HINT: UiId = UiId(13);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Screen {
    Menu,
    Match,
}

struct Tanks {
    screen: Screen,
    white: Option<TextureHandle>,
    font_tex: Option<TextureHandle>,
    font: Option<Font>,
    sheet: Option<TextureHandle>,
    dirt_tex: Option<TextureHandle>,
    dirt_id: Option<TextureId>,
    terrain_id: Option<GameObjectId>,
    weapons: Vec<Weapon>,
    shader_by_effect: HashMap<String, ShaderId>,
    match_state: Option<MatchState>,
    barrel_ids: [Option<GameObjectId>; 2],
    /// Distance driven, used to pick a tread frame.
    drive: [f32; 2],
    prev_x: [f32; 2],
    seed: u64,
    rng: Rng,
    space_was_down: bool,
    /// Ignore Space until it has been released once after entering Match.
    arm_space: bool,
}

impl Tanks {
    fn new() -> Self {
        Self {
            screen: Screen::Menu,
            white: None,
            font_tex: None,
            font: None,
            sheet: None,
            dirt_tex: None,
            dirt_id: None,
            terrain_id: None,
            weapons: Vec::new(),
            shader_by_effect: HashMap::new(),
            match_state: None,
            barrel_ids: [None, None],
            drive: [0.0, 0.0],
            prev_x: [0.0, 0.0],
            seed: 0xA11CE,
            rng: Rng::new(0xA11CE),
            space_was_down: false,
            arm_space: false,
        }
    }

    fn load_weapons(&mut self, ctx: &mut Context) -> Result<(), String> {
        self.weapons.clear();
        self.shader_by_effect.clear();
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
                .ok_or_else(|| format!("bad weapon path {}", path.display()))?
                .to_string();
            let bytes = ctx.gfx.read_bytes(&path)?;
            let text = String::from_utf8(bytes)
                .map_err(|err| format!("{stem}: utf8 {err}"))?;
            let mut weapon = Weapon::parse(&stem, &text)?;
            let shader = self.ensure_shader(ctx, &weapon)?;
            weapon.shader = shader;
            let id = ctx.world.spawn(GameObject {
                pos: Vec3::ZERO,
                rotation: 0.0,
                scale: [1.0, 1.0],
                parent: None,
                size: [1.0, 1.0],
                sprite: None,
                tag: 2,
            });
            ctx.world.insert(id, weapon.clone());
            self.weapons.push(weapon);
        }
        Ok(())
    }

    fn ensure_shader(&mut self, ctx: &mut Context, weapon: &Weapon) -> Result<ShaderId, String> {
        if let Some(&id) = self.shader_by_effect.get(&weapon.effect) {
            return Ok(id);
        }
        let path = Path::new("assets/shaders").join(format!("{}.wgsl", weapon.effect));
        let bytes = ctx
            .gfx
            .read_bytes(&path)
            .map_err(|err| format!("{}: effect `{}`: {err}", weapon.name, weapon.effect))?;
        let source = String::from_utf8(bytes)
            .map_err(|err| format!("{}: effect utf8 {err}", weapon.name))?;
        let id = ctx
            .gfx
            .create_shader(ShaderDesc {
                wgsl: &source,
                blend: weapon.blend,
                vertex: VertexShader::Engine,
                uniform_bytes: 16,
            })
            .map_err(|err| format!("{}: {err}", weapon.name))?;
        self.shader_by_effect.insert(weapon.effect.clone(), id);
        Ok(id)
    }

    fn write_shader_time(&mut self, ctx: &mut Context) {
        let seconds = ctx.time;
        let bytes = [
            seconds.to_le_bytes(),
            [0u8; 4],
            [0u8; 4],
            [0u8; 4],
        ]
        .concat();
        for &id in self.shader_by_effect.values() {
            let _ = ctx.gfx.write_uniform(id, &bytes);
        }
    }

    fn begin_match(&mut self, ctx: &mut Context) {
        ctx.world.clear();
        ctx.particles.clear();
        self.barrel_ids = [None, None];
        self.drive = [0.0, 0.0];
        self.seed = self.seed.wrapping_add(0x9E37_79B9);
        self.rng = Rng::new(self.seed);

        let terrain = Terrain::generate(self.seed);
        let dirt_id = if let Some(id) = self.dirt_id {
            ctx.gfx
                .update_texture(id, &terrain.rgba)
                .expect("update dirt");
            id
        } else {
            let id = ctx
                .gfx
                .create_texture(MAP_W, MAP_H, &terrain.rgba)
                .expect("dirt texture");
            self.dirt_tex = Some(ctx.gfx.pin(id).expect("pin dirt"));
            self.dirt_id = Some(id);
            id
        };

        let white = self.white.as_ref().expect("boot").id();
        let mut sky = Sprite::new(
            white,
            Vec3::new(MAP_W as f32 * 0.5, MAP_H as f32 * 0.5, -1.0),
            [MAP_W as f32, MAP_H as f32],
        );
        sky.color = [0.45, 0.68, 0.90, 1.0];
        ctx.world.spawn(GameObject {
            pos: Vec3::new(MAP_W as f32 * 0.5, MAP_H as f32 * 0.5, -1.0),
            rotation: 0.0,
            scale: [1.0, 1.0],
            parent: None,
            size: [MAP_W as f32, MAP_H as f32],
            sprite: Some(sky),
            tag: 0,
        });

        let mut dirt_sprite = Sprite::new(
            dirt_id,
            Vec3::new(MAP_W as f32 * 0.5, MAP_H as f32 * 0.5, 0.0),
            [MAP_W as f32, MAP_H as f32],
        );
        dirt_sprite.color = [1.0, 1.0, 1.0, 1.0];
        let terrain_id = ctx.world.spawn(GameObject {
            pos: Vec3::new(MAP_W as f32 * 0.5, MAP_H as f32 * 0.5, 0.0),
            rotation: 0.0,
            scale: [1.0, 1.0],
            parent: None,
            size: [MAP_W as f32, MAP_H as f32],
            sprite: Some(dirt_sprite),
            tag: 1,
        });
        ctx.world.insert(terrain_id, terrain);
        self.terrain_id = Some(terrain_id);

        if let Err(err) = self.load_weapons(ctx) {
            crate::warn_boot(&err);
            return;
        }

        let sheet = self.sheet.as_ref().expect("sheet").id();
        let mut tank_ids = [None, None];
        for side in 0..2 {
            let (x, y) = MatchState::tank_spawn_y(
                ctx.world
                    .get_component::<Terrain>(terrain_id)
                    .expect("terrain"),
                side,
            );
            let origin = Vec3::new(x, y, 0.1);
            self.prev_x[side] = x;
            let mut body = Sprite::new(sheet, body_center(origin), [BODY_W, BODY_H]);
            body.uv = body_uv(0, side as u32, side == 1);
            let id = ctx.world.spawn(GameObject {
                pos: origin,
                rotation: 0.0,
                scale: [1.0, 1.0],
                parent: None,
                size: [TANK_W, TANK_H],
                sprite: Some(body),
                tag: 10 + side as u32,
            });
            let aim = if side == 0 {
                0.55
            } else {
                std::f32::consts::PI - 0.55
            };
            let (bpos, brot) = barrel_sprite_pose(gun_pivot(origin, side), aim);
            let mut barrel = Sprite::new(sheet, bpos, [GUN_SIZE, GUN_SIZE]);
            barrel.uv = gun_uv();
            barrel.rotation = brot;
            let bid = ctx.world.spawn(GameObject {
                pos: bpos,
                rotation: brot,
                scale: [1.0, 1.0],
                parent: None,
                size: [TANK_W * 0.7, 3.0],
                sprite: Some(barrel),
                tag: 20 + side as u32,
            });
            tank_ids[side] = Some(id);
            self.barrel_ids[side] = Some(bid);
        }

        let mut state = MatchState::spawn_tanks(
            ctx.world
                .get_component::<Terrain>(terrain_id)
                .expect("terrain"),
            tank_ids[0].unwrap(),
            tank_ids[1].unwrap(),
        );
        state.begin_turn();
        self.match_state = Some(state);
        self.screen = Screen::Match;
        self.space_was_down = false;
        self.arm_space = !ctx.input.down(Key::Space);
    }

    fn sync_dirt(&mut self, ctx: &mut Context) {
        let Some(tid) = self.terrain_id else {
            return;
        };
        let Some(dirt_id) = self.dirt_id else {
            return;
        };
        let Some(terrain) = ctx.world.get_component_mut::<Terrain>(tid) else {
            return;
        };
        if !terrain.dirty {
            return;
        }
        let rgba = terrain.rgba.clone();
        terrain.dirty = false;
        let _ = ctx.gfx.update_texture(dirt_id, &rgba);
    }

    fn update_menu(&mut self, ctx: &mut Context) {
        let font = self.font.clone().expect("boot");
        let start_hot = ctx.ui.hovered(UI_START_BTN) || ctx.ui.hovered(UI_START_LABEL);
        let [fb_w, fb_h] = ctx.fb_size;
        ctx.ui.node(
            UI_MENU_ROOT,
            style!(
                position: absolute,
                width: (fb_w) pt,
                height: (fb_h) pt,
                top: 0 pt,
                left: 0 pt,
                flex_direction: column,
                justify_content: center,
                align_items: center,
                gap: 24 pt,
                background: Some([0.08, 0.1, 0.14, 1.0]),
            ),
            |ui| {
                ui.text(
                    UI_TITLE,
                    &font,
                    [28.0, 42.0],
                    style!(color: [0.95, 0.9, 0.75, 1.0]),
                    "TANKS",
                );
                ui.node(
                    UI_START_BTN,
                    style!(
                        width: 220 pt,
                        height: 56 pt,
                        justify_content: center,
                        align_items: center,
                        background: Some(if start_hot {
                            [0.35, 0.7, 0.4, 1.0]
                        } else {
                            [0.22, 0.5, 0.28, 1.0]
                        }),
                    ),
                    |ui| {
                        ui.text(
                            UI_START_LABEL,
                            &font,
                            [18.0, 26.0],
                            style!(color: [1.0, 1.0, 1.0, 1.0]),
                            "START",
                        );
                    },
                );
            },
        );
        if ctx.ui.clicked(UI_START_BTN)
            || ctx.ui.clicked(UI_START_LABEL)
            || ctx.input.pressed(Key::Space)
        {
            self.begin_match(ctx);
        }
    }

    fn update_match(&mut self, ctx: &mut Context) {
        self.write_shader_time(ctx);
        let white = self.white.as_ref().expect("boot").id();
        let terrain_id = self.terrain_id.expect("match");

        if ctx.input.pressed(Key::KeyR) {
            self.begin_match(ctx);
            return;
        }

        // Weapon select 1-9
        for (i, key) in [
            Key::Digit1,
            Key::Digit2,
            Key::Digit3,
            Key::Digit4,
            Key::Digit5,
            Key::Digit6,
            Key::Digit7,
            Key::Digit8,
            Key::Digit9,
        ]
        .into_iter()
        .enumerate()
        {
            if ctx.input.pressed(key) && i < self.weapons.len() {
                if let Some(state) = self.match_state.as_mut() {
                    if state.phase == TurnPhase::Aim {
                        state.weapon_index = i;
                    }
                }
            }
        }

        let mut hit_event: Option<(Vec3, usize)> = None;
        let mut fire_event: Option<(Vec3, Shell)> = None;
        let mut end_settle = false;
        let mut next_turn = false;
        let mut winner: Option<usize> = None;

        {
            let state = self.match_state.as_mut().expect("match");
            let mut positions = [
                ctx.world
                    .get(state.tanks[0].id)
                    .map(|o| o.pos)
                    .unwrap_or(Vec3::ZERO),
                ctx.world
                    .get(state.tanks[1].id)
                    .map(|o| o.pos)
                    .unwrap_or(Vec3::ZERO),
            ];

            match state.phase {
                TurnPhase::Aim => {
                    let mut dir = 0.0;
                    if ctx.input.down(Key::ArrowLeft) || ctx.input.down(Key::KeyA) {
                        dir -= 1.0;
                    }
                    if ctx.input.down(Key::ArrowRight) || ctx.input.down(Key::KeyD) {
                        dir += 1.0;
                    }
                    if dir != 0.0 {
                        let terrain = ctx
                            .world
                            .get_component::<Terrain>(terrain_id)
                            .expect("terrain")
                            .clone();
                        state.try_move(&terrain, &mut positions, dir, ctx.dt);
                    }
                    let mut aim_delta = 0.0;
                    if ctx.input.down(Key::ArrowUp) || ctx.input.down(Key::KeyW) {
                        aim_delta += AIM_SPEED * ctx.dt;
                    }
                    if ctx.input.down(Key::ArrowDown) || ctx.input.down(Key::KeyS) {
                        aim_delta -= AIM_SPEED * ctx.dt;
                    }
                    state.aim(aim_delta);

                    if !self.arm_space {
                        if !ctx.input.down(Key::Space) {
                            self.arm_space = true;
                        }
                    } else if ctx.input.down(Key::Space) {
                        state.charging = true;
                        state.charge = (state.charge + ctx.dt).min(CHARGE_TIME);
                        self.space_was_down = true;
                    } else if self.space_was_down {
                        // Fire only after a press during Aim (release or lost key-up).
                        self.space_was_down = false;
                        let muzzle = muzzle_point(
                            gun_pivot(positions[state.active], state.active),
                            state.tanks[state.active].aim,
                        );
                        if let Some(event) = state.fire(&self.weapons, muzzle) {
                            fire_event = Some(event);
                        }
                    }
                }
                TurnPhase::Flying => {
                    if let Some(shell_id) = state.shell_id {
                        let terrain = ctx
                            .world
                            .get_component::<Terrain>(terrain_id)
                            .expect("terrain")
                            .clone();
                        let mut pos = ctx
                            .world
                            .get(shell_id)
                            .map(|o| o.pos)
                            .unwrap_or(Vec3::ZERO);
                        if let Some(mut shell) = ctx.world.remove_component::<Shell>(shell_id) {
                            let weapon = self.weapons[shell.weapon_index].clone();
                            shell.trail_acc += ctx.dt;
                            let trail_period = 1.0 / weapon.trail.max(1.0);
                            let mut trails = Vec::new();
                            while shell.trail_acc >= trail_period {
                                shell.trail_acc -= trail_period;
                                trails.push(pos);
                            }
                            let hit = step_shell(&mut pos, &mut shell, &weapon, &terrain, ctx.dt);
                            let weapon_index = shell.weapon_index;
                            let shader = weapon.shader;
                            ctx.world.insert(shell_id, shell);
                            if let Some(object) = ctx.world.get_mut(shell_id) {
                                object.pos = pos;
                                if let Some(sprite) = object.sprite.as_mut() {
                                    sprite.center = pos;
                                }
                            }
                            for p in trails {
                                emit_trail(
                                    ctx.particles,
                                    white,
                                    shader,
                                    p,
                                    [1.0, 0.85, 0.4, 0.7],
                                );
                            }
                            if let Some(hit) = hit {
                                hit_event = Some((hit, weapon_index));
                            }
                        } else {
                            // Orphan shell id — don't soft-lock the match.
                            state.shell_id = None;
                            state.phase = TurnPhase::Settle;
                        }
                    } else {
                        // Fired but shell never spawned — recover.
                        state.phase = TurnPhase::Settle;
                    }
                }
                TurnPhase::Settle => {
                    let terrain = ctx
                        .world
                        .get_component::<Terrain>(terrain_id)
                        .expect("terrain")
                        .clone();
                    if settle_tanks(&terrain, &mut positions, ctx.dt) {
                        end_settle = true;
                    }
                    for i in 0..2 {
                        if positions[i].y < -TANK_H {
                            winner = Some(1 - i);
                        }
                        if state.tanks[i].hp <= 0.0 {
                            winner = Some(1 - i);
                        }
                    }
                }
                TurnPhase::Over { .. } => {}
            }

            // Write tank / barrel poses.
            for i in 0..2 {
                let dx = (positions[i].x - self.prev_x[i]).abs();
                self.prev_x[i] = positions[i].x;
                let moving = dx > 0.02;
                if moving {
                    self.drive[i] += dx;
                }
                let frame = ((self.drive[i] / TREAD_STEP) as u32) % SHEET_COLS;
                let bob = ride_bob(self.drive[i], moving);
                let center = body_center(positions[i]);
                if let Some(obj) = ctx.world.get_mut(state.tanks[i].id) {
                    obj.pos = positions[i];
                    if let Some(sprite) = obj.sprite.as_mut() {
                        sprite.center = Vec3::new(center.x, center.y + bob, center.z);
                        sprite.uv = body_uv(frame, i as u32, i == 1);
                    }
                }
                let mut pivot = gun_pivot(positions[i], i);
                pivot.y += bob;
                let (bpos, brot) = barrel_sprite_pose(pivot, state.tanks[i].aim);
                if let Some(bid) = self.barrel_ids[i] {
                    if let Some(obj) = ctx.world.get_mut(bid) {
                        obj.pos = bpos;
                        obj.rotation = brot;
                        if let Some(sprite) = obj.sprite.as_mut() {
                            sprite.center = bpos;
                            sprite.rotation = brot;
                        }
                    }
                }
            }

            if end_settle {
                if let Some(w) = winner {
                    state.phase = TurnPhase::Over { winner: w };
                } else {
                    next_turn = true;
                }
            }
        }

        if let Some((muzzle, shell)) = fire_event {
            let white = self.white.as_ref().expect("boot").id();
            let mut sprite = Sprite::new(white, muzzle, [5.0, 5.0]);
            sprite.color = [1.0, 0.95, 0.5, 1.0];
            sprite.center.z = 0.3;
            let id = ctx.world.spawn(GameObject {
                pos: muzzle,
                rotation: 0.0,
                scale: [1.0, 1.0],
                parent: None,
                size: [5.0, 5.0],
                sprite: Some(sprite),
                tag: 30,
            });
            ctx.world.insert(id, shell);
            if let Some(state) = self.match_state.as_mut() {
                state.shell_id = Some(id);
            }
        }

        if let Some((hit, weapon_index)) = hit_event {
            let weapon = self.weapons[weapon_index].clone();
            if let Some(shell_id) = self.match_state.as_ref().and_then(|s| s.shell_id) {
                ctx.world.despawn(shell_id);
            }
            if let Some(state) = self.match_state.as_mut() {
                state.shell_id = None;
            }
            {
                let state = self.match_state.as_mut().expect("match");
                let mut positions = [
                    ctx.world
                        .get(state.tanks[0].id)
                        .map(|o| o.pos)
                        .unwrap_or(Vec3::ZERO),
                    ctx.world
                        .get(state.tanks[1].id)
                        .map(|o| o.pos)
                        .unwrap_or(Vec3::ZERO),
                ];
                let tank_ids = [state.tanks[0].id, state.tanks[1].id];
                {
                    let terrain = ctx
                        .world
                        .get_component_mut::<Terrain>(terrain_id)
                        .expect("terrain");
                    apply_blast(terrain, &mut positions, &mut state.tanks, hit, &weapon);
                }
                for i in 0..2 {
                    if let Some(obj) = ctx.world.get_mut(tank_ids[i]) {
                        obj.pos = positions[i];
                    }
                }
                state.phase = TurnPhase::Settle;
            }
            emit_blast(ctx.particles, white, &weapon, hit);
            emit_fragments(ctx.particles, white, &weapon, hit, &mut self.rng);
        }

        if next_turn {
            if let Some(state) = self.match_state.as_mut() {
                state.next_player();
            }
            self.space_was_down = false;
            self.arm_space = !ctx.input.down(Key::Space);
        }

        {
            let terrain = ctx
                .world
                .get_component::<Terrain>(terrain_id)
                .expect("terrain")
                .clone();
            step_particles(ctx.particles, ctx.dt, |pos| {
                let (c, r) = Terrain::world_to_cell(pos.x, pos.y);
                terrain.solid_at(c, r)
            });
        }

        self.sync_dirt(ctx);
        self.draw_hud(ctx);
        self.draw_bars(ctx);
    }

    fn draw_hud(&self, ctx: &mut Context) {
        let font = self.font.clone().expect("boot");
        let state = self.match_state.as_ref().expect("match");
        let weapon_name = self
            .weapons
            .get(state.weapon_index)
            .map(|w| w.name.as_str())
            .unwrap_or("?");
        let turn = match state.phase {
            TurnPhase::Over { winner } => format!("P{} wins — R", winner + 1),
            _ => format!("P{} turn", state.active + 1),
        };
        let hint = format!("weapon {weapon_name}  fuel {:.0}", state.tanks[state.active].fuel);
        let [fb_w, _] = ctx.fb_size;
        ctx.ui.node(
            UI_HUD,
            style!(
                position: absolute,
                width: (fb_w) pt,
                height: 72 pt,
                top: 0 pt,
                left: 0 pt,
                flex_direction: column,
                padding: 12 pt,
                gap: 4 pt,
                background: Some([0.0, 0.0, 0.0, 0.35]),
            ),
            |ui| {
                ui.text(
                    UI_TURN,
                    &font,
                    [14.0, 22.0],
                    style!(color: [1.0, 1.0, 1.0, 1.0]),
                    &turn,
                );
                ui.text(
                    UI_WEAPON,
                    &font,
                    [12.0, 18.0],
                    style!(color: [0.85, 0.9, 0.7, 1.0]),
                    &hint,
                );
                ui.text(
                    UI_HINT,
                    &font,
                    [10.0, 16.0],
                    style!(color: [0.7, 0.75, 0.8, 1.0]),
                    "Arrows move/aim  1-9 weapon  Space charge  R restart",
                );
            },
        );
    }

    fn draw_bars(&self, ctx: &mut Context) {
        let white = self.white.as_ref().expect("boot").id();
        let state = self.match_state.as_ref().expect("match");
        for i in 0..2 {
            let Some(obj) = ctx.world.get(state.tanks[i].id) else {
                continue;
            };
            let hp = (state.tanks[i].hp / MAX_HP).clamp(0.0, 1.0);
            let fuel = (state.tanks[i].fuel / FUEL_BUDGET).clamp(0.0, 1.0);
            let top = obj.pos.y + (BODY_H - TANK_H) * 0.5 + BODY_H * 0.5;
            let bar_w = BODY_W * 0.5;
            let base = Vec3::new(obj.pos.x, top + 2.2, 0.6);
            let mut bg = Sprite::new(white, base, [bar_w, 2.5]);
            bg.color = [0.1, 0.1, 0.1, 0.8];
            ctx.scene.push(bg);
            let mut hp_bar = Sprite::new(
                white,
                Vec3::new(base.x - bar_w * 0.5 * (1.0 - hp), base.y, 0.61),
                [bar_w * hp, 2.5],
            );
            hp_bar.color = [0.9, 0.25, 0.2, 1.0];
            ctx.scene.push(hp_bar);
            if state.active == i && state.phase == TurnPhase::Aim {
                let fuel_y = base.y - 3.5;
                let mut fbg = Sprite::new(white, Vec3::new(base.x, fuel_y, 0.6), [bar_w, 2.0]);
                fbg.color = [0.1, 0.1, 0.1, 0.8];
                ctx.scene.push(fbg);
                let mut fbar = Sprite::new(
                    white,
                    Vec3::new(base.x - bar_w * 0.5 * (1.0 - fuel), fuel_y, 0.61),
                    [bar_w * fuel, 2.0],
                );
                fbar.color = [0.3, 0.75, 1.0, 1.0];
                ctx.scene.push(fbar);
                if state.charging {
                    let c = (state.charge / CHARGE_TIME).clamp(0.0, 1.0);
                    let cy = fuel_y - 3.5;
                    let mut cbar = Sprite::new(
                        white,
                        Vec3::new(base.x - bar_w * 0.5 * (1.0 - c), cy, 0.62),
                        [bar_w * c, 2.0],
                    );
                    cbar.color = [1.0, 0.85, 0.2, 1.0];
                    ctx.scene.push(cbar);
                }
            }
        }
    }
}

impl Game for Tanks {
    fn assets(&self) -> &[&'static str] {
        &["assets/panzer3.png"]
    }

    fn asset_dirs(&self) -> &[&'static str] {
        ASSET_DIRS
    }

    fn boot(&mut self, ctx: &mut Context) {
        let white = ctx.gfx.white_texture();
        self.white = Some(ctx.gfx.pin(white).expect("pin white"));
        let sheet = ctx
            .gfx
            .load_texture(Path::new("assets/panzer3.png"))
            .expect("panzer3");
        self.sheet = Some(ctx.gfx.pin(sheet).expect("pin sheet"));

        let (atlas, aw, ah) = ascii_atlas_rgba();
        let font_tex = ctx
            .gfx
            .create_texture(aw, ah, &atlas)
            .expect("font atlas");
        self.font_tex = Some(ctx.gfx.pin(font_tex).expect("pin font"));
        self.font = Some(Font::grid(font_tex, 16, 6, ' '));

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

        // Warm-compile shaders / parse weapons once so Match boot is fast.
        let _ = self.load_weapons(ctx);
        ctx.world.clear();
        self.weapons.clear();
        self.screen = Screen::Menu;
    }

    fn update(&mut self, ctx: &mut Context) {
        match self.screen {
            Screen::Menu => self.update_menu(ctx),
            Screen::Match => self.update_match(ctx),
        }
    }
}

/// 16×6 cells of 8×12 for ASCII 32..127.
fn ascii_atlas_rgba() -> (Vec<u8>, u32, u32) {
    const CW: u32 = 8;
    const CH: u32 = 12;
    const COLS: u32 = 16;
    const ROWS: u32 = 6;
    let w = CW * COLS;
    let h = CH * ROWS;
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    for code in 32u8..128 {
        let idx = (code - 32) as u32;
        let col = idx % COLS;
        let row = idx / COLS;
        let ox = col * CW;
        let oy = row * CH;
        let glyph = glyph5x7(code);
        for gy in 0..7u32 {
            for gx in 0..5u32 {
                if (glyph[gy as usize] & (1 << (4 - gx))) == 0 {
                    continue;
                }
                for sy in 0..1u32 {
                    for sx in 0..1u32 {
                        let x = ox + 1 + gx + sx;
                        let y = oy + 2 + gy + sy;
                        let i = ((y * w + x) * 4) as usize;
                        rgba[i] = 255;
                        rgba[i + 1] = 255;
                        rgba[i + 2] = 255;
                        rgba[i + 3] = 255;
                    }
                }
            }
        }
    }
    (rgba, w, h)
}

fn glyph5x7(ch: u8) -> [u8; 7] {
    // Compact 5×7 bitmaps for a readable HUD / menu.
    match ch {
        b' ' => [0; 7],
        b'-' => [0, 0, 0, 0x0E, 0, 0, 0],
        b'0' => [0x0E, 0x11, 0x13, 0x15, 0x19, 0x11, 0x0E],
        b'1' => [0x04, 0x0C, 0x04, 0x04, 0x04, 0x04, 0x0E],
        b'2' => [0x0E, 0x11, 0x01, 0x06, 0x08, 0x10, 0x1F],
        b'3' => [0x0E, 0x11, 0x01, 0x06, 0x01, 0x11, 0x0E],
        b'4' => [0x02, 0x06, 0x0A, 0x12, 0x1F, 0x02, 0x02],
        b'5' => [0x1F, 0x10, 0x1E, 0x01, 0x01, 0x11, 0x0E],
        b'6' => [0x06, 0x08, 0x10, 0x1E, 0x11, 0x11, 0x0E],
        b'7' => [0x1F, 0x01, 0x02, 0x04, 0x08, 0x08, 0x08],
        b'8' => [0x0E, 0x11, 0x11, 0x0E, 0x11, 0x11, 0x0E],
        b'9' => [0x0E, 0x11, 0x11, 0x0F, 0x01, 0x02, 0x0C],
        b'A' | b'a' => [0x0E, 0x11, 0x11, 0x1F, 0x11, 0x11, 0x11],
        b'B' | b'b' => [0x1E, 0x11, 0x11, 0x1E, 0x11, 0x11, 0x1E],
        b'C' | b'c' => [0x0E, 0x11, 0x10, 0x10, 0x10, 0x11, 0x0E],
        b'D' | b'd' => [0x1E, 0x11, 0x11, 0x11, 0x11, 0x11, 0x1E],
        b'E' | b'e' => [0x1F, 0x10, 0x10, 0x1E, 0x10, 0x10, 0x1F],
        b'F' | b'f' => [0x1F, 0x10, 0x10, 0x1E, 0x10, 0x10, 0x10],
        b'G' | b'g' => [0x0E, 0x11, 0x10, 0x17, 0x11, 0x11, 0x0F],
        b'H' | b'h' => [0x11, 0x11, 0x11, 0x1F, 0x11, 0x11, 0x11],
        b'I' | b'i' => [0x0E, 0x04, 0x04, 0x04, 0x04, 0x04, 0x0E],
        b'J' | b'j' => [0x01, 0x01, 0x01, 0x01, 0x11, 0x11, 0x0E],
        b'K' | b'k' => [0x11, 0x12, 0x14, 0x18, 0x14, 0x12, 0x11],
        b'L' | b'l' => [0x10, 0x10, 0x10, 0x10, 0x10, 0x10, 0x1F],
        b'M' | b'm' => [0x11, 0x1B, 0x15, 0x15, 0x11, 0x11, 0x11],
        b'N' | b'n' => [0x11, 0x19, 0x15, 0x13, 0x11, 0x11, 0x11],
        b'O' | b'o' => [0x0E, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0E],
        b'P' | b'p' => [0x1E, 0x11, 0x11, 0x1E, 0x10, 0x10, 0x10],
        b'Q' | b'q' => [0x0E, 0x11, 0x11, 0x11, 0x15, 0x12, 0x0D],
        b'R' | b'r' => [0x1E, 0x11, 0x11, 0x1E, 0x14, 0x12, 0x11],
        b'S' | b's' => [0x0F, 0x10, 0x10, 0x0E, 0x01, 0x01, 0x1E],
        b'T' | b't' => [0x1F, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04],
        b'U' | b'u' => [0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0E],
        b'V' | b'v' => [0x11, 0x11, 0x11, 0x11, 0x11, 0x0A, 0x04],
        b'W' | b'w' => [0x11, 0x11, 0x11, 0x15, 0x15, 0x1B, 0x11],
        b'X' | b'x' => [0x11, 0x11, 0x0A, 0x04, 0x0A, 0x11, 0x11],
        b'Y' | b'y' => [0x11, 0x11, 0x0A, 0x04, 0x04, 0x04, 0x04],
        b'Z' | b'z' => [0x1F, 0x01, 0x02, 0x04, 0x08, 0x10, 0x1F],
        b'/' => [0x01, 0x02, 0x04, 0x04, 0x08, 0x10, 0x10],
        _ => [0x1F, 0x11, 0x11, 0x11, 0x11, 0x11, 0x1F],
    }
}

fn warn_boot(err: &str) {
    eprintln!("tanks: {err}");
}

fn main() {
    if let Err(err) = run(
        Config {
            title: "tanks".into(),
            width: 1440,
            height: 810,
            max_fps: Some(60),
        },
        Tanks::new(),
    ) {
        eprintln!("{err}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_game_is_menu() {
        let g = Tanks::new();
        assert_eq!(g.screen, Screen::Menu);
        assert!(g.match_state.is_none());
    }
}
