//! Flappy Bird sample on the brutal-gfx API.

#![cfg_attr(windows, windows_subsystem = "windows")]

use std::path::{Path, PathBuf};

use brutal_gfx::{
    run, Animation, Camera, Config, Context, Font, Game, GameObject, GameObjectId, Key, Projection,
    Rng, Scene, SoundId, Sprite, SpriteSheet, TextureId, Vec3, World,
};

const SFX_VOL: f32 = 0.4;
const BGM_VOL: f32 = 0.22;

const TAG_BIRD: u32 = 1;
const TAG_PIPE: u32 = 2;
const TAG_SCORE_SENSOR: u32 = 3;

const WORLD_HEIGHT: f32 = 20.0;
const GRAVITY: f32 = -28.0;
const FLAP: f32 = 11.0;
const SCROLL: f32 = -6.0;
const PIPE_WIDTH: f32 = 2.2;
const PIPE_GAP: f32 = 5.0;
const PIPE_INTERVAL: f32 = 1.6;
const BIRD_W: f32 = 1.6;
const BIRD_H: f32 = 1.15;
const FLOOR_Y: f32 = -WORLD_HEIGHT * 0.5 + 0.5;
const CEIL_Y: f32 = WORLD_HEIGHT * 0.5 - 0.5;
const FLAP_FPS: f32 = 10.0;
/// Nose-up radians at full flap velocity.
const BIRD_TILT_UP: f32 = 0.55;
/// Nose-down radians at a hard fall.
const BIRD_TILT_DOWN: f32 = -1.15;
const BIRD_TILT_UP_SPEED: f32 = 12.0;
const BIRD_TILT_DOWN_SPEED: f32 = 3.5;

/// Digit atlas: 10 cells of 16x20 (each source pixel scaled 4x from 4x5).
const DIGIT_CELL_W: u32 = 16;
const DIGIT_CELL_H: u32 = 20;
const DIGIT_ATLAS_W: u32 = DIGIT_CELL_W * 10;
const DIGIT_ATLAS_H: u32 = DIGIT_CELL_H;

#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
    Ready,
    Playing,
    Over,
}

struct Flappy {
    state: State,
    white: Option<TextureId>,
    font: Option<Font>,
    bird_anim: Option<Animation>,
    pipe: Option<TextureId>,
    flap_sfx: Option<SoundId>,
    score_sfx: Option<SoundId>,
    hit_sfx: Option<SoundId>,
    bird: Option<GameObjectId>,
    bird_vy: f32,
    bird_angle: f32,
    rng: Rng,
    next_pipe_at: f32,
    score: u32,
}

impl Flappy {
    fn new() -> Self {
        Self {
            state: State::Ready,
            white: None,
            font: None,
            bird_anim: None,
            pipe: None,
            flap_sfx: None,
            score_sfx: None,
            hit_sfx: None,
            bird: None,
            bird_vy: 0.0,
            bird_angle: 0.0,
            rng: Rng::new(1),
            next_pipe_at: 0.0,
            score: 0,
        }
    }

    fn asset(name: &str) -> PathBuf {
        assets_dir(&std::env::current_exe().expect("current executable")).join(name)
    }

    fn reset(&mut self, world: &mut World) {
        world.clear();
        self.score = 0;
        self.next_pipe_at = 0.0;
        self.bird_vy = 0.0;
        self.bird_angle = 0.0;
        self.state = State::Ready;
        if let Some(anim) = self.bird_anim.as_mut() {
            anim.reset();
            self.bird = Some(spawn_bird(world, anim));
        }
    }

    fn start_run(&mut self, ctx: &mut Context) {
        self.state = State::Playing;
        self.next_pipe_at = ctx.time + PIPE_INTERVAL * 0.5;
        self.bird_vy = FLAP;
        if let Some(sfx) = self.flap_sfx {
            ctx.audio.play(sfx, SFX_VOL);
        }
    }

    fn sync_bird_sprite(&mut self, world: &mut World, dt: f32) {
        let Some(anim) = self.bird_anim.as_ref() else {
            return;
        };
        let Some(id) = self.bird else {
            return;
        };
        if let Some(bird) = world.get_mut(id) {
            if self.state != State::Over {
                let target = bird_tilt(self.bird_vy);
                let speed = if target > self.bird_angle {
                    BIRD_TILT_UP_SPEED
                } else {
                    BIRD_TILT_DOWN_SPEED
                };
                self.bird_angle += (target - self.bird_angle) * (speed * dt).min(1.0);
            }
            if let Some(sprite) = bird.sprite.as_mut() {
                anim.apply(sprite);
                sprite.rotation = self.bird_angle;
            }
        }
    }
}

