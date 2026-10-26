//! Demo: pin a room’s texture/sound on enter, drop on leave, watch residency unload.
//!
//! Keep a console attached — logs report live counts and approximate asset RAM each second.

use std::path::{Path, PathBuf};

use beton::{
    run, Camera, Config, Context, Font, Game, Key, Projection, SoundHandle, SoundId, Sprite,
    TextureHandle, TextureId, Vec3,
};

const WORLD_HEIGHT: f32 = 20.0;

const DIGIT_CELL_W: u32 = 16;
const DIGIT_CELL_H: u32 = 20;
const DIGIT_ATLAS_W: u32 = DIGIT_CELL_W * 10;
const DIGIT_ATLAS_H: u32 = DIGIT_CELL_H;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Room {
    Empty,
    Red,
    Blue,
    Green,
}

struct Gallery {
    white: Option<TextureHandle>,
    font_tex: Option<TextureHandle>,
    font: Option<Font>,
    red: TextureId,
    blue: TextureId,
    green: TextureId,
    beep_red: SoundId,
    beep_blue: SoundId,
    beep_green: SoundId,
    room_tex: Option<TextureHandle>,
    room_sfx: Option<SoundHandle>,
    room: Room,
    log_timer: f32,
}

impl Gallery {
    fn new() -> Self {
        Self {
            white: None,
            font_tex: None,
            font: None,
            red: TextureId::from_raw(0),
            blue: TextureId::from_raw(0),
            green: TextureId::from_raw(0),
            beep_red: SoundId::from_raw(0),
            beep_blue: SoundId::from_raw(0),
            beep_green: SoundId::from_raw(0),
            room_tex: None,
            room_sfx: None,
            room: Room::Empty,
            log_timer: 0.0,
        }
    }

    fn asset(name: &str) -> PathBuf {
        assets_dir(&std::env::current_exe().expect("current executable")).join(name)
    }

    fn log_mem(ctx: &Context, prefix: &str) {
        let tex_live = ctx.gfx.live_count();
        let aud_live = ctx.audio.live_count();
        let tex_res = ctx.gfx.resident_bytes();
        let aud_res = ctx.audio.resident_bytes();
        let tex_src = ctx.gfx.retained_source_bytes();
        let aud_src = ctx.audio.retained_source_bytes();
        eprintln!(
            "{prefix} tex={tex_live} ({})  audio={aud_live} ({})  retained_src tex={} audio={}  total_res={}",
            fmt_bytes(tex_res),
            fmt_bytes(aud_res),
            fmt_bytes(tex_src),
            fmt_bytes(aud_src),
            fmt_bytes(tex_res + aud_res),
        );
    }

    fn enter(&mut self, ctx: &mut Context, room: Room) {
        if self.room == room {
            return;
        }
        self.room_tex = None;
        self.room_sfx = None;
        self.room = room;

        let (tex, sfx, name) = match room {
            Room::Empty => {
                Self::log_mem(ctx, "lobby — dropped room pins");
                return;
            }
            Room::Red => (self.red, self.beep_red, "red"),
            Room::Blue => (self.blue, self.beep_blue, "blue"),
            Room::Green => (self.green, self.beep_green, "green"),
        };

        self.room_tex = ctx.gfx.pin(tex);
        self.room_sfx = ctx.audio.pin(sfx);
        ctx.audio.play(sfx, 0.35);
        Self::log_mem(ctx, &format!("entered {name} — pinned"));
    }
}

