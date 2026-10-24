//! Many top-down animated zombies chase the cursor — stress-tests lots of sprites.

#![cfg_attr(windows, windows_subsystem = "windows")]

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use beton::{
    run, Animation, Camera, Config, Context, Game, GameObject, GameObjectId, Projection, Rng,
    Sprite, SpriteSheet, TextureHandle, Vec3, World,
};

const WORLD_HEIGHT: f32 = 24.0;
/// Continuous spawn rate: 50 000 new zombies per minute (~833/s).
const SPAWN_PER_MINUTE: f32 = 200.0;
const ZOMBIE_SIZE: f32 = 1.15;
const WALK_FPS: f32 = 10.0;
const SPEED_MIN: f32 = 1.5;
const SPEED_MAX: f32 = 7.5;
/// Walk frames across the sheet; each row is a different skin.
const WALK_FRAMES: u32 = 8;
const VARIANT_COUNT: u32 = 5;
/// Art faces +X (right); rotation aligns that nose with movement.
const ART_FORWARD: f32 = 0.0;

/// Soft personal space — about one body width.
const SEPARATE_RADIUS: f32 = 1.15;
const SEPARATE_RADIUS_SQ: f32 = SEPARATE_RADIUS * SEPARATE_RADIUS;
/// Separation vs chase: pack fans out but still hunts the cursor.
const CHASE_WEIGHT: f32 = 1.0;
const SEPARATE_WEIGHT: f32 = 2.4;

struct Zombie {
    id: GameObjectId,
    anim: Animation,
    /// Row in the sheet (`0..VARIANT_COUNT`); idle uses this row’s first frame.
    variant: u32,
    speed: f32,
}

struct Horde {
    white: Option<TextureHandle>,
    sheet_tex: Option<TextureHandle>,
    sheet: Option<SpriteSheet>,
    /// One walk clip per skin row.
    templates: Vec<Animation>,
    zombies: Vec<Zombie>,
    cursor_world: Vec3,
    spawn_debt: f32,
    rng: Rng,
}

impl Horde {
    fn new() -> Self {
        Self {
            white: None,
            sheet_tex: None,
            sheet: None,
            templates: Vec::new(),
            zombies: Vec::new(),
            cursor_world: Vec3::ZERO,
            spawn_debt: 0.0,
            rng: Rng::new(42),
        }
    }

    fn asset(name: &str) -> PathBuf {
        assets_dir(&std::env::current_exe().expect("current executable")).join(name)
    }

    fn spawn_one(&mut self, world: &mut World, aspect: f32) {
        let sheet = self.sheet.as_ref().expect("sheet");
        let variant = (self.rng.range(0.0, VARIANT_COUNT as f32) as u32).min(VARIANT_COUNT - 1);
        let template = &self.templates[variant as usize];
        let mut anim = template.instance();
        anim.tick(self.rng.range(0.0, WALK_FRAMES as f32 / WALK_FPS));

        let mut sprite = Sprite::new(sheet.texture, Vec3::ZERO, [ZOMBIE_SIZE, ZOMBIE_SIZE]);
        anim.apply(&mut sprite);

        let half_h = WORLD_HEIGHT * 0.5;
        let half_w = half_h * aspect;
        // Spawn just outside the visible ortho bounds so they stream in.
        let margin = 1.5;
        let pos = match (self.rng.range(0.0, 4.0) as u32) % 4 {
            0 => Vec3::new(self.rng.range(-half_w, half_w), half_h + margin, 0.0),
            1 => Vec3::new(self.rng.range(-half_w, half_w), -half_h - margin, 0.0),
            2 => Vec3::new(half_w + margin, self.rng.range(-half_h, half_h), 0.0),
            _ => Vec3::new(-half_w - margin, self.rng.range(-half_h, half_h), 0.0),
        };

        let id = world.spawn(GameObject {
            pos,
            rotation: 0.0,
            scale: [1.0, 1.0],
            parent: None,
            size: [ZOMBIE_SIZE * 0.55, ZOMBIE_SIZE * 0.55],
            sprite: Some(sprite),
            tag: 1,
        });

        self.zombies.push(Zombie {
            id,
            anim,
            variant,
            speed: self.rng.range(SPEED_MIN, SPEED_MAX),
        });
    }
}