/// Map vertical speed to a nose tilt in radians (right-facing bird).
fn bird_tilt(vy: f32) -> f32 {
    let t = (vy / FLAP).clamp(-1.25, 1.0);
    if t >= 0.0 {
        t * BIRD_TILT_UP
    } else {
        let fall = (-t) / 1.25;
        fall * BIRD_TILT_DOWN
    }
}

impl Game for Flappy {
    fn boot(&mut self, ctx: &mut Context) {
        let white = ctx
            .gfx
            .create_texture(1, 1, &[255, 255, 255, 255])
            .expect("white texture");
        let (atlas, atlas_w, atlas_h) = digit_atlas_rgba();
        let font_tex = ctx
            .gfx
            .create_texture(atlas_w, atlas_h, &atlas)
            .expect("digit atlas");

        let bird_tex = ctx
            .gfx
            .load_texture(&Self::asset("bird-sheet.png"))
            .expect("bird-sheet");
        let pipe = ctx
            .gfx
            .load_texture(&Self::asset("pipe.png"))
            .expect("pipe");

        self.white = Some(white);
        self.font = Some(Font {
            texture: font_tex,
            columns: 10,
            rows: 1,
            first_char: '0',
        });
        // Sheet cells: 0=up, 1=mid, 2=down. Clip bounces mid twice for a smooth flap.
        self.bird_anim = Some(Animation::new(
            SpriteSheet {
                texture: bird_tex,
                columns: 3,
                rows: 1,
            },
            [0, 1, 2, 1],
            FLAP_FPS,
        ));
        self.pipe = Some(pipe);
        self.flap_sfx = Some(
            ctx.audio
                .create(&tone_wav(620.0, 0.07, 0.55))
                .expect("flap sfx"),
        );
        self.score_sfx = Some(
            ctx.audio
                .create(&tone_wav(980.0, 0.09, 0.45))
                .expect("score sfx"),
        );
        self.hit_sfx = Some(
            ctx.audio
                .create(&tone_wav(140.0, 0.22, 0.7))
                .expect("hit sfx"),
        );
        let bgm = ctx
            .audio
            .load(&Self::asset("bgm.ogg"))
            .expect("bgm");
        ctx.audio.play_loop(bgm, BGM_VOL);

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

        self.reset(ctx.world);
    }

    fn update(&mut self, ctx: &mut Context) {
        let white = self.white.expect("boot");
        let font = self.font.clone().expect("boot");
        let pipe_tex = self.pipe.expect("boot");

        if self.state != State::Over {
            if let Some(anim) = self.bird_anim.as_mut() {
                anim.tick(ctx.dt);
            }
        }
        self.sync_bird_sprite(ctx.world, ctx.dt);

        draw_backdrop(ctx.scene, white);

        match self.state {
            State::Ready => {
                draw_score(ctx.scene, &font, self.score);
                if ctx.input.pressed(Key::Space) {
                    self.start_run(ctx);
                }
            }
            State::Playing => {
                if ctx.input.pressed(Key::Space) {
                    self.bird_vy = FLAP;
                    if let Some(sfx) = self.flap_sfx {
                        ctx.audio.play(sfx, SFX_VOL);
                    }
                }
                self.bird_vy += GRAVITY * ctx.dt;
                if let Some(id) = self.bird {
                    if let Some(bird) = ctx.world.get_mut(id) {
                        bird.pos.y += self.bird_vy * ctx.dt;
                    }
                }

                scroll_pipes(ctx.world, ctx.dt);

                while ctx.time >= self.next_pipe_at {
                    spawn_pipe_pair(ctx.world, pipe_tex, &mut self.rng);
                    self.next_pipe_at += PIPE_INTERVAL;
                }

                despawn_offscreen(ctx.world);
                let gained = update_score(ctx.world, self.bird, &mut self.score);
                if gained > 0 {
                    if let Some(sfx) = self.score_sfx {
                        for _ in 0..gained {
                            ctx.audio.play(sfx, SFX_VOL);
                        }
                    }
                }

                if hit_pipe_or_bounds(ctx.world, self.bird) {
                    self.state = State::Over;
                    self.bird_vy = 0.0;
                    if let Some(sfx) = self.hit_sfx {
                        ctx.audio.play(sfx, SFX_VOL);
                    }
                }

                draw_score(ctx.scene, &font, self.score);
            }
            State::Over => {
                draw_score(ctx.scene, &font, self.score);
                if ctx.input.pressed(Key::Space) {
                    self.reset(ctx.world);
                }
            }
        }
    }
}

