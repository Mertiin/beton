//! Window, clock, and the `Game` loop.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use winit::application::ApplicationHandler;
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy};
use winit::keyboard::{Key, NamedKey, PhysicalKey};
use winit::window::{Window, WindowId};

use crate::core::audio::Audio;
use crate::core::camera::Camera;
use crate::core::input::Input;
use crate::core::net::Net;
use crate::core::object::World;
use crate::core::platform::{self, Duration, Instant};
use crate::core::render::{FrameError, Gfx, Renderer};
use crate::core::scene::Scene;

/// Window title, size, and frame pacing at startup.
#[derive(Clone, Debug)]
pub struct Config {
    /// Window title.
    pub title: String,
    /// Logical width in pixels.
    pub width: u32,
    /// Logical height in pixels.
    pub height: u32,
    /// Soft FPS cap. `None` runs as fast as the swapchain allows (still vsynced with
    /// `Fifo`). `Some(n)` sleeps the event loop so frames land about `n` times per second.
    pub max_fps: Option<u32>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            title: "beton".into(),
            width: 960,
            height: 640,
            max_fps: Some(60),
        }
    }
}

/// Per-frame handles the game reads and writes.
pub struct Context<'a> {
    /// Seconds since the previous frame, capped at `0.1`.
    pub dt: f32,
    /// Seconds since [`run`] started.
    pub time: f32,
    /// Keyboard and cursor snapshot for this frame.
    pub input: &'a Input,
    /// Host / client message channel for this frame.
    pub net: &'a mut Net,
    /// Framebuffer size in physical pixels `[width, height]`.
    pub fb_size: [f32; 2],
    /// Texture loading.
    pub gfx: &'a mut Gfx,
    /// Sound loading and playback.
    pub audio: &'a mut Audio,
    /// Retained objects.
    pub world: &'a mut World,
    /// Immediate sprites for this frame.
    pub scene: &'a mut Scene,
    /// Active camera.
    pub camera: &'a mut Camera,
}

/// Implement this on your game state and pass it to [`run`].
pub trait Game {
    /// Paths to preload before [`boot`](Game::boot) (desktop disk or web `fetch`).
    ///
    /// List every file you will pass to [`Gfx::load_texture`](crate::Gfx::load_texture) /
    /// [`Audio::load`](crate::Audio::load). On the web this is required; use the same
    /// relative paths Trunk (or your host) serves — e.g. `"assets/bird-sheet.png"`.
    fn assets(&self) -> &[&'static str] {
        &[]
    }
    /// Called once after the GPU and [`assets`](Game::assets) are ready. Load textures and
    /// spawn the first objects here.
    fn boot(&mut self, ctx: &mut Context);
    /// Called every frame before drawing. Move game objects by writing their local transforms.
    fn update(&mut self, ctx: &mut Context);
}

enum EngineEvent {
    GpuReady {
        window: Arc<Window>,
        result: Result<(Renderer, Gfx), String>,
        files: Result<HashMap<PathBuf, Vec<u8>>, String>,
    },
}

async fn preload_assets(paths: &[String]) -> Result<HashMap<PathBuf, Vec<u8>>, String> {
    let mut files = HashMap::with_capacity(paths.len());
    for path in paths {
        let bytes = platform::read_bytes(path).await?;
        files.insert(PathBuf::from(path), bytes);
    }
    Ok(files)
}

/// Open a window and run `game` until the window closes.
///
/// On the web this returns `Ok(())` as soon as the event loop is scheduled. Later
/// failures (GPU init, lost surface) are reported through the browser console.
pub fn run(config: Config, game: impl Game + 'static) -> Result<(), String> {
    platform::install_panic_hook();
    let event_loop = EventLoop::with_user_event()
        .build()
        .map_err(|err| format!("failed to create the event loop: {err}"))?;
    let proxy = event_loop.create_proxy();
    let exit_error = Arc::new(Mutex::new(None));
    let now = Instant::now();
    let app = EngineApp {
        config,
        game,
        proxy,
        window: None,
        renderer: None,
        gfx: None,
        audio: Audio::silent(),
        input: Input::default(),
        net: Net::new(),
        world: World::new(),
        scene: Scene::default(),
        camera: Camera::default(),
        start: now,
        last: now,
        next_frame_at: now,
        booted: false,
        gpu_pending: false,
        exit_error: Arc::clone(&exit_error),
    };
    platform::run_loop(event_loop, app)?;
    let result = match exit_error.lock() {
        Ok(mut guard) => match guard.take() {
            Some(err) => Err(err),
            None => Ok(()),
        },
        Err(_) => Ok(()),
    };
    result
}