impl Game for Horde {
    fn boot(&mut self, ctx: &mut Context) {
        let white = ctx
            .gfx
            .create_texture(1, 1, &[255, 255, 255, 255])
            .expect("white");
        let tex = ctx
            .gfx
            .load_texture(&Self::asset("zombie-walk.png"))
            .expect("zombie-walk");

        let sheet = SpriteSheet {
            texture: tex,
            columns: WALK_FRAMES,
            rows: VARIANT_COUNT,
        };
        self.templates = (0..VARIANT_COUNT)
            .map(|variant| {
                let base = variant * WALK_FRAMES;
                let frames: Vec<u32> = (base..base + WALK_FRAMES).collect();
                Animation::new(sheet.clone(), frames, WALK_FPS)
            })
            .collect();
        self.white = Some(ctx.gfx.pin(white).expect("pin white"));
        self.sheet_tex = Some(ctx.gfx.pin(tex).expect("pin sheet"));
        self.sheet = Some(sheet);

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
        let white = self.white.as_ref().expect("boot").id();
        let aspect = (ctx.fb_size[0] / ctx.fb_size[1].max(1.0)).max(0.1);

        // Top-down arena: flat ground fill.
        let mut ground = Sprite::new(white, Vec3::new(0.0, 0.0, -1.0), [60.0, WORLD_HEIGHT]);
        ground.color = [0.16, 0.18, 0.14, 1.0];
        ctx.scene.push(ground);

        if ctx.input.cursor_inside() {
            self.cursor_world =
                ctx.camera
                    .screen_to_world(ctx.input.cursor_px(), ctx.fb_size[0], ctx.fb_size[1]);
        }

        let mut mark = Sprite::new(white, self.cursor_world, [0.4, 0.4]);
        mark.color = [0.95, 0.35, 0.25, 1.0];
        mark.center.z = 0.5;
        ctx.scene.push(mark);

        // Accrue continuous spawns (50k / minute).
        self.spawn_debt += SPAWN_PER_MINUTE / 60.0 * ctx.dt;
        let to_spawn = self.spawn_debt.floor() as u32;
        if to_spawn > 0 {
            self.spawn_debt -= to_spawn as f32;
            for _ in 0..to_spawn {
                self.spawn_one(ctx.world, aspect);
            }
        }

        let target = Vec3::new(self.cursor_world.x, self.cursor_world.y, 0.0);
        let dt = ctx.dt;
        let sheet = self.sheet.clone().expect("sheet");
        let n = self.zombies.len();
        if n == 0 {
            return;
        }

        // Snapshot positions, then bucket into a grid for neighbor queries.
        let mut positions = Vec::with_capacity(n);
        for zombie in &self.zombies {
            let pos = ctx
                .world
                .get(zombie.id)
                .map(|o| (o.pos.x, o.pos.y))
                .unwrap_or((0.0, 0.0));
            positions.push(pos);
        }

        let mut grid: HashMap<(i32, i32), Vec<usize>> = HashMap::with_capacity(n);
        for (i, &(x, y)) in positions.iter().enumerate() {
            let key = cell_key(x, y);
            grid.entry(key).or_default().push(i);
        }

        for i in 0..n {
            let Some(object) = ctx.world.get_mut(self.zombies[i].id) else {
                continue;
            };
            let (px, py) = positions[i];

            let to_x = target.x - px;
            let to_y = target.y - py;
            let chase_dist = (to_x * to_x + to_y * to_y).sqrt();
            let (mut steer_x, mut steer_y) = if chase_dist > 1e-4 {
                (to_x / chase_dist * CHASE_WEIGHT, to_y / chase_dist * CHASE_WEIGHT)
            } else {
                (0.0, 0.0)
            };

            // Soft repulsion from neighbors in this cell and the 8 around it.
            let (cx, cy) = cell_key(px, py);
            let mut sep_x = 0.0;
            let mut sep_y = 0.0;
            for oy in -1..=1 {
                for ox in -1..=1 {
                    let Some(bucket) = grid.get(&(cx + ox, cy + oy)) else {
                        continue;
                    };
                    for &j in bucket {
                        if j == i {
                            continue;
                        }
                        let (ox_pos, oy_pos) = positions[j];
                        let dx = px - ox_pos;
                        let dy = py - oy_pos;
                        let d2 = dx * dx + dy * dy;
                        if d2 >= SEPARATE_RADIUS_SQ || d2 < 1e-8 {
                            continue;
                        }
                        let d = d2.sqrt();
                        // Quadratic falloff: gentle at the edge, strong when overlapping.
                        let t = 1.0 - d / SEPARATE_RADIUS;
                        let strength = t * t;
                        sep_x += (dx / d) * strength;
                        sep_y += (dy / d) * strength;
                    }
                }
            }
            steer_x += sep_x * SEPARATE_WEIGHT;
            steer_y += sep_y * SEPARATE_WEIGHT;

            let steer_len = (steer_x * steer_x + steer_y * steer_y).sqrt();
            let moving = steer_len > 1e-4 && (chase_dist > 0.08 || sep_x * sep_x + sep_y * sep_y > 1e-6);
            if moving {
                let dir_x = steer_x / steer_len;
                let dir_y = steer_y / steer_len;
                let step = self.zombies[i].speed * dt;
                object.pos.x += dir_x * step;
                object.pos.y += dir_y * step;
                object.rotation = dir_y.atan2(dir_x) - ART_FORWARD;

                if let Some(sprite) = object.sprite.as_mut() {
                    self.zombies[i].anim.tick(dt);
                    self.zombies[i].anim.apply(sprite);
                    sprite.rotation = 0.0;
                }
            } else if let Some(sprite) = object.sprite.as_mut() {
                let idle = self.zombies[i].variant * WALK_FRAMES;
                sheet.apply(sprite, idle);
                sprite.rotation = 0.0;
            }
        }
    }
}

fn cell_key(x: f32, y: f32) -> (i32, i32) {
    (
        (x / SEPARATE_RADIUS).floor() as i32,
        (y / SEPARATE_RADIUS).floor() as i32,
    )
}

fn assets_dir(exe: &Path) -> PathBuf {
    exe.parent()
        .expect("executable path has a directory")
        .join("assets")
}

fn main() {
    if let Err(err) = run(
        Config {
            title: "zombies (top-down)".into(),
            width: 960,
            height: 640,
            max_fps: Some(60),
        },
        Horde::new(),
    ) {
        eprintln!("{err}");
        std::process::exit(1);
    }
}