fn spawn_bird(world: &mut World, anim: &Animation) -> GameObjectId {
    let mut sprite = Sprite::new(anim.texture(), Vec3::ZERO, [BIRD_W, BIRD_H]);
    anim.apply(&mut sprite);
    world.spawn(GameObject {
        pos: Vec3::new(-4.0, 0.0, 0.0),
        rotation: 0.0,
        scale: [1.0, 1.0],
        parent: None,
        size: [BIRD_W * 0.7, BIRD_H * 0.7],
        sprite: Some(sprite),
        tag: TAG_BIRD,
    })
}

fn spawn_pipe_pair(world: &mut World, pipe: TextureId, rng: &mut Rng) {
    let gap_center = rng.range(-4.0, 4.0);
    let gap_half = PIPE_GAP * 0.5;
    let top_bottom = gap_center + gap_half;
    let bottom_top = gap_center - gap_half;
    let top_height = CEIL_Y - top_bottom;
    let bottom_height = bottom_top - FLOOR_Y;
    let x = 14.0;

    if top_height > 0.1 {
        let mut sprite = Sprite::new(pipe, Vec3::ZERO, [PIPE_WIDTH, top_height]);
        sprite.uv = [0.0, 1.0, 1.0, 0.0];
        world.spawn(GameObject {
            pos: Vec3::new(x, top_bottom + top_height * 0.5, 0.0),
            rotation: 0.0,
            scale: [1.0, 1.0],
            parent: None,
            size: [PIPE_WIDTH * 0.85, top_height],
            sprite: Some(sprite),
            tag: TAG_PIPE,
        });
    }
    if bottom_height > 0.1 {
        let sprite = Sprite::new(pipe, Vec3::ZERO, [PIPE_WIDTH, bottom_height]);
        world.spawn(GameObject {
            pos: Vec3::new(x, bottom_top - bottom_height * 0.5, 0.0),
            rotation: 0.0,
            scale: [1.0, 1.0],
            parent: None,
            size: [PIPE_WIDTH * 0.85, bottom_height],
            sprite: Some(sprite),
            tag: TAG_PIPE,
        });
    }

    world.spawn(GameObject {
        pos: Vec3::new(x, gap_center, 0.0),
        rotation: 0.0,
        scale: [1.0, 1.0],
        parent: None,
        size: [0.4, PIPE_GAP],
        sprite: None,
        tag: TAG_SCORE_SENSOR,
    });
}

fn scroll_pipes(world: &mut World, dt: f32) {
    for (_, object) in world.iter_mut() {
        if object.tag == TAG_PIPE || object.tag == TAG_SCORE_SENSOR {
            object.pos.x += SCROLL * dt;
        }
    }
}

fn despawn_offscreen(world: &mut World) {
    let doomed: Vec<_> = world
        .iter()
        .filter(|(_, o)| o.pos.x < -16.0)
        .map(|(id, _)| id)
        .collect();
    for id in doomed {
        world.despawn(id);
    }
}

fn update_score(world: &mut World, bird: Option<GameObjectId>, score: &mut u32) -> u32 {
    let Some(bird_id) = bird else {
        return 0;
    };
    let Some(bird_x) = world.world_pos(bird_id).map(|p| p.x) else {
        return 0;
    };
    let sensors: Vec<_> = world
        .iter()
        .filter(|(_, object)| object.tag == TAG_SCORE_SENSOR)
        .map(|(id, _)| id)
        .collect();
    let mut passed = Vec::new();
    for id in sensors {
        if let Some(pos) = world.world_pos(id) {
            if pos.x < bird_x {
                passed.push(id);
            }
        }
    }
    let mut gained = 0u32;
    for id in passed {
        if let Some(object) = world.get_mut(id) {
            object.tag = 0;
            *score += 1;
            gained += 1;
        }
    }
    gained
}