impl Game for Gallery {
    fn boot(&mut self, ctx: &mut Context) {
        let white = ctx
            .gfx
            .create_texture(1, 1, &[255, 255, 255, 255])
            .expect("white");
        let (atlas, aw, ah) = digit_atlas_rgba();
        let font_id = ctx.gfx.create_texture(aw, ah, &atlas).expect("font");

        self.red = ctx
            .gfx
            .load_texture(&Self::asset("room-red.jpg"))
            .expect("room-red.jpg");
        self.blue = ctx
            .gfx
            .load_texture(&Self::asset("room-blue.jpg"))
            .expect("room-blue.jpg");
        self.green = ctx
            .gfx
            .load_texture(&Self::asset("room-green.jpg"))
            .expect("room-green.jpg");

        self.beep_red = ctx
            .audio
            .create(&tone_wav(440.0, 0.12, 0.5))
            .expect("beep red");
        self.beep_blue = ctx
            .audio
            .create(&tone_wav(554.0, 0.12, 0.5))
            .expect("beep blue");
        self.beep_green = ctx
            .audio
            .create(&tone_wav(659.0, 0.12, 0.5))
            .expect("beep green");

        self.white = Some(ctx.gfx.pin(white).expect("pin white"));
        self.font_tex = Some(ctx.gfx.pin(font_id).expect("pin font"));
        self.font = Some(Font::grid(font_id, 10, 1, '0'));

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

        eprintln!("gallery ready — 1/2/3 enter a room, 0 return to lobby");
        eprintln!("HUD digits: textures live, audio live (each capped at 9)");
        eprintln!("Each room photo is ~1920x1080 → ~7.9 MB RGBA on the GPU when resident");
        Self::log_mem(ctx, "boot (photos registered, not uploaded yet)");
    }

    fn update(&mut self, ctx: &mut Context) {
        if ctx.input.pressed(Key::Digit1) || ctx.input.pressed(Key::Numpad1) {
            self.enter(ctx, Room::Red);
        }
        if ctx.input.pressed(Key::Digit2) || ctx.input.pressed(Key::Numpad2) {
            self.enter(ctx, Room::Blue);
        }
        if ctx.input.pressed(Key::Digit3) || ctx.input.pressed(Key::Numpad3) {
            self.enter(ctx, Room::Green);
        }
        if ctx.input.pressed(Key::Digit0) || ctx.input.pressed(Key::Numpad0) {
            self.enter(ctx, Room::Empty);
        }

        self.log_timer += ctx.dt;
        if self.log_timer >= 1.0 {
            self.log_timer = 0.0;
            let name = match self.room {
                Room::Empty => "lobby",
                Room::Red => "red",
                Room::Blue => "blue",
                Room::Green => "green",
            };
            Self::log_mem(ctx, &format!("tick room={name}"));
        }

        let white = self.white.as_ref().expect("boot").id();
        let font = self.font.clone().expect("boot");

        let mut bg = Sprite::new(white, Vec3::new(0.0, 0.0, -1.0), [40.0, WORLD_HEIGHT]);
        bg.color = [0.12, 0.12, 0.14, 1.0];
        ctx.scene.push(bg);

        if let Some(pin) = self.room_tex.as_ref() {
            let id = pin.id();
            let (pw, ph) = ctx.gfx.texture_size(id);
            let aspect = if ph > 0 {
                pw as f32 / ph as f32
            } else {
                16.0 / 9.0
            };
            let panel_h = 11.0;
            let panel_w = panel_h * aspect;
            let mut panel = Sprite::new(id, Vec3::new(0.0, 0.3, 0.0), [panel_w, panel_h]);
            panel.color = [1.0, 1.0, 1.0, 1.0];
            ctx.scene.push(panel);
        }

        let tex_n = ctx.gfx.live_count().min(9);
        let aud_n = ctx.audio.live_count().min(9);
        let label = format!("{tex_n}{aud_n}");
        let glyph = [1.0, 1.3];
        let width = 2.0 * glyph[0];
        ctx.scene.text(
            &font,
            Vec3::new(-width * 0.5, 8.2, 1.0),
            glyph,
            [1.0, 1.0, 1.0, 1.0],
            &label,
        );
    }
}

fn fmt_bytes(n: usize) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    let n = n as f64;
    if n >= MB {
        format!("{:.2} MB", n / MB)
    } else if n >= KB {
        format!("{:.1} KB", n / KB)
    } else {
        format!("{n:.0} B")
    }
}

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

fn assets_dir(exe: &Path) -> PathBuf {
    exe.parent()
        .expect("executable path has a directory")
        .join("assets")
}

fn main() {
    if let Err(err) = run(
        Config {
            title: "gallery — 1/2/3 rooms, 0 lobby".into(),
            width: 960,
            height: 540,
            max_fps: Some(60),
        },
        Gallery::new(),
    ) {
        eprintln!("{err}");
        std::process::exit(1);
    }
}
