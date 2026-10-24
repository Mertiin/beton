//! Axis-aligned rigid-body sandbox on the beton API.

#![cfg_attr(windows, windows_subsystem = "windows")]

use beton::{
    run, step_rigid_bodies, Camera, Config, Context, Game, GameObject, GameObjectId, Key,
    Projection, RigidBody, Sprite, TextureHandle, Vec3, Velocity, World,
};

const WORLD_HEIGHT: f32 = 20.0;
const GRAVITY: Vec3 = Vec3::new(0.0, -32.0, 0.0);
const FLOOR_Y: f32 = -8.0;
const WALL_X: f32 = 8.0;
const BOX_SIZE: f32 = 1.2;
const KICK: f32 = 8.0;

struct Physics {
    white: Option<TextureHandle>,
    newest: Option<GameObjectId>,
    drop_n: u32,
}

impl Physics {
    fn new() -> Self {
        Self {
            white: None,
            newest: None,
            drop_n: 0,
        }
    }

    fn rebuild(&mut self, world: &mut World, white: beton::TextureId) {
        world.clear();
        self.newest = None;
        self.drop_n = 0;

        spawn_fixed(
            world,
            white,
            Vec3::new(0.0, FLOOR_Y, 0.0),
            [18.0, 1.0],
            [0.35, 0.38, 0.42, 1.0],
        );
        spawn_fixed(
            world,
            white,
            Vec3::new(-WALL_X, 0.0, 0.0),
            [1.0, 16.0],
            [0.35, 0.38, 0.42, 1.0],
        );
        spawn_fixed(
            world,
            white,
            Vec3::new(WALL_X, 0.0, 0.0),
            [1.0, 16.0],
            [0.35, 0.38, 0.42, 1.0],
        );

        for i in 0..3 {
            let x = -2.0 + i as f32 * 1.4;
            let y = FLOOR_Y + 0.5 + BOX_SIZE * 0.5 + i as f32 * (BOX_SIZE + 0.05);
            self.newest = Some(spawn_box(world, white, Vec3::new(x, y, 0.0), box_color(i)));
        }
    }

    fn drop_box(&mut self, world: &mut World, white: beton::TextureId) {
        let x = -1.5 + (self.drop_n % 5) as f32 * 0.7;
        self.drop_n += 1;
        let y = 8.0;
        self.newest = Some(spawn_box(
            world,
            white,
            Vec3::new(x, y, 0.0),
            box_color(self.drop_n),
        ));
    }

    fn kick_newest(&mut self, world: &mut World, dir: f32) {
        let Some(id) = self.newest else {
            return;
        };
        if let Some(vel) = world.get_component_mut::<Velocity>(id) {
            vel.linear.x += dir * KICK;
            vel.linear.y += KICK * 0.35;
            // Spin opposite the kick so the box tumbles.
            vel.angular -= dir * 3.5;
        }
    }
}

impl Game for Physics {
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

        let white = self.white.as_ref().expect("boot").id();
        self.rebuild(ctx.world, white);
    }

    fn update(&mut self, ctx: &mut Context) {
        let white = self.white.as_ref().expect("boot").id();

        if ctx.input.pressed(Key::KeyR) {
            self.rebuild(ctx.world, white);
        }
        if ctx.input.pressed(Key::Space) {
            self.drop_box(ctx.world, white);
        }
        if ctx.input.pressed(Key::ArrowLeft) || ctx.input.pressed(Key::KeyA) {
            self.kick_newest(ctx.world, -1.0);
        }
        if ctx.input.pressed(Key::ArrowRight) || ctx.input.pressed(Key::KeyD) {
            self.kick_newest(ctx.world, 1.0);
        }

        step_rigid_bodies(ctx.world, ctx.dt, GRAVITY);

        // Backdrop (immediate; walls/floor/boxes are world sprites).
        let mut sky = Sprite::new(white, Vec3::new(0.0, 0.0, -1.0), [40.0, WORLD_HEIGHT]);
        sky.color = [0.12, 0.14, 0.18, 1.0];
        ctx.scene.push(sky);
    }
}

fn spawn_fixed(
    world: &mut World,
    white: beton::TextureId,
    pos: Vec3,
    size: [f32; 2],
    color: [f32; 4],
) {
    let mut sprite = Sprite::new(white, pos, size);
    sprite.color = color;
    let id = world.spawn(GameObject {
        pos,
        rotation: 0.0,
        scale: [1.0, 1.0],
        parent: None,
        size,
        sprite: Some(sprite),
        tag: 1,
    });
    world.insert(id, RigidBody::fixed());
}

fn spawn_box(
    world: &mut World,
    white: beton::TextureId,
    pos: Vec3,
    color: [f32; 4],
) -> GameObjectId {
    let size = [BOX_SIZE, BOX_SIZE];
    let mut sprite = Sprite::new(white, pos, size);
    sprite.color = color;
    let id = world.spawn(GameObject {
        pos,
        rotation: 0.0,
        scale: [1.0, 1.0],
        parent: None,
        size,
        sprite: Some(sprite),
        tag: 2,
    });
    world.insert(id, Velocity::ZERO);
    world.insert(id, RigidBody::dynamic(1.0));
    id
}

fn box_color(n: u32) -> [f32; 4] {
    match n % 4 {
        0 => [0.90, 0.45, 0.35, 1.0],
        1 => [0.40, 0.75, 0.55, 1.0],
        2 => [0.45, 0.60, 0.95, 1.0],
        _ => [0.95, 0.80, 0.35, 1.0],
    }
}

fn main() {
    if let Err(err) = run(
        Config {
            title: "physics".into(),
            width: 800,
            height: 600,
            max_fps: Some(60),
        },
        Physics::new(),
    ) {
        eprintln!("{err}");
        std::process::exit(1);
    }
}
