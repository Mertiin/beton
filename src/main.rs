//! Open a window and draw one image.
//!
//! The file is decoded before the event loop. The upload happens once, from
//! `resumed`, after the device exists. Later frames only sample that texture.
//! A resize rewrites the four quad vertices so the image stays letterboxed.

mod gpu;
mod image_upload;

use std::path::PathBuf;
use std::sync::Arc;

use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::window::{Window, WindowId};

use gpu::Vertex;
use image_upload::CpuImage;

struct App {
    image: Option<CpuImage>,
    window: Option<Arc<Window>>,
    gpu: Option<gpu::Gpu>,
}

fn main() {
    let path = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("assets/sample.png"));

    let image = match image_upload::decode_rgba8(&path) {
        Ok(image) => image,
        Err(err) => {
            eprintln!("{err}");
            std::process::exit(1);
        }
    };

    let event_loop = EventLoop::new().unwrap_or_else(|err| {
        eprintln!("failed to create the event loop: {err}");
        std::process::exit(1);
    });

    let mut app = App {
        image: Some(image),
        window: None,
        gpu: None,
    };

    if let Err(err) = event_loop.run_app(&mut app) {
        eprintln!("event loop stopped: {err}");
        std::process::exit(1);
    }
}

impl ApplicationHandler for App {
    fn new_events(&mut self, event_loop: &ActiveEventLoop, _cause: winit::event::StartCause) {
        // Sleep until the OS asks for a frame or delivers an input event.
        event_loop.set_control_flow(ControlFlow::Wait);
    }

    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.gpu.is_some() {
            return;
        }

        let window = match event_loop.create_window(
            Window::default_attributes()
                .with_title("brutal-pixels")
                .with_inner_size(LogicalSize::new(960, 640)),
        ) {
            Ok(window) => Arc::new(window),
            Err(err) => {
                eprintln!("failed to create a window: {err}");
                event_loop.exit();
                return;
            }
        };

        let Some(image) = self.image.take() else {
            eprintln!("image was already consumed");
            event_loop.exit();
            return;
        };

        let parts = match gpu::Gpu::new(window.clone()) {
            Ok(parts) => parts,
            Err(err) => {
                eprintln!("{err}");
                event_loop.exit();
                return;
            }
        };

        let format = parts.image_format();
        let uploaded =
            match image_upload::upload_texture(parts.device(), parts.queue(), image, format) {
                Ok(uploaded) => uploaded,
                Err(err) => {
                    eprintln!("{err}");
                    event_loop.exit();
                    return;
                }
            };

        self.gpu = Some(parts.set_image(uploaded));
        self.window = Some(window);
        // Writes the letterboxed quad, then asks for the first paint.
        self.reflow();
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state == ElementState::Pressed
                    && event.logical_key == Key::Named(NamedKey::Escape)
                {
                    event_loop.exit();
                }
            }
            WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => self.reflow(),
            WindowEvent::RedrawRequested => self.draw(event_loop),
            _ => {}
        }
    }
}

impl App {
    /// Fit the quad to the current window and ask for a redraw.
    fn reflow(&mut self) {
        let Some(window) = self.window.clone() else {
            return;
        };
        let Some(gpu) = self.gpu.as_mut() else {
            return;
        };
        let size = window.inner_size();
        gpu.resize(size.width, size.height);
        let (img_w, img_h) = gpu.image_size();
        if let Some(vertices) = letterbox(
            size.width as f32,
            size.height as f32,
            img_w as f32,
            img_h as f32,
        ) {
            gpu.write_quad(&vertices);
        }
        window.request_redraw();
    }

    fn draw(&mut self, event_loop: &ActiveEventLoop) {
        let (Some(window), Some(gpu)) = (self.window.as_ref(), self.gpu.as_mut()) else {
            return;
        };
        match gpu.render() {
            Ok(()) => {}
            Err(gpu::FrameError::Reconfigure) => self.reflow(),
            Err(gpu::FrameError::Retry) => window.request_redraw(),
            Err(gpu::FrameError::Lost) => {
                eprintln!("the window surface was lost");
                event_loop.exit();
            }
        }
    }
}

/// Largest axis-aligned rect of `img_w` x `img_h` that fits in the window.
///
/// Positions are clip space: x right, y up, the window filling [-1, 1].
/// The image's top row is uv.y = 0, matching the top-to-bottom CPU buffer.
fn letterbox(win_w: f32, win_h: f32, img_w: f32, img_h: f32) -> Option<[Vertex; 4]> {
    if win_w <= 0.0 || win_h <= 0.0 || img_w <= 0.0 || img_h <= 0.0 {
        return None;
    }

    let scale = (win_w / img_w).min(win_h / img_h);
    let draw_w = img_w * scale;
    let draw_h = img_h * scale;
    let left = (win_w - draw_w) * 0.5;
    let top = (win_h - draw_h) * 0.5;
    let right = left + draw_w;
    let bottom = top + draw_h;

    let ndc = |x: f32, y: f32| -> [f32; 2] { [x / win_w * 2.0 - 1.0, 1.0 - y / win_h * 2.0] };

    let top_left = ndc(left, top);
    let top_right = ndc(right, top);
    let bottom_right = ndc(right, bottom);
    let bottom_left = ndc(left, bottom);

    Some([
        Vertex {
            pos: top_left,
            uv: [0.0, 0.0],
        },
        Vertex {
            pos: top_right,
            uv: [1.0, 0.0],
        },
        Vertex {
            pos: bottom_right,
            uv: [1.0, 1.0],
        },
        Vertex {
            pos: bottom_left,
            uv: [0.0, 1.0],
        },
    ])
}