struct EngineApp<G: Game> {
    config: Config,
    game: G,
    proxy: EventLoopProxy<EngineEvent>,
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
    gfx: Option<Gfx>,
    audio: Audio,
    input: Input,
    net: Net,
    world: World,
    scene: Scene,
    camera: Camera,
    start: Instant,
    last: Instant,
    next_frame_at: Instant,
    booted: bool,
    gpu_pending: bool,
    exit_error: Arc<Mutex<Option<String>>>,
}

impl<G: Game> EngineApp<G> {
    fn fail(&mut self, event_loop: &ActiveEventLoop, err: String) {
        platform::warn(&err);
        if let Ok(mut guard) = self.exit_error.lock() {
            *guard = Some(err);
        }
        event_loop.exit();
    }

    fn frame_period(&self) -> Option<Duration> {
        self.config
            .max_fps
            .filter(|&fps| fps > 0)
            .map(|fps| Duration::from_secs_f64(1.0 / f64::from(fps)))
    }

    /// Sleep until the next frame is due, or poll when uncapped.
    fn schedule_wake(&self, event_loop: &ActiveEventLoop) {
        match self.frame_period() {
            Some(_) => event_loop.set_control_flow(ControlFlow::WaitUntil(self.next_frame_at)),
            None => event_loop.set_control_flow(ControlFlow::Poll),
        }
    }

    fn install_gpu(
        &mut self,
        event_loop: &ActiveEventLoop,
        window: Arc<Window>,
        result: Result<(Renderer, Gfx), String>,
        files: Result<HashMap<PathBuf, Vec<u8>>, String>,
    ) {
        let (renderer, mut gfx) = match result {
            Ok(pair) => pair,
            Err(err) => {
                self.fail(event_loop, err);
                return;
            }
        };
        let files = match files {
            Ok(files) => Arc::new(files),
            Err(err) => {
                self.fail(event_loop, err);
                return;
            }
        };
        gfx.install_files(Arc::clone(&files));
        self.renderer = Some(renderer);
        self.gfx = Some(gfx);
        self.window = Some(window);
        if platform::open_audio_at_boot() {
            self.audio = Audio::new();
        }
        self.audio.install_files(files);
        self.boot_if_needed();
    }

    fn maybe_open_audio(&mut self, pressed: bool) {
        if !self.audio.has_device() && platform::is_audio_gesture(pressed) {
            self.audio.try_open_device();
        }
    }

    fn boot_if_needed(&mut self) {
        if self.booted {
            return;
        }
        let Some(gfx) = self.gfx.as_mut() else {
            return;
        };
        let fb_size = self
            .window
            .as_ref()
            .map(|w| {
                let s = w.inner_size();
                [s.width as f32, s.height as f32]
            })
            .unwrap_or([1.0, 1.0]);
        self.net.begin_frame();
        let mut ctx = Context {
            dt: 0.0,
            time: 0.0,
            input: &self.input,
            net: &mut self.net,
            fb_size,
            gfx,
            audio: &mut self.audio,
            world: &mut self.world,
            scene: &mut self.scene,
            camera: &mut self.camera,
        };
        self.game.boot(&mut ctx);
        self.net.end_frame();
        self.booted = true;
        let now = Instant::now();
        self.start = now;
        self.last = now;
        self.next_frame_at = now;
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }

    fn frame(&mut self, event_loop: &ActiveEventLoop) {
        if !self.booted {
            self.boot_if_needed();
            if !self.booted {
                return;
            }
        }

        let now = Instant::now();
        let dt = (now - self.last).as_secs_f32().min(0.1);
        self.last = now;
        let time = (now - self.start).as_secs_f32();

        self.scene.clear();
        self.net.begin_frame();

        {
            let fb_size = self
                .window
                .as_ref()
                .map(|w| {
                    let s = w.inner_size();
                    [s.width as f32, s.height as f32]
                })
                .unwrap_or([1.0, 1.0]);
            let gfx = self.gfx.as_mut().unwrap();
            let mut ctx = Context {
                dt,
                time,
                input: &self.input,
                net: &mut self.net,
                fb_size,
                gfx,
                audio: &mut self.audio,
                world: &mut self.world,
                scene: &mut self.scene,
                camera: &mut self.camera,
            };
            self.game.update(&mut ctx);
        }
        self.net.end_frame();
        self.audio.reap_finished();
        self.audio.resolve(dt);

        let draw_ids: Vec<_> = self
            .world
            .iter()
            .filter(|(_, object)| object.sprite.is_some())
            .map(|(id, _)| id)
            .collect();
        for id in draw_ids {
            let Some(object) = self.world.get(id) else {
                continue;
            };
            let Some(sprite) = object.sprite.clone() else {
                continue;
            };
            let Some((pos, rot, scale)) = self.world.draw_pose(id) else {
                continue;
            };
            let mut drawn = sprite;
            drawn.center = pos;
            drawn.rotation += rot;
            drawn.size = [drawn.size[0] * scale[0], drawn.size[1] * scale[1]];
            self.scene.push(drawn);
        }

        let sprites = self.scene.take_sorted();
        let mut used_textures = std::collections::HashSet::new();
        for sprite in &sprites {
            used_textures.insert(sprite.texture);
        }

        let aspect = self.renderer.as_ref().map(|r| r.aspect()).unwrap_or(1.0);
        let view_proj = self.camera.view_proj(aspect);

        let (Some(renderer), Some(gfx), Some(window)) = (
            self.renderer.as_mut(),
            self.gfx.as_mut(),
            self.window.as_ref(),
        ) else {
            return;
        };
        gfx.ensure_resident(&used_textures);

        let mut surface_lost = false;
        match renderer.render(view_proj, &sprites, gfx) {
            Ok(()) => {}
            Err(FrameError::Reconfigure) => {
                let size = window.inner_size();
                renderer.resize(size.width, size.height);
                window.request_redraw();
            }
            Err(FrameError::Retry) => window.request_redraw(),
            Err(FrameError::Lost) => surface_lost = true,
        }

        // Drop unused GPU textures only after present so in-flight draws stay valid.
        gfx.reap(&used_textures, dt);

        if surface_lost {
            self.fail(event_loop, "the window surface was lost".into());
            return;
        }

        // Clear edge-triggered keys after the frame so KeyboardInput events
        // that arrived before RedrawRequested are still visible in update.
        // Net events were already cleared in end_frame after update.
        self.input.begin_frame();

        if let Some(period) = self.frame_period() {
            // Aim from the frame we just finished so slow frames don't pile up forever.
            self.next_frame_at = Instant::now() + period;
        }
        self.schedule_wake(event_loop);
    }
}

impl<G: Game> ApplicationHandler<EngineEvent> for EngineApp<G> {
    fn new_events(&mut self, event_loop: &ActiveEventLoop, _cause: winit::event::StartCause) {
        self.schedule_wake(event_loop);
    }

    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.renderer.is_some() || self.gpu_pending {
            return;
        }

        let window = match event_loop.create_window(platform::window_attributes(
            &self.config.title,
            self.config.width,
            self.config.height,
        )) {
            Ok(window) => Arc::new(window),
            Err(err) => {
                self.fail(event_loop, format!("failed to create a window: {err}"));
                return;
            }
        };

        self.gpu_pending = true;
        let proxy = self.proxy.clone();
        let asset_paths: Vec<String> = self
            .game
            .assets()
            .iter()
            .map(|path| (*path).to_string())
            .collect();
        platform::schedule(async move {
            let result = Renderer::new(window.clone()).await;
            let files = if result.is_ok() {
                preload_assets(&asset_paths).await
            } else {
                Ok(HashMap::new())
            };
            let _ = proxy.send_event(EngineEvent::GpuReady {
                window,
                result,
                files,
            });
        });
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: EngineEvent) {
        match event {
            EngineEvent::GpuReady {
                window,
                result,
                files,
            } => {
                self.gpu_pending = false;
                self.install_gpu(event_loop, window, result, files);
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::KeyboardInput { event, .. } => {
                let pressed = event.state == ElementState::Pressed;
                if pressed
                    && event.logical_key == Key::Named(NamedKey::Escape)
                    && platform::exit_on_escape()
                {
                    event_loop.exit();
                    return;
                }
                self.maybe_open_audio(pressed);
                if let PhysicalKey::Code(code) = event.physical_key {
                    self.input.key_event(code, pressed);
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let pressed =
                    state == ElementState::Pressed && matches!(button, MouseButton::Left);
                self.maybe_open_audio(pressed);
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.input
                    .cursor_moved(position.x as f32, position.y as f32);
            }
            WindowEvent::CursorLeft { .. } => {
                self.input.cursor_left();
            }
            WindowEvent::Resized(size) => {
                if let Some(renderer) = self.renderer.as_mut() {
                    renderer.resize(size.width, size.height);
                }
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            WindowEvent::ScaleFactorChanged { .. } => {
                if let Some(window) = &self.window {
                    let size = window.inner_size();
                    if let Some(renderer) = self.renderer.as_mut() {
                        renderer.resize(size.width, size.height);
                    }
                    window.request_redraw();
                }
            }
            WindowEvent::RedrawRequested => self.frame(event_loop),
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let Some(window) = self.window.as_ref() else {
            return;
        };

        if let Some(_period) = self.frame_period() {
            let now = Instant::now();
            if now < self.next_frame_at {
                event_loop.set_control_flow(ControlFlow::WaitUntil(self.next_frame_at));
                return;
            }
        }

        window.request_redraw();
        self.schedule_wake(event_loop);
    }
}