fn hit_pipe_or_bounds(world: &World, bird: Option<GameObjectId>) -> bool {
    let Some(bird_id) = bird else {
        return false;
    };
    let Some(size_y) = world.get(bird_id).map(|b| b.size[1]) else {
        return false;
    };
    let Some(pos) = world.world_pos(bird_id) else {
        return false;
    };
    let Some(scale) = world.world_scale(bird_id) else {
        return false;
    };
    let half_h = size_y * scale[1] * 0.5;
    if pos.y - half_h < FLOOR_Y || pos.y + half_h > CEIL_Y {
        return true;
    }
    let pipes: Vec<_> = world
        .iter()
        .filter(|(_, other)| other.tag == TAG_PIPE)
        .map(|(id, _)| id)
        .collect();
    for id in pipes {
        if world.overlaps(bird_id, id) {
            return true;
        }
    }
    false
}

fn draw_backdrop(scene: &mut Scene, white: TextureId) {
    let mut sky = Sprite::new(white, Vec3::new(0.0, 0.0, -1.0), [40.0, WORLD_HEIGHT]);
    sky.color = [0.45, 0.75, 0.95, 1.0];
    scene.push(sky);

    let floor_h = 1.0;
    let mut floor = Sprite::new(
        white,
        Vec3::new(0.0, FLOOR_Y - floor_h * 0.5, -0.5),
        [40.0, floor_h],
    );
    floor.color = [0.55, 0.4, 0.2, 1.0];
    scene.push(floor);
}

fn draw_score(scene: &mut Scene, font: &Font, score: u32) {
    let text = score.to_string();
    let glyph = [1.1, 1.4];
    let width = text.len() as f32 * glyph[0];
    scene.text(
        font,
        Vec3::new(-width * 0.5 + 0.06, 8.0 - 0.06, 0.9),
        glyph,
        [0.0, 0.0, 0.0, 0.55],
        &text,
    );
    scene.text(
        font,
        Vec3::new(-width * 0.5, 8.0, 1.0),
        glyph,
        [1.0, 1.0, 1.0, 1.0],
        &text,
    );
}

/// Short mono PCM WAV (16-bit) with a linear fade-out.
fn tone_wav(freq_hz: f32, duration_s: f32, amplitude: f32) -> Vec<u8> {
    const SAMPLE_RATE: u32 = 22_050;
    let n = (duration_s * SAMPLE_RATE as f32).round() as usize;
    let mut samples = Vec::with_capacity(n);
    for i in 0..n {
        let t = i as f32 / SAMPLE_RATE as f32;
        let env = 1.0 - t / duration_s;
        let sample = (t * freq_hz * std::f32::consts::TAU).sin() * amplitude * env;
        samples.push((sample.clamp(-1.0, 1.0) * 32767.0) as i16);
    }
    write_wav(SAMPLE_RATE, &samples)
}

fn write_wav(sample_rate: u32, samples: &[i16]) -> Vec<u8> {
    let data_len = (samples.len() * 2) as u32;
    let mut out = Vec::with_capacity(44 + samples.len() * 2);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVE");
    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&sample_rate.to_le_bytes());
    out.extend_from_slice(&(sample_rate * 2).to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for sample in samples {
        out.extend_from_slice(&sample.to_le_bytes());
    }
    out
}

/// Build a large nearest-filtered digit atlas from 4x5 bitmaps scaled 4x.
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
    debug_assert_eq!(DIGIT_CELL_W, SRC_W * SCALE);
    debug_assert_eq!(DIGIT_CELL_H, SRC_H * SCALE);

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

/// `assets/` is copied beside the executable by `build.rs`.
fn assets_dir(exe: &Path) -> PathBuf {
    exe.parent()
        .expect("executable path has a directory")
        .join("assets")
}

fn main() {
    if let Err(err) = run(
        Config {
            title: "flappy".into(),
            width: 480,
            height: 720,
            max_fps: Some(60),
        },
        Flappy::new(),
    ) {
        eprintln!("{err}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::assets_dir;
    use std::path::PathBuf;

    #[test]
    fn assets_are_loaded_from_beside_the_executable() {
        let exe = PathBuf::from("target").join("release").join("flappy.exe");
        assert_eq!(
            assets_dir(&exe),
            PathBuf::from("target").join("release").join("assets")
        );
    }
}
